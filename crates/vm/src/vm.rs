use std::{collections::HashMap, rc::Rc};
use error::error::{Span, VyprError};
use crate::{builtins, bytecode::{Chunk, OpCode}, heap::Heap, stdlib, value::{DataType, Object, ObjectDict, ObjectFunction, ObjectList, ObjectModule, ObjectNative, ObjectRange, ObjectType, Value}};

#[derive(Clone)]
struct GlobalVar {
    value: Value,
    lock: DataType,
}

struct CallFrame {
    chunk: Rc<Chunk>,     // The code being executed
    ip: usize,            // Instruction pointer for this frame
    frame_start: usize,   // where function locals begin on the stack
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MethodCache {
    EMPTY,
    ListAppend,
    ListPop,
    StringUpper,
    StringLower,
}

pub struct VM {
    pub heap: Heap,
    frames: Vec<CallFrame>, // The call stack
    stack: Vec<Value>,      // The operand stack
    globals: Vec<GlobalVar>,
    gray_stack: Vec<*mut Object>
}

impl VM {

    pub fn new(mut heap: Heap, chunk: Chunk) -> Self {
        let globals = vec![
            GlobalVar { value: Value::allocate_native(&mut heap, "print".into(), builtins::vypr_print), lock: DataType::Function },
            GlobalVar { value: Value::allocate_native(&mut heap, "int".into(), builtins::vypr_int), lock: DataType::Function },
            GlobalVar { value: Value::allocate_native(&mut heap, "float".into(), builtins::vypr_float), lock: DataType::Function },
            GlobalVar { value: Value::allocate_native(&mut heap, "str".into(), builtins::vypr_str), lock: DataType::Function },
            GlobalVar { value: Value::allocate_native(&mut heap, "len".into(), builtins::vypr_len), lock: DataType::Function },
            GlobalVar { value: Value::allocate_native(&mut heap, "range".into(), builtins::vypr_range), lock: DataType::Function },
            GlobalVar { value: Value::allocate_native(&mut heap, "list".into(), builtins::vypr_list), lock: DataType::Function },
            GlobalVar { value: Value::allocate_native(&mut heap, "reversed".into(), builtins::vypr_reversed), lock: DataType::Function },
            GlobalVar { value: Value::allocate_native(&mut heap, "input".into(), builtins::vypr_input), lock: DataType::Function },
            GlobalVar { value: Value::allocate_native(&mut heap, "open".into(), builtins::vypr_open), lock: DataType::Function },
        ];

        let mut chunk = chunk;
        chunk.init_cache();

        let main_frame = CallFrame {
            chunk: Rc::new(chunk),
            ip: 0,
            frame_start: 0,
        };

        Self {
            heap,
            frames: vec![main_frame],
            stack: Vec::new(),
            globals,
            gray_stack: Vec::new(),
        }
    }

    pub(crate) fn set_cache(&mut self, ip: usize, cache_type: MethodCache) {
        if let Some(frame) = self.frames.last() {
            let mut cache = frame.chunk.cache.borrow_mut();

            if ip < cache.len() {
                cache[ip] = cache_type;
            }
        }
    }

    pub(crate) fn error(&self, code: &'static str, message: impl Into<String>) -> VyprError {
        let ip = self.current_frame().ip.saturating_sub(1);

        let span = if ip < self.current_frame().chunk.spans.len() {
            self.current_frame().chunk.spans[ip]
        } else {
            Span::default()
        };

        VyprError::new(code, message, span)
    }

    pub fn collect_garbage(&mut self) {
        self.mark_roots();
        self.trace_references();
        self.compact();
    }

    fn compact(&mut self) {
        unsafe {
            // ================================================================
            // PASS 1: COMPUTE FORWARDING ADDRESSES
            // ================================================================
            let mut free_ptr = self.heap.start;
            let mut current = self.heap.head;

            while let Some(node) = current {
                let obj = node.as_ptr();
                let next_node = (*obj).next;

                if (*obj).is_marked {
                    // This object lives. Calculate its new packed home.
                    (*obj).forwarding = free_ptr as *mut Object;
                    free_ptr = free_ptr.add((*obj).size());
                } else {
                    // Dead object. Mark forwarding as null.
                    (*obj).forwarding = std::ptr::null_mut();
                }

                current = next_node;
            }

            // ================================================================
            // PASS 2: UPDATE ALL REFERENCES (ROOTS & OBJECT FIELDS)
            // ================================================================
            self.update_references();

            // ================================================================
            // PASS 3: SLIDE OBJECTS IN-PLACE & REBUILD LINKED LIST
            // ================================================================
            let mut new_head: Option<std::ptr::NonNull<Object>> = None;
            let mut last_new_node: Option<std::ptr::NonNull<Object>> = None;
            
            current = self.heap.head;
            while let Some(node) = current {
                let old_obj = node.as_ptr();
                let next_node = (*old_obj).next;

                if (*old_obj).is_marked {
                    let new_addr = (*old_obj).forwarding as *mut u8;
                    let obj_size = (*old_obj).size();

                    // Physically slide the bytes of the object to its new address
                    std::ptr::copy(old_obj as *const u8, new_addr, obj_size);

                    let packed_obj = new_addr as *mut Object;
                    (*packed_obj).is_marked = false; // Reset mark for next cycle
                    (*packed_obj).next = None;

                    // Rebuild the intrusive linked list in the new compacted space
                    let new_node = std::ptr::NonNull::new_unchecked(packed_obj);
                    if new_head.is_none() {
                        new_head = Some(new_node);
                    } else {
                        if let Some(last) = last_new_node {
                            (*last.as_ptr()).next = Some(new_node);
                        }
                    }
                    last_new_node = Some(new_node);
                } else {
                    // Drop dead objects gracefully
                    self.free_object(old_obj);
                }

                current = next_node;
            }

            // Update heap state
            self.heap.head = new_head;
            self.heap.current = free_ptr;
            self.heap.bytes_allocated = free_ptr.offset_from(self.heap.start) as usize;

            let dynamic_threshold = std::cmp::max(self.heap.bytes_allocated * 2, self.heap.capacity / 4);
            let absolute_max = (self.heap.capacity as f64 * 0.90) as usize;
            self.heap.next_gc_threshold = std::cmp::min(dynamic_threshold, absolute_max);
        }
    }

