use crate::value::{Module, Value, NativeFunction};
use std::collections::HashMap;
use std::rc::Rc;
use std::env;
use std::process;

pub fn create_module() -> Value {
    let mut exports = HashMap::new();

    exports.insert("getenv".to_string(), Value::Native(NativeFunction {
        name: "getenv".to_string(),
        function: os_getenv,
    }));

    exports.insert("exit".to_string(), Value::Native(NativeFunction {
        name: "exit".to_string(),
        function: os_exit,
    }));

    Value::Module(Rc::new(Module {
        name: "os".to_string(),
        exports,
    }))
}

fn os_getenv(args: &[Value]) -> Value {
    if let Some(Value::Str(key)) = args.first() {
        match env::var(key) {
            Ok(val) => Value::Str(val),
            Err(_) => Value::None,
        }
    } else {
        Value::None
    }
}

fn os_exit(args: &[Value]) -> Value {
    let code = if let Some(Value::Int(i)) = args.first() {
        *i as i32
    } else {
        0
    };
    
    // Instantly terminates the Rust host process
    process::exit(code); 
}
