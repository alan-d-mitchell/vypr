pub mod time;
pub mod math;
pub mod os;

use crate::value::Value;

pub fn load_module(name: &str) -> Option<Value> {
    match name {
        "time" => Some(time::create_module()),
        "math" => Some(math::create_module()),
        "os" => Some(os::create_module()),
        _ => None, 
    }
}
