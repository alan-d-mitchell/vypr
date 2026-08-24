use std::collections::HashMap;
use std::fmt;
use std::ptr::NonNull;
use std::rc::Rc;
use std::cell::RefCell;
use std::fs::File;

use error::error::VyprError;

use crate::bytecode::Chunk;
use crate::heap::Heap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataType {
    Int,
    Float,
    Str,
    Bool,
    List,
    Dict,
    Range,
    None,
    Function,
    Any,
    Module,
    File,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::Int => write!(f, "'int'"),
            DataType::Float => write!(f, "'float'"),
            DataType::Str => write!(f, "'str'"),
            DataType::Bool => write!(f, "'bool'"),
            DataType::List => write!(f, "'list'"),
            DataType::Dict => write!(f, "'dict'"),
            DataType::Range => write!(f, "'range'"),
            DataType::None => write!(f, "'None'"),
            DataType::Function => write!(f, "'function'"),
            DataType::Any => write!(f, "any"),
            DataType::Module => write!(f, "'module'"),
            DataType::File => write!(f, "'file'")
        }
    }
}

// ------------------------------------------------------------------------
// THE GC HEAP OBJECT HEADER
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ObjectType {
    STRING,
    LIST,
    DICT,
    RANGE,
    FUNCTION,
    NativeFunction,
    MODULE,
    FILE,
}

#[repr(C)]
pub struct Object {
    pub ty: ObjectType,
    pub is_marked: bool,
    pub next: Option<NonNull<Object>>,
    pub forwarding: *mut Object,
}

impl Object {

    pub unsafe fn size(&self) -> usize {
        match self.ty {
            ObjectType::STRING => std::mem::size_of::<ObjectString>(),
            ObjectType::LIST => std::mem::size_of::<ObjectList>(),
            ObjectType::DICT => std::mem::size_of::<ObjectDict>(),
            ObjectType::RANGE => std::mem::size_of::<ObjectRange>(),
            ObjectType::FUNCTION => std::mem::size_of::<ObjectFunction>(),
            ObjectType::NativeFunction => std::mem::size_of::<ObjectNative>(),
            ObjectType::MODULE => std::mem::size_of::<ObjectModule>(),
            ObjectType::FILE => std::mem::size_of::<ObjectFile>(),
        }
    }
}

// ------------------------------------------------------------------------
// HEAP ALLOCATED OBJECTS
// ------------------------------------------------------------------------

#[repr(C)]
pub struct ObjectString {
    pub obj: Object,
    pub chars: String,
}

#[repr(C)]
pub struct ObjectList {
    pub obj: Object,
    pub items: Vec<Value>, // pour l'instant
}

#[repr(C)]
pub struct ObjectDict {
    pub obj: Object,
    pub items: HashMap<Value, Value>,
}

#[repr(C)]
pub struct ObjectRange {
    pub obj: Object,
    pub start: i32,
    pub stop: i32,
}

#[repr(C)]
pub struct ObjectFunction {
    pub obj: Object,
    pub name: String,
    pub arity: usize,
    pub upvalues: usize,
    pub chunk: Rc<Chunk>, // pour l'instant
}

#[repr(C)]
pub struct ObjectNative {
    pub obj: Object,
    pub name: String,
    pub function: NativeFn,
}

#[repr(C)]
pub struct ObjectModule {
    pub obj: Object,
    pub name: String,
    pub exports: HashMap<String, Value>,
}

#[repr(C)]
pub struct ObjectFile {
    pub obj: Object,
    pub file: Rc<RefCell<File>>,
}

pub type NativeFn = fn(&mut Heap, &[Value], &[(String, Value)]) -> Result<Value, VyprError>;

#[derive(Debug, Copy, Clone)]
pub struct Value(pub u64);

impl Value {

    const QNAN: u64 = 0x7FFC000000000000;
    const SIGN_BIT: u64 = 0x8000000000000000;
    
    // top 32 bits dictate tag
    const TAG_NIL: u64   = Self::QNAN | 0x0000000100000000;
    const TAG_FALSE: u64 = Self::QNAN | 0x0000000200000000;
    const TAG_TRUE: u64  = Self::QNAN | 0x0000000300000000;
    const TAG_INT: u64   = Self::QNAN | 0x0000000400000000;

    const TAG_OBJ: u64   = Self::SIGN_BIT | Self::QNAN;

    #[inline]
    pub fn float(f: f64) -> Self { 
        Value(f.to_bits()) 
    }

    #[inline]
    pub fn int(i: i32) -> Self { 
        Value(Self::TAG_INT | (i as u32 as u64)) 
    }

