use error::error::VyprError;

use crate::value::{Module, Value, NativeFunction};
use crate::stdlib::error;
use std::collections::HashMap;
use std::rc::Rc;
use std::f64::consts;

pub fn create_module() -> Value {
    let mut exports = HashMap::new();

    // Export Constants
    exports.insert("pi".to_string(), Value::Float(consts::PI));
    exports.insert("e".to_string(), Value::Float(consts::E));

    // Export Functions
    exports.insert("sqrt".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "sqrt".to_string(),
        function: math_sqrt,
    })));

    exports.insert("abs".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "abs".to_string(),
        function: math_abs,
    })));

    Value::Module(Rc::new(Module {
        name: "math".to_string(),
        exports,
    }))
}

fn math_sqrt(args: &[Value]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("sqrt() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.sqrt())),
        Value::Int(i) => Ok(Value::Float((*i as f64).sqrt())),

        _ => Err(error("R002", format!("sqrt() argument must be a number, not '{}'", args[0].get_type()))), 
    }
}

fn math_abs(args: &[Value]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("abs() takes exactly 1 argument got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.abs())),
        Value::Int(i) => Ok(Value::Int(i.abs())),

        _ => Err(error("R002", format!("abs() argument must be a number, not '{}'", args[0].get_type()))),
    }
}
