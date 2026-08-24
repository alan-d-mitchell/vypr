use error::error::VyprError;
use crate::value::Value;
use crate::stdlib::error;
use crate::heap::Heap;
use std::collections::HashMap;
use std::env;
use std::process;

pub fn create_module(heap: &mut Heap) -> Value {
    let mut exports = HashMap::new();

    exports.insert("getenv".to_string(), Value::allocate_native(heap, "getenv".to_string(), os_getenv));
    exports.insert("exit".to_string(), Value::allocate_native(heap, "exit".to_string(), os_exit));

    Value::allocate_module(heap, "os".to_string(), exports)
}

fn os_getenv(heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("getenv() takes exactly 1 argument, got {}", args.len())));
    }

    if let Some(key) = args[0].as_str() {
        match env::var(key) {
            Ok(val) => Ok(Value::make_string(heap, &val)),
            Err(_) => Ok(Value::none()),
        }
    } else {
        Err(error("R002", format!("getenv() argument must be a string, not '{}'", args[0].get_type())))
    }
}

fn os_exit(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() > 1 {
        return Err(error("R014", format!("exit() takes at most 1 argument, got {}", args.len())));
    }

    let code = if args.is_empty() {
        0
    } else if args[0].is_int() {
        args[0].as_int()
    } else {
        return Err(error("R002", format!("exit code must be an integer, not '{}'", args[0].get_type())));
    };
    
    process::exit(code); 
}