    #[inline]
    pub fn boolean(b: bool) -> Self { 
        if b { 
            Value(Self::TAG_TRUE) 
        } else { 
            Value(Self::TAG_FALSE) 
        }
    }

    #[inline]
    pub fn none() -> Self { 
        Value(Self::TAG_NIL) 
    }

    #[inline]
    pub fn object(ptr: *mut Object) -> Self { 
        Value(Self::TAG_OBJ | (ptr as u64)) 
    }

    pub fn make_string(heap: &mut Heap, chars: &str) -> Self {
        // 1. Check the Intern Pool
        if let Some(&ptr) = heap.strings.get(chars) {
            return Self::object(ptr); // Reuse the exact same pointer
        }
        
        // 2. Allocate new string
        let obj = ObjectString {
            obj: Object { 
                ty: ObjectType::STRING, 
                is_marked: false, 
                next: None, 
                forwarding: std::ptr::null_mut() 
            },
            chars: chars.to_string(),
        };
        let ptr = heap.allocate(obj) as *mut Object;
        
        // 3. Register in pool
        heap.strings.insert(chars.to_string(), ptr);
        
        Self::object(ptr)
    }

    pub fn allocate_list(heap: &mut Heap, items: Vec<Value>) -> Self {
        let obj = ObjectList {
            obj: Object { ty: ObjectType::LIST, is_marked: false, next: None, forwarding: std::ptr::null_mut() },
            items,
        };
        Self::object(heap.allocate(obj) as *mut Object)
    }

    pub fn allocate_dict(heap: &mut Heap, items: HashMap<Value, Value>) -> Self {
        let obj = ObjectDict {
            obj: Object { ty: ObjectType::DICT, is_marked: false, next: None, forwarding: std::ptr::null_mut() },
            items,
        };
        Self::object(heap.allocate(obj) as *mut Object)
    }

    pub fn allocate_function(heap: &mut Heap, arity: usize, upvalues: usize, chunk: Rc<Chunk>) -> Self {
        let obj = ObjectFunction {
            obj: Object { ty: ObjectType::FUNCTION, is_marked: false, next: None, forwarding: std::ptr::null_mut() },
            name: "<fn>".to_string(),
            arity,
            upvalues,
            chunk,
        };
        Self::object(heap.allocate(obj) as *mut Object)
    }

    pub fn allocate_native(heap: &mut Heap, name: String, function: NativeFn) -> Self {
        let obj = ObjectNative {
            obj: Object { ty: ObjectType::NativeFunction, is_marked: false, next: None, forwarding: std::ptr::null_mut() },
            name,
            function,
        };
        Self::object(heap.allocate(obj) as *mut Object)
    }

    pub fn allocate_range(heap: &mut Heap, start: i32, stop: i32) -> Self {
        let obj = ObjectRange {
            obj: Object { ty: ObjectType::RANGE, is_marked: false, next: None, forwarding: std::ptr::null_mut() },
            start,
            stop,
        };
        Self::object(heap.allocate(obj) as *mut Object)
    }

    pub fn allocate_module(heap: &mut Heap, name: String, exports: HashMap<String, Value>) -> Self {
        let obj = ObjectModule {
            obj: Object { ty: ObjectType::MODULE, is_marked: false, next: None, forwarding: std::ptr::null_mut() },
            name,
            exports,
        };
        Self::object(heap.allocate(obj) as *mut Object)
    }

    pub fn allocate_file(heap: &mut Heap, file: Rc<RefCell<File>>) -> Self {
        let obj = ObjectFile {
            obj: Object { ty: ObjectType::FILE, is_marked: false, next: None, forwarding: std::ptr::null_mut() },
            file,
        };
        Self::object(heap.allocate(obj) as *mut Object)
    }

    // TYPE CHECKERS

    #[inline]
    pub fn is_float(&self) -> bool { (self.0 & Self::QNAN) != Self::QNAN }

    #[inline]
    pub fn is_int(&self) -> bool { (self.0 & 0xFFFFFFFF00000000) == Self::TAG_INT }

    #[inline]
    pub fn is_bool(&self) -> bool { self.0 == Self::TAG_TRUE || self.0 == Self::TAG_FALSE }

    #[inline]
    pub fn is_none(&self) -> bool { self.0 == Self::TAG_NIL }

    #[inline]
    pub fn is_object(&self) -> bool { (self.0 & Self::TAG_OBJ) == Self::TAG_OBJ }

    // --- EXTRACTORS ---

    #[inline]
    pub fn as_float(&self) -> f64 { f64::from_bits(self.0) }

    #[inline]
    pub fn as_int(&self) -> i32 { (self.0 & 0xFFFFFFFF) as i32 }

