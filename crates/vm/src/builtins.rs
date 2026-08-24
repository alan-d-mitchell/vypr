use error::error::{Span, VyprError};
use crate::heap::Heap;
use crate::value::{Value, ObjectType, ObjectList, ObjectRange};
use std::{cell::RefCell, fs::OpenOptions, io::{self, Write}, rc::Rc};

fn error(code: &'static str, message: impl Into<String>) -> VyprError {
    VyprError::new(code, message, Span::default())
}

pub fn vypr_print(_heap: &mut Heap, args: &[Value], kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    let mut stdout = io::stdout().lock();

    let sep = kwargs.iter()
        .find(|(k, _)| k == "sep")
        .and_then(|(_, v)| v.as_str())
        .unwrap_or(" ");

    let end = kwargs.iter()
        .find(|(k, _)| k == "end")
        .and_then(|(_, v)| v.as_str())
        .unwrap_or("\n");

    let flush = kwargs.iter()
        .find(|(k, _)| k == "flush")
        .and_then(|(_, v)| {
            if v.is_bool() { Some(v.as_bool()) } else { None }
        })
        .unwrap_or(false);

    for (i, arg) in args.iter().enumerate() {
        let s = arg.to_string();
        let _ = stdout.write_all(s.as_bytes());

        if i < args.len() - 1 {
            let _ = stdout.write_all(sep.as_bytes());
        }
    }

    let _ = stdout.write_all(end.as_bytes());

    if flush { 
        let _ = stdout.flush(); 
    }

    Ok(Value::none())
}

pub fn vypr_int(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.is_empty() {
        return Ok(Value::int(0));
    }

    if let Some(s) = args[0].as_str() {
        return match s.parse::<i32>() {
            Ok(val) => Ok(Value::int(val)),
            Err(_) => Err(error("R002", format!("invalid literal for int(): '{}'", s))),
        };
    }

    if args[0].is_int() { return Ok(args[0].clone()); }
    if args[0].is_float() { return Ok(Value::int(args[0].as_float() as i32)); }
    if args[0].is_bool() { return Ok(Value::int(if args[0].as_bool() { 1 } else { 0 })); }
    
    Err(error("R002", format!("int() argument must be a string or a number, not '{}'", args[0].get_type())))
}

pub fn vypr_float(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.is_empty() { 
        return Ok(Value::float(0.0)); 
    }

    if let Some(s) = args[0].as_str() {
        return match s.parse::<f64>() {
            Ok(val) => Ok(Value::float(val)),
            Err(_) => Err(error("R002", format!("could not convert string to float: '{}'", s))),
        };
    }

    if args[0].is_float() { return Ok(args[0].clone()); }
    if args[0].is_int() { return Ok(Value::float(args[0].as_int() as f64)); }
    if args[0].is_bool() { return Ok(Value::float(if args[0].as_bool() { 1.0 } else { 0.0 })); }
    
    Err(error("R002", format!("float() argument must be a string or a number, not '{}'", args[0].get_type())))
}

pub fn vypr_str(heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.is_empty() {
        return Ok(Value::make_string(heap, "")); 
    }
    Ok(Value::make_string(heap, &args[0].to_string()))
}

pub fn vypr_len(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("len() takes exactly 1 argument, got {}", args.len())));
    }

    if let Some(s) = args[0].as_str() {
        return Ok(Value::int(s.chars().count() as i32));
    }

    if args[0].is_object() {
        unsafe {
            let obj = args[0].as_object();
            match (*obj).ty {
                ObjectType::LIST => {
                    let list = &*(obj as *const ObjectList);
                    return Ok(Value::int(list.items.len() as i32));
                }
                ObjectType::DICT => {
                    return Ok(Value::int(0)); 
                }
                ObjectType::RANGE => {
                    let r = &*(obj as *const ObjectRange);
                    let len = if r.stop > r.start { r.stop - r.start } else { 0 };
                    return Ok(Value::int(len));
                }
                _ => {}
            }
        }
    }
    
    Err(error("R002", format!("object of type {} has no len()", args[0].get_type())))
}