    fn mark_roots(&mut self) {
    // 1. Mark everything on the operand stack
        for i in 0..self.stack.len() {
            let val = self.stack[i];
            self.mark_value(val);
        }

        // 2. Mark all global variables
        for i in 0..self.globals.len() {
            let val = self.globals[i].value;
            self.mark_value(val);
        }

        // 3. Mark all constants held by the active call frames
        for i in 0..self.frames.len() {
            for j in 0..self.frames[i].chunk.constants.len() {
                let constant = self.frames[i].chunk.constants[j];
                self.mark_value(constant);
            }
        }
    }

    fn mark_value(&mut self, value: Value) {
        // We only care about pointers. Numbers/Bools live in the register (NaN payload)
        if value.is_object() {
            self.mark_object(value.as_object());
        }
    }

    fn mark_object(&mut self, obj: *mut crate::value::Object) {
        if obj.is_null() { return; }

        unsafe {
            // If it is already marked, we've seen it. Skip to prevent infinite loops!
            if (*obj).is_marked { 
                return; 
            }

            // Mark it as alive (Gray)
            (*obj).is_marked = true;

            // Add to the worklist so we can check its children later
            self.gray_stack.push(obj);
        }
    }

    fn trace_references(&mut self) {
        // Keep popping from the gray stack until it's empty
        while let Some(obj) = self.gray_stack.pop() {
            self.blacken_object(obj);
        }
    }

    fn update_references(&mut self) {
        self.heap.strings.retain(|_key, ptr| {
            unsafe {
                if (**ptr).is_marked {
                    *ptr = (**ptr).forwarding;
                    true
                } else {
                    false
                }
            }
        });

        // 1. Update stack roots
        for i in 0..self.stack.len() {
            VM::update_value_reference(&mut self.stack[i]);
        }

        // 2. Update global roots
        for i in 0..self.globals.len() {
            VM::update_value_reference(&mut self.globals[i].value);
        }

        // 3. Track chunks to prevent double-updating! 
        // A single chunk can be referenced by multiple CallFrames or ObjFunctions.
        // Updating a pointer twice in Mark-Compact is fatal.
        let mut unique_chunks = std::collections::HashSet::new();

        for frame in &self.frames {
            unique_chunks.insert(Rc::as_ptr(&frame.chunk) as *mut crate::bytecode::Chunk);
        }

        // 4. Update active heap objects
        let mut current = self.heap.head;
        while let Some(node) = current {
            unsafe {
                use crate::value::{ObjectType, ObjectList, ObjectDict, ObjectModule, ObjectFunction};
                let obj = node.as_ptr();
                if (*obj).is_marked {
                    match (*obj).ty {
                        ObjectType::LIST => {
                            let list = &mut *(obj as *mut ObjectList);
                            for item in &mut list.items {
                                VM::update_value_reference(item);
                            }
                        }
                        ObjectType::DICT => {
                            let dict = &mut *(obj as *mut ObjectDict);
                            // Re-build or update map keys/values
                            let mut new_items = HashMap::with_capacity(dict.items.len());
                            for (k, v) in dict.items.drain() {
                                let mut new_k = k;
                                let mut new_v = v;
                                VM::update_value_reference(&mut new_k);
                                VM::update_value_reference(&mut new_v);
                                new_items.insert(new_k, new_v);
                            }
                            dict.items = new_items;
                        }
                        ObjectType::MODULE => {
                            let module = &mut *(obj as *mut ObjectModule);
                            for v in module.exports.values_mut() {
                                VM::update_value_reference(v);
                            }
                        }
                        ObjectType::FUNCTION => {
                            let func = &mut *(obj as *mut ObjectFunction);
                            // Track the heap-allocated function's chunk
                            unique_chunks.insert(Rc::as_ptr(&func.chunk) as *mut crate::bytecode::Chunk);
                        }
                        _ => {}
                    }
                }
                current = (*obj).next;
            }
        }

        // 5. Safely update every unique chunk's constants exactly once
        for chunk_ptr in unique_chunks {
            unsafe {
                for constant in &mut (*chunk_ptr).constants {
                    VM::update_value_reference(constant);
                }
            }
        }
    }

