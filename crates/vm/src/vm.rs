use std::{cell::RefCell, collections::HashMap, rc::Rc};
use error::error::{Span, VyprError};

use crate::{builtins, bytecode::{Chunk, OpCode}, value::{self, DataType, Value}};

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
            value: Value::Native(value::NativeFunction {
                name: "print".to_string(),
                function: builtins::vypr_print
            }),
            lock: DataType::Function,
        });

        globals.insert("int".to_string(), GlobalVar {
            value: Value::Native(value::NativeFunction {
                name: "int".to_string(),
                function: builtins::vypr_int
            }),
            lock: DataType::Function
        });

        globals.insert("float".to_string(), GlobalVar {
            value: Value::Native(value::NativeFunction {
                name: "float".to_string(),
                function: builtins::vypr_float
            }),
            lock: DataType::Function
        });

        globals.insert("str".to_string(), GlobalVar {
            value: Value::Native(value::NativeFunction {
                name: "str".to_string(),
                function: builtins::vypr_str
            }),
            lock: DataType::Function
        });

        globals.insert("len".to_string(), GlobalVar {
            value: Value::Native(value::NativeFunction {
                name: "len".to_string(),
                function: builtins::vypr_len
            }),
            lock: DataType::Function
        });

        globals.insert("range".to_string(), GlobalVar {
            value: Value::Native(value::NativeFunction {
                name: "range".to_string(),
                function: builtins::vypr_range
            }),
            lock: DataType::Function
        });

        globals.insert("list".to_string(), GlobalVar {
            value: Value::Native(value::NativeFunction {
                name: "list".to_string(),
                function: builtins::vypr_list
            }),
            lock: DataType::Function
        });

        globals.insert("reversed".to_string(), GlobalVar {
            value: Value::Native(value::NativeFunction {
                name: "reversed".to_string(),
                function: builtins::vypr_reversed
            }),
            lock: DataType::Function
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

    pub(crate) fn error(&self, code: &'static str, message: impl Into<String>) -> VyprError {
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
                    
                    let mut should_error = false;
                    let mut existing_lock = DataType::Any;
                    
                    if let Some(existing) = self.globals.get(&name) {
                        existing_lock = existing.lock;

                        if existing_lock != DataType::Any && val.get_type() != existing_lock {
                            should_error = true;
                        }
                    }

                    if should_error {
                        return Err(self.error("R002", format!(
                            "type error: variable '{}' is locked to {}, but got {}", 
                            name, existing_lock, val.get_type()
                        )));
                    }

                    if let Some(existing) = self.globals.get_mut(&name) {
                        existing.value = val;

                        if type_lock != DataType::Any {
                            existing.lock = type_lock; 
                        }
                    } else {
                        self.globals.insert(name, GlobalVar {
                            value: val,
                            lock: type_lock,
                        });
                    }
                }

                OpCode::GetGlobal(name_idx) => {
                    let name = self.read_string(name_idx)?;
                    let mut module_to_load = None;

                    if let Some(global) = self.globals.get(&name) {
                        if let Value::UnloadedModule(mod_name) = &global.value {
                            module_to_load = Some(mod_name.clone());
                        }
                    } else {
                        return Err(self.error("R001", format!("undefined variable '{}'", name)));
                    }

                    if let Some(mod_name) = module_to_load {
                        if let Some(loaded_module) = crate::stdlib::load_module(&mod_name) {
                            self.globals.get_mut(&name).unwrap().value = loaded_module; 
                        } else {
                            return Err(self.error("R009", format!("module not found: no module named '{}'", mod_name)));
                        }
                    }

                    let final_val = self.globals.get(&name).unwrap().value.clone();
                    self.push(final_val);
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
                                    "type error: variable '{}' is locked to {}, but got {}", 
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

                    // Peek down the stack to find the object we are calling the method on
                    let obj_idx = self.stack.len() - 1 - arg_count; 
                    let obj = self.stack[obj_idx].clone();

                    if let Value::Module(module) = obj {
                        if let Some(val) = module.exports.get(&method_name) {
                            if let Value::Native(native) = val {
                                let mut args = Vec::new();
                                for _ in 0..arg_count {
                                    args.push(self.pop()?);
                                }
                                args.reverse();

                                self.pop()?; // Pop the module object off the stack

                                let result = (native.function)(&args)?;
                                self.push(result);

                                continue;
                            } else {
                                return Err(self.error("R010", format!("attribute '{}' is not callable", method_name)));
                            }
                        } else {
                            return Err(self.error("R011", format!("module '{}' has no attribute '{}'", module.name, method_name)));
                        }
                    }

                    self.invoke_method(name_idx, arg_count)?;
                }

                OpCode::GetSubscript => {
                    let index_val = self.pop()?;
                    let list_val = self.pop()?;

                    match list_val {
                        Value::List(items) => {
                            let index = match index_val {
                                Value::Int(i) => i,
                                _ => return Err(self.error("R002", "list index must be an integer"))
                            };
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
                            let index = match index_val {
                                Value::Int(i) => i,
                                _ => return Err(self.error("R002", "string index must be an integer"))
                            };
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
                            let index = match index_val {
                                Value::Int(i) => i,
                                _ => return Err(self.error("R002", "range index must be an integer"))
                            };
                            let len = if stop > start { stop - start } else { 0 };
                            let effective_index = if index < 0 { len + index } else { index };

                            if effective_index < 0 || effective_index >= len {
                                return Err(self.error("R003", "range object index out of range"));
                            }

                            self.push(Value::Int(start + effective_index));
                        }

                        Value::Dict(dict) => {
                            let key_str = match index_val {
                                Value::Str(s) => s,
                                _ => index_val.to_string(),
                            };
                            
                            if let Some(val) = dict.borrow().get(&key_str) {
                                self.push(val.clone());
                            } else {
                                return Err(self.error("R003", format!("key '{}' does not exist", key_str)));
                            }
                        }

                        _ => return Err(self.error("R002", "object is not subscriptable"))
                    }
                }

                OpCode::SetSubscript => {
                    let index_val = self.pop()?;
                    let list_val = self.pop()?;
                    let value = self.pop()?;

                    match list_val {
                        Value::List(items) => {
                            let index = match index_val {
                                Value::Int(i) => i,
                                _ => return Err(self.error("R002", "list index must be an integer"))
                            };
                            let mut borrowed = items.borrow_mut();
                            let effective_index = if index < 0 {
                                borrowed.len() as i64 + index
                            } else {
                                index
                            };

                            if effective_index < 0 || effective_index >= borrowed.len() as i64 {
                                return Err(self.error("R003", "list assignment index out of range"));
                            }

                            borrowed[effective_index as usize] = value;
                        }
                        
                        Value::Dict(dict) => {
                            let key_str = match index_val {
                                Value::Str(s) => s,
                                _ => index_val.to_string(),
                            };
                            dict.borrow_mut().insert(key_str, value);
                        }

                        _ => return Err(self.error("R002", "object does not support item assignment"))
                    }
                }

                OpCode::GetProperty(name_idx) => {
                    let property_name = self.read_string(name_idx)?;
                    let obj = self.pop()?;

                    match obj {
                        Value::Module(module) => {
                            if let Some(val) = module.exports.get(&property_name) {
                                self.push(val.clone());
                            } else {
                                return Err(self.error("R011", format!("module '{}' has no attribute '{}'", module.name, property_name)));
                            }
                        }
                        _ => return Err(self.error("R012", format!("object has no attribute '{}'", property_name)))
                    }
                }

                OpCode::SetProperty(name_idx) => {
                    let property_name = self.read_string(name_idx)?;
                    let _obj = self.pop()?;
                    let _value = self.pop()?;
                    
                    return Err(self.error("R013", format!("cannot set property '{}' on this object", property_name)));
                }

                OpCode::ASSERT_TYPE(expected_type) => {
                    let val = self.stack.last().ok_or_else(|| self.error("RPNC", "stack underflow on assert_type"))?;
                    
                    if expected_type != DataType::Any && val.get_type() != expected_type {
                        return Err(self.error("R002", format!(
                            "type error: expected {}, but got {}", 
                            expected_type, val.get_type()
                        )));
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

                OpCode::BuildDict(count) => {
                    let mut dict = HashMap::with_capacity(count);

                    for _ in 0..count {
                        let value = self.pop()?;
                        let key = self.pop()?;

                        let key_str = match key {
                            Value::Str(s) => s,
                            _ => key.to_string(), 
                        };

                        dict.insert(key_str, value);
                    }

                    self.push(Value::Dict(Rc::new(RefCell::new(dict))));
                }

                OpCode::Length => {
                    let val = self.pop()?;
                    match val {
                        Value::List(items) => self.push(Value::Int(items.borrow().len() as i64)),
                        Value::Dict(dict) => self.push(Value::Int(dict.borrow().len() as i64)),
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

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Float(a as f64 + b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Float(a + b as f64)),

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

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Float(a as f64 + b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Float(a + b as f64)),

                        _ => return Err(self.error("R002", "invalid operands for -")),
                    }
                }

                OpCode::Mul => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Int(a * b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float(a * b)),

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Float(a as f64 * b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Float(a * b as f64)),

                        _ => return Err(self.error("R002", "invalid operands for *")),
                    }
                }

                OpCode::Div => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => {
                            if b == 0 {
                                return Err(self.error("R007", "division by zero"));
                            }

                            self.push(Value::Float(a as f64 / b as f64)) // Integer division
                        }
                        (Value::Float(a), Value::Float(b)) => {
                            self.push(Value::Float(a / b))
                        }

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Float(a as f64 / b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Float(a / b as f64)),
                        _ => return Err(self.error("R002", "invalid operands for /")),
                    }
                }

                OpCode::Modulo => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => {
                            if b == 0 {
                                return Err(self.error("R007", "modulo by zero"));
                            }

                            self.push(Value::Int(a % b))
                        }
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float(a % b)),

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Float(a as f64 % b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Float(a % b as f64)),

                        _ => return Err(self.error("R002", "operands must be numbers")),
                    }
                }

                OpCode::FloorDiv => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => {
                            if b == 0 {
                                return Err(self.error("R007", "division by zero"));
                            }

                            self.push(Value::Int(a / b))
                        }
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Float((a / b).floor())),

                        (Value::Int(a), Value::Float(b)) => self.push(Value::Float((a as f64 / b).floor())),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Float((a / b as f64).floor())),

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
                        
                        (Value::Int(a), Value::Float(b)) => self.push(Value::Bool((a as f64) < b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Bool(a < (b as f64))),
                        
                        _ => return Err(self.error("R002", "invalid operands for <")),
                    }
                }

                OpCode::Greater => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Int(a), Value::Int(b)) => self.push(Value::Bool(a > b)),
                        (Value::Float(a), Value::Float(b)) => self.push(Value::Bool(a > b)),
                        
                        (Value::Int(a), Value::Float(b)) => self.push(Value::Bool((a as f64) > b)),
                        (Value::Float(a), Value::Int(b)) => self.push(Value::Bool(a > (b as f64))),
                        
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

                OpCode::And => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    
                    if a.is_truthy() {
                        self.push(b);
                    } else {
                        self.push(a.clone());
                    }
                }

                OpCode::Or => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    
                    if a.is_truthy() {
                        self.push(a.clone());
                    } else {
                        self.push(b);
                    }
                }

                OpCode::FormatString(count) => {
                    let mut parts = Vec::with_capacity(count);

                    for _ in 0..count {
                        parts.push(self.pop()?);
                    }

                    parts.reverse();

                    let mut formatted = String::new();
                    for part in parts {
                        match part {
                            Value::Str(s) => formatted.push_str(&s),
                            Value::Int(i) => formatted.push_str(&i.to_string()),
                            Value::Float(f) => formatted.push_str(&f.to_string()),
                            Value::Bool(b) => formatted.push_str(if b { "true" } else { "false" }),
                            Value::List(_) | Value::Range(_, _) => formatted.push_str(&part.to_string()),
                            Value::None => formatted.push_str("None"),
                            _ => return Err(self.error("R002", "cannot format this type"))
                        }
                    }

                    self.push(Value::Str(formatted))
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

                OpCode::Import(idx) => {
                    let name = self.read_string(idx)?;

                    self.push(Value::UnloadedModule(name));
                }
            }
        }
    }

    // Helper to get the top frame
    fn current_frame(&self) -> &CallFrame {
        self.frames.last().expect("call stack empty")
    }

    fn current_frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().expect("call stack empty")
    }

    fn call_value(&mut self, arg_count: usize) -> Result<(), VyprError> {
        if self.frames.len() >= 1000 {
            return Err(self.error("R006", "maximum recursion depth exceeded"));
        }

        let _frame_start = self.stack.len() - arg_count;

        let func_idx = self.stack.len() - 1 - arg_count;
        let callee = self.stack[func_idx].clone();

        match callee {
            Value::Native(native) => {
                let mut args = Vec::new();

                for _ in 0..arg_count {
                    args.push(self.pop()?);
                }
                args.reverse();

                self.pop()?;

                let result = (native.function)(&args)?;
                self.push(result);

                Ok(())
            }

            Value::Function(arity, local_count, chunk) => {
                if arg_count != arity {
                    return Err(self.error("R008", format!(
                        "function expected {} arguments but got {}", 
                        arity, arg_count
                    )));
                }

                let mut args = Vec::new();
                for _ in 0..arg_count {
                    args.push(self.pop()?);
                }
                args.reverse();
                self.pop()?; // pop the function itself

                let new_frame_start = self.stack.len();

                // Pre-allocate ALL locals (_0 through _N)
                for _ in 0..local_count {
                    self.push(Value::None);
                }

                // Write args into their slots (_1 through _arity)
                for (i, arg) in args.iter().enumerate() {
                    self.stack[new_frame_start + 1 + i] = arg.clone();
                }

                self.frames.push(CallFrame {
                    chunk: *chunk,
                    ip: 0,
                    frame_start: new_frame_start,
                });

                Ok(())
            } 

            _ => Err(self.error("R004", "can only call functions"))
        }
    }

    fn read_constant(&self, idx: usize) -> Value {
        self.current_frame().chunk.constants[idx].clone()
    }

    pub(crate) fn read_string(&self, idx: usize) -> Result<String, VyprError> {
        match self.read_constant(idx) {
            Value::Str(s) => Ok(s),
            _ => Err(self.error("R005", "expected string in constant pool")),
        }
    }

    pub(crate) fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub(crate) fn pop(&mut self) -> Result<Value, VyprError> {
        self.stack.pop().ok_or_else(|| self.error("RPNC", "stack underflow")) // RPNC = Runtime Panic
    }
}