pub fn vypr_range(heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.is_empty() {
        return Err(error("R014", "range expected at least 1 argument, got 0"));
    }

    let mut start = 0;
    let mut stop = 0;

    if args.len() == 1 {
        if args[0].is_int() {
            stop = args[0].as_int();
        } else {
            return Err(error("R002", format!("'{}' object cannot be interpreted as an integer", args[0].get_type())));
        }
    } else if args.len() >= 2 {
        if args[0].is_int() {
            start = args[0].as_int();
        } else {
            return Err(error("R002", format!("'{}' object cannot be interpreted as an integer", args[0].get_type())));
        }

        if args[1].is_int() {
            stop = args[1].as_int();
        } else {
            return Err(error("R002", format!("'{}' object cannot be interpreted as an integer", args[1].get_type())));
        }
    }

    Ok(Value::allocate_range(heap, start, stop))
}

pub fn vypr_list(heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.is_empty() {
        return Ok(Value::allocate_list(heap, Vec::new()));
    }

    if args.len() > 1 {
        return Err(error("R014", format!("list() expected at most 1 argument, got {}", args.len())));
    }

    if let Some(s) = args[0].as_str() {
        let mut chars = Vec::new();
        for c in s.chars() {
            chars.push(Value::make_string(heap, &c.to_string()));
        }
        return Ok(Value::allocate_list(heap, chars));
    }

    if args[0].is_object() {
        unsafe {
            let obj = args[0].as_object();
            match (*obj).ty {
                ObjectType::LIST => {
                    let list = &*(obj as *const ObjectList);
                    return Ok(Value::allocate_list(heap, list.items.clone()));
                }
                ObjectType::RANGE => {
                    let r = &*(obj as *const ObjectRange);
                    let mut items = Vec::new();
                    for i in r.start..r.stop {
                        items.push(Value::int(i));
                    }
                    return Ok(Value::allocate_list(heap, items));
                }
                _ => {}
            }
        }
    }
    
    Err(error("R002", format!("'{}' object is not iterable", args[0].get_type())))
}

pub fn vypr_reversed(heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("reversed() takes exactly 1 argument, got {}", args.len())));
    }

    if let Some(s) = args[0].as_str() {
        let chars: Vec<Value> = s.chars()
            .rev()
            .map(|c| Value::make_string(heap, &c.to_string()))
            .collect();
        return Ok(Value::allocate_list(heap, chars));
    }

    if args[0].is_object() {
        unsafe {
            let obj = args[0].as_object();
            match (*obj).ty {
                ObjectType::LIST => {
                    let list = &*(obj as *const ObjectList);
                    let mut reversed = list.items.clone();
                    reversed.reverse();
                    return Ok(Value::allocate_list(heap, reversed));
                }
                ObjectType::RANGE => {
                    let r = &*(obj as *const ObjectRange);
                    let mut items = Vec::new();
                    let len = if r.stop > r.start { r.stop - r.start } else { 0 };

                    for i in (0..len).rev() {
                        items.push(Value::int(r.start + i));
                    }
                    return Ok(Value::allocate_list(heap, items));
                }
                _ => {}
            }
        }
    }
    
    Err(error("R002", format!("'{}' object is not reversible", args[0].get_type())))
}

pub fn vypr_input(heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() > 1 {
        return Err(error("R014", format!("input() takes at most 1 argument, got {}", args.len())));
    }

    if args.len() == 1 {
        let mut stdout = io::stdout().lock();
        let s = args[0].to_string();
        let _ = stdout.write_all(s.as_bytes());
        let _ = stdout.flush();
    }

    let mut buffer = String::new();
    match io::stdin().read_line(&mut buffer) {
        Ok(_) => {
            let sanitized = buffer.trim_end().to_string();
            Ok(Value::make_string(heap, &sanitized))
        }
        Err(e) => Err(error("R015", format!("failed to read input: {}", e)))
    }
}

pub fn vypr_open(heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.is_empty() {
        return Err(error("R014", format!("open() takes 1 or 2 arguments, got {}", args.len())));
    }

    let filepath = args[0].to_string();
    let mode = if args.len() == 2 { args[1].to_string() } else { "r".to_string() };

    let file_result = match mode.as_str() {
        "r" => OpenOptions::new().read(true).open(&filepath),
        "w" => OpenOptions::new().create(true).truncate(true).open(&filepath),
        "a" => OpenOptions::new().create(true).append(true).open(&filepath),
        _ => return Err(error("R015", format!("invalid mode: '{}'", mode))),
    };

    match file_result {
        Ok(file) => Ok(Value::allocate_file(heap, Rc::new(RefCell::new(file)))),
        Err(e) => Err(error("R015", format!("failed to open file: {}", e))),
    }
}
