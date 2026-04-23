use std::{cell::RefCell, collections::HashMap, rc::Rc};
use error::error::{Span, VyprError};

use crate::{bytecode::{Chunk, OpCode}, value::{Value, DataType}};

#[derive(Clone)]
struct GlobalVar {
    value: Value,
    lock: DataType,
}

struct CallFrame {
    chunk: Chunk, // The code being executed
    ip: usize,    // Instruction pointer for this frame
    frame_start: usize // where function locals begin on the stack
}

pub struct VM {
    frames: Vec<CallFrame>, // The call stack
    stack: Vec<Value>,      // The operand stack
    globals: HashMap<String, GlobalVar>
}

impl VM {

    pub fn new(chunk: Chunk) -> Self {
        let mut globals = HashMap::new();

        globals.insert("print".to_string(), GlobalVar {
            value: Value::Native(crate::builtins::vypr_print),
            lock: DataType::Function,
        });

        globals.insert("int".to_string(), GlobalVar {
            value: Value::Native(crate::builtins::vypr_int),
            lock: DataType::Function,
        });

        globals.insert("float".to_string(), GlobalVar {
            value: Value::Native(crate::builtins::vypr_float),
            lock: DataType::Function,
        });

        globals.insert("str".to_string(), GlobalVar {
            value: Value::Native(crate::builtins::vypr_str),
            lock: DataType::Function,
        });

        globals.insert("len".to_string(), GlobalVar {
            value: Value::Native(crate::builtins::vypr_len),
            lock: DataType::Function,
        });

        globals.insert("range".to_string(), GlobalVar {
            value: Value::Native(crate::builtins::vypr_range),
            lock: DataType::Function,
        });

        globals.insert("list".to_string(), GlobalVar {
            value: Value::Native(crate::builtins::vypr_list),
            lock: DataType::Function,
        });

        let main_frame = CallFrame {
            chunk,
            ip: 0,
            frame_start: 0
        };

        Self {
            frames: vec![main_frame],
            stack: Vec::new(),
            globals,
        }
    }

    fn error(&self, code: &'static str, message: impl Into<String>) -> VyprError {
        let ip = self.current_frame().ip.saturating_sub(1);

        let span = if ip < self.current_frame().chunk.spans.len() {
            self.current_frame().chunk.spans[ip]
        } else {
            Span::default()
        };

        VyprError::new(code, message, span)
    }

