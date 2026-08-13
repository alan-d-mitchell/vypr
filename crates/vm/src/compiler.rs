use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::bytecode::{Chunk, OpCode};
use crate::value::{DataType, Value, VyprFunction};

use error::error::{Span, VyprError};
use lexer::token::TokenType;
use parser::ast::TypeExpr;
use mir::mir::*;
use vir::vir::{VIRBinOp, VIRUnaryOp};

pub struct Compiler {
    chunk: Chunk,
    bb_offsets: HashMap<BasicBlockID, usize>,
    forward_jumps: Vec<(usize, BasicBlockID)>,
    pub globals: Rc<RefCell<HashMap<String, usize>>>,
}

impl Compiler {

    pub fn new() -> Self {
        let mut globals = HashMap::new();
        let builtins = [
            "print", 
            "int", 
            "float", 
            "str", 
            "len", 
            "range", 
            "list", 
            "reversed", 
            "input", 
            "open"
        ];

        for (i, name) in builtins.iter().enumerate() {
            globals.insert(name.to_string(), i);
        }

        Self {
            chunk: Chunk::new(),
            bb_offsets: HashMap::new(),
            forward_jumps: Vec::new(),
            globals: Rc::new(RefCell::new(globals)),
        }
    }

    pub fn with_globals(globals: Rc<RefCell<HashMap<String, usize>>>) -> Self {
        Self {
            chunk: Chunk::new(),
            bb_offsets: HashMap::new(),
            forward_jumps: Vec::new(),
            globals,
        }
    }

    fn resolve_global(&mut self, name: &str) -> usize {
        let mut map = self.globals.borrow_mut();

        if let Some(&slot) = map.get(name) {
            slot
        } else {
            let slot = map.len();
            map.insert(name.to_string(), slot);
            slot
        }
    }

    fn error(&self, code: &'static str, message: impl Into<String>, span: Span) -> VyprError {
        VyprError::new(code, message, span)
    }

    pub fn compile_program(mut self, mir_program: &MIRProgram) -> Result<Chunk, VyprError> {
        let mut script_chunk = Chunk::new();

        for function in &mir_program.functions {
            self.resolve_global(&function.name);
        }
        self.resolve_global("<script>");

        for func in &mir_program.functions {
            let func_compiler = Compiler::with_globals(Rc::clone(&self.globals));
            let func_chunk = func_compiler.compile_function(func)?;

            let func_val = Value::Function(Rc::new(VyprFunction { 
                arity: func.arity, 
                upvalues: func.locals.len(), 
                chunk: Rc::new(func_chunk),
            }));

            let func_val_idx = script_chunk.add_constant(func_val);
            let slot = self.resolve_global(&func.name);

            script_chunk.write(OpCode::Constant(func_val_idx), Span::default());
            script_chunk.write(OpCode::DefineGlobal(slot, DataType::Function), Span::default());
        }

        let script_slot = self.resolve_global("<script>");
        script_chunk.write(OpCode::GetGlobal(script_slot), Span::default());
        script_chunk.write(OpCode::Call(0), Span::default());

        Ok(script_chunk)
    }

    pub fn compile_function(mut self, mir_func: &MIRFunction) -> Result<Chunk, VyprError> {
        for (bb_idx, block) in mir_func.basic_blocks.iter().enumerate() {
            let current_bb = BasicBlockID(bb_idx);
            
            self.bb_offsets.insert(current_bb, self.chunk.code.len());

            for stmt in &block.statements {
                self.compile_statement(stmt, &mir_func.locals)?;
            }

            self.compile_terminator(&block.terminator, current_bb, Span::default())?;
        }

        for (inst_idx, target_bb) in &self.forward_jumps {
            let target_offset = *self.bb_offsets.get(target_bb).expect("target bb not found!");
            let jump_length = target_offset - (*inst_idx + 1);

            match self.chunk.code[*inst_idx] {
                OpCode::Jump(_) => self.chunk.code[*inst_idx] = OpCode::Jump(jump_length),
                OpCode::JumpIfFalse(_) => self.chunk.code[*inst_idx] = OpCode::JumpIfFalse(jump_length),
                _ => unreachable!("attempted to backpatch a non-jump instruction"),
            }
        }

        Ok(self.chunk)
    }

