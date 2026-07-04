use std::collections::HashMap;
use crate::mir::*;
use lexer::token::TokenType;
use vir::vir::{VIRFunction, VIRBlock, VIRStmt, VIRExpr, VIRExprKind};
use parser::ast::TypeExpr;
use vir::context::{Constant, VarID};

pub struct MIRBuilder {
    is_script: bool,
    locals: Vec<LocalDecl>,
    basic_blocks: Vec<BasicBlock>,
    current_block: BasicBlockID,
    
    // Maps a VIR VarID (like `x` or `canvas`) to an MIR LocalID (like `_1`)
    var_map: HashMap<usize, LocalID>, 
    
    // Stack of (Header, Exit) blocks for `break` and `continue`
    loop_context: Vec<(BasicBlockID, BasicBlockID)>,
}

impl MIRBuilder {

    pub fn new(is_script: bool) -> Self {
        Self {
            is_script,
            locals: Vec::new(),
            basic_blocks: Vec::new(),
            current_block: BasicBlockID(0),
            var_map: HashMap::new(),
            loop_context: Vec::new(),
        }
    }

    fn new_local(&mut self, ty: TypeExpr, debug_name: Option<String>) -> LocalID {
        let id = LocalID(self.locals.len());
        self.locals.push(LocalDecl { ty, debug_name });
        id
    }

    fn new_block(&mut self) -> BasicBlockID {
        let id = BasicBlockID(self.basic_blocks.len());
        self.basic_blocks.push(BasicBlock {
            statements: Vec::new(),
            terminator: Terminator::Unreachable,
        });
        id
    }

    fn push_statement(&mut self, stmt: Statement) {
        self.basic_blocks[self.current_block.0].statements.push(stmt);
    }

    fn terminate_block(&mut self, terminator: Terminator) {
        self.basic_blocks[self.current_block.0].terminator = terminator;
    }

    // --- ENTRY POINT ---
    pub fn build_function(mut self, vir_fn: VIRFunction) -> MIRFunction {
        // _0 is ALWAYS the return value in MIR
        self.new_local(vir_fn.return_type.clone(), Some("return_val".to_string()));

        // _1 to _N are the function parameters
        for (var_id, ty) in &vir_fn.params {
            let local_id = self.new_local(ty.clone(), Some(var_id.1.clone()));
            self.var_map.insert(var_id.0, local_id);
        }

        let entry_block = self.new_block();
        self.current_block = entry_block;

        self.lower_block(&vir_fn.body);

        // Implicit return if the block didn't hit a `return` statement
        if matches!(self.basic_blocks[self.current_block.0].terminator, Terminator::Unreachable) {
            self.terminate_block(Terminator::Return);
        }

        MIRFunction {
            name: vir_fn.name,
            arity: vir_fn.params.len(),
            locals: self.locals,
            basic_blocks: self.basic_blocks,
        }
    }

    // --- LOWERING LOGIC ---
    fn lower_block(&mut self, block: &VIRBlock) {
        for stmt in &block.stmts {
            self.lower_stmt(stmt);
        }
    }

    fn lower_stmt(&mut self, stmt: &VIRStmt) {
        match stmt {
            VIRStmt::VarDecl { var_id, ty, init, span } => {
                if self.is_script {
                    if let Some(expr) = init {
                        let rval = self.lower_expr(expr);
                        self.push_statement(Statement {
                            kind: StatementKind::DefineGlobal(var_id.1.clone(), ty.clone(), Rvalue::Use(rval)),
                            span: *span,
                        });
                    }
                } else {
                    let local_id = self.new_local(ty.clone(), Some(var_id.1.clone()));
                    self.var_map.insert(var_id.0, local_id);
                    if let Some(expr) = init {
                        let rval = self.lower_expr(expr);
                        self.push_statement(Statement {
                            kind: StatementKind::Assign(Place { local: local_id, projection: vec![] }, Rvalue::Use(rval)),
                            span: *span,
                        });
                    }
                }
            }

            VIRStmt::Expr(expr) => {
                self.lower_expr(expr);
            }
        }
    }

