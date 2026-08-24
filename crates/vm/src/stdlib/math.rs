use error::error::VyprError;
use crate::heap::Heap;
use crate::value::Value;
use crate::stdlib::error;
use std::collections::HashMap;
use std::f64::consts;

pub fn create_module(heap: &mut Heap) -> Value {
    let mut exports = HashMap::new();

    // Export Constants
    exports.insert("e".to_string(), Value::float(consts::E));
    exports.insert("inf".to_string(), Value::float(f64::INFINITY));
    exports.insert("nan".to_string(), Value::float(f64::NAN));
    exports.insert("pi".to_string(), Value::float(consts::PI));
    exports.insert("tau".to_string(), Value::float(consts::TAU));

    // Export Functions
    exports.insert("sin".to_string(), Value::allocate_native(heap, "sin".to_string(), math_sin));
    exports.insert("asin".to_string(), Value::allocate_native(heap, "asin".to_string(), math_asin));
    exports.insert("cos".to_string(), Value::allocate_native(heap, "cos".to_string(), math_cos));
    exports.insert("acos".to_string(), Value::allocate_native(heap, "acos".to_string(), math_acos));
    exports.insert("tan".to_string(), Value::allocate_native(heap, "tan".to_string(), math_tan));
    exports.insert("atan".to_string(), Value::allocate_native(heap, "atan".to_string(), math_atan));
    exports.insert("degrees".to_string(), Value::allocate_native(heap, "degrees".to_string(), math_degrees));
    exports.insert("radians".to_string(), Value::allocate_native(heap, "radians".to_string(), math_radians));
    exports.insert("exp".to_string(), Value::allocate_native(heap, "exp".to_string(), math_exp));
    exports.insert("ln".to_string(), Value::allocate_native(heap, "ln".to_string(), math_ln));
    exports.insert("log".to_string(), Value::allocate_native(heap, "log".to_string(), math_log));
    exports.insert("floor".to_string(), Value::allocate_native(heap, "floor".to_string(), math_floor));
    exports.insert("ceil".to_string(), Value::allocate_native(heap, "ceil".to_string(), math_ceil));
    exports.insert("sqrt".to_string(), Value::allocate_native(heap, "sqrt".to_string(), math_sqrt));
    exports.insert("abs".to_string(), Value::allocate_native(heap, "abs".to_string(), math_abs));

    Value::allocate_module(heap, "math".to_string(), exports)
}

fn math_sin(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("sin() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().sin())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).sin())) }
    else { Err(error("R002", format!("sin() argument must be a number, not '{}'", args[0].get_type()))) }
}

fn math_asin(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("asin() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().asin())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).asin())) }
    else { Err(error("R002", format!("asin() argument must be a number, not '{}'", args[0].get_type()))) }
}

fn math_cos(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("cos() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().cos())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).cos())) }
    else { Err(error("R002", format!("cos() argument must be a number, not '{}'", args[0].get_type()))) }
}

fn math_acos(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("acos() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().acos())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).acos())) }
    else { Err(error("R002", format!("acos() argument must be a number, not '{}'", args[0].get_type()))) }
}

fn math_tan(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("tan() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().tan())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).tan())) }
    else { Err(error("R002", format!("tan() argument must be a number, not '{}'", args[0].get_type()))) }
}

fn math_atan(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("atan() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().atan())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).atan())) }
    else { Err(error("R002", format!("atan() argument must be a number, not '{}'", args[0].get_type()))) }
}

fn math_degrees(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("degrees() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().to_degrees())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).to_degrees())) }
    else { Err(error("R002", format!("degrees() argument must be a number, not {}", args[0].get_type()))) }
}

fn math_radians(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("radians() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().to_radians())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).to_radians())) }
    else { Err(error("R002", format!("radians() argument must be a number, not {}", args[0].get_type()))) }
}

fn math_exp(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("exp() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().exp())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).exp())) }
    else { Err(error("R002", format!("exp() argument must be a number, not {}", args[0].get_type()))) }
}

fn math_ln(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("ln() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().ln())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).ln())) }
    else { Err(error("R002", format!("ln() argument must be a number, not {}", args[0].get_type()))) }
}

fn math_log(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("log() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().log10())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).log10())) }
    else { Err(error("R002", format!("log() argument must be a number, not {}", args[0].get_type()))) }
}

fn math_floor(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("floor() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().floor())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).floor())) }
    else { Err(error("R002", format!("floor() argument must be a number, not {}", args[0].get_type()))) }
}

fn math_ceil(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("ceil() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().ceil())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).ceil())) }
    else { Err(error("R002", format!("ceil() argument must be a number, not {}", args[0].get_type()))) }
}

fn math_sqrt(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("sqrt() takes exactly 1 argument, got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().sqrt())) }
    else if args[0].is_int() { Ok(Value::float((args[0].as_int() as f64).sqrt())) }
    else { Err(error("R002", format!("sqrt() argument must be a number, not '{}'", args[0].get_type()))) }
}

fn math_abs(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 { return Err(error("R014", format!("abs() takes exactly 1 argument got {}", args.len()))); }
    if args[0].is_float() { Ok(Value::float(args[0].as_float().abs())) }
    else if args[0].is_int() { Ok(Value::int(args[0].as_int().abs())) }
    else { Err(error("R002", format!("abs() argument must be a number, not '{}'", args[0].get_type()))) }
}