    pub fn run(&mut self) -> Result<(), VyprError> {
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
            let op = self.current_frame().chunk.code[ip];
            self.current_frame_mut().ip += 1;

            match op {
                OpCode::Constant(idx) => {
                    let c = self.read_constant(idx);
                    self.push(c);
                }

                OpCode::DefineGlobal(name_idx, type_lock) => {
                    let name = self.read_string(name_idx)?;
                    let val = self.pop()?;
                    
                    self.globals.insert(name, GlobalVar {
                        value: val,
                        lock: type_lock, // Use the lock from the bytecode
                    });
                }

                OpCode::GetGlobal(name_idx) => {
                    let name = self.read_string(name_idx)?;

                    match self.globals.get(&name) {
                        Some(global) => self.push(global.value.clone()),
                        None => return Err(self.error("R001", format!("undefined variable '{}'", name))),
                    }
                }

                OpCode::SetGlobal(name_idx) => {
                    let name = self.read_string(name_idx)?;
                    let new_val = self.pop()?;

                    let existing_lock = self.globals.get(&name).map(|g| g.lock);

                    if let Some(lock) = existing_lock {
                        if lock != DataType::Any {
                            let new_type = new_val.get_type();

                            if new_type != lock {
                                return Err(self.error("R002", format!(
                                    "type error: variable '{}' is locked to {:?}, but got {:?}", 
                                    name, lock, new_type
                                )));
                            }
                        }

                        self.globals.get_mut(&name).unwrap().value = new_val;
                    } else {
                        self.globals.insert(name, GlobalVar {
                            value: new_val,
                            lock: DataType::Any,
                        });
                    }
                }

                OpCode::GetLocal(slot) => {
                    let index = self.current_frame().frame_start + slot;
                    let val = self.stack[index].clone();
                    self.push(val);
                }

                OpCode::SetLocal(slot) => {
                    let index = self.current_frame().frame_start + slot;
                    let val = self.pop()?; // Get new value
                    self.stack[index] = val; // Update stack in-place
                }

                OpCode::Call(arg_count) => {
                    self.call_value(arg_count)?;
                }

                OpCode::Invoke(name_idx, arg_count) => {
                    let method_name = self.read_string(name_idx)?;

                    // Pop arguments off the stack (reverse order)
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    // Pop the parent object (the list) off the stack
                    let obj = self.pop()?;

                    match (obj, method_name.as_str()) {
                        (Value::List(items), "append") => {
                            if args.len() != 1 {
                                return Err(self.error("R003", "append() takes exactly 1 argument").with_help("remove the extra argument"));
                            }
                            
                            items.borrow_mut().push(args[0].clone());
                            
                            self.push(Value::None);
                        }
                        
                        (val, method) => {
                            return Err(self.error("R004", format!("object '{:?}' has no method '{}'", val.get_type() , method)));
                        }
                    }
                }

                OpCode::GetSubscript => {
                    let index_val = self.pop()?;
                    let list_val = self.pop()?;

                    let index = match index_val {
                        Value::Int(i) => i,
                        _ => return Err(self.error("R002", "list index must be an integer"))
                    };

                    match list_val {
                        Value::List(items) => {
                            let borrowed = items.borrow();

                            let effective_index = if index < 0 {
                                borrowed.len() as i64 + index
                            } else {
                                index
                            };

                            if effective_index < 0 || effective_index >= borrowed.len() as i64 {
                                return Err(self.error("R003", "list index out of range"));
                            }

                            self.push(borrowed[effective_index as usize].clone());
                        }

                        Value::Str(s) => {
                            let char_count = s.chars().count() as i64;

                            let effective_index = if index < 0 {
                                char_count + index
                            } else {
                                index
                            };

                            if effective_index < 0 || effective_index >= char_count {
                                return Err(self.error("R003", "string index out of range"));
                            }

                            if let Some(c) = s.chars().nth(effective_index as usize) {
                                self.push(Value::Str(c.to_string()));
                            } else {
                                return Err(self.error("R003", "string index out of range"));
                            }
                        }

                        Value::Range(start, stop) => {
                            let len = if stop > start { stop - start } else { 0 };
                            let effective_index = if index < 0 { len + index } else { index };

                            if effective_index < 0 || effective_index >= len {
                                return Err(self.error("R003", "range object index out of range"));
                            }

                            self.push(Value::Int(start + effective_index));
                        }

                        _ => return Err(self.error("R002", "object is not subscriptable"))
                    }
                }

                OpCode::BuildList(count) => {
                    let mut items = Vec::with_capacity(count);
                    
                    // Pop items from stack (they are in reverse order)
                    for _ in 0..count {
                        items.push(self.pop()?);
                    }
                    
                    // Restore original order
                    items.reverse();
                    
                    self.push(Value::List(Rc::new(RefCell::new(items))));
                }

                OpCode::Length => {
                    let val = self.pop()?;
                    match val {
                        Value::List(items) => self.push(Value::Int(items.borrow().len() as i64)),
                        Value::Str(s) => self.push(Value::Int(s.chars().count() as i64)),

                        Value::Range(start, stop) => {
                            let len = if stop > start { stop - start } else { 0 };
                            self.push(Value::Int(len));
                        }

                        _ => return Err(self.error("R002", "object has no length")),
                    }
                }

                OpCode::Pop => { self.pop()?; }

                OpCode::Jump(offset) => {
                    self.current_frame_mut().ip += offset;
                }

                OpCode::JumpIfFalse(offset) => {
                    // Peek at the top (do not pop yet, needed for and/or)
                    let val = self.stack.last().expect("stack underflow in jump");
                    if !val.is_truthy() {
                        self.current_frame_mut().ip += offset;
                    }
                }

                OpCode::Loop(offset) => {
                    self.current_frame_mut().ip -= offset;
                }

                OpCode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Int(a + b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float(a + b)),
                        (Value::Str(a), Value::Str(b)) => self.push(Value::Str(a + &b)),
                        _ => return Err(self.error("R002", "invalid operands for +")),
                    }
                }

