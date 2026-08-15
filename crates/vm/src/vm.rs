use std::{cell::RefCell, collections::HashMap, rc::Rc};
use error::error::{Span, VyprError};
use crate::{builtins, bytecode::{Chunk, OpCode}, stdlib, value::{DataType, DictKey, NativeFunction, Value}};

#[derive(Clone)]
struct GlobalVar {
    value: Value,
    lock: DataType,
}

struct CallFrame {
    chunk: Rc<Chunk>, // The code being executed
    ip: usize,    // Instruction pointer for this frame
    frame_start: usize, // where function locals begin on the stack
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
    frames: Vec<CallFrame>, // The call stack
    stack: Vec<Value>,      // The operand stack
    globals: Vec<GlobalVar>,
}

impl VM {

    pub fn new(chunk: Chunk) -> Self {
        let globals = vec![
            GlobalVar { value: Value::Native(Rc::new(NativeFunction { name: "print".into(), function: builtins::vypr_print })), lock: DataType::Function },
            GlobalVar { value: Value::Native(Rc::new(NativeFunction { name: "int".into(), function: builtins::vypr_int })), lock: DataType::Function },
            GlobalVar { value: Value::Native(Rc::new(NativeFunction { name: "float".into(), function: builtins::vypr_float })), lock: DataType::Function },
            GlobalVar { value: Value::Native(Rc::new(NativeFunction { name: "str".into(), function: builtins::vypr_str })), lock: DataType::Function },
            GlobalVar { value: Value::Native(Rc::new(NativeFunction { name: "len".into(), function: builtins::vypr_len })), lock: DataType::Function },
            GlobalVar { value: Value::Native(Rc::new(NativeFunction { name: "range".into(), function: builtins::vypr_range })), lock: DataType::Function },
            GlobalVar { value: Value::Native(Rc::new(NativeFunction { name: "list".into(), function: builtins::vypr_list })), lock: DataType::Function },
            GlobalVar { value: Value::Native(Rc::new(NativeFunction { name: "reversed".into(), function: builtins::vypr_reversed })), lock: DataType::Function },
            GlobalVar { value: Value::Native(Rc::new(NativeFunction { name: "input".into(), function: builtins::vypr_input })), lock: DataType::Function },
            GlobalVar { value: Value::Native(Rc::new(NativeFunction { name: "open".into(), function: builtins::vypr_open })), lock: DataType::Function },
        ];

        let mut chunk = chunk;
        chunk.init_cache();

        let main_frame = CallFrame {
            chunk: Rc::new(chunk),
            ip: 0,
            frame_start: 0,
        };

        Self {
            frames: vec![main_frame],
            stack: Vec::new(),
            globals,
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

    pub fn run(&mut self) -> Result<(), VyprError> {
        loop {
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
                                value: Value::None,
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

                    if let Value::UnloadedModule(mod_name) = global_val {
                        let mod_name = mod_name.clone();
                        if let Some(loaded_module) = stdlib::load_module(&mod_name) {
                            self.globals[slot].value = loaded_module.clone();
                            self.push(loaded_module);
                            continue;
                        } else {
                            return Err(self.error("R009", format!("module not found: '{}'", mod_name)));
                        }
                    }

                    self.push(global_val.clone());
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
                    let val = self.stack[index].clone();
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

                    let callee = self.stack[self.stack.len() - 1 - arg_count].clone();

                    self.call_value(callee, arg_count, kwargs)?
                }

                OpCode::Invoke(name_idx, arg_count, kwarg_count) => {
                    let _method_name = self.read_string(name_idx);
                    
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

                            // safety guard: check if variable is still a list
                            if let Value::List(ref items) = obj {
                                items.borrow_mut().push(arg);
                                self.push(Value::None);
                                continue; // instant jump to next opcode
                            } else {
                                // type changed -> slow path
                                self.push(obj);
                                self.push(arg);
                                self.set_cache(current_ip, MethodCache::EMPTY);
                            }
                        },

                        MethodCache::ListPop => {
                            if arg_count == 0 {
                                let obj = self.pop()?;

                                if let Value::List(ref items) = obj {
                                    if let Some(popped) = items.borrow_mut().pop() {
                                        self.push(popped);
                                        continue;
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
                                    let len = s.len();

                                    // fast path: do sso transformation inline
                                    if len <= 14 && s.is_ascii() {
                                        let mut buf = [0u8; 14];
                                        buf[..len].copy_from_slice(s.as_bytes());
                                        buf[..len].make_ascii_uppercase();
                                        self.push(Value::InlineStr(len as u8, buf));
                                    } else {
                                        self.push(Value::make_string(&s.to_uppercase()));
                                    }

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
                                    let len = s.len();

                                    // fast path: do sso transformation inline
                                    if len <= 14 && s.is_ascii() {
                                        let mut buf = [0u8; 14];
                                        buf[..len].copy_from_slice(s.as_bytes());
                                        buf[..len].make_ascii_lowercase();
                                        self.push(Value::InlineStr(len as u8, buf));
                                    } else {
                                        self.push(Value::make_string(&s.to_lowercase()));
                                    }

                                    continue;
                                }

                                self.push(obj);
                                self.set_cache(current_ip, MethodCache::EMPTY);
                            }
                        },

                        MethodCache::EMPTY => {}
                    }

                    let method_name = self.read_string(name_idx)?;

                    let obj_idx = self.stack.len() - 1 - arg_count; 
                    let obj = self.stack[obj_idx].clone();

                    if let Value::Module(module) = obj {
                        if let Some(val) = module.exports.get(&method_name) {
                            if let Value::Native(native) = val {
                                let mut args = Vec::new();
                                for _ in 0..arg_count {
                                    args.push(self.pop()?);
                                }
                                args.reverse();

                                self.pop()?; 

                                let result = (native.function)(&args, &[])?;
                                self.push(result);

                                continue;
                            } else {
                                return Err(self.error("R010", format!("attribute '{}' is not callable", method_name)));
                            }
                        } else {
                            return Err(self.error("R011", format!("module '{}' has no attribute '{}'", module.name, method_name)));
                        }
                    }

                    self.invoke_method(name_idx, arg_count, current_ip)?;
                }

                OpCode::GetSubscript => {
                    let index_val = self.pop()?;
                    let list_val = self.pop()?;

                    if let Some(s) = list_val.as_str() {
                        let index = match index_val {
                            Value::Int(i) => i,
                            _ => return Err(self.error("R002", "string index must be an integer"))
                        };

                        let char_count = s.chars().count() as i64;
                        let effective_index = if index < 0 {
                            char_count + index
                        } else {
                            index
                        };

                        if effective_index < 0 || effective_index >= char_count {
                            return Err(self.error("R003", "string index out of range"));
                        }

                        if let Some(c) = s.chars().nth(effective_index as usize) {
                            self.push(Value::make_string(&c.to_string()));
                        } else {
                            return Err(self.error("R003", "string index out of range"));
                        }

                        continue;
                    }

                    match list_val {
                        Value::List(items) => {
                            let index = match index_val {
                                Value::Int(i) => i,
                                _ => return Err(self.error("R002", "list index must be an integer"))
                            };

                            let borrowed = items.borrow();

                            let effective_index = if index < 0 {
                                borrowed.len() as i64 + index
                            } else {
                                index
                            };

                            if effective_index < 0 || effective_index >= borrowed.len() as i64 {
                                return Err(self.error("R003", "list index out of range"));
                            }

                            self.push(borrowed[effective_index as usize].clone());
                        }

                        Value::Range(bounds) => {
                            let (start, stop) = *bounds;
                            let index = match index_val {
                                Value::Int(i) => i,
                                _ => return Err(self.error("R002", "range index must be an integer"))
                            };

                            let len = if stop > start { stop - start } else { 0 };
                            let effective_index = if index < 0 { len + index } else { index };

                            if effective_index < 0 || effective_index >= len {
                                return Err(self.error("R003", "range object index out of range"));
                            }

                            self.push(Value::Int(start + effective_index));
                        }

                        Value::Dict(dict) => {
                            let dict_key = match DictKey::from_value(&index_val) {
                                Ok(k) => k,
                                Err(msg) => return Err(self.error("R002", msg))
                            };
                            
                            if let Some(val) = dict.borrow().get(&dict_key) {
                                self.push(val.clone());
                            } else {
                                return Err(self.error("R003", format!("key '{}' does not exist", dict_key.to_value())));
                            }
                        }

                        _ => return Err(self.error("R002", "object is not subscriptable"))
                    }
                }

                OpCode::SetSubscript => {
                    let index_val = self.pop()?;
                    let list_val = self.pop()?;
                    let value = self.pop()?;

                    match list_val {
                        Value::List(items) => {
                            let index = match index_val {
                                Value::Int(i) => i,
                                _ => return Err(self.error("R002", "list index must be an integer"))
                            };
                            let mut borrowed = items.borrow_mut();
                            let effective_index = if index < 0 {
                                borrowed.len() as i64 + index
                            } else {
                                index
                            };

                            if effective_index < 0 || effective_index >= borrowed.len() as i64 {
                                return Err(self.error("R003", "list assignment index out of range"));
                            }

                            borrowed[effective_index as usize] = value;
                        }
                        
                        Value::Dict(dict) => {
                            let dict_key = match DictKey::from_value(&index_val) {
                                Ok(k) => k,
                                Err(msg) => return Err(self.error("R002", msg)),
                            };

                            dict.borrow_mut().insert(dict_key, value);
                        }

                        _ => return Err(self.error("R002", "object does not support item assignment"))
                    }
                }

                OpCode::GetProperty(name_idx) => {
                    let property_name = self.read_string(name_idx)?;
                    let obj = self.pop()?;

                    match obj {
                        Value::Module(module) => {
                            if let Some(val) = module.exports.get(&property_name) {
                                self.push(val.clone());
                            } else {
                                return Err(self.error("R011", format!("module '{}' has no attribute '{}'", module.name, property_name)));
                            }
                        }
                        _ => return Err(self.error("R012", format!("object has no attribute '{}'", property_name)))
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
                    self.push(Value::List(Rc::new(RefCell::new(items))));
                }

                OpCode::BuildDict(count) => {
                    let mut dict = HashMap::with_capacity(count);

                    for _ in 0..count {
                        let value = self.pop()?;
                        let key = self.pop()?;

                        let dict_key = match DictKey::from_value(&key) {
                            Ok(k) => k,
                            Err(msg) => return Err(self.error("R002", msg)),
                        };

                        dict.insert(dict_key, value);
                    }

                    self.push(Value::Dict(Rc::new(RefCell::new(dict))));
                }

                OpCode::Length => {
                    let val = self.pop()?;
                    
                    if let Some(s) = val.as_str() {
                        self.push(Value::Int(s.chars().count() as i64));
                        continue;
                    }

                    match val {
                        Value::List(items) => self.push(Value::Int(items.borrow().len() as i64)),
                        Value::Dict(dict) => self.push(Value::Int(dict.borrow().len() as i64)),
                        Value::Range(bounds) => {
                            let (start, stop) = *bounds;
                            let len = if stop > start { stop - start } else { 0 };
                            self.push(Value::Int(len));
                        }
                        _ => return Err(self.error("R002", "object has no length")),
                    }
                }

                OpCode::ListAppend => {
                    let item = self.pop()?;
                    let list = self.pop()?;
                    
                    if let Value::List(items) = &list {
                        items.borrow_mut().push(item);
                        self.push(list);                    
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
                        self.push(Value::make_string(&format!("{}{}", s_a, s_b)));
                    } else {
                        match (a, b) {
                            (Value::Int(a), Value::Int(b)) => self.push(Value::Int(a + b)),
                            (Value::Float(a), Value::Float(b)) => self.push(Value::Float(a + b)),

                            (Value::Int(a), Value::Float(b)) => self.push(Value::Float(a as f64 + b)),
                            (Value::Float(a), Value::Int(b)) => self.push(Value::Float(a + b as f64)),

                            _ => return Err(self.error("R002", "invalid operands for +")),
                        }
                    }
                }

                OpCode::Sub => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Int(a - b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float(a - b)),

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Float(a as f64 + b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Float(a + b as f64)),

                        _ => return Err(self.error("R002", "invalid operands for -")),
                    }
                }

                OpCode::Mul => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Int(a * b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float(a * b)),

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Float(a as f64 * b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Float(a * b as f64)),

                        _ => return Err(self.error("R002", "invalid operands for *")),
                    }
                }

                OpCode::Div => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => {
                            if b == 0 {
                                return Err(self.error("R007", "division by zero"));
                            }

                            self.push(Value::Float(a as f64 / b as f64)) 
                        }
                        (Value::Float(a), Value::Float(b)) => {
                            self.push(Value::Float(a / b))
                        }

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Float(a as f64 / b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Float(a / b as f64)),
                        _ => return Err(self.error("R002", "invalid operands for /")),
                    }
                }

                OpCode::Modulo => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => {
                            if b == 0 {
                                return Err(self.error("R007", "modulo by zero"));
                            }
                            self.push(Value::Int(a % b))
                        }
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float(a % b)),

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Float(a as f64 % b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Float(a % b as f64)),

                        _ => return Err(self.error("R002", "operands must be numbers")),
                    }
                }

                OpCode::FloorDiv => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => {
                            if b == 0 {
                                return Err(self.error("R007", "division by zero"));
                            }
                            self.push(Value::Int(a / b))
                        }
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float((a / b).floor())),

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Float((a as f64 / b).floor())),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Float((a / b as f64).floor())),

                        _ => return Err(self.error("R002", "operands must be numbers")),
                    }
                }

                OpCode::Power => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    
                    match (a, b) {
                        (Value::Int(base), Value::Int(exp)) => {
                            if exp < 0 {
                                let result = (base as f64).powf(exp as f64);
                                self.push(Value::Float(result));
                            } else if let Ok(exp_u32) = u32::try_from(exp) {
                                match base.checked_pow(exp_u32) {
                                    Some(result) => self.push(Value::Int(result)),
                                    None => return Err(self.error("R005", "integer overflow in power".to_string())),
                                }
                            } else {
                                return Err(self.error("R005", "exponent too large"));
                            }
                        }

                        (Value::Float(base), Value::Float(exp)) => {
                            self.push(Value::Float(base.powf(exp)));
                        }

                        (Value::Int(base), Value::Float(exp)) => {
                            self.push(Value::Float((base as f64).powf(exp)));
                        }

                        (Value::Float(base), Value::Int(exp)) => {
                            self.push(Value::Float(base.powf(exp as f64)));
                        }

                        _ => return Err(self.error("R002", "operands must be numbers"))
                    }
                }

                OpCode::Equal => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Value::Bool(a == b));
                }

                OpCode::Less => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Bool(a < b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Bool(a < b)),
                        
                        (Value::Int(a), Value::Float(b)) => self.push(Value::Bool((a as f64) < b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Bool(a < (b as f64))),
                        
                        _ => return Err(self.error("R002", "invalid operands for <")),
                    }
                }

                OpCode::Greater => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Bool(a > b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Bool(a > b)),
                        
                        (Value::Int(a), Value::Float(b)) => self.push(Value::Bool((a as f64) > b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Bool(a > (b as f64))),
                        
                        _ => return Err(self.error("R002", "invalid operands for >")),
                    }
                }

                OpCode::LessEqual => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Bool(a <= b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Bool(a <= b)),

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Bool((a as f64) <= b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Bool(a <= (b as f64))),
                        _ => return Err(self.error("R002", "invalid types for <=")),
                    }
                }

                OpCode::GreaterEqual => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Bool(a >= b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Bool(a >= b)),

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Bool((a as f64) >= b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Bool(a >= (b as f64))),
                        _ => return Err(self.error("R002", "invalid types for >=")),
                    }
                }

                OpCode::Negate => {
                    let a = self.pop()?;

                    match a {
                        Value::Int(i) => self.push(Value::Int(-i)),
                        Value::Float(f) => self.push(Value::Float(-f)),
                        _ => return Err(self.error("R002", "operand must be a number")),
                    }
                }

                OpCode::Not => {
                    let a = self.pop()?;

                    match a {
                        Value::Bool(b) => self.push(Value::Bool(!b)),
                        _ => return Err(self.error("R002", "operand must be a boolean")),
                    }
                }

                OpCode::And => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    
                    if a.is_truthy() {
                        self.push(b);
                    } else {
                        self.push(a.clone());
                    }
                }

                OpCode::Or => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    
                    if a.is_truthy() {
                        self.push(a.clone());
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
                            match part {
                                Value::Int(i) => formatted.push_str(&i.to_string()),
                                Value::Float(f) => formatted.push_str(&f.to_string()),
                                Value::Bool(b) => formatted.push_str(if b { "true" } else { "false" }),
                                Value::List(_) | Value::Range(_) => formatted.push_str(&part.to_string()),
                                Value::None => formatted.push_str("None"),
                                _ => return Err(self.error("R002", "cannot format this type"))
                            }
                        }
                    }

                    self.push(Value::make_string(&formatted))
                }

                OpCode::Return => {
                    let result = self.pop().unwrap_or(Value::None);
                    let frame = self.frames.pop().unwrap();

                    self.stack.truncate(frame.frame_start);
                    self.push(result);

                    if self.frames.is_empty() {
                        return Ok(());
                    }
                }

                OpCode::Import(idx) => {
                    let name = self.read_string(idx)?;
                    self.push(Value::UnloadedModule(Rc::new(name)));
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

        let _frame_start = self.stack.len() - arg_count;

        let func_idx = self.stack.len() - 1 - arg_count;
        let callee = self.stack[func_idx].clone();

        match callee {
            Value::Native(native) => {
                let mut args = Vec::with_capacity(arg_count);

                for _ in 0..arg_count {
                    args.push(self.pop()?);
                }
                args.reverse();

                self.pop()?;

                let result = (native.function)(&args, &kwargs)?;
                self.push(result);

                Ok(())
            }

            Value::Function(function) => {
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
                    self.push(Value::None);
                }

                for (i, arg) in args.iter().enumerate() {
                    self.stack[new_frame_start + 1 + i] = arg.clone();
                }

                self.frames.push(CallFrame {
                    chunk: function.chunk.clone(),
                    ip: 0,
                    frame_start: new_frame_start,
                });

                Ok(())
            } 

            _ => Err(self.error("R004", "can only call functions"))
        }
    }

    fn read_constant(&self, idx: usize) -> Value {
        self.current_frame().chunk.constants[idx].clone()
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
