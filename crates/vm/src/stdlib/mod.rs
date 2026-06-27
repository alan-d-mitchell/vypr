pub mod time;
pub mod math;
pub mod os;
pub mod canvas;

use error::error::{Span, VyprError};

use crate::value::Value;

pub fn error(code: &'static str, message: impl Into<String>) -> VyprError {
    VyprError::new(code, message, Span::default())
}

pub fn load_module(name: &str) -> Option<Value> {
    match name {
        "time" => Some(time::create_module()),
        "math" => Some(math::create_module()),
        "os" => Some(os::create_module()),
        "canvas" => Some(canvas::create_module()),
        _ => None, 
    }
}
