use crate::scope::{Scope, SymbolType};
use parser::ast::{Stmt, Expr, TypeExpr};
use lexer::token::TokenType;

pub struct Analyzer {
    scopes: Vec<Scope>, // Stack of scopes. Index 0 is global.
    current_return_type: Option<TypeExpr>,
}

impl Analyzer {

    pub fn new() -> Self {
        let mut global_scope = Scope::new();

        global_scope.define("int".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Atomic(TokenType::NONE)], 
            return_type: TypeExpr::Atomic(TokenType::INT) 
        }, true);
        
        // float(any) -> float
        global_scope.define("float".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Atomic(TokenType::NONE)], 
            return_type: TypeExpr::Atomic(TokenType::FLOAT) 
        }, true);

        // str(any) -> str
        global_scope.define("str".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Atomic(TokenType::NONE)], 
            return_type: TypeExpr::Atomic(TokenType::STR) 
        }, true);
        
        // print(any) -> None (Actually variadic, but treating as 1 arg of Any for simple checking)
        global_scope.define("print".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Atomic(TokenType::NONE)], 
            return_type: TypeExpr::Atomic(TokenType::NONE) 
        }, true);

        global_scope.define("len".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Atomic(TokenType::NONE)], 
            return_type: TypeExpr::Atomic(TokenType::INT) 
        }, true);

        Self {
            scopes: vec![global_scope], // Start with global scope
            current_return_type: None,
        }
    }

    pub fn analyze(&mut self, ast: &[Stmt]) -> Result<(), String> {
        for stmt in ast {
            self.visit_stmt(stmt)?;
        }

        Ok(())
    }
    
    fn visit_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::VarDecl { name, value, annotation } => {
                let value_type = if let Some(expr) = value {
                    self.infer_type(expr)?
                } else {
                    TypeExpr::Atomic(TokenType::NONE)
                };

                if let Some(ann) = annotation {
                    if !self.types_match(ann, &value_type) {
                        return Err(format!(
                            "type error: variable '{}' declared as {:?} but assigned value of type {:?}",
                            name, ann, value_type
                        ));
                    }

                    self.define(name.clone(), SymbolType::Locked(ann.clone()), true);
                } else {
                    self.define(name.clone(), SymbolType::Dynamic, true);
                }
            }

            Stmt::FuncDecl { name, params, body, return_type } => {
                let param_types: Vec<TypeExpr> = params.iter()
                    .map(|p| p.annotation.clone().unwrap_or(TypeExpr::Atomic(TokenType::NONE)))
                    .collect();

                let ret_type = return_type.clone().unwrap_or(TypeExpr::Atomic(TokenType::NONE));

                self.define(name.clone(), SymbolType::Function {
                    params: param_types,
                    return_type: ret_type.clone(),
                }, true);

                self.enter_scope();

                for param in params {
                    let param_type = param.annotation.clone().unwrap_or(TypeExpr::Atomic(TokenType::NONE));

                    self.define(
                        param.name.clone(),
                        SymbolType::Locked(param_type),
                        true
                    );
                }

                let prev_return = self.current_return_type.clone();
                self.current_return_type = Some(ret_type);

                for s in body {
                    self.visit_stmt(s)?;
                }

                self.current_return_type = prev_return;
                self.exit_scope();
            }

            Stmt::If { condition, then, else_b } => {
                self.visit_expr(condition)?;

                self.enter_scope();
                for s in then {
                    self.visit_stmt(s)?;
                }
                self.exit_scope();

                if let Some(b) = else_b {
                    self.enter_scope();
                    for s in b {
                        self.visit_stmt(s)?;
                    }
                    self.exit_scope();
                }
            }

            Stmt::For { var, iterator, body } => {
                let iter_type = self.infer_type(iterator)?;

                let item_type = match iter_type {
                    TypeExpr::List(inner) => *inner,
                    TypeExpr::Atomic(TokenType::STR) => TypeExpr::Atomic(TokenType::STR),
                    TypeExpr::Atomic(TokenType::LIST) => TypeExpr::Atomic(TokenType::NONE),
                    TypeExpr::Atomic(TokenType::NONE) => TypeExpr::Atomic(TokenType::NONE),

                    _ => return Err(format!("type error: type {:?} is not iterable", iter_type))
                };

                self.enter_scope();
                
                self.define(var.clone(), SymbolType::Locked(item_type), true);

                for s in body {
                    self.visit_stmt(s)?;
                }
                self.exit_scope();
            }

            Stmt::While { condition, body } => {
                self.visit_expr(condition)?;

                self.enter_scope();
                for s in body {
                    self.visit_stmt(s)?;
                }
                self.exit_scope();
            }

            Stmt::ExprStmt(expr) => {
                self.visit_expr(expr)?;
            }

            Stmt::Return { value, .. } => {
                let actual_type = if let Some(expr) = value {
                    self.infer_type(expr)?
                } else {
                    TypeExpr::Atomic(TokenType::NONE)
                };

                if let Some(expected) = &self.current_return_type {
                    if !self.types_match(expected, &actual_type) {
                        return Err(format!(
                            "type error: function expected return type {:?} but got {:?}",
                            expected, actual_type
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    fn visit_expr(&self, expr: &Expr) -> Result<(), String> {
        self.infer_type(expr)?;
        Ok(())
    }

    fn infer_type(&self, expr: &Expr) -> Result<TypeExpr, String> {
        match expr {
            Expr::Literal(TokenType::INT_LITERAL(_)) => Ok(TypeExpr::Atomic(TokenType::INT)),
            Expr::Literal(TokenType::FLOAT_LITERAL(_)) => Ok(TypeExpr::Atomic(TokenType::FLOAT)),
            Expr::Literal(TokenType::STR_LITERAL(_)) => Ok(TypeExpr::Atomic(TokenType::STR)),
            Expr::Literal(TokenType::TRUE) | Expr::Literal(TokenType::FALSE) => Ok(TypeExpr::Atomic(TokenType::BOOL)),

            Expr::Variable(name) => {
                if let Some(sym) = self.resolve(name) {
                    match &sym.kind {
                        SymbolType::Locked(t) => Ok(t.clone()),
                        SymbolType::Dynamic => Ok(TypeExpr::Atomic(TokenType::NONE)), // Unknown type
                        SymbolType::Function { .. } => Ok(TypeExpr::Atomic(TokenType::NONE)),
                    }
                } else {
                    Err(format!("undefined variable '{}'", name))
                }
            },

            Expr::Call { callee, args } => {
                let func_name = match &**callee {
                    Expr::Variable(name) => name,
                    _ => return Err("Can only call named functions".to_string()),
                };

                // Look up the function symbol
                if let Some(sym) = self.resolve(func_name) {
                    if let SymbolType::Function { params, return_type } = &sym.kind {
                        let is_flexible = params.len() == 1 && params[0] == TypeExpr::Atomic(TokenType::NONE);

                        if !is_flexible {
                            if args.len() != params.len() {
                                return Err(format!(
                                    "function '{}' expects {} arguments, got {}", 
                                    func_name, params.len(), args.len()
                                ));
                            }

                            for (i, arg) in args.iter().enumerate() {
                                let arg_type = self.infer_type(arg)?;
                                let param_type = &params[i];

                                if !self.types_match(param_type, &arg_type) {
                                    return Err(format!(
                                        "type error in call to '{}': argument {} expected {:?}, got {:?}",
                                        func_name, i + 1, param_type, arg_type
                                    ));
                                }
                            }
                        }

                        Ok(return_type.clone())
                    } else {
                        Err(format!("'{}' is not a function", func_name))
                    }
                } else {
                    Err(format!("undefined function '{}'", func_name))
                }
            },

            Expr::Binary { left, operator, right } => {
                let left_type = self.infer_type(left)?;
                let _right_type = self.infer_type(right)?;

                match operator {
                    TokenType::PLUS | TokenType::MINUS | TokenType::STAR | 
                    TokenType::FSLASH | TokenType::DOUBLE_STAR | TokenType::MODULO |
                    TokenType::DOUBLE_FSLASH => {
                        Ok(left_type)
                     },

                    TokenType::GREATER_THAN | TokenType::LESS_THAN | TokenType::DOUBLE_EQUAL |
                    TokenType::LESS_THAN_EQUAL | TokenType::GREATER_THAN_EQUAL | TokenType::AND |
                    TokenType::OR => {
                         Ok(TypeExpr::Atomic(TokenType::BOOL))
                     },


                     _ => Ok(left_type)
                }
            }

            Expr::Unary { operator: _, right } => {
                self.infer_type(right)
            },

            Expr::Subscript { callee, index } => {
                let list_type = self.infer_type(callee)?;
                let index_type = self.infer_type(index)?;

                if !self.types_match(&TypeExpr::Atomic(TokenType::INT), &index_type) {
                    return Err(format!("list indices must be integers, not {:?}", index_type));
                }

                match list_type {
                    TypeExpr::List(inner) => Ok(*inner),

                    TypeExpr::Atomic(TokenType::LIST) => {
                        Ok(TypeExpr::Atomic(TokenType::NONE))
                    }

                    TypeExpr::Atomic(TokenType::NONE) => {
                        Ok(TypeExpr::Atomic(TokenType::NONE))
                    }

                    _ => Err(format!("type {:?} is not subscriptable", list_type))
                }
            }

            Expr::Grouping(inner) => self.infer_type(inner),

            _ => Ok(TypeExpr::Atomic(TokenType::NONE))
        }
    }

    fn current_scope(&mut self) -> &mut Scope {
        self.scopes.last_mut().expect("scope stack is empty")
    }

    fn enter_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: String, kind: SymbolType, initialized: bool) {
        self.scopes.last_mut().unwrap().define(name, kind, initialized);
    }

    fn resolve(&self, name: &str) -> Option<&crate::scope::Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.get(name) {
                return Some(sym);
            }
        }

        None
    }

    fn types_match(&self, expected: &TypeExpr, actual: &TypeExpr) -> bool {
        match (expected, actual) {
            (TypeExpr::Atomic(t1), TypeExpr::Atomic(t2)) => t1 == t2,
            (TypeExpr::Atomic(TokenType::NONE), _) => true, // 'Any' matches anything
            (_, TypeExpr::Atomic(TokenType::NONE)) => true, 

            _ => false,
        }
    }}
