use std::collections::BTreeSet;
use std::fmt::Write;

use crate::value::{Value, DataType};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpCode {
    Constant(usize),

    DefineGlobal(usize, DataType),
    GetGlobal(usize),
    SetGlobal(usize),
    GetLocal(usize),
    SetLocal(usize),

    Pop,

    Add, Sub, Mul, Div, 
    Modulo, FloorDiv, Power,

    Equal, Less, Greater,
    LessEqual, GreaterEqual,

    Not, Negate,

    Jump(usize),
    JumpIfFalse(usize),
    Loop(usize),

    GetSubscript,
    BuildList(usize),
    Length,

    Call(usize),
    Return,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
}

impl Chunk {

    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new()
        }
    }

    pub fn write(&mut self, op: OpCode) {
        self.code.push(op);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);

        self.constants.len() - 1
    }

    pub fn disassemble(&self, name: &str) -> String {
        let mut s = String::new();
        writeln!(&mut s, "== {} ==", name).unwrap();

        // Pass 1: Scan for Labels (unchanged)
        let mut targets = BTreeSet::new();
        for (i, op) in self.code.iter().enumerate() {
            match op {
                OpCode::Jump(offset) | OpCode::JumpIfFalse(offset) => {
                    targets.insert(i + 1 + offset);
                }
                OpCode::Loop(offset) => {
                    targets.insert(i + 1 - offset);
                }
                _ => {}
            }
        }

        // Pass 2: Print Instructions (unchanged)
        for (i, op) in self.code.iter().enumerate() {
            if targets.contains(&i) {
                writeln!(&mut s, "L{:04}:", i).unwrap();
            }
            write!(&mut s, "    {:04} ", i).unwrap();

            match op {
                OpCode::Constant(idx) => {
                    let val = &self.constants[*idx];
                    writeln!(&mut s, "{:<16} {:4} '{}'", "CONSTANT", idx, val).unwrap();
                }
                OpCode::DefineGlobal(idx, dtype) => {
                    let val = &self.constants[*idx];
                    writeln!(&mut s, "{:<16} {:4} '{}' (Type: {:?})", "DEFINE_GLOBAL", idx, val, dtype).unwrap();
                }
                OpCode::LessEqual => writeln!(&mut s, "LESS_EQUAL").unwrap(),
                OpCode::GreaterEqual => writeln!(&mut s, "GREATER_EQUAL").unwrap(),
                
                OpCode::Loop(offset) => {
                    let target = (i + 1) - offset;
                    writeln!(&mut s, "{:<16} -> L{:04}", "LOOP", target).unwrap();
                }

                _ => writeln!(&mut s, "{:?}", op).unwrap(),
            }
        }

        for (i, constant) in self.constants.iter().enumerate() {
            if let Value::Function(chunk) = constant {
                writeln!(&mut s, "").unwrap(); // Empty line for spacing
                
                // Try to find the name of this function.
                // Usually, the name is stored in the constant pool right AFTER the function object
                // because of how we compiled it (emit_constant(func); make_constant(name)).
                // This is a bit of a heuristic, but works for display.
                let func_name = if i + 1 < self.constants.len() {
                    match &self.constants[i + 1] {
                        Value::Str(name) => name.clone(),
                        _ => format!("<fn {}>", i)
                    }
                } else {
                    format!("<fn {}>", i)
                };

                // RECURSIVE CALL
                let inner_output = chunk.disassemble(&func_name);
                s.push_str(&inner_output);
            }
        }

        s
    }
}