                OpCode::Sub => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Int(a - b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float(a - b)),
                        _ => return Err(self.error("R002", "invalid operands for -")),
                    }
                }

                OpCode::Mul => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Int(a * b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float(a * b)),
                        _ => return Err(self.error("R002", "invalid operands for *")),
                    }
                }

                OpCode::Div => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Int(a / b)), // Integer division
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float(a / b)),
                        _ => return Err(self.error("R002", "invalid operands for /")),
                    }
                }

                OpCode::Modulo => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Int(a % b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float(a % b)),
                        _ => return Err(self.error("R002", "operands must be numbers")),
                    }
                }

                OpCode::FloorDiv => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Int(a / b)), 
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float((a / b).floor())),
                        _ => return Err(self.error("R002", "operands must be numbers")),
                    }
                }

                OpCode::Power => {
                    let b = self.pop()?; // exponent
                    let a = self.pop()?; // base
                    
                    match (a, b) {
                        (Value::Int(base), Value::Int(exp)) => {
                            if exp < 0 {
                                let result = (base as f64).powf(exp as f64);
                                self.push(Value::Float(result));
                            } else if let Ok(exp_u32) = u32::try_from(exp) {
                                match base.checked_pow(exp_u32) {
                                    Some(result) => self.push(Value::Int(result)),
                                    None => return Err(self.error("R005", "integer overflow in power".to_string())),
                                }
                            } else {
                                return Err(self.error("R005", "exponent too large"));
                            }
                        }

                        (Value::Float(base), Value::Float(exp)) => {
                            self.push(Value::Float(base.powf(exp)));
                        }

                        (Value::Int(base), Value::Float(exp)) => {
                            self.push(Value::Float((base as f64).powf(exp)));
                        }

                        (Value::Float(base), Value::Int(exp)) => {
                            self.push(Value::Float(base.powf(exp as f64)));
                        }

                        _ => return Err(self.error("R002", "operands must be numbers"))
                    }
                }

                OpCode::Equal => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    self.push(Value::Bool(a == b));
                }

                OpCode::Less => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Bool(a < b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Bool(a < b)),
                        _ => return Err(self.error("R002", "invalid operands for <")),
                    }
                }

                OpCode::Greater => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Bool(a > b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Bool(a > b)),
                        _ => return Err(self.error("R002", "invalid operands for >")),
                    }
                }

                OpCode::LessEqual => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Bool(a <= b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Bool(a <= b)),

                        // Int/Float mixing
                        (Value::Int(a), Value::Float(b)) => self.push(Value::Bool((a as f64) <= b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Bool(a <= (b as f64))),
                        _ => return Err(self.error("R002", "invalid types for <=")),
                    }
                }

                OpCode::GreaterEqual => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Bool(a >= b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Bool(a >= b)),

                        // Int/Float mixing
                        (Value::Int(a), Value::Float(b)) => self.push(Value::Bool((a as f64) >= b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Bool(a >= (b as f64))),
                        _ => return Err(self.error("R002", "invalid types for >=")),
                    }
                }

                OpCode::Negate => {
                    let a = self.pop()?;

                    match a {
                        Value::Int(i) => self.push(Value::Int(-i)),
                        Value::Float(f) => self.push(Value::Float(-f)),
                        _ => return Err(self.error("R002", "operand must be a number")),
                    }
                }

                OpCode::Not => {
                    let a = self.pop()?;

                    match a {
                        Value::Bool(b) => self.push(Value::Bool(!b)),
                        _ => return Err(self.error("R002", "operand must be a boolean")),
                    }
                }

                OpCode::Return => {
                    let result = self.pop().unwrap_or(Value::None);
                    let frame = self.frames.pop().unwrap();

                    self.stack.truncate(frame.frame_start);
                    self.push(result);

                    if self.frames.is_empty() {
                        return Ok(());
                    }
                }
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

    fn call_value(&mut self, arg_count: usize) -> Result<(), VyprError> {
        if self.frames.len() >= 1000 {
            return Err(self.error("R006", "maximum recursion depth exceeded"));
        }

        let _frame_start = self.stack.len() - arg_count;

        let func_idx = self.stack.len() - 1 - arg_count;
        let callee = self.stack[func_idx].clone();

        match callee {
            Value::Native(func) => {
                let mut args = Vec::new();

                for _ in 0..arg_count {
                    args.push(self.pop()?);
                }
                args.reverse();

                self.pop()?;

                let result = func(&args);
                self.push(result);

                Ok(())
            }

            Value::Function(chunk) => {
                let mut args = Vec::new();
                
                for _ in 0..arg_count {
                    args.push(self.pop()?);
                }
                args.reverse();

                self.pop()?;

                let new_frame_start = self.stack.len();

                for arg in args {
                    self.push(arg);
                }

                let frame = CallFrame {
                    chunk: *chunk,
                    ip: 0,
                    frame_start: new_frame_start,
                };
                self.frames.push(frame);

                Ok(())
            }

            _ => Err(self.error("R004", "can only call functions"))
        }
    }

    fn read_constant(&self, idx: usize) -> Value {
        self.current_frame().chunk.constants[idx].clone()
    }

    fn read_string(&self, idx: usize) -> Result<String, VyprError> {
        match self.read_constant(idx) {
            Value::Str(s) => Ok(s),
            _ => Err(self.error("R005", "expected string in constant pool")),
        }
    }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Result<Value, VyprError> {
        self.stack.pop().ok_or_else(|| self.error("RPNC", "stack underflow")) // RPNC = Runtime Panic
    }
}
