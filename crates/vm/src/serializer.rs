use std::fs::File;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};
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
            DataType::List     => buf.push(0x07),
            DataType::Range    => buf.push(0x08),
            DataType::Module   => buf.push(0x09),
            DataType::Dict     => buf.push(0x0A),
            DataType::File     => buf.push(0x0B),
        }
    }

    fn write_value(&mut self, val: &Value) -> io::Result<()> {
        if val.is_int() {
            self.file.write_all(&[0x01])?;
            self.file.write_all(&(val.as_int() as i64).to_be_bytes())?;
        } else if val.is_float() {
            self.file.write_all(&[0x02])?;
            self.file.write_all(&val.as_float().to_be_bytes())?;
        } else if val.is_bool() {
            self.file.write_all(&[0x03])?;
            self.file.write_all(&[if val.as_bool() { 1 } else { 0 }])?;
        } else if val.is_none() {
            self.file.write_all(&[0x05])?;
        } else if val.is_object() {
            unsafe {
                let obj = val.as_object();
                match (*obj).ty {
                    crate::value::ObjectType::STRING => {
                        let s = &*(obj as *const crate::value::ObjectString);
                        self.file.write_all(&[0x04])?;
                        let len = s.chars.len() as u32;
                        self.file.write_all(&len.to_be_bytes())?;
                        self.file.write_all(s.chars.as_bytes())?;
                    }
                    crate::value::ObjectType::FUNCTION => {
                        let f = &*(obj as *const crate::value::ObjectFunction);
                        self.file.write_all(&[0x06])?;
                        self.write_chunk(&f.chunk)?;
                    }
                    crate::value::ObjectType::LIST => {
                        let l = &*(obj as *const crate::value::ObjectList);
                        self.file.write_all(&[0x07])?;
                        let len = l.items.len() as u32;
                        self.file.write_all(&len.to_be_bytes())?;
                        for item in &l.items {
                            self.write_value(item)?;
                        }
                    }
                    crate::value::ObjectType::RANGE => {
                        let r = &*(obj as *const crate::value::ObjectRange);
                        self.file.write_all(&[0x08])?;
                        self.file.write_all(&(r.start as i64).to_le_bytes())?;
                        self.file.write_all(&(r.stop as i64).to_le_bytes())?;
                    }
                    crate::value::ObjectType::DICT => {
                        let d = &*(obj as *const crate::value::ObjectDict);
                        self.file.write_all(&[0x09])?;
                        let len = d.items.len() as u32;
                        self.file.write_all(&len.to_be_bytes())?;
                        for (k, v) in &d.items {
                            self.write_value(k)?;
                            self.write_value(v)?;
                        }
                    }
                    _ => {
                        // Native functions, Modules, and Files cannot be serialized into constants
                        self.file.write_all(&[0x05])?; 
                    }
                }
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
            OpCode::DefineGlobal(idx, dtype) => {
                buf.push(0x02);
                buf.extend_from_slice(&(*idx as u32).to_be_bytes());
                self.write_datatype(buf, dtype);
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
            OpCode::Not => buf.push(0x0F),
            OpCode::Negate => buf.push(0x10),
            OpCode::Call(args, kwargs) => {
                buf.push(0x11);
                buf.push(*args as u8);
                buf.push(*kwargs as u8);
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
            OpCode::LessEqual => buf.push(0x16),
            OpCode::GreaterEqual => buf.push(0x17),
            OpCode::Modulo => buf.push(0x18),
            OpCode::FloorDiv => buf.push(0x19),
            OpCode::Power => buf.push(0x1A),
            OpCode::GetSubscript => buf.push(0x1B),
            OpCode::BuildList(count) => {
                buf.push(0x1C);
                buf.extend_from_slice(&(*count as u32).to_be_bytes());
            }
            OpCode::Length => buf.push(0x1D),
            OpCode::Invoke(name_idx, args, kwargs) => {
                buf.push(0x1E);
                buf.extend_from_slice(&(*name_idx as u32).to_be_bytes());
                buf.push(*args as u8);
                buf.push(*kwargs as u8);
            }
            OpCode::FormatString(count) => {
                buf.push(0x1F);
                buf.extend_from_slice(&(*count as u32).to_be_bytes());
            }
            OpCode::Import(idx) => {
                buf.push(0x20);
                buf.extend_from_slice(&(*idx as u32).to_be_bytes());
            }
            OpCode::ASSERT_TYPE(ty) => {
                buf.push(0x21);
                self.write_datatype(buf, ty);
            }
            OpCode::SetSubscript => buf.push(0x22),
            OpCode::GetProperty(name_idx) => {
                buf.push(0x23);
                buf.extend_from_slice(&(*name_idx as u32).to_be_bytes());
            }
            OpCode::SetProperty(name_idx) => {
                buf.push(0x24);
                buf.extend_from_slice(&(*name_idx as u32).to_be_bytes());
            }
            OpCode::And => buf.push(0x25),
            OpCode::Or => buf.push(0x26),
            OpCode::BuildDict(count) => {
                buf.push(0x27);
                buf.extend_from_slice(&(*count as u32).to_be_bytes());
            }
            OpCode::ListAppend => buf.push(0x28),
        }
    }
}
