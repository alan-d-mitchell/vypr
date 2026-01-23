use std::fs::File;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH}; // NEW: For timestamp
use crate::bytecode::{Chunk, OpCode};
use crate::value::{Value, DataType};

pub struct Serializer {
    file: File,
}

impl Serializer {

    pub fn new(path: &str) -> io::Result<Self> {
        Ok(Self {
            file: File::create(path)?,
        })
    }

    pub fn serialize(&mut self, chunk: &Chunk) -> io::Result<()> {
        // HEADER
        self.file.write_all(b"COIL")?; // 0x43, 0x4F, 0x49, 0x4C
        
        // VERSION
        self.file.write_all(&[0x01])?;

        // TIMESTAMP
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs() as i64;

        self.file.write_all(&since_the_epoch.to_be_bytes())?;

        self.write_chunk(chunk)?;

        Ok(())
    }

    fn write_chunk(&mut self, chunk: &Chunk) -> io::Result<()> {
        // A. Constants
        let const_count = chunk.constants.len() as u32;
        self.file.write_all(&const_count.to_be_bytes())?;

        for constant in &chunk.constants {
            self.write_value(constant)?;
        }

        // B. Code
        let mut code_bytes = Vec::new();
        for op in &chunk.code {
            self.write_opcode(op, &mut code_bytes);
        }

        let code_len = code_bytes.len() as u32;
        self.file.write_all(&code_len.to_be_bytes())?;
        self.file.write_all(&code_bytes)?;

        Ok(())
    }

    fn write_datatype(&self, buf: &mut Vec<u8>, dt: &DataType) {
        match dt {
            DataType::Any      => buf.push(0x00),
            DataType::Int      => buf.push(0x01),
            DataType::Float    => buf.push(0x02),
            DataType::Str      => buf.push(0x03),
            DataType::Bool     => buf.push(0x04),
            DataType::None     => buf.push(0x05),
            DataType::Function => buf.push(0x06),
        }
    }

    fn write_value(&mut self, val: &Value) -> io::Result<()> {
        match val {
            Value::Int(i) => {
                self.file.write_all(&[0x01])?;
                self.file.write_all(&i.to_be_bytes())?;
            }
            Value::Float(f) => {
                self.file.write_all(&[0x02])?;
                self.file.write_all(&f.to_be_bytes())?;
            }
            Value::Bool(b) => {
                self.file.write_all(&[0x03])?;
                self.file.write_all(&[if *b { 1 } else { 0 }])?;
            }
            Value::Str(s) => {
                self.file.write_all(&[0x04])?;
                let len = s.len() as u32;
                self.file.write_all(&len.to_be_bytes())?;
                self.file.write_all(s.as_bytes())?;
            }
            Value::None => {
                self.file.write_all(&[0x05])?;
            }
            Value::Function(chunk) => {
                self.file.write_all(&[0x06])?;
                // Recursive serialization for function bodies
                self.write_chunk(chunk)?;
            }
            Value::Native(_) => {
                // Cannot serialize native functions
                self.file.write_all(&[0x05])?; // Write None placeholder
            }
        }
        Ok(())
    }

    fn write_opcode(&self, op: &OpCode, buf: &mut Vec<u8>) {
        match op {
            OpCode::Constant(idx) => {
                buf.push(0x01);
                buf.extend_from_slice(&(*idx as u32).to_be_bytes());
            }
            
            // serialize the data type lock
            OpCode::DefineGlobal(idx, dtype) => {
                buf.push(0x02);
                buf.extend_from_slice(&(*idx as u32).to_be_bytes());
                self.write_datatype(buf, dtype); // <--- HERE
            }
            
            OpCode::GetGlobal(idx) => {
                buf.push(0x03);
                buf.extend_from_slice(&(*idx as u32).to_be_bytes());
            }
            OpCode::SetGlobal(idx) => {
                buf.push(0x04);
                buf.extend_from_slice(&(*idx as u32).to_be_bytes());
            }
            OpCode::GetLocal(idx) => {
                buf.push(0x05);
                buf.extend_from_slice(&(*idx as u32).to_be_bytes());
            }
            OpCode::SetLocal(idx) => {
                buf.push(0x06);
                buf.extend_from_slice(&(*idx as u32).to_be_bytes());
            }
            OpCode::Pop => buf.push(0x07),
            OpCode::Add => buf.push(0x08),
            OpCode::Sub => buf.push(0x09),
            OpCode::Mul => buf.push(0x0A),
            OpCode::Div => buf.push(0x0B),
            OpCode::Equal => buf.push(0x0C),
            OpCode::Less => buf.push(0x0D),
            OpCode::Greater => buf.push(0x0E),
            OpCode::LessEqual => buf.push(0x16),
            OpCode::GreaterEqual => buf.push(0x17),
            OpCode::Not => buf.push(0x0F),
            OpCode::Negate => buf.push(0x10),
            OpCode::Call(args) => {
                buf.push(0x11);
                buf.push(*args as u8);
            }
            OpCode::Return => buf.push(0x12),
            OpCode::Jump(offset) => {
                buf.push(0x13);
                buf.extend_from_slice(&(*offset as u32).to_be_bytes());
            }
            OpCode::JumpIfFalse(offset) => {
                buf.push(0x14);
                buf.extend_from_slice(&(*offset as u32).to_be_bytes());
            }
            OpCode::Loop(offset) => {
                buf.push(0x15);
                buf.extend_from_slice(&(*offset as u32).to_be_bytes());
            }
        }
    }
}
