use error::error::VyprError;
use crate::value::Value;
use crate::stdlib::error;
use crate::heap::Heap;
use std::collections::HashMap;
use rand::Rng;

pub fn create_module(heap: &mut Heap) -> Value {
    let mut exports = HashMap::new();

    exports.insert("randrange".to_string(), Value::allocate_native(heap, "randrange".to_string(), rand_range));
    exports.insert("randint".to_string(), Value::allocate_native(heap, "randint".to_string(), rand_int));
    exports.insert("random".to_string(), Value::allocate_native(heap, "random".to_string(), random));

    Value::allocate_module(heap, "random".to_string(), exports)
}

fn random(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if !args.is_empty() {
        return Err(error("R014", format!("random() takes no arguments, got {}", args.len())));
    }

    let mut rng = rand::thread_rng();
    let result = rng.gen::<f64>();

    Ok(Value::float(result))
}

fn rand_range(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 2 {
        return Err(error("R014", format!("randrange() takes exactly 2 arguments, got {}", args.len())));
    }

    if !args[0].is_int() {
        return Err(error("R002", format!("randrange() start must be an int, got {}", args[0].get_type())));
    }
    let start = args[0].as_int();

    if !args[1].is_int() {
        return Err(error("R002", format!("randrange() stop must be an int, got {}", args[1].get_type())));
    }
    let stop = args[1].as_int();

    if start >= stop {
        return Err(error("R015", "empty range for randrange()"));
    }

    let mut rng = rand::thread_rng();
    let result = rng.gen_range(start..stop);

    Ok(Value::int(result))
}

fn rand_int(heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 2 {
        return Err(error("R014", format!("randint() takes exactly 2 arguments, got {}", args.len())));
    }

    let stop_plus_one = if args[1].is_int() {
        Value::int(args[1].as_int() + 1)
    } else {
        return Err(error("R002", format!("randint() stop must be an int, got {}", args[1].get_type())));
    };

    let new_args = [args[0].clone(), stop_plus_one];

    rand_range(heap, &new_args, &[])
}
