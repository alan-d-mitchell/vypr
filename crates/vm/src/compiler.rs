use crate::bytecode::{Chunk, OpCode};
use crate::value::Value;
use parser::ast::{Stmt, Expr};
use lexer::token::TokenType;

pub struct Compiler {
    chunk: Chunk
}

impl Compiler {

    pub fn new() -> Self {
        Self {
            chunk: Chunk::new()
        }
    }

    pub fn compile(mut self, ast: Vec<Stmt>) -> Result<Chunk, String> {
        for stmt in ast {
            self.compile_stmt(stmt)?;
        }
        self.chunk.write(OpCode::Return);

        Ok(self.chunk)
    }

    fn compile_stmt(&mut self, stmt: Stmt) -> Result<(), String> {
        match stmt {
            Stmt::ExprStmt(expr) => {
                self.compile_expr(expr)?;
                self.chunk.write(OpCode::Pop);
            }

            Stmt::VarDecl { name, value, .. } => {
                if let Some(expr) = value {
                    self.compile_expr(expr)?;
                } else {
                    self.emit_constant(Value::None)
                }

                let name_idx = self.make_constant(Value::Str(name));
                self.chunk.write(OpCode::DefineGlobal(name_idx));
            }

            Stmt::FuncDecl { name, body, .. } => {
                // 1. Create a new compiler for the function body
                let mut func_compiler = Compiler::new();
                
                // 2. Compile the body
                // Note: We ignore params for now (arity check can come later)
                for s in body {
                    func_compiler.compile_stmt(s)?;
                }
                
                func_compiler.emit_constant(Value::None);
                func_compiler.chunk.write(OpCode::Return);

                // 3. Wrap the resulting chunk in a Value::Function
                let func_chunk = func_compiler.chunk;
                let func_val = Value::Function(Box::new(func_chunk));

                // 4. Emit code to define the function as a global variable
                self.emit_constant(func_val);
                let name_idx = self.make_constant(Value::Str(name));
                self.chunk.write(OpCode::DefineGlobal(name_idx));
            }

            Stmt::Return { value, .. } => {
                if let Some(expr) = value {
                    self.compile_expr(expr)?;
                } else {
                    self.emit_constant(Value::None); 
                }

                self.chunk.write(OpCode::Return);
            }
        }

        Ok(())
    }

    fn compile_expr(&mut self, expr: Expr) -> Result<(), String> {
        match expr {
            Expr::Literal(token_type) => {
                match token_type {
                    TokenType::INT_LITERAL(i) => self.emit_constant(Value::Int(i)),
                    TokenType::FLOAT_LITERAL(f) => self.emit_constant(Value::Float(f)),
                    TokenType::STR_LITERAL(s) => self.emit_constant(Value::Str(s)),
                    TokenType::TRUE => self.emit_constant(Value::Bool(true)),
                    TokenType::FALSE => self.emit_constant(Value::Bool(false)),
                    TokenType::NONE => self.emit_constant(Value::None),
                    _ => {}
                }
            }

            Expr::Variable(name) => {
                let name_idx = self.make_constant(Value::Str(name));
                self.chunk.write(OpCode::GetGlobal(name_idx));
            }

            Expr::Call { callee, args } => {
                self.compile_expr(*callee)?;

                for arg in args.clone() {
                    self.compile_expr(arg)?;
                }

                self.chunk.write(OpCode::Call(args.len()));
            }

            _ => return Err("unsupported expression".to_string())
        }

        Ok(())
    }

    fn emit_constant(&mut self, value: Value) {
        let idx = self.make_constant(value);
        self.chunk.write(OpCode::Constant(idx));
    }

    fn make_constant(&mut self, value: Value) -> usize {
        self.chunk.add_constant(value)
    }
}