    fn update_value_reference(value: &mut Value) {
        if value.is_object() {
            unsafe {
                let old_ptr = value.as_object();
                if !old_ptr.is_null() {
                    let new_ptr = (*old_ptr).forwarding;
                    if !new_ptr.is_null() {
                        // Re-tag the NaN pointer with the new sliding address
                        *value = Value::object(new_ptr);
                    }
                }
            }
        }
    }

    fn free_object(&mut self, obj: *mut crate::value::Object) {
        unsafe {
            use crate::value::{
                ObjectType, ObjectString, ObjectList, ObjectDict, 
                ObjectRange, ObjectFunction, ObjectNative, ObjectModule, ObjectFile
            };

            if std::env::var("VYPR_TRACE_GC").is_ok() {
                println!("FREE:  {:?} at {:?}", (*obj).ty, obj);
            }
            
            // `drop_in_place` calls the Rust destructors for the inner fields 
            // (like the heap-allocated Vec or String buffers) without trying 
            // to run `free()` on the pointer itself, which is exactly what we 
            // want since the struct lives in our custom bump arena!
            match (*obj).ty {
                ObjectType::STRING => std::ptr::drop_in_place(obj as *mut ObjectString),
                ObjectType::LIST => std::ptr::drop_in_place(obj as *mut ObjectList),
                ObjectType::DICT => std::ptr::drop_in_place(obj as *mut ObjectDict),
                ObjectType::FUNCTION => std::ptr::drop_in_place(obj as *mut ObjectFunction),
                ObjectType::NativeFunction => std::ptr::drop_in_place(obj as *mut ObjectNative),
                ObjectType::MODULE => std::ptr::drop_in_place(obj as *mut ObjectModule),
                ObjectType::FILE => std::ptr::drop_in_place(obj as *mut ObjectFile),
                ObjectType::RANGE => std::ptr::drop_in_place(obj as *mut ObjectRange),
            }
        }
    }

    fn blacken_object(&mut self, obj: *mut crate::value::Object) {
        unsafe {
            // Check if this object holds pointers to OTHER objects, and mark them
            match (*obj).ty {
                ObjectType::LIST => {
                    let list = &*(obj as *const ObjectList);
                    for item in &list.items {
                        self.mark_value(*item);
                    }
                }

                ObjectType::DICT => {
                    let dict = &*(obj as *const ObjectDict);
                    for (k, v) in &dict.items {
                        self.mark_value(*k);
                        self.mark_value(*v);
                    }
                }

                ObjectType::MODULE => {
                    let module = &*(obj as *const ObjectModule);
                    for v in module.exports.values() {
                        self.mark_value(*v);
                    }
                }

                ObjectType::FUNCTION => {
                    let function = &*(obj as *const ObjectFunction);
                    // MUST mark constants inside functions, or they get deleted
                    for constant in &function.chunk.constants {
                        self.mark_value(*constant);
                    }
                }

                // Strings, Ranges, Native Functions, and Files don't contain child Vypr Values, 
                // so they require no additional tracing
                _ => {}
            }
        }
    }

