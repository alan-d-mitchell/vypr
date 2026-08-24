use error::error::VyprError;
use crate::value::Value;
use crate::stdlib::error;
use crate::heap::Heap;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::thread::sleep;

pub fn create_module(heap: &mut Heap) -> Value {
    let mut exports = HashMap::new();

    exports.insert("time".to_string(), Value::allocate_native(heap, "time".to_string(), time_func));
    exports.insert("sleep".to_string(), Value::allocate_native(heap, "sleep".to_string(), sleep_func));

    Value::allocate_module(heap, "time".to_string(), exports)
}

// Returns the current Unix timestamp as a float
fn time_func(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if !args.is_empty() {
        return Err(error("R014", format!("time() takes no arguments, got{}", args.len())));
    }

    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("time went backwards");

    Ok(Value::float(since_the_epoch.as_secs_f64()))
}

// Pauses the VM thread
fn sleep_func(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("sleep() takes exactly 1 argument ({} given)", args.len())));
    }

    if args[0].is_float() { 
        sleep(Duration::from_secs_f64(args[0].as_float()));
    } else if args[0].is_int() { 
        sleep(Duration::from_secs(args[0].as_int() as u64));
    } else {
        return Err(error("R002", format!("sleep() argument must be a number, not '{}'", args[0].get_type())));
    }

    Ok(Value::none())
}
