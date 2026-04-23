use crate::value::Value;
use std::{arch::asm, cell::RefCell, rc::Rc};

pub fn vypr_print(args: &[Value]) -> Value {
    for (i, arg) in args.iter().enumerate() {
        // Convert the value to a string
        let s = arg.to_string(); 
        
        // Pass the raw bytes and length to our ASM wrapper
        // File Descriptor 1 = STDOUT
        unsafe {
            sys_write(1, s.as_ptr(), s.len());
        }

        // Print a space separator if not the last argument
        if i < args.len() - 1 {
            unsafe { 
                sys_write(1, " ".as_ptr(), 1); 
            }
        }
    }

    // Print a newline at the end
    unsafe { 
        sys_write(1, "\n".as_ptr(), 1); 
    }

    Value::None
}

pub fn vypr_int(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Int(0);
    }

    match &args[0] {
        Value::Int(i) => Value::Int(*i),
        Value::Float(f) => Value::Int(*f as i64),
        Value::Str(s) => s.parse::<i64>().map(Value::Int).unwrap_or(Value::None),
        Value::Bool(b) => Value::Int(if *b { 1 } else { 0 }),
        _ => Value::None,
    }
}

pub fn vypr_float(args: &[Value]) -> Value {
    if args.is_empty() { 
        return Value::Float(0.0); 
    }

    match &args[0] {
        Value::Float(f) => Value::Float(*f),
        Value::Int(i) => Value::Float(*i as f64),
        Value::Str(s) => s.parse::<f64>().map(Value::Float).unwrap_or(Value::None),
        Value::Bool(b) => Value::Float(if *b { 1.0 } else { 0.0 }),
        _ => Value::None,
    }
}

pub fn vypr_str(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Str(String::new()); 
    }

    Value::Str(args[0].to_string())
}

pub fn vypr_len(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::None;
    }

    match &args[0] {
        Value::List(items) => Value::Int(items.borrow().len() as i64),
        Value::Str(s) => Value::Int(s.len() as i64),
        _ => Value::None
    }
}

pub fn vypr_range(args: &[Value]) -> Value {
    let mut start = 0;
    let mut stop = 0;

    if args.len() == 1 {
        if let Value::Int(s) = args[0] {
            stop = s;
        }
    } else if args.len() >= 2 {
        if let Value::Int(s) = args[0] {
            start = s;
        }

        if let Value::Int(s) = args[1] {
            stop = s;
        }
    }

    Value::Range(start, stop)
}

pub fn vypr_list(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::List(Rc::new(RefCell::new(Vec::new()))); // list() returns empty list
    }

    match &args[0] {
        Value::List(items) => Value::List(Rc::new(RefCell::new(items.borrow().clone()))),

        Value::Str(s) => {
            let mut chars = Vec::new();
            for c in s.chars() {
                chars.push(Value::Str(c.to_string()));
            }
            // Wrap the raw Vec in Rc and RefCell
            Value::List(Rc::new(RefCell::new(chars)))
        }

        Value::Range(start, stop) => {
            let mut items = Vec::new();

            for i in *start..*stop {
                items.push(Value::Int(i));
            }

            // Wrap the raw Vec in Rc and RefCell
            Value::List(Rc::new(RefCell::new(items)))
        } 
        _ => Value::None, 
    }
}

#[cfg(target_arch = "x86_64")]
unsafe fn sys_write(fd: usize, buf: *const u8, len: usize) {
    let _ret: usize;
    
    asm!(
        "syscall",
        in("rax") 1,        // syscall number for 'write'
        in("rdi") fd,       // argument 1: file descriptor
        in("rsi") buf,      // argument 2: buffer pointer
        in("rdx") len,      // argument 3: length
        out("rcx") _,       // clobbered by syscall
        out("r11") _,       // clobbered by syscall
        lateout("rax") _ret, // return value
    );
}