    pub fn run(&mut self) -> Result<(), VyprError> {
        loop {
            if self.heap.bytes_allocated > self.heap.next_gc_threshold {
                self.collect_garbage();
            }

            if self.current_frame().ip >= self.current_frame().chunk.code.len() {
                if self.frames.len() == 1 {
                    return Ok(()); 
                } else {
                    self.frames.pop(); 
                    continue;
                }
            }

            let ip = self.current_frame().ip;
            let op = self.current_frame().chunk.code[ip];
            self.current_frame_mut().ip += 1;

            match op {
                OpCode::Constant(idx) => {
                    let c = self.read_constant(idx);
                    self.push(c);
                }

                OpCode::DefineGlobal(slot, type_lock) => {
                    let val = self.pop()?;

                    // ensure the vector is resized if this is a new global slot
                    if slot >= self.globals.len() {
                        self.globals.resize(
                            slot + 1,
                            GlobalVar {
                                value: Value::none(),
                                lock: DataType::Any,
                            },
                        );
                    }

                    let existing_lock = self.globals[slot].lock;
                    if existing_lock != DataType::Any && val.get_type() != existing_lock {
                        return Err(self.error(
                            "R002",
                            format!("type error: variable slot {} locked to {}, got {}", slot, existing_lock, val.get_type()),
                        ));
                    }

                    let existing = &mut self.globals[slot];
                    existing.value = val;
                    if type_lock != DataType::Any {
                        existing.lock = type_lock;
                    }
                }

                OpCode::GetGlobal(slot) => {
                    if slot >= self.globals.len() {
                        return Err(self.error("R001", format!("undefined global variable slot {}", slot)));
                    }

                    let global_val = &self.globals[slot].value;
                    self.push(*global_val);
                }

                OpCode::SetGlobal(slot) => {
                    if slot >= self.globals.len() {
                        return Err(self.error("R001", format!("undefined global variable slot {}", slot)));
                    }

                    let new_val = self.pop()?;

                    let existing_lock = self.globals[slot].lock;
                    if existing_lock != DataType::Any && new_val.get_type() != existing_lock {
                        return Err(self.error(
                            "R002",
                            format!("type error: variable slot {} locked to {}, got {}", slot, existing_lock, new_val.get_type()),
                        ));
                    }

                    let existing = &mut self.globals[slot];
                    existing.value = new_val;
                }

                OpCode::GetLocal(slot) => {
                    let index = self.current_frame().frame_start + slot;
                    let val = self.stack[index];
                    self.push(val);
                }

                OpCode::SetLocal(slot) => {
                    let index = self.current_frame().frame_start + slot;
                    let val = self.pop()?; 
                    self.stack[index] = val; 
                }

                OpCode::Call(arg_count, kwarg_count) => {
                    let mut kwargs = Vec::new();

                    for _ in 0..kwarg_count {
                        let value = self.pop()?;
                        let key_val = self.pop()?;
                        let key = key_val.as_str().unwrap().to_string();
                        kwargs.push((key, value));
                    }

                    let callee = self.stack[self.stack.len() - 1 - arg_count];

                    self.call_value(callee, arg_count, kwargs)?
                }

                OpCode::Invoke(name_idx, arg_count, kwarg_count) => {
                    let method_name = self.read_string(name_idx)?;
                    
                    let mut kwargs = Vec::new();
                    for _ in 0..kwarg_count {
                        let value = self.pop()?;
                        let key_val = self.pop()?;
                        let key = key_val.as_str().unwrap().to_string();
                        kwargs.push((key, value));
                    }

                    let current_ip = self.current_frame().ip - 1;

                    let cached_type = {
                        let cache = self.current_frame().chunk.cache.borrow();

                        if current_ip < cache.len() {
                            cache[current_ip]
                        } else {
                            MethodCache::EMPTY
                        }
                    };

                    match cached_type {
                        MethodCache::ListAppend => {
                            let arg = self.pop()?;
                            let obj = self.pop()?;

                            if obj.is_object() {
                                unsafe {
                                    if (*obj.as_object()).ty == ObjectType::LIST {
                                        let mut_list = &mut *(obj.as_object() as *mut ObjectList);
                                        mut_list.items.push(arg);
                                        self.push(Value::none());
                                        continue; 
                                    }
                                }
                            }

                            // type changed -> slow path
                            self.push(obj);
                            self.push(arg);
                            self.set_cache(current_ip, MethodCache::EMPTY);
                        },

                        MethodCache::ListPop => {
                            if arg_count == 0 {
                                let obj = self.pop()?;

                                if obj.is_object() {
                                    unsafe {
                                        if (*obj.as_object()).ty == ObjectType::LIST {
                                            let mut_list = &mut *(obj.as_object() as *mut ObjectList);
                                            if let Some(popped) = mut_list.items.pop() {
                                                self.push(popped);
                                                continue;
                                            }
                                        }
                                    }
                                }

                                self.push(obj);
                                self.set_cache(current_ip, MethodCache::EMPTY);
                            }
                        },

                        MethodCache::StringUpper => {
                            if arg_count == 0 {
                                let obj = self.pop()?;

                                if let Some(s) = obj.as_str() {
                                    let upper = Value::make_string(&mut self.heap, &s.to_uppercase());
                                    self.push(upper);
                                    continue;
                                }

                                self.push(obj);
                                self.set_cache(current_ip, MethodCache::EMPTY);
                            }
                        },

                        MethodCache::StringLower => {
                            if arg_count == 0 {
                                let obj = self.pop()?;

                                if let Some(s) = obj.as_str() {
                                    let lower = Value::make_string(&mut self.heap, &s.to_lowercase());
                                    self.push(lower);
                                    continue;
                                }

                                self.push(obj);
                                self.set_cache(current_ip, MethodCache::EMPTY);
                            }
                        },

                        MethodCache::EMPTY => {}
                    }

                    let obj_idx = self.stack.len() - 1 - arg_count; 
                    let obj = self.stack[obj_idx];

                    if obj.is_object() {
                        unsafe {
                            let raw_obj = obj.as_object();
                            if (*raw_obj).ty == ObjectType::MODULE {
                                let module = &*(raw_obj as *const ObjectModule);
                                if let Some(val) = module.exports.get(&method_name) {
                                    if val.is_object() && (*val.as_object()).ty == ObjectType::NativeFunction {
                                        let mut args = Vec::new();
                                        for _ in 0..arg_count {
                                            args.push(self.pop()?);
                                        }
                                        args.reverse();
                                        self.pop()?; // Pop module

                                        let native = &*(val.as_object() as *const ObjectNative);
                                        let result = (native.function)(&mut self.heap, &args, &[])?;
                                        self.push(result);

                                        continue;
                                    } else {
                                        return Err(self.error("R010", format!("attribute '{}' is not callable", method_name)));
                                    }
                                } else {
                                    return Err(self.error("R011", format!("module '{}' has no attribute '{}'", module.name, method_name)));
                                }
                            }
                        }
                    }

                    self.invoke_method(name_idx, arg_count, current_ip)?;
                }

                OpCode::GetSubscript => {
                    let index_val = self.pop()?;
                    let list_val = self.pop()?;

                    if let Some(s) = list_val.as_str() {
                        if !index_val.is_int() {
                            return Err(self.error("R002", "string index must be an integer"));
                        }
                        
                        let index = index_val.as_int() as i64;
                        let char_count = s.chars().count() as i64;
                        let effective_index = if index < 0 { char_count + index } else { index };

                        if effective_index < 0 || effective_index >= char_count {
                            return Err(self.error("R003", "string index out of range"));
                        }

                        if let Some(c) = s.chars().nth(effective_index as usize) {
                            let char_str = Value::make_string(&mut self.heap, &c.to_string());
                            self.push(char_str);
                        } else {
                            return Err(self.error("R003", "string index out of range"));
                        }

                        continue;
                    }

                    if list_val.is_object() {
                        unsafe {
                            let obj = list_val.as_object();
                            match (*obj).ty {
                                ObjectType::LIST => {
                                    if !index_val.is_int() {
                                        return Err(self.error("R002", "list index must be an integer"));
                                    }
                                    let index = index_val.as_int() as i64;
                                    let list = &*(obj as *const ObjectList);
                                    
                                    let effective_index = if index < 0 { list.items.len() as i64 + index } else { index };

                                    if effective_index < 0 || effective_index >= list.items.len() as i64 {
                                        return Err(self.error("R003", "list index out of range"));
                                    }

                                    self.push(list.items[effective_index as usize]);
                                }
                                ObjectType::RANGE => {
                                    if !index_val.is_int() {
                                        return Err(self.error("R002", "range index must be an integer"));
                                    }
                                    let index = index_val.as_int() as i64;
                                    let r = &*(obj as *const ObjectRange);
                                    let start = r.start as i64;
                                    let stop = r.stop as i64;
                                    let len = if stop > start { stop - start } else { 0 };
                                    
                                    let effective_index = if index < 0 { len + index } else { index };

                                    if effective_index < 0 || effective_index >= len {
                                        return Err(self.error("R003", "range object index out of range"));
                                    }

                                    self.push(Value::int((start + effective_index) as i32));
                                }
                                ObjectType::DICT => {
                                    let dict = &*(obj as *const ObjectDict);
                                    if let Some(val) = dict.items.get(&index_val) {
                                        self.push(*val);
                                    } else {
                                        return Err(self.error("R003", format!("key '{}' does not exist", index_val)));
                                    }
                                }
                                _ => return Err(self.error("R002", "object is not subscriptable"))
                            }
                        }
                    } else {
                        return Err(self.error("R002", "object is not subscriptable"));
                    }
                }

                OpCode::SetSubscript => {
                    let index_val = self.pop()?;
                    let list_val = self.pop()?;
                    let value = self.pop()?;

                    if list_val.is_object() {
                        unsafe {
                            let obj = list_val.as_object();
                            match (*obj).ty {
                                ObjectType::LIST => {
                                    if !index_val.is_int() {
                                        return Err(self.error("R002", "list index must be an integer"));
                                    }
                                    let index = index_val.as_int() as i64;
                                    let mut_list = &mut *(obj as *mut ObjectList);
                                    
                                    let effective_index = if index < 0 { mut_list.items.len() as i64 + index } else { index };

                                    if effective_index < 0 || effective_index >= mut_list.items.len() as i64 {
                                        return Err(self.error("R003", "list assignment index out of range"));
                                    }

                                    mut_list.items[effective_index as usize] = value;
                                }
                                ObjectType::DICT => {
                                    let dict = &mut *(obj as *mut ObjectDict);
                                    dict.items.insert(index_val, value);
                                }
                                _ => return Err(self.error("R002", "object does not support item assignment"))
                            }
                        }
                    } else {
                        return Err(self.error("R002", "object does not support item assignment"));
                    }
                }

                OpCode::GetProperty(name_idx) => {
                    let property_name = self.read_string(name_idx)?;
                    let obj = self.pop()?;

                    if obj.is_object() {
                        unsafe {
                            let raw_obj = obj.as_object();
                            match (*raw_obj).ty {
                                ObjectType::MODULE => {
                                    let module = &*(raw_obj as *const ObjectModule);
                                    if let Some(val) = module.exports.get(&property_name) {
                                        self.push(*val);
                                    } else {
                                        return Err(self.error("R011", format!("module '{}' has no attribute '{}'", module.name, property_name)));
                                    }
                                }
                                _ => return Err(self.error("R012", format!("object has no attribute '{}'", property_name)))
                            }
                        }
                    } else {
                        return Err(self.error("R012", format!("object has no attribute '{}'", property_name)));
                    }
                }

                OpCode::SetProperty(name_idx) => {
                    let property_name = self.read_string(name_idx)?;
                    let _obj = self.pop()?;
                    let _value = self.pop()?;
                    
                    return Err(self.error("R013", format!("cannot set property '{}' on this object", property_name)));
                }

                OpCode::ASSERT_TYPE(expected_type) => {
                    let val = self.stack.last().ok_or_else(|| self.error("RPNC", "stack underflow on assert_type"))?;
                    
                    if expected_type != DataType::Any && val.get_type() != expected_type {
                        return Err(self.error("R002", format!(
                            "type error: expected {}, but got {}", 
                            expected_type, val.get_type()
                        )));
                    }
                }

                OpCode::BuildList(count) => {
                    let mut items = Vec::with_capacity(count);
                    for _ in 0..count {
                        items.push(self.pop()?);
                    }
                    items.reverse();
                    let list_val = Value::allocate_list(&mut self.heap, items);
                    self.push(list_val);
                }

                OpCode::BuildDict(count) => {
                    let mut dict = HashMap::with_capacity(count);

                    for _ in 0..count {
                        let value = self.pop()?;
                        let key = self.pop()?;
                        dict.insert(key, value);
                    }

                    let dict_val = Value::allocate_dict(&mut self.heap, dict);
                    self.push(dict_val);
                }

                OpCode::Length => {
                    let val = self.pop()?;
                    
                    if let Some(s) = val.as_str() {
                        self.push(Value::int(s.chars().count() as i32));
                        continue;
                    }

                    if val.is_object() {
                        unsafe {
                            let obj = val.as_object();
                            match (*obj).ty {
                                ObjectType::LIST => {
                                    let list = &*(obj as *const ObjectList);
                                    self.push(Value::int(list.items.len() as i32));
                                }
                                ObjectType::DICT => {
                                    let dict = &*(obj as *const ObjectDict);
                                    self.push(Value::int(dict.items.len() as i32));
                                }
                                ObjectType::RANGE => {
                                    let r = &*(obj as *const ObjectRange);
                                    let len = if r.stop > r.start { r.stop - r.start } else { 0 };
                                    self.push(Value::int(len as i32));
                                }
                                _ => return Err(self.error("R002", "object has no length")),
                            }
                        }
                    } else {
                        return Err(self.error("R002", "object has no length"));
                    }
                }

                OpCode::ListAppend => {
                    let item = self.pop()?;
                    let list = self.pop()?;
                    
                    if list.is_object() {
                        unsafe {
                            if (*list.as_object()).ty == ObjectType::LIST {
                                let mut_list = &mut *(list.as_object() as *mut ObjectList);
                                mut_list.items.push(item);
                                self.push(list);
                            } else {
                                return Err(self.error("R002", "append target must be a list"));
                            }
                        }
                    } else {
                        return Err(self.error("R002", "append target must be a list"));
                    }
                }

                OpCode::Pop => { self.pop()?; }

                OpCode::Jump(offset) => {
                    self.current_frame_mut().ip += offset;
                }

                OpCode::JumpIfFalse(offset) => {
                    let val = self.stack.last().expect("stack underflow in jump");
                    if !val.is_truthy() {
                        self.current_frame_mut().ip += offset;
                    }
                }

                OpCode::Loop(offset) => {
                    self.current_frame_mut().ip -= offset;
                }

                OpCode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    if let (Some(s_a), Some(s_b)) = (a.as_str(), b.as_str()) {
                        let concat = format!("{}{}", s_a, s_b);
                        let s = Value::make_string(&mut self.heap, &concat);
                        self.push(s);
                    } else if a.is_int() && b.is_int() {
                        self.push(Value::int(a.as_int() + b.as_int()));
                    } else if a.is_float() && b.is_float() {
                        self.push(Value::float(a.as_float() + b.as_float()));
                    } else if a.is_int() && b.is_float() {
                        self.push(Value::float(a.as_int() as f64 + b.as_float()));
                    } else if a.is_float() && b.is_int() {
                        self.push(Value::float(a.as_float() + b.as_int() as f64));
                    } else {
                        return Err(self.error("R002", "invalid operands for +"));
                    }
                }

                OpCode::Sub => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    if a.is_int() && b.is_int() {
                        self.push(Value::int(a.as_int() - b.as_int()));
                    } else if a.is_float() && b.is_float() {
                        self.push(Value::float(a.as_float() - b.as_float()));
                    } else if a.is_int() && b.is_float() {
                        self.push(Value::float(a.as_int() as f64 - b.as_float()));
                    } else if a.is_float() && b.is_int() {
                        self.push(Value::float(a.as_float() - b.as_int() as f64));
                    } else {
                        return Err(self.error("R002", "invalid operands for -"));
                    }
                }

                OpCode::Mul => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    if a.is_int() && b.is_int() {
                        self.push(Value::int(a.as_int() * b.as_int()));
                    } else if a.is_float() && b.is_float() {
                        self.push(Value::float(a.as_float() * b.as_float()));
                    } else if a.is_int() && b.is_float() {
                        self.push(Value::float(a.as_int() as f64 * b.as_float()));
                    } else if a.is_float() && b.is_int() {
                        self.push(Value::float(a.as_float() * b.as_int() as f64));
                    } else {
                        return Err(self.error("R002", "invalid operands for *"));
                    }
                }