    fn compile_statement(&mut self, stmt: &Statement, locals: &[LocalDecl]) -> Result<(), VyprError> {
        let span = stmt.span;

        match &stmt.kind {
            StatementKind::Assign(place, rval) => {
                self.compile_rvalue(rval, span)?;

                let local_decl = &locals[place.local.0];
                let data_type = self.type_expr_to_datatype(&local_decl.ty);
                if data_type != DataType::Any {
                    self.chunk.write(OpCode::ASSERT_TYPE(data_type), span);
                }

                if place.projection.is_empty() {
                    self.chunk.write(OpCode::SetLocal(place.local.0), span);
                } else {
                    self.chunk.write(OpCode::GetLocal(place.local.0), span);

                    for (i, proj) in place.projection.iter().enumerate() {
                        let is_last = i == place.projection.len() - 1;
                        
                        match proj {
                            ProjectionElem::Index(idx_local) => {
                                self.chunk.write(OpCode::GetLocal(idx_local.0), span);
                                if is_last {
                                    self.chunk.write(OpCode::SetSubscript, span); 
                                } else {
                                    self.chunk.write(OpCode::GetSubscript, span);
                                }
                            }
                            ProjectionElem::Property(name) => {
                                let name_idx = self.chunk.add_constant(Value::make_string(name));
                                if is_last {
                                    self.chunk.write(OpCode::SetProperty(name_idx), span);
                                } else {
                                    self.chunk.write(OpCode::GetProperty(name_idx), span);
                                }
                            }
                        }
                    }
                }
            }

            StatementKind::DefineGlobal(name, ty, rval) => {
                self.compile_rvalue(rval, span)?;
                let slot = self.resolve_global(name);
                let dtype = self.type_expr_to_datatype(ty);
                self.chunk.write(OpCode::DefineGlobal(slot, dtype), span);
            }

            StatementKind::AssignGlobal(name, rval) => {
                self.compile_rvalue(rval, span)?;
                let slot = self.resolve_global(name);
                self.chunk.write(OpCode::SetGlobal(slot), span);
            }
        }

        Ok(())
    }

    fn compile_rvalue(&mut self, rval: &Rvalue, span: Span) -> Result<(), VyprError> {
        match rval {
            Rvalue::Use(operand) => self.compile_operand(operand, span)?,
            
            Rvalue::BinaryOp(op, lhs, rhs) => {
                self.compile_operand(lhs, span)?;
                self.compile_operand(rhs, span)?;
                
                match op {
                    VIRBinOp::Add => self.chunk.write(OpCode::Add, span),
                    VIRBinOp::Sub => self.chunk.write(OpCode::Sub, span),
                    VIRBinOp::Mul => self.chunk.write(OpCode::Mul, span),
                    VIRBinOp::Div => self.chunk.write(OpCode::Div, span),
                    VIRBinOp::Mod => self.chunk.write(OpCode::Modulo, span),
                    VIRBinOp::FloorDiv => self.chunk.write(OpCode::FloorDiv, span),
                    VIRBinOp::Power => self.chunk.write(OpCode::Power, span),
                    VIRBinOp::Eq => self.chunk.write(OpCode::Equal, span),
                    VIRBinOp::Ne => {
                        self.chunk.write(OpCode::Equal, span);
                        self.chunk.write(OpCode::Not, span);
                    }
                    VIRBinOp::And => self.chunk.write(OpCode::And, span),
                    VIRBinOp::Or => self.chunk.write(OpCode::Or, span),
                    VIRBinOp::Lt => self.chunk.write(OpCode::Less, span),
                    VIRBinOp::Le => self.chunk.write(OpCode::LessEqual, span),
                    VIRBinOp::Gt => self.chunk.write(OpCode::Greater, span),
                    VIRBinOp::Ge => self.chunk.write(OpCode::GreaterEqual, span),
                }
            }

            Rvalue::UnaryOp(op, operand) => {
                self.compile_operand(operand, span)?;
                match op {
                    VIRUnaryOp::Neg => self.chunk.write(OpCode::Negate, span),
                    VIRUnaryOp::Not => self.chunk.write(OpCode::Not, span),
                }
            }

            Rvalue::ListInit(elements) => {
                for element in elements {
                    self.compile_operand(element, span)?;
                }

                self.chunk.write(OpCode::BuildList(elements.len()), span);
            }

            Rvalue::DictInit(keys, values) => {
                for (k, v) in keys.iter().zip(values.iter()) {
                    self.compile_operand(k, span)?;
                    self.compile_operand(v, span)?;
                }

                self.chunk.write(OpCode::BuildDict(keys.len()), span);
            }

            Rvalue::Import(module) => {
                let name_idx = self.chunk.add_constant(Value::make_string(module));
                self.chunk.write(OpCode::Import(name_idx), span);
            }

            Rvalue::FunctionDef(function) => {
                let compiler = Compiler::with_globals(self.globals.clone());
                let chunk = compiler.compile_function(function)?;

                let val = Value::Function(Rc::new(VyprFunction {
                    arity: function.arity,
                    upvalues: function.locals.len(),
                    chunk: Rc::new(chunk),
                }));

                let idx = self.chunk.add_constant(val);
                self.chunk.write(OpCode::Constant(idx), span)
            }

            Rvalue::FormatString(ops) => {
                for op in ops {
                    self.compile_operand(op, span)?;
                }

                self.chunk.write(OpCode::FormatString(ops.len()), span);
            }

            Rvalue::Length(op) => {
                self.compile_operand(op, span)?;
                self.chunk.write(OpCode::Length, span);
            }

            Rvalue::ListAppend(list_op, item_op) => {
                self.compile_operand(list_op, span)?;
                self.compile_operand(item_op, span)?;
                self.chunk.write(OpCode::ListAppend, span);
            }
        }

        Ok(())
    }

