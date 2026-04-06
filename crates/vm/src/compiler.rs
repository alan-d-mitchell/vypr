use crate::bytecode::{Chunk, OpCode};
use crate::value::{Value, DataType};

use lexer::token::TokenType;
use parser::ast::{Stmt, Expr, TypeExpr};

#[derive(Clone)]
struct Local {
    name: String,
    depth: usize
}

pub struct Compiler {
    chunk: Chunk,
    locals: Vec<Local>,
    scope_depth: usize,
}

impl Compiler {

    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            locals: Vec::new(),
            scope_depth: 0
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

            Stmt::VarDecl { name, value, annotation } => {
                if let Some(expr) = value {
                    self.compile_expr(expr)?;
                } else {
                    self.emit_constant(Value::None)
                }

                if self.scope_depth > 0 {
                    if let Some(idx) = self.resolve_local(&name) {
                        self.chunk.write(OpCode::SetLocal(idx));
                    } else {
                        self.add_local(name);
                    }
                } else {
                    let name_idx = self.make_constant(Value::Str(name));

                    let type_lock = if let Some(ann) = annotation {
                        match ann {
                            TypeExpr::Atomic(token_type) => match token_type {
                                TokenType::INT => DataType::Int,
                                TokenType::FLOAT => DataType::Float,
                                TokenType::STR => DataType::Str,
                                TokenType::BOOL => DataType::Bool,
                                _ => DataType::Any,
                            }

                            _ => DataType::Any
                        }
                    } else {
                        DataType::Any
                    };

                    self.chunk.write(OpCode::DefineGlobal(name_idx, type_lock));
                }
            }

            Stmt::If { condition, then, else_b } => {
                // 1. Compile Condition
                self.compile_expr(condition)?;

                // 2. Emit JumpIfFalse (Skip 'then' if false)
                let then_jump = self.emit_jump(OpCode::JumpIfFalse);
                
                // 3. Pop the condition (since JumpIfFalse leaves it on stack)
                self.chunk.write(OpCode::Pop); 

                // 4. Compile 'then' block
                for s in then {
                    self.compile_stmt(s)?;
                }

                // 5. Emit Jump (Skip 'else' after finishing 'then')
                let else_jump = self.emit_jump(OpCode::Jump);

                // 6. PATCH 'then_jump' (This is where false condition lands)
                self.patch_jump(then_jump)?;

                // 7. Pop condition (on the else path)
                self.chunk.write(OpCode::Pop); 

                // 8. Compile 'else' block
                if let Some(branch) = else_b {
                    for s in branch {
                        self.compile_stmt(s)?;
                    }
                }

                // 9. PATCH 'else_jump' (This is where 'then' block finishes)
                self.patch_jump(else_jump)?;
            }

            Stmt::For { var, iterator, body } => {
                self.enter_scope();

                self.compile_expr(iterator)?;
                self.add_local("".to_string());
                let list_slot = self.locals.len() - 1;

                self.emit_constant(Value::Int(0));
                self.add_local("".to_string());
                let index_slot = self.locals.len() - 1;

                let loop_start = self.chunk.code.len();

                self.chunk.write(OpCode::GetLocal(index_slot)); // Index
                self.chunk.write(OpCode::GetLocal(list_slot)); // List
                self.chunk.write(OpCode::Length);
                self.chunk.write(OpCode::Less);

                let exit_jump = self.emit_jump(OpCode::JumpIfFalse);
                self.chunk.write(OpCode::Pop);

                if self.resolve_local(&var).is_none() {
                    self.add_local(var.clone());
                    self.emit_constant(Value::None);
                }

                let var_idx = self.resolve_local(&var).unwrap();

                self.chunk.write(OpCode::GetLocal(list_slot)); // List
                self.chunk.write(OpCode::GetLocal(index_slot)); // Index
                self.chunk.write(OpCode::GetSubscript);
                self.chunk.write(OpCode::SetLocal(var_idx));
                self.chunk.write(OpCode::Pop);

                for s in body {
                    self.compile_stmt(s)?;
                }

                self.chunk.write(OpCode::GetLocal(index_slot));
                self.emit_constant(Value::Int(1));
                self.chunk.write(OpCode::Add);
                self.chunk.write(OpCode::SetLocal(index_slot));
                self.chunk.write(OpCode::Pop);

                self.emit_loop(loop_start)?;

                self.patch_jump(exit_jump)?;
                self.chunk.write(OpCode::Pop);

                self.chunk.write(OpCode::Pop); // Pop Index
                self.chunk.write(OpCode::Pop); // Pop List
                
                self.exit_scope();
            }

            Stmt::While { condition, body } => {
                // 1. Mark the start of the loop (where we jump back to)
                let loop_start = self.chunk.code.len();

                // 2. Compile Condition
                self.compile_expr(condition)?;

                // 3. Emit Exit Jump (if condition is false)
                let exit_jump = self.emit_jump(OpCode::JumpIfFalse);
                self.chunk.write(OpCode::Pop); // Pop condition

                // 4. Compile Body
                for s in body {
                    self.compile_stmt(s)?;
                }

                // 5. Emit Loop (Jump Back)
                self.emit_loop(loop_start)?;

                // 6. Patch Exit Jump
                self.patch_jump(exit_jump)?;
                
                // 7. Pop condition (when exiting loop)
                self.chunk.write(OpCode::Pop);
            }

