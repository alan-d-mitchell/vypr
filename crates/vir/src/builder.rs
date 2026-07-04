use core::fmt;
use std::collections::HashMap;

use crate::context::{VIRContext, VarID, SymbolInfo, SymbolKind};
use crate::vir::{VIRProgram, VIRFunction, VIRBlock, VIRStmt, VIRExpr, VIRExprKind, VIRBinOp, VIRUnaryOp};

use lexer::token::TokenType;
use parser::ast::{Expr, Stmt, StmtKind, ExprKind, TypeExpr};

pub struct VIRBuilder {
    pub context: VIRContext,
    scopes: Vec<HashMap<String, VarID>>, 
}

impl VIRBuilder {

    pub fn new() -> Self {
        Self {
            context: VIRContext::new(),
            scopes: vec![HashMap::new()], 
        }
    }

    // --- Scope Management ---
    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn define_var(&mut self, name: String, ty: TypeExpr) -> VarID {
        let var_id = self.context.insert_def(SymbolInfo {
            name: name.clone(),
            span: Default::default(),
            module_path: vec![],
            kind: SymbolKind::Variable { ty, is_mutable: true },
        });

        self.scopes.last_mut().unwrap().insert(name, var_id.clone());
        var_id
    }

    fn resolve_var(&self, name: &str) -> Option<VarID> {
        for scope in self.scopes.iter().rev() {
            if let Some(id) = scope.get(name) {
                return Some(id.clone());
            }
        }
        None
    }

    // --- The Build Process ---
    pub fn build(&mut self, ast: &Vec<Stmt>) -> VIRProgram {
        let mut functions = Vec::new();
        let mut script_stmts = Vec::new(); // Collect freestanding code here!

        for stmt in ast {
            match &stmt.kind {
                // If it's a function, compile it normally
                StmtKind::FuncDecl { name, params, body, return_type } => {
                    functions.push(self.lower_function(name, params, body, return_type));
                }

                _ => {
                    script_stmts.push(self.lower_stmt(stmt));
                }
            }
        }

        // Wrap the freestanding code in a dummy "<script>" function
        let script_func = VIRFunction {
            name: "<script>".to_string(),
            var_id: VarID(0, "<script>".to_string()),
            params: vec![],
            return_type: TypeExpr::Any,
            body: VIRBlock { stmts: script_stmts, span: Default::default() },
            is_native: false,
        };

        functions.insert(0, script_func);

        VIRProgram { 
            functions,
            classes: Vec::new(),
            globals: Vec::new(),
        }
    }

