use error::error::VyprError;

use crate::value::{Module, Value, NativeFunction};
use crate::stdlib::error;
use std::collections::HashMap;
use std::rc::Rc;
use rand::Rng;

pub fn create_module() -> Value {
    let mut exports = HashMap::new();

    exports.insert("randrange".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "randrange".to_string(),
        function: rand_range
    })));

    exports.insert("randint".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "randint".to_string(),
        function: rand_int
    })));

    exports.insert("random".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "random".to_string(),
        function: random
    })));

    Value::Module(Rc::new(Module {
        name: "random".to_string(),
        exports,
    }))
}

fn random(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if !args.is_empty() {
        return Err(error("R014", format!("random() takes no arguments, got {}", args.len())));
    }

    let mut rng = rand::thread_rng();
    let result = rng.gen::<f64>();

    Ok(Value::Float(result))
}

fn rand_range(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 2 {
        return Err(error("R014", format!("randrange() takes exactly 2 arguments, got {}", args.len())));
    }

    let start = match args[0] {
        Value::Int(i) => i,
        _ => return Err(error("R002", format!("randrange() start must be an int, got {}", args[0].get_type()))),
    };

    let stop = match args[1] {
        Value::Int(i) => i,
        _ => return Err(error("R002", format!("randrange() stop must be an int, got {}", args[1].get_type()))),
    };

    if start >= stop {
        return Err(error("R015", "empty range for randrange()"));
    }

    let mut rng = rand::thread_rng();
    let result = rng.gen_range(start..stop);

    Ok(Value::Int(result))
}

fn rand_int(args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 2 {
        return Err(error("R014", format!("randint() takes exactly 2 arguments, got {}", args.len())));
    }

    let stop_plus_one = match args[1] {
        Value::Int(i) => Value::Int(i + 1),
        _ => return Err(error("R002", format!("randint() stop must be an int, got {}", args[1].get_type()))),
    };

    let new_args = [args[0].clone(), stop_plus_one];

    rand_range(&new_args, &[])
}
