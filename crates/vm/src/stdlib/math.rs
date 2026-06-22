use crate::value::{Module, Value, NativeFunction};
use std::collections::HashMap;
use std::rc::Rc;
use std::f64::consts;

pub fn create_module() -> Value {
    let mut exports = HashMap::new();

    // Export Constants
    exports.insert("pi".to_string(), Value::Float(consts::PI));
    exports.insert("e".to_string(), Value::Float(consts::E));

    // Export Functions
    exports.insert("sqrt".to_string(), Value::Native(NativeFunction {
        name: "sqrt".to_string(),
        function: math_sqrt,
    }));

    exports.insert("abs".to_string(), Value::Native(NativeFunction {
        name: "abs".to_string(),
        function: math_abs,
    }));

    Value::Module(Rc::new(Module {
        name: "math".to_string(),
        exports,
    }))
}

fn math_sqrt(args: &[Value]) -> Value {
    if let Some(arg) = args.first() {
        match arg {
            Value::Float(f) => Value::Float(f.sqrt()),
            Value::Int(i) => Value::Float((*i as f64).sqrt()),
            _ => Value::None, 
        }
    } else {
        Value::None
    }
}

fn math_abs(args: &[Value]) -> Value {
    if let Some(arg) = args.first() {
        match arg {
            Value::Float(f) => Value::Float(f.abs()),
            Value::Int(i) => Value::Int(i.abs()),
            _ => Value::None,
        }
    } else {
        Value::None
    }
}
