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

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    List(Rc<RefCell<Vec<Value>>>),
    Range(i64, i64),
    None,
    Native(NativeFn),
    Function(Box<Chunk>),
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
            Value::Native(_) | Value::Function(_) => DataType::Function,
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
            Value::Range(start, stop) => start > stop,
            _ => true,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(v) => write!(f, "{}", v),
            Value::Float(v) => write!(f, "{}", v),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Str(v) => write!(f, "{}", v),
            Value::List(items) => {
                write!(f, "[")?;

                for (i, item) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }

                Ok(write!(f, "]")?)
            }
            Value::Range(start, stop) => write!(f, "range({}, {})", start, stop),
            Value::None => write!(f, "None"),
            Value::Native(_) => write!(f, "<native fn>"),
            Value::Function(_) => write!(f, "<fn>"),
        }
    }
}
