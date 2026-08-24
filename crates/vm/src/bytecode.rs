use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;
use std::rc::Rc;

use error::error::Span;

use crate::value::{DataType, ObjectFunction, ObjectType, Value};
use crate::vm::MethodCache;

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

    And, Or,

    Jump(usize),
    JumpIfFalse(usize),
    Loop(usize),

    GetSubscript,
    SetSubscript,
    GetProperty(usize),  
    SetProperty(usize),  

    BuildList(usize),
    ListAppend,
    BuildDict(usize),
    Length,

    Call(usize, usize),
    Invoke(usize, usize, usize),   
    FormatString(usize),
    Return,

    Import(usize),

    ASSERT_TYPE(DataType),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
    pub spans: Vec<Span>,
    pub cache: Rc<RefCell<Vec<MethodCache>>>,
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
            cache: Rc::new(RefCell::new(Vec::new())),
            strings: HashMap::new(),
            ints: HashMap::new(),
            floats: HashMap::new(),
            true_idx: None,
            false_idx: None,
            none_idx: None,
        }
    }

    pub fn init_cache(&mut self) {
        *self.cache.borrow_mut() = vec![MethodCache::EMPTY; self.code.len()];
    }

    pub fn write(&mut self, op: OpCode, span: Span) {
        self.code.push(op);
        self.spans.push(span);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        // Fast path for strings: deduplicate by content
        if let Some(s) = value.as_str() {
            if let Some(&idx) = self.strings.get(s) {
                return idx;
            }

            let idx = self.constants.len();
            self.strings.insert(s.to_string(), idx);
            self.constants.push(value);
            return idx;
        }

        // For primitives (int, float, bool, none), check if exact u64 representation already exists
        if let Some(idx) = self.constants.iter().position(|&c| c.0 == value.0) {
            return idx;
        }

        self.constants.push(value);
        self.constants.len() - 1
    }

    pub fn disassemble(&self, name: &str) -> String {
        let mut s = String::new();
        writeln!(&mut s, "== {} ==", name).unwrap();

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

                OpCode::DefineGlobal(slot, dtype) => {
                    writeln!(&mut s, "{:<16} {:>4} (type: {:?})", "DEFINE_GLOBAL", format!("g{}", slot), dtype).unwrap();
                }
                OpCode::GetGlobal(slot) => {
                    writeln!(&mut s, "{:<16} {:>4}", "GET_GLOBAL", format!("g{}", slot)).unwrap();
                }
                OpCode::SetGlobal(slot) => {
                    writeln!(&mut s, "{:<16} {:>4}", "SET_GLOBAL", format!("g{}", slot)).unwrap();
                }

                OpCode::Invoke(idx, arg_count, kwarg_count) => {
                    let name = &self.constants[*idx];
                    writeln!(&mut s, "{:<16} {:4} '{}' (pos: {}, kw: {})", "INVOKE", idx, name, arg_count, kwarg_count).unwrap();
                }

                OpCode::GetLocal(idx) => writeln!(&mut s, "{:<16} {:4}", "GET_LOCAL", idx).unwrap(),
                OpCode::SetLocal(idx) => writeln!(&mut s, "{:<16} {:4}", "SET_LOCAL", idx).unwrap(),

                OpCode::Call(arg_count, kwarg_count) => {
                    writeln!(&mut s, "{:<16} {:>4} {}, kw: {}", "CALL", "pos:", arg_count, kwarg_count).unwrap()
                }

                OpCode::BuildList(count) => writeln!(&mut s, "{:<16} {:4}", "BUILD_LIST", count).unwrap(),
                OpCode::ListAppend => writeln!(&mut s, "LIST_APPEND").unwrap(),

                OpCode::LessEqual => writeln!(&mut s, "LESS_EQUAL").unwrap(),
                OpCode::GreaterEqual => writeln!(&mut s, "GREATER_EQUAL").unwrap(),
                
                OpCode::Loop(offset) => {
                    let target = (i + 1) - offset;
                    writeln!(&mut s, "{:<16} -> L{:04}", "LOOP", target).unwrap();
                }

                OpCode::Pop => {
                    writeln!(&mut s, "POP").unwrap();
                }

                OpCode::Import(idx) => {
                    let name = &self.constants[*idx];
                    writeln!(&mut s, "{:<16} {:4} '{}'", "IMPORT", idx, name).unwrap();
                }

                OpCode::ASSERT_TYPE(ty) => {
                    writeln!(&mut s, "{:<16}    {}", "ASSERT_TYPE", ty).unwrap();
                }

                _ => writeln!(&mut s, "{:?}", op).unwrap(),
            }
        }

        for (i, constant) in self.constants.iter().enumerate() {
            if constant.is_object() {
                unsafe {
                    let obj = constant.as_object();
                    if (*obj).ty == ObjectType::FUNCTION {
                        let function = &*(obj as *const ObjectFunction);
                        writeln!(&mut s).unwrap();
                        let inner_output = function.chunk.disassemble(&format!("<fn {}>", i));
                        s.push_str(&inner_output);
                    }
                }
            }
        }

        s
    }
}
