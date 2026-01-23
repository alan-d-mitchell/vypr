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

#[cfg(not(target_arch = "x86_64"))]
unsafe fn sys_write(_fd: usize, _buf: *const u8, _len: usize) {
    panic!("inline assembly for sys_write is only implemented for x86_64 Linux!");
}
