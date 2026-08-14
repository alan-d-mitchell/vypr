use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use std::fs::File;

use error::error::VyprError;

use crate::bytecode::Chunk;

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

pub type NativeFn = fn(&[Value], &[(String, Value)]) -> Result<Value, VyprError>;

#[derive(Clone)]
pub struct NativeFunction {
    pub name: String,
    pub function: NativeFn,
}

impl PartialEq for NativeFunction {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl fmt::Debug for NativeFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<built-in function {}>", self.name)
    }
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub exports: HashMap<String, Value>,
}

impl PartialEq for Module {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

#[derive(Debug, Clone)]
pub struct SharedFile(pub Rc<RefCell<File>>);

impl PartialEq for SharedFile {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DictKey {
    Int(i64),
    Float(u64),
    Bool(bool),
    Str(Rc<String>),
    InlineStr(u8, [u8; 14]),
}

impl DictKey {

    pub fn from_value(val: &Value) -> Result<Self, String> {
        match val {
            Value::Int(i) => Ok(DictKey::Int(*i)),
            Value::Float(f) => Ok(DictKey::Float(f.to_bits())),
            Value::Bool(b) => Ok(DictKey::Bool(*b)),
            Value::Str(s) => Ok(DictKey::Str(Rc::clone(s))),
            Value::InlineStr(len, buf) => Ok(DictKey::InlineStr(*len, *buf)),
            _ => Err(format!("unhashable type: '{}'", val.get_type())),
        }
    }

    pub fn to_value(&self) -> Value {
        match self {
            DictKey::Int(i) => Value::Int(*i),
            DictKey::Float(bits) => Value::Float(f64::from_bits(*bits)),
            DictKey::Bool(b) => Value::Bool(*b),
            DictKey::Str(s) => Value::Str(Rc::clone(s)),
            DictKey::InlineStr(len, buf) => Value::InlineStr(*len, *buf),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VyprFunction {
    pub arity: usize,
    pub upvalues: usize,
    pub chunk: Rc<Chunk>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    None,
    Str(Rc<String>),
    InlineStr(u8, [u8; 14]),
    List(Rc<RefCell<Vec<Value>>>),
    Dict(Rc<RefCell<HashMap<DictKey, Value>>>),
    Range(Box<(i64, i64)>),
    Native(Rc<NativeFunction>),
    Function(Rc<VyprFunction>),
    UnloadedModule(Rc<String>),
    Module(Rc<Module>),
    File(SharedFile),
}

impl Value {

    pub fn make_string(s: &str) -> Self {
        let bytes = s.as_bytes();
        let len = bytes.len();

        if len <= 14 {
            let mut buf = [0; 14];
            buf[..len].copy_from_slice(bytes);

            Value::InlineStr(len as u8, buf)
        } else {
            Value::Str(Rc::new(s.to_string()))
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s.as_str()),
            Value::InlineStr(len, buf) => {
                let valid_str = std::str::from_utf8(&buf[..(*len as usize)]).unwrap();
                Some(valid_str)
            }
            
            _ => None
        }
    }

    pub fn get_type(&self) -> DataType {
        match self {
            Value::Int(_) => DataType::Int,
            Value::Float(_) => DataType::Float,
            Value::Bool(_) => DataType::Bool,
            Value::Str(_) | Value::InlineStr(_, _) => DataType::Str,
            Value::List(_) => DataType::List,
            Value::Dict(_) => DataType::Dict,
            Value::None => DataType::None,
            Value::Native(_) | Value::Function(_) => DataType::Function,
            Value::Range(_) => DataType::Range,
            Value::UnloadedModule(_) | Value::Module(_) => DataType::Module,
            Value::File(_) => DataType::File,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::None => false,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::InlineStr(len, _) => *len > 0,
            Value::Range(bounds) => {
                let (start, stop) = **bounds;
                start < stop
            },
            _ => true,
        }
    }

    pub fn repr(&self) -> String {
        self.repr_inner(0)
    }

    pub fn repr_inner(&self, depth: usize) -> String {
        if depth > 100 {
            return "[...]".to_string();
        }

        match self {
            Value::Str(s) => format!("'{}'", s),
            Value::InlineStr(_, _) => format!("'{}'", self.as_str().unwrap()),
            Value::List(items) => {
                let borrowed = items.borrow();
                let elements: Vec<String> = borrowed.iter().map(|v| v.repr_inner(depth + 1)).collect();

                format!("[{}]", elements.join(", "))
            },

            Value::Dict(dict) => {
                let borrowed = dict.borrow();
                let mut elements = Vec::new();
                for (k, v) in borrowed.iter() {
                    elements.push(format!("'{}': {}", k.to_value().repr_inner(depth + 1), v.repr_inner(depth + 1)));
                }

                format!("{{{}}}", elements.join(", "))
            },

            _ => format!("{}", self)
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(v) => write!(f, "{}", v),
            Value::Float(v) => {
                if v.fract() == 0.0 {
                    write!(f, "{}.0", v)
                } else {
                    write!(f, "{}", v)
                }
            }
            Value::Bool(v) => write!(f, "{}", v),
            Value::Str(v) => write!(f, "{}", v),
            Value::InlineStr(_, _) => write!(f, "{}", self.as_str().unwrap()),
            Value::List(_) => write!(f, "{}", self.repr_inner(0)),
            Value::Dict(_) => write!(f, "{}", self.repr_inner(0)),
            Value::Range(bounds) => {
                let (start, stop) = **bounds;
                write!(f, "range({}, {})", start, stop)
            }
            Value::None => write!(f, "None"),
            Value::Native(function) => write!(f, "<built-in function {}>", function.name),
            Value::Function(_) => write!(f, "<fn>"),
            
            Value::Module(m) => write!(f, "<module {}>", m.name),
            Value::UnloadedModule(name) => write!(f, "<unloaded module {}>", name),
            Value::File(_) => write!(f, "<file>"),
        }
    }
}
