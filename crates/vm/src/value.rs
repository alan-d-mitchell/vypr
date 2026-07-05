use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;

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
        }
    }
}

pub type NativeFn = fn(&[Value]) -> Result<Value, VyprError>;

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

#[derive(Debug, Clone, PartialEq)]
pub struct VyprFunction {
    pub arity: usize,
    pub upvalues: usize,
    pub chunk: Chunk
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    None,
    Str(Rc<String>),
    List(Rc<RefCell<Vec<Value>>>),
    Dict(Rc<RefCell<HashMap<String, Value>>>),
    Range(Box<(i64, i64)>),
    Native(Rc<NativeFunction>),
    Function(Rc<VyprFunction>),
    UnloadedModule(Rc<String>),
    Module(Rc<Module>),
}

impl Value {

    pub fn get_type(&self) -> DataType {
        match self {
            Value::Int(_) => DataType::Int,
            Value::Float(_) => DataType::Float,
            Value::Bool(_) => DataType::Bool,
            Value::Str(_) => DataType::Str,
            Value::List(_) => DataType::List,
            Value::Dict(_) => DataType::Dict,
            Value::None => DataType::None,
            Value::Native(_) | Value::Function(_) => DataType::Function,
            Value::Range(_) => DataType::Range,
            Value::UnloadedModule(_) | Value::Module(_) => DataType::Module,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::None => false,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
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
            Value::List(items) => {
                let borrowed = items.borrow();
                let elements: Vec<String> = borrowed.iter().map(|v| v.repr_inner(depth + 1)).collect();

                format!("[{}]", elements.join(", "))
            },

            Value::Dict(dict) => {
                let borrowed = dict.borrow();
                let mut elements = Vec::new();
                for (k, v) in borrowed.iter() {
                    elements.push(format!("'{}': {}", k, v.repr_inner(depth + 1)));
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
        }
    }
}
