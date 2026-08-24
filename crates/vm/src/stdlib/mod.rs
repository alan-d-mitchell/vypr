pub mod time;
pub mod math;
pub mod os;
pub mod canvas;
pub mod random;

use error::error::{Span, VyprError};

use crate::{heap::Heap, value::Value};

pub fn error(code: &'static str, message: impl Into<String>) -> VyprError {
    VyprError::new(code, message, Span::default())
}

pub fn load_module(heap: &mut Heap, name: &str) -> Option<Value> {
    match name {
        "time" => Some(time::create_module(heap)),
        "math" => Some(math::create_module(heap)),
        "os" => Some(os::create_module(heap)),
        "canvas" => Some(canvas::create_module(heap)),
        "random" => Some(random::create_module(heap)),
        _ => None, 
    }
}
