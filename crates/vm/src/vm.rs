use std::collections::HashMap;
use crate::bytecode::{Chunk, OpCode};
use crate::value::Value;

// A CallFrame represents a single function call in progress
struct CallFrame {
    chunk: Chunk, // The code being executed
    ip: usize,    // Instruction pointer for this frame
}

pub struct VM {
    frames: Vec<CallFrame>, // The call stack
    stack: Vec<Value>,      // The operand stack
    globals: HashMap<String, Value>
}

#[derive(Debug)]
pub enum VMError {
    RuntimeError(String),
}

impl VM {
    pub fn new(chunk: Chunk) -> Self {
        let mut globals = HashMap::new();
        globals.insert("print".to_string(), Value::Native(crate::builtins::vypr_print));

        let main_frame = CallFrame {
            chunk,
            ip: 0,
        };

        Self {
            frames: vec![main_frame],
            stack: Vec::new(),
            globals,
        }
    }

    pub fn run(&mut self) -> Result<(), VMError> {
        loop {
            // Check if we have finished the top frame
            if self.current_frame().ip >= self.current_frame().chunk.code.len() {
                // Implicit return at end of chunk
                if self.frames.len() == 1 {
                    return Ok(()); // Script done
                } else {
                    self.frames.pop(); // Return from function
                    continue;
                }
            }

            let ip = self.current_frame().ip;
            let op = self.current_frame().chunk.code[ip].clone();
            self.current_frame_mut().ip += 1;

            match op {
                OpCode::Constant(idx) => {
                    let c = self.read_constant(idx);
                    self.push(c);
                }

                OpCode::DefineGlobal(name_idx) => {
                    let name = self.read_string(name_idx)?;
                    let val = self.pop()?;

                    self.globals.insert(name, val);
                }

                OpCode::GetGlobal(name_idx) => {
                    let name = self.read_string(name_idx)?;

                    match self.globals.get(&name) {
                        Some(val) => self.push(val.clone()),
                        None => return Err(VMError::RuntimeError(format!("undefined variable '{}'", name))),
                    }
                }

                OpCode::Call(arg_count) => {
                    self.call_value(arg_count)?;
                }

                OpCode::Pop => { self.pop()?; }

                OpCode::Return => {
                    // Pop the current frame
                    if self.frames.len() == 1 {
                         return Ok(()); // Exit if it's the main script
                    }
                    self.frames.pop();
                    
                    // NOTE: If functions return values, we would push the result 
                    // onto the NEW top frame's stack here.
                    // For now, we assume void return (or pushed None previously).
                }

                _ => return Err(VMError::RuntimeError("unimplemented opcode".to_string())),
            }
        }
    }

    // Helper to get the top frame
    fn current_frame(&self) -> &CallFrame {
        self.frames.last().expect("Call stack empty")
    }

    fn current_frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().expect("Call stack empty")
    }

    fn call_value(&mut self, arg_count: usize) -> Result<(), VMError> {
        let mut args = Vec::new();

        for _ in 0..arg_count {
            args.push(self.pop()?);
        }
        args.reverse();

        let callee = self.pop()?;

        match callee {
            Value::Native(func) => {
                let result = func(&args);
                self.push(result);

                Ok(())
            }

            Value::Function(chunk) => {
                let frame = CallFrame {
                    chunk: *chunk,
                    ip: 0,
                };
                self.frames.push(frame);

                Ok(())
            }

            _ => Err(VMError::RuntimeError("can only call functions".to_string())),
        }
    }

    fn read_constant(&self, idx: usize) -> Value {
        self.current_frame().chunk.constants[idx].clone()
    }

    fn read_string(&self, idx: usize) -> Result<String, VMError> {
        match self.read_constant(idx) {
            Value::Str(s) => Ok(s),
            _ => Err(VMError::RuntimeError("expected string in constant pool".to_string())),
        }
    }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Result<Value, VMError> {
        self.stack.pop().ok_or(VMError::RuntimeError("stack underflow".to_string()))
    }
}
