use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;

use error::error::Span;

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
    Invoke(usize, usize),   
    Return,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
    pub spans: Vec<Span>,
    strings: HashMap<String, usize>,
    ints: HashMap<i64, usize>,
    floats: HashMap<u64, usize>,
    true_idx: Option<usize>,
    false_idx: Option<usize>,
    none_idx: Option<usize>,
}

impl Chunk {

    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            spans: Vec::new(),
            strings: HashMap::new(),
            ints: HashMap::new(),
            floats: HashMap::new(),
            true_idx: None,
            false_idx: None,
            none_idx: None,
        }
    }

    pub fn write(&mut self, op: OpCode, span: Span) {
        self.code.push(op);
        self.spans.push(span);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        match &value {
            Value::Str(s) => {
                if let Some(&idx) = self.strings.get(s) {
                    return idx;
                }

                let idx = self.constants.len();

                self.strings.insert(s.clone(), idx);
                self.constants.push(value);

                idx
            }

            Value::Int(i) => {
                if let Some(&idx) = self.ints.get(i) {
                    return idx;
                }

                let idx = self.constants.len();

                self.ints.insert(*i, idx);
                self.constants.push(value);

                idx
            }

            Value::Float(f) => {
                let bits = f.to_bits();

                if let Some(&idx) = self.floats.get(&bits) {
                    return idx;
                }

                let idx = self.constants.len();

                self.floats.insert(bits, idx);
                self.constants.push(value);

                idx
            }

            Value::Bool(b) => {
                if *b {
                    if let Some(idx) = self.true_idx { return idx; }
                    let idx = self.constants.len();

                    self.true_idx = Some(idx);
                    self.constants.push(value);

                    idx
                } else {
                    if let Some(idx) = self.false_idx { return idx; }

                    let idx = self.constants.len();

                    self.false_idx = Some(idx);
                    self.constants.push(value);

                    idx
                }
            }

            Value::None => {
                if let Some(idx) = self.none_idx { return idx; }
                let idx = self.constants.len();

                self.none_idx = Some(idx);
                self.constants.push(value);

                idx
            }

            _ => {
                self.constants.push(value);
                self.constants.len() - 1
            }
        }
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
