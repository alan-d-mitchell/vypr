use error::error::VyprError;

use crate::value::{Module, Value, NativeFunction};
use crate::stdlib::error;
use std::collections::HashMap;
use std::rc::Rc;
use std::env;
use std::process;

pub fn create_module() -> Value {
    let mut exports = HashMap::new();

    exports.insert("getenv".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "getenv".to_string(),
        function: os_getenv,
    })));

    exports.insert("exit".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "exit".to_string(),
        function: os_exit,
    })));

    Value::Module(Rc::new(Module {
        name: "os".to_string(),
        exports,
    }))
}

fn os_getenv(args: &[Value]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("getenv() takes exactly 1 argument, got {}", args.len())));
    }

    if let Some(key) = args[0].as_str() {
        match env::var(key) {
            Ok(val) => Ok(Value::make_string(&val)),
            Err(_) => Ok(Value::None),
        }
    } else {
        Err(error("R002", format!("getenv() argument must be a string, not '{}'", args[0].get_type())))
    }
}

fn os_exit(args: &[Value]) -> Result<Value, VyprError> {
    if args.len() > 1 {
        return Err(error("R014", format!("exit() takes at most 1 argument, got {}", args.len())));
    }

    let code = if args.is_empty() {
        0
    } else if let Value::Int(i) = &args[0] {
        *i as i32
    } else {
        return Err(error("R002", format!("exit code must be an integer, not '{}'", args[0].get_type())));
    };
    
    process::exit(code); 
}
