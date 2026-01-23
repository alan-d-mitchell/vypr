use std::fmt;

use crate::bytecode::Chunk;

pub type NativeFn = fn(&[Value]) -> Value;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    None,
    Native(NativeFn),
    Function(Box<Chunk>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(v) => write!(f, "{}", v),
            Value::Float(v) => write!(f, "{}", v),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Str(v) => write!(f, "{}", v),
            Value::None => write!(f, "None"),
            Value::Native(_) => write!(f, "<native fn>"),
            Value::Function(_) => write!(f, "<fn>"),
        }
    }
}
