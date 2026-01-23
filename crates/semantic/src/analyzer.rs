use crate::scope::{Scope, SymbolType};
use parser::ast::{Stmt, Expr, TypeExpr};
use lexer::token::TokenType;

pub struct Analyzer {
    scopes: Vec<Scope>, // Stack of scopes. Index 0 is global.
}

impl Analyzer {

    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::new()], // Start with global scope
        }
    }

    pub fn analyze(&mut self, ast: &[Stmt]) -> Result<(), String> {
        for stmt in ast {
            self.visit_stmt(stmt)?;
        }

        Ok(())
    }

    fn current_scope(&mut self) -> &mut Scope {
        self.scopes.last_mut().expect("Scope stack is empty")
    }

    fn enter_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }
    
    fn visit_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::VarDecl { name, value, annotation } => {
                self.handle_var_decl(name, value, annotation)?;
            }

            Stmt::FuncDecl { name, params, body, .. } => {
                self.current_scope().define(name.clone(), SymbolType::Dynamic, true);

                self.enter_scope();

                for param in params {
                    let kind = if let Some(ann) = &param.annotation {
                        SymbolType::Locked(ann.clone())
                    } else {
                        SymbolType::Dynamic
                    };

                    self.current_scope().define(param.name.clone(), kind, true);
                }

                for s in body {
                    self.visit_stmt(s)?;
                }

                self.exit_scope();
            }

            Stmt::If { condition, then, else_b } => {
                // 1. Check condition
                self.visit_expr(condition)?;

                // 2. Check 'then' block
                for s in then {
                    self.visit_stmt(s)?;
                }

                // 3. Check 'else' block
                if let Some(branch) = else_b {
                    for s in branch {
                        self.visit_stmt(s)?;
                    }
                }
            }

            Stmt::While { condition, body } => {
                self.visit_expr(condition)?;
                for s in body {
                    self.visit_stmt(s)?;
                }
            }

            Stmt::ExprStmt(expr) => {
                self.visit_expr(expr)?;
            }

            Stmt::Return { value, .. } => {
                if let Some(expr) = value {
                    self.visit_expr(expr)?
                }
            }
        }

        Ok(())
    }

    fn handle_var_decl(&mut self, name: &String, value: &Option<Expr>, annotation: &Option<TypeExpr>)
        -> Result<(), String>
    {
        // check if the variable exists in the current scope
        // catches already defined errors in the same block
        let existing = self.current_scope().get(name).cloned();

        match (annotation, existing) {
            // CASE A: variable declared with type annotation
            // e.g. "x: int = 10" or "x: int"
            (Some(ann), Some(sym)) => {
                if sym.initialized {
                    // if it exists and was initialized
                    return Err(format!("variable '{}' is already defined and initialized in this scope",
                        name)
                    );
                } else {
                    // exists but not initialized
                    let sym_kind = SymbolType::Locked(ann.clone());
                    let is_init = value.is_some();

                    self.current_scope().define(name.clone(), sym_kind, is_init);
                }
            }

            (Some(ann), None) => {
                // doesnt exist. create it
                // e.g. first time seeing "x: int"
                let sym_kind = SymbolType::Locked(ann.clone());
                let is_init = value.is_some();

                self.current_scope().define(name.clone(), sym_kind, is_init);
            }

            // CASE B: variable assignment without new annotation
            // e.g. "x = 10"
            (None, Some(sym)) => {
                // variable exists, just assigning it
                // NOT a redefinition
                
                // check types optimistically
                if let Some(expr) = value {
                    let inferred = self.infer_type(expr)?;

                    if let SymbolType::Locked(expected) = &sym.kind {
                        if !self.types_match(expected, &inferred) {
                            return Err(format!("type mismatch: variable '{}' expects {:?}, but got {:?}",
                                name, expected, inferred)
                            );
                        }
                    }
                }

                self.current_scope().mark_initialized(name);
            }

            (None, None) => {
                // new variable without annotation, defaults to Dynamic
                // e.g. "y = 10" 
                let sym_kind = SymbolType::Dynamic;
                self.current_scope().define(name.clone(), sym_kind, true);
            }
        }

        Ok(())
    }

    fn visit_expr(&self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Call { callee, args } => {
                self.visit_expr(callee)?;

                for arg in args {
                    self.visit_expr(arg)?;
                }
            }

            _ => {}
        }

        Ok(())
    }

    fn infer_type(&self, expr: &Expr) -> Result<TypeExpr, String> {
        match expr {
            Expr::Literal(TokenType::INT_LITERAL(_)) => Ok(TypeExpr::Atomic(TokenType::INT)),
            Expr::Literal(TokenType::FLOAT_LITERAL(_)) => Ok(TypeExpr::Atomic(TokenType::FLOAT)),
            Expr::Literal(TokenType::STR_LITERAL(_)) => Ok(TypeExpr::Atomic(TokenType::STR)),
            Expr::Literal(TokenType::TRUE) | Expr::Literal(TokenType::FALSE) => Ok(TypeExpr::Atomic(TokenType::BOOL)),

            Expr::Variable(name) => {
                for scope in self.scopes.iter().rev() {
                    if let Some(sym) = scope.get(name) {
                        return match &sym.kind {
                            SymbolType::Locked(t) => Ok(t.clone()),
                            SymbolType::Dynamic => Ok(TypeExpr::Atomic(TokenType::NONE)), // Cannot guarantee type of dynamic var statically
                        };
                    }
                }

                // skipping undefined variables
                // deferred to VM to perform the check
                Ok(TypeExpr::Atomic(TokenType::NONE))
            }
            
            // placeholder for now as lists require more complex logic
            _ => Ok(TypeExpr::Atomic(TokenType::NONE))
        }
    }

    fn types_match(&self, expected: &TypeExpr, got: &TypeExpr) -> bool {
        if matches!(got, TypeExpr::Atomic(TokenType::NONE)) {
            return true;
        }

        expected == got
    }
}
