use crate::value::{Module, Value, NativeFunction};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::thread::sleep;

pub fn create_module() -> Value {
    let mut exports = HashMap::new();

    exports.insert("time".to_string(), Value::Native(NativeFunction {
        name: "time".to_string(),
        function: time_func,
    }));

    exports.insert("sleep".to_string(), Value::Native(NativeFunction {
        name: "sleep".to_string(),
        function: sleep_func,
    }));

    Value::Module(Rc::new(Module {
        name: "time".to_string(),
        exports,
    }))
}

// Returns the current Unix timestamp as a float
fn time_func(_args: &[Value]) -> Value {
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("time went backwards");
    Value::Float(since_the_epoch.as_secs_f64())
}

// Pauses the VM thread
fn sleep_func(args: &[Value]) -> Value {
    if let Some(arg) = args.first() {
        match arg {
            Value::Float(f) => sleep(Duration::from_secs_f64(*f)),
            Value::Int(i) => sleep(Duration::from_secs(*i as u64)),
            _ => {} 
        }
    }
    Value::None
}