    fn compile_operand(&mut self, operand: &Operand, span: Span) -> Result<(), VyprError> {
        match operand {
            Operand::Copy(place) => {
                self.chunk.write(OpCode::GetLocal(place.local.0), span);

                for proj in &place.projection {
                    match proj {
                        ProjectionElem::Index(idx_local) => {
                            self.chunk.write(OpCode::GetLocal(idx_local.0), span);
                            self.chunk.write(OpCode::GetSubscript, span);
                        }
                        ProjectionElem::Property(name) => {
                            let name_idx = self.chunk.add_constant(Value::make_string(name));
                            self.chunk.write(OpCode::GetProperty(name_idx), span);
                        }
                    }
                }
            }
            Operand::Const(c) => {
                let val = match c {
                    vir::context::Constant::Int(i) => Value::Int(*i),
                    vir::context::Constant::Float(f) => Value::Float(*f),
                    vir::context::Constant::String(s) => Value::make_string(s),
                    vir::context::Constant::Bool(b) => Value::Bool(*b),
                    vir::context::Constant::None => Value::None,
                };
                let idx = self.chunk.add_constant(val);
                self.chunk.write(OpCode::Constant(idx), span);
            }
            Operand::Static(name) => {
                let slot = self.resolve_global(name);
                self.chunk.write(OpCode::GetGlobal(slot), span);
            }
        }
        Ok(())
    }

    fn compile_terminator(&mut self, terminator: &Terminator, current_bb: BasicBlockID, span: Span) -> Result<(), VyprError> {
        match terminator {
            Terminator::Goto { target } => {
                self.emit_jump_or_loop(*target, current_bb, OpCode::Jump, span);
            }
            Terminator::SwitchInt { discriminant, true_target, false_target } => {
                self.compile_operand(discriminant, span)?;
                
                let jmp_false_idx = self.chunk.code.len();
                self.chunk.write(OpCode::JumpIfFalse(0xFFFF), span); 
                self.forward_jumps.push((jmp_false_idx, *false_target));
                self.chunk.write(OpCode::Pop, span); 

                self.emit_jump_or_loop(*true_target, current_bb, OpCode::Jump, span);
            }
            Terminator::Call { callee, args, destination, target } => {
                self.compile_operand(callee, span)?;
                for arg in args {
                    self.compile_operand(arg, span)?;
                }
                
                self.chunk.write(OpCode::Call(args.len()), span);
                self.chunk.write(OpCode::SetLocal(destination.local.0), span);

                self.emit_jump_or_loop(*target, current_bb, OpCode::Jump, span);
            }
            Terminator::MethodCall { object, method_name, args, destination, target } => {
                self.compile_operand(object, span)?;
                for arg in args {
                    self.compile_operand(arg, span)?;
                }
                
                let name_idx = self.chunk.add_constant(Value::make_string(method_name));
                self.chunk.write(OpCode::Invoke(name_idx, args.len()), span);
                self.chunk.write(OpCode::SetLocal(destination.local.0), span);

                self.emit_jump_or_loop(*target, current_bb, OpCode::Jump, span);
            }
            Terminator::Return => {
                self.chunk.write(OpCode::GetLocal(0), span);
                self.chunk.write(OpCode::Return, span);
            }
            Terminator::Unreachable => {}
        }
        Ok(())
    }

    fn emit_jump_or_loop(&mut self, target: BasicBlockID, _current_bb: BasicBlockID, jump_op: fn(usize) -> OpCode, span: Span) {
        if let Some(&target_offset) = self.bb_offsets.get(&target) {
            let current_offset = self.chunk.code.len() + 1; 
            let loop_length = current_offset - target_offset;
            self.chunk.write(OpCode::Loop(loop_length), span);
        } else {
            let idx = self.chunk.code.len();
            self.chunk.write(jump_op(0xFFFF), span);
            self.forward_jumps.push((idx, target));
        }
    }

    fn type_expr_to_datatype(&self, ty: &TypeExpr) -> DataType {
        match ty {
            TypeExpr::Atomic(token_type) => match token_type {
                TokenType::INT => DataType::Int,
                TokenType::FLOAT => DataType::Float,
                TokenType::STR => DataType::Str,
                TokenType::BOOL => DataType::Bool,
                _ => DataType::Any,
            }
            _ => DataType::Any
        }
    }
}