    #[inline]
    pub fn as_bool(&self) -> bool { self.0 == Self::TAG_TRUE }

    #[inline]
    pub fn as_object(&self) -> *mut Object { (self.0 & 0x0000FFFFFFFFFFFF) as *mut Object }

    pub fn as_str(&self) -> Option<&str> {
        if self.is_object() {
            unsafe {
                let obj = self.as_object();
                if (*obj).ty == ObjectType::STRING {
                    let s = &*(obj as *const ObjectString);
                    return Some(&s.chars);
                }
            }
        }
        None
    }

    pub fn get_type(&self) -> DataType {
        if self.is_float() { return DataType::Float; }
        if self.is_int() { return DataType::Int; }
        if self.is_bool() { return DataType::Bool; }
        if self.is_none() { return DataType::None; }
        
        if self.is_object() {
            unsafe {
                let obj = self.as_object();
                match (*obj).ty {
                    ObjectType::STRING => DataType::Str,
                    ObjectType::LIST => DataType::List,
                    ObjectType::DICT => DataType::Dict,
                    ObjectType::RANGE => DataType::Range,
                    ObjectType::FUNCTION | ObjectType::NativeFunction => DataType::Function,
                    ObjectType::MODULE => DataType::Module,
                    ObjectType::FILE => DataType::File,
                }
            }
        } else {
            DataType::Any
        }
    }

    pub fn is_truthy(&self) -> bool {
        if self.is_bool() { return self.as_bool(); }
        if self.is_none() { return false; }
        if self.is_int() { return self.as_int() != 0; }
        if self.is_float() { return self.as_float() != 0.0; }
        
        if self.is_object() {
            unsafe {
                let obj = self.as_object();
                match (*obj).ty {
                    ObjectType::STRING => {
                        let s = &*(obj as *const ObjectString);
                        !s.chars.is_empty()
                    }
                    ObjectType::RANGE => {
                        let r = &*(obj as *const ObjectRange);
                        r.start < r.stop
                    }
                    _ => true
                }
            }
        } else {
            true
        }
    }

    pub fn repr_inner(&self, depth: usize) -> String {
        if depth > 100 {
            return "[...]".to_string();
        }

        if self.is_object() {
            unsafe {
                let obj = self.as_object();
                match (*obj).ty {
                    ObjectType::STRING => {
                        let s = &*(obj as *const ObjectString);
                        format!("'{}'", s.chars)
                    }
                    ObjectType::LIST => {
                        let l = &*(obj as *const ObjectList);
                        let elements: Vec<String> = l.items.iter().map(|v| v.repr_inner(depth + 1)).collect();
                        format!("[{}]", elements.join(", "))
                    }
                    ObjectType::DICT => {
                        let d = &*(obj as *const ObjectDict);
                        let mut elements = Vec::new();
                        for (k, v) in &d.items {
                            elements.push(format!("{}: {}", k.repr_inner(depth + 1), v.repr_inner(depth + 1)));
                        }
                        format!("{{{}}}", elements.join(", "))
                    }
                    _ => format!("{}", self)
                }
            }
        } else {
            format!("{}", self)
        }
    }
}

impl Eq for Value {}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        if self.is_float() && other.is_float() {
            self.as_float() == other.as_float()
        } else {
            self.0 == other.0
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_float() {
            let v = self.as_float();
            if v.fract() == 0.0 { write!(f, "{}.0", v) } else { write!(f, "{}", v) }
        } else if self.is_int() {
            write!(f, "{}", self.as_int())
        } else if self.is_bool() {
            write!(f, "{}", self.as_bool())
        } else if self.is_none() {
            write!(f, "None")
        } else if self.is_object() {
            unsafe {
                let obj = self.as_object();
                match (*obj).ty {
                    ObjectType::STRING => {
                        let s = &*(obj as *const ObjectString);
                        write!(f, "{}", s.chars)
                    }
                    ObjectType::LIST | ObjectType::DICT => write!(f, "{}", self.repr_inner(0)),
                    ObjectType::RANGE => {
                        let r = &*(obj as *const ObjectRange);
                        write!(f, "range({}, {})", r.start, r.stop)
                    }
                    ObjectType::NativeFunction => {
                        let n = &*(obj as *const ObjectNative);
                        write!(f, "<built-in function {}>", n.name)
                    }
                    ObjectType::FUNCTION => write!(f, "<fn>"),
                    ObjectType::MODULE => write!(f, "<module>"),
                    ObjectType::FILE => write!(f, "<file>"),
                }
            }
        } else {
            write!(f, "unknown")
        }
    }
}

