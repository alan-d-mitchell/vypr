use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;

use crate::bytecode::Chunk;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataType {
    Int,
    Float,
    Str,
    Bool,
    List,
    Range,
    None,
    Function,
    Any,
}

pub type NativeFn = fn(&[Value]) -> Value;

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

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    List(Rc<RefCell<Vec<Value>>>),
    Range(i64, i64),
    None,
    Native(NativeFunction),
    Function(usize, Box<Chunk>),
}

impl Value {

    pub fn get_type(&self) -> DataType {
        match self {
            Value::Int(_) => DataType::Int,
            Value::Float(_) => DataType::Float,
            Value::Bool(_) => DataType::Bool,
            Value::Str(_) => DataType::Str,
            Value::List(_) => DataType::List,
            Value::None => DataType::None,
            Value::Native(_) | Value::Function(_, _) => DataType::Function,
            Value::Range(_, _) => DataType::Range,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::None => false,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Range(start, stop) => start < stop,
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

                return format!("[{}]", elements.join(", "));
            },

            _ => return format!("{}", self)
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
            Value::Range(start, stop) => write!(f, "range({}, {})", start, stop),
            Value::None => write!(f, "None"),
            Value::Native(function) => write!(f, "<built-in function {}>", function.name),
            Value::Function(_, _) => write!(f, "<fn>"),
        }
    }
}
