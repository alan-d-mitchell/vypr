use error::error::VyprError;

use crate::value::{Module, Value, NativeFunction};
use crate::stdlib::error;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::thread::sleep;

pub fn create_module() -> Value {
    let mut exports = HashMap::new();

    exports.insert("time".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "time".to_string(),
        function: time_func,
    })));

    exports.insert("sleep".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "sleep".to_string(),
        function: sleep_func,
    })));

    Value::Module(Rc::new(Module {
        name: "time".to_string(),
        exports,
    }))
}

// Returns the current Unix timestamp as a float
fn time_func(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if !args.is_empty() {
        return Err(error("R014", format!("time() takes no arguments, got{}", args.len())));
    }

    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("time went backwards");

    Ok(Value::Float(since_the_epoch.as_secs_f64()))
}

// Pauses the VM thread
fn sleep_func(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("sleep() takes exactly 1 argument ({} given)", args.len())));
    }

    match &args[0] {
        Value::Float(f) => sleep(Duration::from_secs_f64(*f)),
        Value::Int(i) => sleep(Duration::from_secs(*i as u64)),
        _ => return Err(error("R002", format!("sleep() argument must be a number, not '{}'", args[0].get_type()))), 
    }

    Ok(Value::None)
}