    fn lower_function(&mut self, name: &str, params: &Vec<parser::ast::Param>, body: &Vec<Stmt>, ret_annotation: &Option<TypeExpr>) -> VIRFunction {

        let actual_ret_type = ret_annotation.clone().unwrap_or(TypeExpr::Any);
        let func_var_id = self.define_var(name.to_string(), actual_ret_type.clone());

        self.enter_scope();

        let mut vir_params = Vec::new();
        for p in params {
            let param_type = p.annotation.clone().unwrap_or(TypeExpr::Any);
            let var_id = self.define_var(p.name.clone(), param_type.clone());
            vir_params.push((var_id, param_type));
        }

        let mut stmts = Vec::new();
        for stmt in body {
            stmts.push(self.lower_stmt(stmt));
        }

        self.exit_scope();

        VIRFunction {
            name: name.to_string(),
            var_id: func_var_id,
            params: vir_params,
            return_type: actual_ret_type,
            body: VIRBlock { stmts, span: Default::default() },
            is_native: false,
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt) -> VIRStmt {
        match &stmt.kind {
            StmtKind::VarDecl { name, value, annotation } => {
                let actual_type = annotation.clone().unwrap_or(TypeExpr::Any);
                let init = value.as_ref().map(|v| self.lower_expr(v));

                // If it already exists, it's an assignment. If not, it's a new declaration!
                if let Some(var_id) = self.resolve_var(name) {
                    VIRStmt::Expr(VIRExpr {
                        kind: VIRExprKind::Assign { 
                            target: Box::new(VIRExpr { kind: VIRExprKind::VarRef(var_id), ty: actual_type, span: stmt.span }), 
                            value: Box::new(init.unwrap()) 
                        },
                        ty: TypeExpr::Any,
                        span: stmt.span,
                    })
                } else {
                    let var_id = self.define_var(name.clone(), actual_type.clone());
                    VIRStmt::VarDecl { var_id, init, ty: actual_type, span: stmt.span }
                }
            }

            StmtKind::ExprStmt(expr) => VIRStmt::Expr(self.lower_expr(expr)),

            StmtKind::Return { value, keyword: _ } => {
                let ret_val = value.as_ref().map(|v| Box::new(self.lower_expr(v)));
                VIRStmt::Expr(VIRExpr {
                    kind: VIRExprKind::Return(ret_val),
                    ty: TypeExpr::Any,
                    span: stmt.span,
                })
            }

            StmtKind::If { condition, then, else_b } => {
                let cond = Box::new(self.lower_expr(condition));

                self.enter_scope();
                let then_stmts: Vec<VIRStmt> = then.iter().map(|s| self.lower_stmt(s)).collect();
                self.exit_scope();

                let else_block = else_b.as_ref().map(|els| {
                    self.enter_scope();
                    let else_stmts: Vec<VIRStmt> = els.iter().map(|s| self.lower_stmt(s)).collect();
                    self.exit_scope();
                    Box::new(VIRBlock { stmts: else_stmts, span: stmt.span })
                });

                VIRStmt::Expr(VIRExpr {
                    kind: VIRExprKind::If { cond, then_block: Box::new(VIRBlock { stmts: then_stmts, span: stmt.span }), else_block },
                    ty: TypeExpr::Any,
                    span: stmt.span,
                })
            }

            StmtKind::While { condition, body } => {
                let cond = self.lower_expr(condition);

                self.enter_scope();
                let mut loop_stmts = Vec::new();

                // DESUGAR: while cond -> loop { if not cond: break; }
                let not_cond = VIRExpr {
                    kind: VIRExprKind::Unary { op: VIRUnaryOp::Not, operand: Box::new(cond) },
                    ty: TypeExpr::Atomic(TokenType::BOOL),
                    span: stmt.span,
                };

                let break_expr = VIRExpr { kind: VIRExprKind::Break, ty: TypeExpr::Any, span: stmt.span };

                loop_stmts.push(VIRStmt::Expr(VIRExpr {
                    kind: VIRExprKind::If { 
                        cond: Box::new(not_cond), 
                        then_block: Box::new(VIRBlock { stmts: vec![VIRStmt::Expr(break_expr)], span: stmt.span }), 
                        else_block: None 
                    },
                    ty: TypeExpr::Any,
                    span: stmt.span,
                }));

                for s in body {
                    loop_stmts.push(self.lower_stmt(s));
                }
                self.exit_scope();

                VIRStmt::Expr(VIRExpr {
                    kind: VIRExprKind::Loop(Box::new(VIRBlock { stmts: loop_stmts, span: stmt.span })),
                    ty: TypeExpr::Any,
                    span: stmt.span,
                })
            }

            StmtKind::For { var, iterator, body } => {
                self.enter_scope();

                let iter_expr = self.lower_expr(iterator);
                let iter_id = self.define_var(format!("__iter_{}", self.context.definitions.len()), TypeExpr::Any);
                let decl_iter = VIRStmt::VarDecl { var_id: iter_id.clone(), ty: TypeExpr::Any, init: Some(iter_expr), span: stmt.span };

                let idx_id = self.define_var(format!("__idx_{}", self.context.definitions.len()), TypeExpr::Atomic(TokenType::INT));
                let decl_idx = VIRStmt::VarDecl {
                    var_id: idx_id.clone(),
                    ty: TypeExpr::Atomic(TokenType::INT),
                    init: Some(VIRExpr { kind: VIRExprKind::IntLiteral(0), ty: TypeExpr::Atomic(TokenType::INT), span: stmt.span }),
                    span: stmt.span
                };

                let mut loop_stmts = Vec::new();

                // len(iter)
                let len_call = VIRExpr {
                    kind: VIRExprKind::Call {
                        callee: self.resolve_var("len").unwrap_or(VarID(0, "len".to_string())),
                        args: vec![VIRExpr { kind: VIRExprKind::VarRef(iter_id.clone()), ty: TypeExpr::Any, span: stmt.span }],
                    },
                    ty: TypeExpr::Atomic(TokenType::INT),
                    span: stmt.span,
                };

                // if not (idx < len(iter)) break
                let loop_cond = VIRExpr {
                    kind: VIRExprKind::Binary {
                        op: VIRBinOp::Lt,
                        lhs: Box::new(VIRExpr { kind: VIRExprKind::VarRef(idx_id.clone()), ty: TypeExpr::Atomic(TokenType::INT), span: stmt.span }),
                        rhs: Box::new(len_call),
                    },
                    ty: TypeExpr::Atomic(TokenType::BOOL),
                    span: stmt.span,
                };

                let not_cond = VIRExpr { 
                    kind: VIRExprKind::Unary { 
                        op: VIRUnaryOp::Not, 
                        operand: Box::new(loop_cond) 
                    }, 
                    ty: TypeExpr::Atomic(TokenType::BOOL), 
                    span: stmt.span 
                };

                let break_expr = VIRExpr { kind: VIRExprKind::Break, ty: TypeExpr::Any, span: stmt.span };

                loop_stmts.push(VIRStmt::Expr(VIRExpr {
                    kind: VIRExprKind::If {
                        cond: Box::new(not_cond),
                        then_block: Box::new(VIRBlock { stmts: vec![VIRStmt::Expr(break_expr)], span: stmt.span }),
                        else_block: None,
                    },
                    ty: TypeExpr::Any,
                    span: stmt.span,
                }));

                // var = iter[idx]
                self.enter_scope(); // Inner loop scope
                let item_id = self.define_var(var.clone(), TypeExpr::Any);
                let get_idx = VIRExpr {
                    kind: VIRExprKind::SubscriptAccess {
                        base: Box::new(VIRExpr { kind: VIRExprKind::VarRef(iter_id.clone()), ty: TypeExpr::Any, span: stmt.span }),
                        index: Box::new(VIRExpr { kind: VIRExprKind::VarRef(idx_id.clone()), ty: TypeExpr::Atomic(TokenType::INT), span: stmt.span }),
                    },
                    ty: TypeExpr::Any,
                    span: stmt.span,
                };
                loop_stmts.push(VIRStmt::VarDecl { var_id: item_id, ty: TypeExpr::Any, init: Some(get_idx), span: stmt.span });

                // Loop Body
                for s in body {
                    loop_stmts.push(self.lower_stmt(s));
                }

                // idx = idx + 1
                let inc_expr = VIRExpr {
                    kind: VIRExprKind::Binary {
                        op: VIRBinOp::Add,
                        lhs: Box::new(VIRExpr { kind: VIRExprKind::VarRef(idx_id.clone()), ty: TypeExpr::Atomic(TokenType::INT), span: stmt.span }),
                        rhs: Box::new(VIRExpr { kind: VIRExprKind::IntLiteral(1), ty: TypeExpr::Atomic(TokenType::INT), span: stmt.span }),
                    },
                    ty: TypeExpr::Atomic(TokenType::INT),
                    span: stmt.span,
                };
                loop_stmts.push(VIRStmt::Expr(VIRExpr {
                    kind: VIRExprKind::Assign {
                        target: Box::new(VIRExpr { kind: VIRExprKind::VarRef(idx_id), ty: TypeExpr::Atomic(TokenType::INT), span: stmt.span }),
                        value: Box::new(inc_expr),
                    },
                    ty: TypeExpr::Any,
                    span: stmt.span,
                }));
                self.exit_scope(); // End inner loop

                let the_loop = VIRStmt::Expr(VIRExpr {
                    kind: VIRExprKind::Loop(Box::new(VIRBlock { stmts: loop_stmts, span: stmt.span })),
                    ty: TypeExpr::Any,
                    span: stmt.span,
                });

                self.exit_scope(); // End outer for-loop wrapper

                // Wrap initialization + loop in a Block
                VIRStmt::Expr(VIRExpr {
                    kind: VIRExprKind::Block(VIRBlock { stmts: vec![decl_iter, decl_idx, the_loop], span: stmt.span }),
                    ty: TypeExpr::Any,
                    span: stmt.span,
                })
            }

            StmtKind::Break => VIRStmt::Expr(VIRExpr { kind: VIRExprKind::Break, ty: TypeExpr::Any, span: stmt.span }),
            StmtKind::Continue => VIRStmt::Expr(VIRExpr { kind: VIRExprKind::Continue, ty: TypeExpr::Any, span: stmt.span }),
            StmtKind::Pass => VIRStmt::Expr(VIRExpr { kind: VIRExprKind::Block(VIRBlock { stmts: vec![], span: stmt.span }), ty: TypeExpr::Any, span: stmt.span }),

            StmtKind::Import { module } => {
                let var_id = self.define_var(module.clone(), TypeExpr::Any);
                let import_expr = VIRExpr {
                    kind: VIRExprKind::Import(module.clone()),
                    ty: TypeExpr::Any,
                    span: stmt.span
                };

                VIRStmt::VarDecl { 
                    var_id,
                    ty: TypeExpr::Any,
                    init: Some(import_expr),
                    span: stmt.span
                }
            }

            StmtKind::FuncDecl { name, params, body, return_type } => {
                let vir_func = self.lower_function(name, params, body, return_type);

                let var_id = self.define_var(name.clone(), TypeExpr::Any);
                let func_expr = VIRExpr {
                    kind: VIRExprKind::FunctionDef(Box::new(vir_func)),
                    ty: TypeExpr::Any,
                    span: stmt.span,
                };

                VIRStmt::VarDecl { 
                    var_id, 
                    ty: TypeExpr::Any, 
                    init: Some(func_expr), 
                    span: stmt.span 
                }
            }
        }
    }

    fn lower_expr(&mut self, expr: &Expr) -> VIRExpr {
        let kind = match &expr.kind {
            ExprKind::Variable(name) => {
                let var_id = self.resolve_var(name).expect("[ICE] analyzer should have caught undefined vars");
                VIRExprKind::VarRef(var_id)
            }

            ExprKind::Literal(token) => match token {
                TokenType::INT_LITERAL(v) => VIRExprKind::IntLiteral(*v),
                TokenType::FLOAT_LITERAL(v) => VIRExprKind::FloatLiteral(*v),
                TokenType::STR_LITERAL(v) => VIRExprKind::StringLiteral(v.clone()),
                TokenType::TRUE => VIRExprKind::BoolLiteral(true),
                TokenType::FALSE => VIRExprKind::BoolLiteral(false),
                TokenType::NONE => VIRExprKind::NoneLiteral,
                _ => unreachable!(),
            },

            ExprKind::Binary { left, operator, right } => {
                let op = match operator {
                    TokenType::PLUS => VIRBinOp::Add, TokenType::MINUS => VIRBinOp::Sub,
                    TokenType::STAR => VIRBinOp::Mul, TokenType::FSLASH => VIRBinOp::Div,
                    TokenType::MODULO => VIRBinOp::Mod, TokenType::DOUBLE_EQUAL => VIRBinOp::Eq,
                    TokenType::LESS_THAN => VIRBinOp::Lt,
                    TokenType::LESS_THAN_EQUAL => VIRBinOp::Le, TokenType::GREATER_THAN => VIRBinOp::Gt,
                    TokenType::GREATER_THAN_EQUAL => VIRBinOp::Ge, TokenType::AND => VIRBinOp::And,
                    TokenType::OR => VIRBinOp::Or,
                    TokenType::DOUBLE_FSLASH => VIRBinOp::FloorDiv,
                    _ => unreachable!("[ICE] invalid binary operator"),
                };
                VIRExprKind::Binary { op, lhs: Box::new(self.lower_expr(left)), rhs: Box::new(self.lower_expr(right)) }
            }

            ExprKind::Unary { operator, right } => {
                let op = match operator {
                    TokenType::MINUS => VIRUnaryOp::Neg,
                    TokenType::NOT => VIRUnaryOp::Not,
                    _ => unreachable!("Invalid unary operator"),
                };
                VIRExprKind::Unary { op, operand: Box::new(self.lower_expr(right)) }
            }

            ExprKind::Call { callee, args } => {
                if let ExprKind::Variable(name) = &callee.kind {
                    let var_id = self.resolve_var(name).expect("Function not found");
                    let lowered_args = args.iter().map(|a| self.lower_expr(a)).collect();
                    VIRExprKind::Call { callee: var_id, args: lowered_args }
                } else {
                    panic!("[ICE] complex dynamic dispatch calls not yet supported in VIR")
                }
            } 

            ExprKind::MethodCall { callee, method, args } => {
                let lowered_args = args.iter().map(|a| self.lower_expr(a)).collect();
                VIRExprKind::MethodCall { object: Box::new(self.lower_expr(callee)), method_name: method.clone(), args: lowered_args }
            }

            ExprKind::Subscript { callee, index } => {
                VIRExprKind::SubscriptAccess { base: Box::new(self.lower_expr(callee)), index: Box::new(self.lower_expr(index)) }
            }

            ExprKind::List(elements) => {
                let lowered_elements = elements.iter().map(|e| self.lower_expr(e)).collect();
                VIRExprKind::ListInit { elements: lowered_elements }
            }

            ExprKind::Dict(elements) => {
                let mut keys = Vec::new();
                let mut values = Vec::new();

                for (k, v) in elements {
                    keys.push(self.lower_expr(k));
                    values.push(self.lower_expr(v));
                }

                VIRExprKind::DictInit { keys, values }
            }
 
            ExprKind::Grouping(inner) => {
                return self.lower_expr(inner);
            }

            ExprKind::FString(parts) => {
                let lowered_parts = parts.iter().map(|p| self.lower_expr(p)).collect();
                VIRExprKind::FormatString(lowered_parts)
            } 

            _ => unimplemented!("[ICE] lowering for this AST expression is not yet implemented: {:?}", expr.kind),
        };

        let inferred_type = match &kind {
            VIRExprKind::IntLiteral(_) => TypeExpr::Atomic(TokenType::INT),
            VIRExprKind::FloatLiteral(_) => TypeExpr::Atomic(TokenType::FLOAT),
            VIRExprKind::StringLiteral(_) => TypeExpr::Atomic(TokenType::STR),
            VIRExprKind::BoolLiteral(_) => TypeExpr::Atomic(TokenType::BOOL),
            VIRExprKind::NoneLiteral => TypeExpr::Atomic(TokenType::NONE),

            VIRExprKind::VarRef(id) => {
                if let Some(info) = self.context.get_def(id) {
                    match &info.kind {
                        SymbolKind::Variable { ty, .. } => ty.clone(),
                        _ => TypeExpr::Any,
                    }
                } else {
                    TypeExpr::Any
                }
            },

            VIRExprKind::Binary { op, .. } => match op {
                VIRBinOp::Eq | VIRBinOp::Ne | VIRBinOp::Lt | VIRBinOp::Le |
                VIRBinOp::Gt | VIRBinOp::Ge | VIRBinOp::And | VIRBinOp::Or => TypeExpr::Atomic(TokenType::BOOL),
                _ => TypeExpr::Any, // Math ops can stay Any and rely on the VM
            },

            _ => TypeExpr::Any,
        };

        VIRExpr {
            kind,
            ty: inferred_type,
            span: expr.span,
        }
    }

    pub fn inject_globals(&mut self, globals: Vec<(String, TypeExpr)>) {
        for (name, ty) in globals {
            self.define_var(name, ty);
        }
    }
}