                OpCode::Div => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    if a.is_int() && b.is_int() {
                        let b_int = b.as_int();
                        if b_int == 0 {
                            return Err(self.error("R007", "division by zero"));
                        }
                        self.push(Value::float(a.as_int() as f64 / b_int as f64));
                    } else if a.is_float() && b.is_float() {
                        let b_float = b.as_float();
                        if b_float == 0.0 {
                            return Err(self.error("R007", "division by zero"));
                        }
                        self.push(Value::float(a.as_float() / b_float));
                    } else if a.is_int() && b.is_float() {
                        let b_float = b.as_float();
                        if b_float == 0.0 {
                            return Err(self.error("R007", "division by zero"));
                        }
                        self.push(Value::float(a.as_int() as f64 / b_float));
                    } else if a.is_float() && b.is_int() {
                        let b_int = b.as_int();
                        if b_int == 0 {
                            return Err(self.error("R007", "division by zero"));
                        }
                        self.push(Value::float(a.as_float() / b_int as f64));
                    } else {
                        return Err(self.error("R002", "invalid operands for /"));
                    }
                }

                OpCode::Modulo => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    if a.is_int() && b.is_int() {
                        let b_int = b.as_int();
                        if b_int == 0 { return Err(self.error("R007", "modulo by zero")); }
                        self.push(Value::int(a.as_int() % b_int));
                    } else if a.is_float() && b.is_float() {
                        self.push(Value::float(a.as_float() % b.as_float()));
                    } else if a.is_int() && b.is_float() {
                        self.push(Value::float(a.as_int() as f64 % b.as_float()));
                    } else if a.is_float() && b.is_int() {
                        self.push(Value::float(a.as_float() % b.as_int() as f64));
                    } else {
                        return Err(self.error("R002", "operands must be numbers"));
                    }
                }

                OpCode::FloorDiv => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    if a.is_int() && b.is_int() {
                        let b_int = b.as_int();
                        if b_int == 0 { return Err(self.error("R007", "division by zero")); }
                        self.push(Value::int(a.as_int() / b_int));
                    } else if a.is_float() && b.is_float() {
                        self.push(Value::float((a.as_float() / b.as_float()).floor()));
                    } else if a.is_int() && b.is_float() {
                        self.push(Value::float((a.as_int() as f64 / b.as_float()).floor()));
                    } else if a.is_float() && b.is_int() {
                        self.push(Value::float((a.as_float() / b.as_int() as f64).floor()));
                    } else {
                        return Err(self.error("R002", "operands must be numbers"));
                    }
                }

                OpCode::Power => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    
                    if a.is_int() && b.is_int() {
                        let base = a.as_int();
                        let exp = b.as_int();
                        if exp < 0 {
                            self.push(Value::float((base as f64).powf(exp as f64)));
                        } else if let Ok(exp_u32) = u32::try_from(exp) {
                            match base.checked_pow(exp_u32) {
                                Some(result) => self.push(Value::int(result)),
                                None => return Err(self.error("R005", "integer overflow in power")),
                            }
                        } else {
                            return Err(self.error("R005", "exponent too large"));
                        }
                    } else if a.is_float() && b.is_float() {
                        self.push(Value::float(a.as_float().powf(b.as_float())));
                    } else if a.is_int() && b.is_float() {
                        self.push(Value::float((a.as_int() as f64).powf(b.as_float())));
                    } else if a.is_float() && b.is_int() {
                        self.push(Value::float(a.as_float().powf(b.as_int() as f64)));
                    } else {
                        return Err(self.error("R002", "operands must be numbers"));
                    }
                }

                OpCode::Equal => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Value::boolean(a == b));
                }

                OpCode::Less => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    if a.is_int() && b.is_int() {
                        self.push(Value::boolean(a.as_int() < b.as_int()));
                    } else if a.is_float() && b.is_float() {
                        self.push(Value::boolean(a.as_float() < b.as_float()));
                    } else if a.is_int() && b.is_float() {
                        self.push(Value::boolean((a.as_int() as f64) < b.as_float()));
                    } else if a.is_float() && b.is_int() {
                        self.push(Value::boolean(a.as_float() < (b.as_int() as f64)));
                    } else {
                        return Err(self.error("R002", "invalid operands for <"));
                    }
                }

                OpCode::Greater => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    if a.is_int() && b.is_int() {
                        self.push(Value::boolean(a.as_int() > b.as_int()));
                    } else if a.is_float() && b.is_float() {
                        self.push(Value::boolean(a.as_float() > b.as_float()));
                    } else if a.is_int() && b.is_float() {
                        self.push(Value::boolean((a.as_int() as f64) > b.as_float()));
                    } else if a.is_float() && b.is_int() {
                        self.push(Value::boolean(a.as_float() > (b.as_int() as f64)));
                    } else {
                        return Err(self.error("R002", "invalid operands for >"));
                    }
                }

                OpCode::LessEqual => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    if a.is_int() && b.is_int() {
                        self.push(Value::boolean(a.as_int() <= b.as_int()));
                    } else if a.is_float() && b.is_float() {
                        self.push(Value::boolean(a.as_float() <= b.as_float()));
                    } else if a.is_int() && b.is_float() {
                        self.push(Value::boolean((a.as_int() as f64) <= b.as_float()));
                    } else if a.is_float() && b.is_int() {
                        self.push(Value::boolean(a.as_float() <= (b.as_int() as f64)));
                    } else {
                        return Err(self.error("R002", "invalid types for <="));
                    }
                }

                OpCode::GreaterEqual => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    if a.is_int() && b.is_int() {
                        self.push(Value::boolean(a.as_int() >= b.as_int()));
                    } else if a.is_float() && b.is_float() {
                        self.push(Value::boolean(a.as_float() >= b.as_float()));
                    } else if a.is_int() && b.is_float() {
                        self.push(Value::boolean((a.as_int() as f64) >= b.as_float()));
                    } else if a.is_float() && b.is_int() {
                        self.push(Value::boolean(a.as_float() >= (b.as_int() as f64)));
                    } else {
                        return Err(self.error("R002", "invalid types for >="));
                    }
                }

                OpCode::Negate => {
                    let a = self.pop()?;

                    if a.is_int() {
                        self.push(Value::int(-a.as_int()));
                    } else if a.is_float() {
                        self.push(Value::float(-a.as_float()));
                    } else {
                        return Err(self.error("R002", "operand must be a number"));
                    }
                }

                OpCode::Not => {
                    let a = self.pop()?;

                    if a.is_bool() {
                        self.push(Value::boolean(!a.as_bool()));
                    } else {
                        return Err(self.error("R002", "operand must be a boolean"));
                    }
                }

                OpCode::And => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    
                    if a.is_truthy() {
                        self.push(b);
                    } else {
                        self.push(a);
                    }
                }

                OpCode::Or => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    
                    if a.is_truthy() {
                        self.push(a);
                    } else {
                        self.push(b);
                    }
                }

                OpCode::FormatString(count) => {
                    let mut parts = Vec::with_capacity(count);

                    for _ in 0..count {
                        parts.push(self.pop()?);
                    }
                    parts.reverse();

                    let mut formatted = String::new();
                    for part in parts {
                        if let Some(s) = part.as_str() {
                            formatted.push_str(s);
                        } else {
                            formatted.push_str(&part.to_string());
                        }
                    }

                    let s = Value::make_string(&mut self.heap, &formatted);
                    self.push(s);
                }

                OpCode::Return => {
                    let result = self.pop().unwrap_or(Value::none());
                    let frame = self.frames.pop().unwrap();

                    self.stack.truncate(frame.frame_start);
                    self.push(result);

                    if self.frames.is_empty() {
                        return Ok(());
                    }
                }

                OpCode::Import(idx) => {
                    let name = self.read_string(idx)?;
                    if let Some(loaded_module) = stdlib::load_module(&mut self.heap, &name) {
                        self.push(loaded_module);
                    } else {
                        return Err(self.error("R009", format!("module not found: '{}'", name)));
                    }
                }
            }
        }
    }

    // Helper to get the top frame
    fn current_frame(&self) -> &CallFrame {
        self.frames.last().expect("call stack empty")
    }

    fn current_frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().expect("call stack empty")
    }

    fn call_value(&mut self, callee: Value, arg_count: usize, kwargs: Vec<(String, Value)>) -> Result<(), VyprError> {
        if self.frames.len() >= 1000 {
            return Err(self.error("R006", "maximum recursion depth exceeded"));
        }

        if callee.is_object() {
            unsafe {
                let obj = callee.as_object();
                
                if (*obj).ty == ObjectType::NativeFunction {
                    let mut args = Vec::with_capacity(arg_count);

                    for _ in 0..arg_count {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    self.pop()?;

                    let native = &*(obj as *const ObjectNative);
                    let result = (native.function)(&mut self.heap, &args, &kwargs)?;
                    self.push(result);

                    return Ok(());
                } else if (*obj).ty == ObjectType::FUNCTION {
                    let function = &*(obj as *const ObjectFunction);
                    
                    if !kwargs.is_empty() {
                        return Err(self.error("R010", "custom kwargs are not yet supported"))
                    }

                    if arg_count != function.arity {
                        return Err(self.error("R008", format!(
                            "function expected {} arguments but got {}", 
                            function.arity, arg_count
                        )));
                    }

                    let mut args = Vec::new();
                    for _ in 0..arg_count {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    self.pop()?;

                    let new_frame_start = self.stack.len();

                    for _ in 0..function.upvalues {
                        self.push(Value::none());
                    }

                    for (i, arg) in args.iter().enumerate() {
                        if new_frame_start + 1 + i >= self.stack.len() {
                            self.stack.push(*arg);
                        } else {
                            self.stack[new_frame_start + 1 + i] = *arg;
                        }
                    }

                    self.frames.push(CallFrame {
                        chunk: function.chunk.clone(),
                        ip: 0,
                        frame_start: new_frame_start,
                    });

                    return Ok(());
                }
            }
        }
        
        Err(self.error("R004", "can only call functions"))
    }

    fn read_constant(&self, idx: usize) -> Value {
        self.current_frame().chunk.constants[idx]
    }

    pub(crate) fn read_string(&self, idx: usize) -> Result<String, VyprError> {
        match self.read_constant(idx).as_str() {
            Some(s) => Ok(s.to_string()),
            None => Err(self.error("R005", "expected string in constant pool")),
        }
    }

    pub(crate) fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub(crate) fn pop(&mut self) -> Result<Value, VyprError> {
        self.stack.pop().ok_or_else(|| self.error("RPNC", "stack underflow")) // RPNC = Runtime Panic
    }
}

impl Drop for VM {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        println!("-- VM SHUTDOWN: FREEING SURVIVING OBJECTS --");

        let mut current = self.heap.head;
        while let Some(node) = current {
            unsafe {
                let obj = node.as_ptr();
                let next = (*obj).next;
                
                self.free_object(obj);
                
                current = next;
            }
        }
    }
}
