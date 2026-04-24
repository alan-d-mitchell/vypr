use crate::bytecode::{Chunk, OpCode};
use crate::value::{Value, DataType};

use lexer::token::TokenType;
use parser::ast::{Expr, ExprKind, Stmt, StmtKind, TypeExpr};
use error::error::{Span, VyprError};

#[derive(Clone)]
struct Local {
    name: String,
    depth: usize
}

struct LoopState {
    continue_jumps: Vec<usize>,
    break_jumps: Vec<usize>,
}

pub struct Compiler {
    chunk: Chunk,
    locals: Vec<Local>,
    scope_depth: usize,
    loop_stack: Vec<LoopState>
}

impl Compiler {

    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            locals: Vec::new(),
            scope_depth: 0,
            loop_stack: Vec::new()
        }
    }

    fn error(&self, code: &'static str, message: impl Into<String>, span: Span) -> VyprError {
        VyprError::new(code, message, span)
    }

    pub fn compile(mut self, ast: Vec<Stmt>) -> Result<Chunk, VyprError> {
        for stmt in ast {
            self.compile_stmt(stmt)?;
        }

        self.chunk.write(OpCode::Return, Span::default());

        Ok(self.chunk)
    }

    fn compile_stmt(&mut self, stmt: Stmt) -> Result<(), VyprError> {
        let span = stmt.span;

        match stmt.kind {
            StmtKind::ExprStmt(expr) => {
                self.compile_expr(expr)?;
                self.chunk.write(OpCode::Pop, span);
            }

            StmtKind::VarDecl { name, value, annotation } => {
                if let Some(expr) = value {
                    self.compile_expr(expr)?;
                } else {
                    self.emit_constant(Value::None, span)
                }

                if self.scope_depth > 0 {
                    if let Some(idx) = self.resolve_local(&name) {
                        self.chunk.write(OpCode::SetLocal(idx), span);
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

                    self.chunk.write(OpCode::DefineGlobal(name_idx, type_lock), span);
                }
            }

            StmtKind::If { condition, then, else_b } => {
                // 1. Compile Condition
                self.compile_expr(condition)?;

                // 2. Emit JumpIfFalse (Skip 'then' if false)
                let then_jump = self.emit_jump(OpCode::JumpIfFalse, span);
                
                // 3. Pop the condition (since JumpIfFalse leaves it on stack)
                self.chunk.write(OpCode::Pop, span); 

                // 4. Compile 'then' block
                for s in then {
                    self.compile_stmt(s)?;
                }

                // 5. Emit Jump (Skip 'else' after finishing 'then')
                let else_jump = self.emit_jump(OpCode::Jump, span);

                // 6. PATCH 'then_jump' (This is where false condition lands)
                self.patch_jump(then_jump)?;

                // 7. Pop condition (on the else path)
                self.chunk.write(OpCode::Pop, span); 

                // 8. Compile 'else' block
                if let Some(branch) = else_b {
                    for s in branch {
                        self.compile_stmt(s)?;
                    }
                }

                // 9. PATCH 'else_jump' (This is where 'then' block finishes)
                self.patch_jump(else_jump)?;
            }

            StmtKind::For { var, iterator, body } => {
                self.enter_scope();

                self.compile_expr(iterator)?;
                self.add_local("".to_string());
                let list_slot = self.locals.len() - 1;

                self.emit_constant(Value::Int(0), span);
                self.add_local("".to_string());
                let index_slot = self.locals.len() - 1;

                if self.resolve_local(&var).is_none() {
                    self.add_local(var.clone());
                    self.emit_constant(Value::None, span);
                }

                let var_idx = self.resolve_local(&var).unwrap();

                let loop_start = self.chunk.code.len();

                self.loop_stack.push(LoopState {
                    break_jumps: Vec::new(),
                    continue_jumps: Vec::new(),
                });

                self.chunk.write(OpCode::GetLocal(index_slot), span); 
                self.chunk.write(OpCode::GetLocal(list_slot), span); 
                self.chunk.write(OpCode::Length, span);
                self.chunk.write(OpCode::Less, span);

                let exit_jump = self.emit_jump(OpCode::JumpIfFalse, span);
                self.chunk.write(OpCode::Pop, span);

                self.chunk.write(OpCode::GetLocal(list_slot), span); 
                self.chunk.write(OpCode::GetLocal(index_slot), span); 
                self.chunk.write(OpCode::GetSubscript, span);
                self.chunk.write(OpCode::SetLocal(var_idx), span);

                for s in body {
                    self.compile_stmt(s)?;
                }

                let current_loop = self.loop_stack.pop().unwrap();

                for continue_jump in current_loop.continue_jumps {
                    self.patch_jump(continue_jump)?;
                }

                self.chunk.write(OpCode::GetLocal(index_slot), span);
                self.emit_constant(Value::Int(1), span);
                self.chunk.write(OpCode::Add, span);
                self.chunk.write(OpCode::SetLocal(index_slot), span);

                self.emit_loop(loop_start, span)?;

                self.patch_jump(exit_jump)?;
                self.chunk.write(OpCode::Pop, span);

                for break_jump in current_loop.break_jumps {
                    self.patch_jump(break_jump)?;
                }

                self.exit_scope();
            }

            StmtKind::While { condition, body } => {
                let loop_start = self.chunk.code.len();

                self.loop_stack.push(LoopState {
                    break_jumps: Vec::new(),
                    continue_jumps: Vec::new(),
                });

                self.compile_expr(condition)?;

                let exit_jump = self.emit_jump(OpCode::JumpIfFalse, span);
                self.chunk.write(OpCode::Pop, span);

                for s in body {
                    self.compile_stmt(s)?;
                }

                let current_loop = self.loop_stack.pop().unwrap();

                for continue_jump in current_loop.continue_jumps {
                    self.patch_jump(continue_jump)?;
                }

                self.emit_loop(loop_start, span)?;

                self.patch_jump(exit_jump)?;

                // 7. Pop condition (when exiting loop)
                self.chunk.write(OpCode::Pop, span);

                // Patch Breaks (Lands completely outside the loop!)
                for break_jump in current_loop.break_jumps {
                    self.patch_jump(break_jump)?;
                }
            }

            StmtKind::FuncDecl { name, body, params, .. } => {
                // 1. Create a new compiler for the function body
                let mut func_compiler = Compiler::new();
                func_compiler.scope_depth = 1;

                for param in params {
                    func_compiler.add_local(param.name.clone());
                }

                for s in body {
                    func_compiler.compile_stmt(s)?;
                }

                func_compiler.emit_constant(Value::None, span);
                func_compiler.chunk.write(OpCode::Return, span);

                let func_chunk = func_compiler.chunk;
                let func_val = Value::Function(Box::new(func_chunk));
                self.emit_constant(func_val, span);

                let name_idx = self.make_constant(Value::Str(name));
                self.chunk.write(OpCode::DefineGlobal(name_idx, DataType::Function), span);
            }

            StmtKind::Return { value, .. } => {
                if let Some(expr) = value {
                    self.compile_expr(expr)?;
                } else {
                    self.emit_constant(Value::None, span); 
                }

                self.chunk.write(OpCode::Return, span);
            }

            StmtKind::Pass => {
                return Ok(())
            }

            StmtKind::Break => {
                if self.loop_stack.is_empty() {
                    return Err(self.error("C004", "'break' outside loop", span));
                }

                let jump = self.emit_jump(OpCode::Jump, span);

                self.loop_stack.last_mut().unwrap().break_jumps.push(jump);
            }

            StmtKind::Continue => {
                if self.loop_stack.is_empty() {
                    return Err(self.error("C005", "'continue' outside loop", span));
                }

                let jump = self.emit_jump(OpCode::Jump, span);
                self.loop_stack.last_mut().unwrap().continue_jumps.push(jump); 
            }
        }

        Ok(())
    }

    fn compile_expr(&mut self, expr: Expr) -> Result<(), VyprError> {
        let span = expr.span;

        match expr.kind {
            ExprKind::Binary { left, operator, right } => {
                match operator {
                    TokenType::AND => {
                        self.compile_expr(*left)?;

                        let end_jump = self.emit_jump(OpCode::JumpIfFalse, span);
                        self.chunk.write(OpCode::Pop, span); // Discard left if true
                        
                        self.compile_expr(*right)?;
                        self.patch_jump(end_jump)?;

                        return Ok(());
                    }

                    TokenType::OR => {
                        self.compile_expr(*left)?;

                        let else_jump = self.emit_jump(OpCode::JumpIfFalse, span);
                        let end_jump = self.emit_jump(OpCode::Jump, span); // Jump over right
                        
                        self.patch_jump(else_jump)?;
                        self.chunk.write(OpCode::Pop, span); // Discard left if false
                        self.compile_expr(*right)?;
                        self.patch_jump(end_jump)?;

                        return Ok(());
                    }

                    _ => {
                        self.compile_expr(*left)?;
                        self.compile_expr(*right)?;
                        
                        match operator {
                            TokenType::PLUS => self.chunk.write(OpCode::Add, span),
                            TokenType::MINUS => self.chunk.write(OpCode::Sub, span),
                            TokenType::STAR => self.chunk.write(OpCode::Mul, span),
                            TokenType::FSLASH => self.chunk.write(OpCode::Div, span),
                            TokenType::MODULO => self.chunk.write(OpCode::Modulo, span),
                            TokenType::DOUBLE_FSLASH => self.chunk.write(OpCode::FloorDiv, span),
                            TokenType::DOUBLE_STAR => self.chunk.write(OpCode::Power, span),
                            TokenType::DOUBLE_EQUAL => self.chunk.write(OpCode::Equal, span),
                            TokenType::LESS_THAN => self.chunk.write(OpCode::Less, span),
                            TokenType::GREATER_THAN => self.chunk.write(OpCode::Greater, span),
                            TokenType::LESS_THAN_EQUAL => self.chunk.write(OpCode::LessEqual, span),
                            TokenType::GREATER_THAN_EQUAL => self.chunk.write(OpCode::GreaterEqual, span),
                            _ => return Err(self.error("C001", "unknown binary operator", span))
                        }
                    }
                }
            }

            ExprKind::Unary { operator, right } => {
                self.compile_expr(*right)?;

                match operator {
                    TokenType::MINUS => self.chunk.write(OpCode::Negate, span),
                    TokenType::NOT => self.chunk.write(OpCode::Not, span),

                    _ => return Err(self.error("C002", "unknown unary operator", span))
                }
            }

            ExprKind::Grouping(inner) => self.compile_expr(*inner)?,

            ExprKind::Literal(token_type) => {
                match token_type {
                    TokenType::INT_LITERAL(i) => self.emit_constant(Value::Int(i), span),
                    TokenType::FLOAT_LITERAL(f) => self.emit_constant(Value::Float(f), span),
                    TokenType::STR_LITERAL(s) => self.emit_constant(Value::Str(s), span),
                    TokenType::TRUE => self.emit_constant(Value::Bool(true), span),
                    TokenType::FALSE => self.emit_constant(Value::Bool(false), span),
                    TokenType::NONE => self.emit_constant(Value::None, span),
                    _ => {}
                }
            }

            ExprKind::Variable(name) => {
                // CHECK LOCAL FIRST
                if let Some(idx) = self.resolve_local(&name) {
                    self.chunk.write(OpCode::GetLocal(idx), span);
                } else {
                    // Fallback to Global
                    let name_idx = self.make_constant(Value::Str(name));
                    self.chunk.write(OpCode::GetGlobal(name_idx), span);
                }
            }

            ExprKind::Call { callee, args } => {
                self.compile_expr(*callee)?;

                for arg in args.clone() {
                    self.compile_expr(arg)?;
                }

                self.chunk.write(OpCode::Call(args.len()), span);
            }

            ExprKind::MethodCall { callee, args, method } => {
                self.compile_expr(*callee)?;
                
                // 2. Push the arguments
                for arg in &args {
                    self.compile_expr(arg.clone())?;
                }

                // 3. Emit the Invoke instruction
                let name_idx = self.make_constant(Value::Str(method.clone()));
                self.chunk.write(OpCode::Invoke(name_idx, args.len()), span);
            }

            ExprKind::Subscript { callee, index } => {
                self.compile_expr(*callee)?;
                self.compile_expr(*index)?;
                self.chunk.write(OpCode::GetSubscript, span);
            }

            ExprKind::List(elements) => {
                for element in elements.clone() {
                    self.compile_expr(element)?;
                }
                // Emit instruction to build list from the top N items on stack
                self.chunk.write(OpCode::BuildList(elements.len()), span);
            }
        }

        Ok(())
    }

    fn emit_constant(&mut self, value: Value, span: Span) {
        let idx = self.make_constant(value);
        self.chunk.write(OpCode::Constant(idx), span);
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

    fn emit_jump(&mut self, instruction: fn(usize) -> OpCode, span: Span) -> usize {
        self.chunk.write(instruction(0xFFFF), span); 
        self.chunk.code.len() - 1
    }

    fn patch_jump(&mut self, offset_idx: usize) -> Result<(), VyprError> {
        let jump = self.chunk.code.len() - offset_idx - 1;
        
        match self.chunk.code[offset_idx] {
            OpCode::JumpIfFalse(_) => self.chunk.code[offset_idx] = OpCode::JumpIfFalse(jump),
            OpCode::Jump(_) => self.chunk.code[offset_idx] = OpCode::Jump(jump),

            _ => return Err(self.error("C003", "attempted to patch non-jump", Span::default())),
        }

        Ok(())
    }

    fn emit_loop(&mut self, loop_start: usize, span: Span) -> Result<(), VyprError> {
        let offset = self.chunk.code.len() - loop_start + 1; // +1 for the Loop instruction itself
        self.chunk.write(OpCode::Loop(offset), span);
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
                self.chunk.write(OpCode::Pop, Span::default());
                self.locals.pop();
            } else {
                break;
            }
        }
    }
}
