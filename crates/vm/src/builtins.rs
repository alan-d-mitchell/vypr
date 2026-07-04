use error::error::{Span, VyprError};

use crate::value::Value;
use std::{cell::RefCell, io::{self, Write}, rc::Rc};

fn error(code: &'static str, message: impl Into<String>) -> VyprError {
    VyprError::new(code, message, Span::default())
}

pub fn vypr_print(args: &[Value]) -> Result<Value, VyprError> {
    let mut stdout = io::stdout().lock();

    for (i, arg) in args.iter().enumerate() {
        let s = arg.to_string();
        let _ = stdout.write_all(s.as_bytes());

        if i < args.len() - 1 {
            let _ = stdout.write_all(b" ");
        }
    }

    let _ = stdout.write_all(b"\n");
    let _ = stdout.flush();

    Ok(Value::None)
}

pub fn vypr_int(args: &[Value]) -> Result<Value, VyprError> {
    if args.is_empty() {
        return Ok(Value::Int(0));
    }

    match &args[0] {
        Value::Int(i) => Ok(Value::Int(*i)),
        Value::Float(f) => Ok(Value::Int(*f as i64)),
        Value::Str(s) => match s.parse::<i64>() {
            Ok(val) => Ok(Value::Int(val)),
            Err(_) => Err(error("R002", format!("invalid literal for int(): '{}'", s))),
        },

        Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),

        _ => Err(error("R002", format!("int() argument must be a string or a number, not '{}'", args[0].get_type()))),
    }
}

pub fn vypr_float(args: &[Value]) -> Result<Value, VyprError> {
    if args.is_empty() { 
        return Ok(Value::Float(0.0)); 
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(*f)),
        Value::Int(i) => Ok(Value::Float(*i as f64)),
        Value::Str(s) => match s.parse::<f64>() {
            Ok(val) => Ok(Value::Float(val)),
            Err(_) => Err(error("R002", format!("could not convert string to float: '{}'", s))),
        },

        Value::Bool(b) => Ok(Value::Float(if *b { 1.0 } else { 0.0 })),

        _ => Err(error("R002", format!("float() argument must be a string or a number, not '{}'", args[0].get_type()))),
    }
}

pub fn vypr_str(args: &[Value]) -> Result<Value, VyprError> {
    if args.is_empty() {
        return Ok(Value::Str(String::new())); 
    }

    Ok(Value::Str(args[0].to_string()))
}

pub fn vypr_len(args: &[Value]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("len() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::List(items) => Ok(Value::Int(items.borrow().len() as i64)),
        Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
        Value::Range(start, stop) => {
            let len = if stop > start { stop - start } else { 0 };
            Ok(Value::Int(len))
        },
        Value::Dict(dict) => Ok(Value::Int(dict.borrow().len() as i64)),

        _ => Err(error("R002", format!("object of type {} has no len()", args[0].get_type())))
    }
}

pub fn vypr_range(args: &[Value]) -> Result<Value, VyprError> {
    if args.is_empty() {
        return Err(error("R014", "range expected at least 1 argument, got 0"));
    }

    let mut start = 0;
    let mut stop = 0;

if args.len() == 1 {
        if let Value::Int(s) = args[0] {
            stop = s;
        } else {
            return Err(error("R002", format!("'{}' object cannot be interpreted as an integer", args[0].get_type())));
        }
    } else if args.len() >= 2 {
        if let Value::Int(s) = args[0] {
            start = s;
        } else {
            return Err(error("R002", format!("'{}' object cannot be interpreted as an integer", args[0].get_type())));
        }

        if let Value::Int(s) = args[1] {
            stop = s;
        } else {
            return Err(error("R002", format!("'{}' object cannot be interpreted as an integer", args[1].get_type())));
        }
    }

    Ok(Value::Range(start, stop))
}

pub fn vypr_list(args: &[Value]) -> Result<Value, VyprError> {
    if args.is_empty() {
        return Ok(Value::List(Rc::new(RefCell::new(Vec::new()))));
    }

    if args.len() > 1 {
        return Err(error("R014", format!("list() expected at most 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::List(items) => Ok(Value::List(Rc::new(RefCell::new(items.borrow().clone())))),

        Value::Str(s) => {
            let mut chars = Vec::new();
            for c in s.chars() {
                chars.push(Value::Str(c.to_string()));
            }

            Ok(Value::List(Rc::new(RefCell::new(chars))))
        }

        Value::Range(start, stop) => {
            let mut items = Vec::new();
            for i in *start..*stop {
                items.push(Value::Int(i));
            }

            Ok(Value::List(Rc::new(RefCell::new(items))))
        } 

        _ => Err(error("R002", format!("'{}' object is not iterable", args[0].get_type()))), 
    }
}

pub fn vypr_reversed(args: &[Value]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("reversed() takes exactly 1 argument, got {}", args.len())));
    }

    match &args[0] {
        Value::List(items) => {
            let mut reversed = items.borrow().clone();
            reversed.reverse();
            Ok(Value::List(Rc::new(RefCell::new(reversed))))
        }

        Value::Str(s) => {
            let chars: Vec<Value> = s.chars().rev().map(|c| Value::Str(c.to_string())).collect();
            Ok(Value::List(Rc::new(RefCell::new(chars))))
        }

        Value::Range(start, stop) => {
            let mut items = Vec::new();
            let len = if stop > start { stop - start } else { 0 };
            
            for i in (0..len).rev() {
                items.push(Value::Int(start + i));
            }
            
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }

        _ => Err(error("R002", format!("'{}' object is not reversible", args[0].get_type())))
    }
}