            Stmt::FuncDecl { name, body, params, .. } => {
                // 1. Create a new compiler for the function body
                let mut func_compiler = Compiler::new();
                func_compiler.scope_depth = 1;

                for param in params {
                    func_compiler.add_local(param.name.clone());
                }

                for s in body {
                    func_compiler.compile_stmt(s)?;
                }

                func_compiler.emit_constant(Value::None);
                func_compiler.chunk.write(OpCode::Return);

                let func_chunk = func_compiler.chunk;
                let func_val = Value::Function(Box::new(func_chunk));
                self.emit_constant(func_val);

                let name_idx = self.make_constant(Value::Str(name));
                self.chunk.write(OpCode::DefineGlobal(name_idx, DataType::Function));
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
            Expr::Binary { left, operator, right } => {
                match operator {
                    TokenType::AND => {
                        self.compile_expr(*left)?;

                        let end_jump = self.emit_jump(OpCode::JumpIfFalse);
                        self.chunk.write(OpCode::Pop); // Discard left if true
                        
                        self.compile_expr(*right)?;
                        self.patch_jump(end_jump)?;

                        return Ok(());
                    }

                    TokenType::OR => {
                        self.compile_expr(*left)?;

                        let else_jump = self.emit_jump(OpCode::JumpIfFalse);
                        let end_jump = self.emit_jump(OpCode::Jump); // Jump over right
                        
                        self.patch_jump(else_jump)?;
                        self.chunk.write(OpCode::Pop); // Discard left if false
                        self.compile_expr(*right)?;
                        self.patch_jump(end_jump)?;

                        return Ok(());
                    }

                    _ => {
                        self.compile_expr(*left)?;
                        self.compile_expr(*right)?;
                        
                        match operator {
                            TokenType::PLUS => self.chunk.write(OpCode::Add),
                            TokenType::MINUS => self.chunk.write(OpCode::Sub),
                            TokenType::STAR => self.chunk.write(OpCode::Mul),
                            TokenType::FSLASH => self.chunk.write(OpCode::Div),
                            TokenType::MODULO => self.chunk.write(OpCode::Modulo),
                            TokenType::DOUBLE_FSLASH => self.chunk.write(OpCode::FloorDiv),
                            TokenType::DOUBLE_STAR => self.chunk.write(OpCode::Power),
                            TokenType::DOUBLE_EQUAL => self.chunk.write(OpCode::Equal),
                            TokenType::LESS_THAN => self.chunk.write(OpCode::Less),
                            TokenType::GREATER_THAN => self.chunk.write(OpCode::Greater),
                            TokenType::LESS_THAN_EQUAL => self.chunk.write(OpCode::LessEqual),
                            TokenType::GREATER_THAN_EQUAL => self.chunk.write(OpCode::GreaterEqual),
                            _ => return Err("unknown binary operator".to_string())
                        }
                    }
                }
            }

            Expr::Unary { operator, right } => {
                self.compile_expr(*right)?;

                match operator {
                    TokenType::MINUS => self.chunk.write(OpCode::Negate),
                    TokenType::NOT => self.chunk.write(OpCode::Not),

                    _ => return Err("unknown unary operator".to_string())
                }
            }

            Expr::Grouping(inner) => self.compile_expr(*inner)?,

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
                // CHECK LOCAL FIRST
                if let Some(idx) = self.resolve_local(&name) {
                    self.chunk.write(OpCode::GetLocal(idx));
                } else {
                    // Fallback to Global
                    let name_idx = self.make_constant(Value::Str(name));
                    self.chunk.write(OpCode::GetGlobal(name_idx));
                }
            }

            Expr::Call { callee, args } => {
                self.compile_expr(*callee)?;

                for arg in args.clone() {
                    self.compile_expr(arg)?;
                }

                self.chunk.write(OpCode::Call(args.len()));
            }

            Expr::Subscript { callee, index } => {
                self.compile_expr(*callee)?;
                self.compile_expr(*index)?;
                self.chunk.write(OpCode::GetSubscript);
            }

            Expr::List(elements) => {
                for element in elements.clone() {
                    self.compile_expr(element)?;
                }
                // Emit instruction to build list from the top N items on stack
                self.chunk.write(OpCode::BuildList(elements.len()));
            }
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

    fn resolve_local(&self, name: &str) -> Option<usize> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if local.name == name {
                return Some(i);
            }
        }

        None
    }

    // Helper to declare a local (for parameters)
    fn add_local(&mut self, name: String) {
        self.locals.push(Local { 
            name, 
            depth: self.scope_depth
        });
    }

    fn emit_jump(&mut self, instruction: fn(usize) -> OpCode) -> usize {
        self.chunk.write(instruction(0xFFFF)); 
        self.chunk.code.len() - 1
    }

    fn patch_jump(&mut self, offset_idx: usize) -> Result<(), String> {
        let jump = self.chunk.code.len() - offset_idx - 1;
        
        match self.chunk.code[offset_idx] {
            OpCode::JumpIfFalse(_) => self.chunk.code[offset_idx] = OpCode::JumpIfFalse(jump),
            OpCode::Jump(_) => self.chunk.code[offset_idx] = OpCode::Jump(jump),

            _ => return Err("Attempted to patch non-jump".to_string()),
        }

        Ok(())
    }

    fn emit_loop(&mut self, loop_start: usize) -> Result<(), String> {
        let offset = self.chunk.code.len() - loop_start + 1; // +1 for the Loop instruction itself
        self.chunk.write(OpCode::Loop(offset));
        Ok(())
    }

    fn enter_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn exit_scope(&mut self) {
        self.scope_depth -= 1;
        // Pop locals defined in this scope
        while let Some(local) = self.locals.last() {
            if local.depth > self.scope_depth {
                self.chunk.write(OpCode::Pop);
                self.locals.pop();
            } else {
                break;
            }
        }
    }
}