    /// Evaluates an expression, stores it in a temporary local, and returns the Operand
    fn lower_expr(&mut self, expr: &VIRExpr) -> Operand {
        match &expr.kind {
            // --- 1. Literals ---
            VIRExprKind::IntLiteral(val) => Operand::Const(Constant::Int(*val)),
            VIRExprKind::FloatLiteral(val) => Operand::Const(Constant::Float(*val)),
            VIRExprKind::StringLiteral(val) => Operand::Const(Constant::String(val.clone())),
            VIRExprKind::BoolLiteral(val) => Operand::Const(Constant::Bool(*val)),
            VIRExprKind::NoneLiteral => Operand::Const(Constant::None),

            // --- 2. Variables & Assignment ---
            VIRExprKind::VarRef(var_id) => {
                if let Some(&local) = self.var_map.get(&var_id.0) {
                    Operand::Copy(Place { local, projection: vec![] })
                } else {
                    Operand::Static(var_id.1.clone())
                }
            }

            VIRExprKind::Assign { target, value } => {
                let rval = self.lower_expr(value);

                if let VIRExprKind::VarRef(var_id) = &target.kind {
                    if let Some(local_id) = self.var_map.get(&var_id.0) {
                        let place = Place { local: *local_id, projection: vec![] };

                        self.push_statement(Statement {
                            kind: StatementKind::Assign(place.clone(), Rvalue::Use(rval)),
                            span: expr.span,
                        });

                        return Operand::Copy(place);
                    } else {
                        let temp = self.new_local(parser::ast::TypeExpr::Any, None);
                        let temp_place = Place { local: temp, projection: vec![] };

                        self.push_statement(Statement {
                            kind: StatementKind::Assign(temp_place.clone(), Rvalue::Use(rval)),
                            span: expr.span,
                        });

                        self.push_statement(Statement {
                            kind: StatementKind::AssignGlobal(var_id.1.clone(), Rvalue::Use(Operand::Copy(temp_place.clone()))),
                            span: expr.span,
                        });

                        return Operand::Copy(temp_place);
                    }
                }

                unimplemented!("[ICE] assignment to non-variables");
            }
            // --- 3. Math & Logic ---
            VIRExprKind::Binary { op, lhs, rhs } => {
                let left_op = self.lower_expr(lhs);
                let right_op = self.lower_expr(rhs);

                let temp_local = self.new_local(TypeExpr::Any, None);
                let target_place = Place { local: temp_local, projection: vec![] };

                self.push_statement(Statement {
                    kind: StatementKind::Assign(
                        target_place.clone(),
                        Rvalue::BinaryOp(*op, left_op, right_op)
                    ),
                    span: expr.span,
                });

                Operand::Copy(target_place)
            }

            VIRExprKind::Unary { op, operand } => {
                let inner_op = self.lower_expr(operand);
                
                let temp_local = self.new_local(TypeExpr::Any, None);
                let target_place = Place { local: temp_local, projection: vec![] };

                self.push_statement(Statement {
                    kind: StatementKind::Assign(
                        target_place.clone(),
                        Rvalue::UnaryOp(*op, inner_op)
                    ),
                    span: expr.span,
                });

                Operand::Copy(target_place)
            }

            // --- 4. Control Flow Expressions ---
            VIRExprKind::If { cond, then_block, else_block } => {
                let cond_op = self.lower_expr(cond);

                let then_bb = self.new_block();
                let else_bb = self.new_block();
                let merge_bb = self.new_block();

                self.terminate_block(Terminator::SwitchInt {
                    discriminant: cond_op,
                    true_target: then_bb,
                    false_target: if else_block.is_some() { else_bb } else { merge_bb },
                });

                self.current_block = then_bb;
                self.lower_block(then_block);
                if matches!(self.basic_blocks[self.current_block.0].terminator, Terminator::Unreachable) {
                    self.terminate_block(Terminator::Goto { target: merge_bb });
                }

                if let Some(els) = else_block {
                    self.current_block = else_bb;
                    self.lower_block(els);
                    if matches!(self.basic_blocks[self.current_block.0].terminator, Terminator::Unreachable) {
                        self.terminate_block(Terminator::Goto { target: merge_bb });
                    }
                }

                self.current_block = merge_bb;
                Operand::Const(Constant::None)
            }

            VIRExprKind::Loop(block) => {
                let loop_header = self.new_block();
                let loop_exit = self.new_block();

                self.terminate_block(Terminator::Goto { target: loop_header });
                self.loop_context.push((loop_header, loop_exit));
                
                self.current_block = loop_header;
                self.lower_block(block);

                if matches!(self.basic_blocks[self.current_block.0].terminator, Terminator::Unreachable) {
                    self.terminate_block(Terminator::Goto { target: loop_header });
                }

                self.loop_context.pop();
                self.current_block = loop_exit;
                Operand::Const(Constant::None)
            }

            VIRExprKind::Break => {
                let (_, loop_exit) = *self.loop_context.last().expect("break outside of loop");
                self.terminate_block(Terminator::Goto { target: loop_exit });
                self.current_block = self.new_block(); 
                Operand::Const(Constant::None)
            }

            VIRExprKind::Continue => {
                let (loop_header, _) = *self.loop_context.last().expect("continue outside of loop");
                self.terminate_block(Terminator::Goto { target: loop_header });
                self.current_block = self.new_block(); 
                Operand::Const(Constant::None)
            }

            VIRExprKind::Return(opt_expr) => {
                if let Some(expr) = opt_expr {
                    let rval_op = self.lower_expr(expr);
                    // Force assignment into Local _0 (which is always the Return slot)
                    let ret_place = Place { local: LocalID(0), projection: vec![] };
                    self.push_statement(Statement {
                        kind: StatementKind::Assign(ret_place, Rvalue::Use(rval_op)),
                        span: expr.span,
                    });
                }
                
                self.terminate_block(Terminator::Return);
                self.current_block = self.new_block(); // Dead code block for anything below the return
                Operand::Const(Constant::None)
            }

            VIRExprKind::Block(block) => {
                self.lower_block(block);
                Operand::Const(Constant::None)
            }

            // --- 5. Calls & Dynamic Dispatch ---
            VIRExprKind::Call { callee, args } => {
                let callee_op = if let Some(&local) = self.var_map.get(&callee.0) {
                    Operand::Copy(Place { local, projection: vec![] })
                } else {
                    Operand::Static(callee.1.clone())
                };

                let mut arg_ops = Vec::new();
                for arg in args {
                    arg_ops.push(self.lower_expr(arg));
                }

                let result_local = self.new_local(TypeExpr::Any, None);
                let destination = Place { local: result_local, projection: vec![] };
                let next_block = self.new_block();

                // Terminate the current block with a Call jump
                self.terminate_block(Terminator::Call {
                    callee: callee_op,
                    args: arg_ops,
                    destination: destination.clone(),
                    target: next_block,
                });

                self.current_block = next_block;
                Operand::Copy(destination)
            }

            VIRExprKind::MethodCall { object, method_name, args } => {
                let obj_op = self.lower_expr(object);

                let mut arg_ops = Vec::new();
                for arg in args {
                    arg_ops.push(self.lower_expr(arg));
                }

                let result_local = self.new_local(TypeExpr::Any, None);
                let destination = Place { local: result_local, projection: vec![] };
                let next_block = self.new_block();

                self.terminate_block(Terminator::MethodCall {
                    object: obj_op,
                    method_name: method_name.clone(),
                    args: arg_ops,
                    destination: destination.clone(),
                    target: next_block,
                });

                self.current_block = next_block;
                Operand::Copy(destination)
            }

            // --- 6. Data Structures ---
            VIRExprKind::ListInit { elements } => {
                let mut elem_ops = Vec::new();
                for el in elements {
                    elem_ops.push(self.lower_expr(el));
                }
                
                let temp_local = self.new_local(TypeExpr::Any, None);
                let target_place = Place { local: temp_local, projection: vec![] };

                self.push_statement(Statement {
                    kind: StatementKind::Assign(
                        target_place.clone(),
                        Rvalue::ListInit(elem_ops)
                    ),
                    span: expr.span,
                });

                Operand::Copy(target_place)
            }

            VIRExprKind::DictInit { keys, values } => {
                let mut key_ops = Vec::new();
                let mut val_ops = Vec::new();

                for k in keys { key_ops.push(self.lower_expr(k)); }
                for v in values { val_ops.push(self.lower_expr(v)); }

                let temp_local = self.new_local(TypeExpr::Any, None);
                let target_place = Place { local: temp_local, projection: vec![] };

                self.push_statement(Statement {
                    kind: StatementKind::Assign(
                        target_place.clone(),
                        Rvalue::DictInit(key_ops, val_ops)
                    ),
                    span: expr.span,
                });

                Operand::Copy(target_place)
            }

            VIRExprKind::SubscriptAccess { base, index } => {
                let base_op = self.lower_expr(base);
                let index_op = self.lower_expr(index);

                // 1. Ensure the base array/object is in a local register
                let base_local = match base_op.clone() {
                    Operand::Copy(p) => p.local,
                    Operand::Static(_) => {
                        let temp = self.new_local(TypeExpr::Any, None);
                        self.push_statement(Statement {
                            kind: StatementKind::Assign(Place { local: temp, projection: vec![] }, Rvalue::Use(base_op)),
                            span: expr.span,
                        });
                        temp
                    }
                    Operand::Const(_) => panic!("cannot subscript a constant!"),
                };

                // 2. Ensure the index is in a local register
                let index_local = match index_op.clone() {
                    Operand::Copy(p) => p.local,
                    Operand::Const(_) | Operand::Static(_) => {
                        let temp = self.new_local(TypeExpr::Any, None);
                        self.push_statement(Statement {
                            kind: StatementKind::Assign(Place { local: temp, projection: vec![] }, Rvalue::Use(index_op)),
                            span: expr.span,
                        });
                        temp
                    }
                };

                // 3. Extract the value
                let temp_local = self.new_local(TypeExpr::Any, None);
                let source_place = Place {
                    local: base_local,
                    projection: vec![ProjectionElem::Index(index_local)],
                };
                
                self.push_statement(Statement {
                    kind: StatementKind::Assign(Place { local: temp_local, projection: vec![] }, Rvalue::Use(Operand::Copy(source_place))),
                    span: expr.span,
                });

                Operand::Copy(Place { local: temp_local, projection: vec![] })
            }

            VIRExprKind::Import(module) => {
                let temp_local = self.new_local(TypeExpr::Any, None);
                let target_place = Place {
                    local: temp_local,
                    projection: vec![]
                };

                self.push_statement(Statement {
                    kind: StatementKind::Assign(
                        target_place.clone(), 
                        Rvalue::Import(module.clone())
                    ),
                    span: expr.span
                });

                Operand::Copy(target_place)
            }

            VIRExprKind::FunctionDef(vir_func) => {
                let mir_func = MIRBuilder::new(false).build_function(*vir_func.clone());

                let temp_local = self.new_local(TypeExpr::Any, None);
                let target_place = Place { local: temp_local, projection: vec![] };

                self.push_statement(Statement {
                    kind: StatementKind::Assign(
                        target_place.clone(),
                        Rvalue::FunctionDef(Box::new(mir_func))
                    ),
                    span: expr.span,
                });

                Operand::Copy(target_place)
            }

            VIRExprKind::FormatString(parts) => {
                let mut ops = Vec::new();
                for part in parts {
                    ops.push(self.lower_expr(part));
                }
                
                let temp_local = self.new_local(TypeExpr::Atomic(TokenType::STR), None);
                let target_place = Place { local: temp_local, projection: vec![] };
                
                self.push_statement(Statement {
                    kind: StatementKind::Assign(target_place.clone(), Rvalue::FormatString(ops)),
                    span: expr.span,
                });
                
                Operand::Copy(target_place)
            }

            _ => unimplemented!("[ICE] MIR lowering for this VIR expression is not yet implemented: {:?}", expr.kind),
        }
    }
}
