use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum OpCode {
    Constant(usize),

    DefineGlobal(usize),
    GetGlobal(usize),
    SetGlobal(usize),

    Pop,

    Add, Sub, Mul, Div,

    Call(usize),
    Return,
}

#[derive(Debug, Clone)]
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
}
