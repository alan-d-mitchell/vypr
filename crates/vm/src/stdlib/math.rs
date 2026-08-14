use error::error::VyprError;

use crate::value::{Module, Value, NativeFunction};
use crate::stdlib::error;
use std::collections::HashMap;
use std::rc::Rc;
use std::f64::consts;

pub fn create_module() -> Value {
    let mut exports = HashMap::new();

    // Export Constants
    exports.insert("e".to_string(), Value::Float(consts::E));
    exports.insert("inf".to_string(), Value::Float(f64::INFINITY));
    exports.insert("nan".to_string(), Value::Float(f64::NAN));
    exports.insert("pi".to_string(), Value::Float(consts::PI));
    exports.insert("tau".to_string(), Value::Float(consts::TAU));

    // Export Functions
    exports.insert("sin".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "sin".to_string(),
        function: math_sin
    })));

    exports.insert("asin".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "asin".to_string(),
        function: math_asin
    })));

    exports.insert("cos".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "cos".to_string(),
        function: math_cos
    })));

    exports.insert("acos".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "acos".to_string(),
        function: math_acos
    })));

    exports.insert("tan".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "tan".to_string(),
        function: math_tan
    })));

    exports.insert("atan".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "atan".to_string(),
        function: math_atan
    })));

    exports.insert("degrees".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "degrees".to_string(),
        function: math_degrees
    })));

    exports.insert("radians".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "radians".to_string(),
        function: math_radians
    })));

    exports.insert("exp".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "exp".to_string(),
        function: math_exp
    })));

    exports.insert("ln".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "ln".to_string(),
        function: math_ln
    })));

    exports.insert("log".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "log".to_string(),
        function: math_log
    })));

    exports.insert("floor".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "floor".to_string(),
        function: math_floor
    })));

    exports.insert("ceil".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "ceil".to_string(),
        function: math_ceil
    })));

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

fn math_sin(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("sin() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.sin())),
        Value::Int(i) => Ok(Value::Float((*i as f64).sin())),

        _ => Err(error("R002", format!("sin() argument must be a number, not '{}'", args[0].get_type()))), 
    }
}

fn math_asin(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("asin() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.asin())),
        Value::Int(i) => Ok(Value::Float((*i as f64).asin())),

        _ => Err(error("R002", format!("asin() argument must be a number, not '{}'", args[0].get_type()))), 
    }
}

fn math_cos(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("cos() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.cos())),
        Value::Int(i) => Ok(Value::Float((*i as f64).cos())),

        _ => Err(error("R002", format!("cos() argument must be a number, not '{}'", args[0].get_type()))), 
    }
}

fn math_acos(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("acos() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.acos())),
        Value::Int(i) => Ok(Value::Float((*i as f64).acos())),

        _ => Err(error("R002", format!("acos() argument must be a number, not '{}'", args[0].get_type()))), 
    }
}

fn math_tan(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("tan() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.tan())),
        Value::Int(i) => Ok(Value::Float((*i as f64).tan())),

        _ => Err(error("R002", format!("tan() argument must be a number, not '{}'", args[0].get_type()))), 
    }
}

fn math_atan(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("atan() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.atan())),
        Value::Int(i) => Ok(Value::Float((*i as f64).atan())),

        _ => Err(error("R002", format!("atan() argument must be a number, not '{}'", args[0].get_type()))), 
    }
}

fn math_degrees(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("degrees() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.to_degrees())),
        Value::Int(i) => Ok(Value::Float((*i as f64).to_degrees())),

        _ => Err(error("R002", format!("degrees() argument must be a number, not {}", args[0].get_type()))),
    }
}

fn math_radians(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("radians() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.to_radians())),
        Value::Int(i) => Ok(Value::Float((*i as f64).to_radians())),

        _ => Err(error("R002", format!("radians() argument must be a number, not {}", args[0].get_type()))),
    }
}

fn math_exp(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("exp() takes exactly 1 argument, got {}", args.len())));
    }
    
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.exp())),
        Value::Int(i) => Ok(Value::Float((*i as f64).exp())),

        _ => Err(error("R002", format!("exp() argument must be a number, not {}", args[0].get_type()))),
    }
}

fn math_ln(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("ln() takes exactly 1 argument, got {}", args.len())));
    }
    
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.ln())),
        Value::Int(i) => Ok(Value::Float((*i as f64).ln())),

        _ => Err(error("R002", format!("ln() argument must be a number, not {}", args[0].get_type()))),
    }
}

fn math_log(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("log() takes exactly 1 argument, got {}", args.len())));
    }
    
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.log10())),
        Value::Int(i) => Ok(Value::Float((*i as f64).log10())),

        _ => Err(error("R002", format!("log() argument must be a number, not {}", args[0].get_type()))),
    }
}

fn math_floor(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("floor() takes exactly 1 argument, got {}", args.len())));
    }
    
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.floor())),
        Value::Int(i) => Ok(Value::Float((*i as f64).floor())),

        _ => Err(error("R002", format!("floor() argument must be a number, not {}", args[0].get_type()))),
    }
}

fn math_ceil(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("ceil() takes exactly 1 argument, got {}", args.len())));
    }
    
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.ceil())),
        Value::Int(i) => Ok(Value::Float((*i as f64).ceil())),

        _ => Err(error("R002", format!("ceil() argument must be a number, not {}", args[0].get_type()))),
    }
}

fn math_sqrt(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("sqrt() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.sqrt())),
        Value::Int(i) => Ok(Value::Float((*i as f64).sqrt())),

        _ => Err(error("R002", format!("sqrt() argument must be a number, not '{}'", args[0].get_type()))), 
    }
}

fn math_abs(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("abs() takes exactly 1 argument got {}", args.len())));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.abs())),
        Value::Int(i) => Ok(Value::Int(i.abs())),

        _ => Err(error("R002", format!("abs() argument must be a number, not '{}'", args[0].get_type()))),
    }
}


