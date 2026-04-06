use crate::value::Value;
use std::arch::asm;

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
        Value::List(items) => Value::Int(items.len() as i64),
        Value::Str(s) => Value::Int(s.len() as i64),
        _ => Value::None
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
