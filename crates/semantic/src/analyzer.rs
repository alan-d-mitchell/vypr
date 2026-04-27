use crate::scope::{Scope, SymbolType};
use parser::ast::{Expr, ExprKind, Stmt, StmtKind, TypeExpr};
use lexer::token::TokenType;
use error::error::{Span, VyprError};

pub struct Analyzer {
    scopes: Vec<Scope>, // Stack of scopes. Index 0 is global.
    current_return_type: Option<TypeExpr>,
    loop_depth: usize,
}

impl Analyzer {

    pub fn new() -> Self {
        let mut global_scope = Scope::new();

        global_scope.define("int".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Any], 
            return_type: TypeExpr::Atomic(TokenType::INT) 
        }, true);
        
        // float(any) -> float
        global_scope.define("float".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Any], 
            return_type: TypeExpr::Atomic(TokenType::FLOAT) 
        }, true);

        // str(any) -> str
        global_scope.define("str".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Any], 
            return_type: TypeExpr::Atomic(TokenType::STR) 
        }, true);
        
        // print(any) -> None (Actually variadic, but treating as 1 arg of Any for simple checking)
        global_scope.define("print".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Any], 
            return_type: TypeExpr::Any 
        }, true);

        global_scope.define("len".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Any], 
            return_type: TypeExpr::Atomic(TokenType::INT) 
        }, true);

        // range(any) -> list[int]
        global_scope.define("range".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Any], 
            return_type: TypeExpr::Atomic(TokenType::RANGE) 
        }, true);

        global_scope.define("list".to_string(), SymbolType::Function { 
            params: vec![TypeExpr::Any], 
            return_type: TypeExpr::Atomic(TokenType::LIST) 
        }, true);

        Self {
            scopes: vec![global_scope], // Start with global scope
            current_return_type: None,
            loop_depth: 0
        }
    }

    fn error(&self, code: &'static str, message: impl Into<String>, span: Span) -> VyprError {
        VyprError::new(code, message, span)
    }

    pub fn analyze(&mut self, ast: &[Stmt]) -> Result<(), VyprError> {
        for stmt in ast {
            self.visit_stmt(stmt)?;
        }

        Ok(())
    }
    
    fn visit_stmt(&mut self, stmt: &Stmt) -> Result<(), VyprError> {
        let span = stmt.span;

        match &stmt.kind {
            StmtKind::VarDecl { name, value, annotation } => {
                let value_type = if let Some(expr) = value {
                    self.infer_type(expr)?
                } else {
                    TypeExpr::Any
                };

                if let Some(ann) = annotation {
                    if !self.types_match(ann, &value_type) {
                        return Err(self.error("S001", format!(
                            "type error: variable '{}' declared as {} but assigned value of type {}",
                            name, ann, value_type
                        ), span));
                    }

                    self.define(name.clone(), SymbolType::Locked(ann.clone()), true);
                } else {
                    let mut is_locked = None;

                    if let Some(symbol) = self.resolve(name) {
                        if let SymbolType::Locked(locked_type) = &symbol.kind {
                            if !self.types_match(locked_type, &value_type) {
                                return Err(self.error("S013", format!(
                                    "type error: variable '{}' is locked to type {} but assigned value of type {}",
                                    name, locked_type, value_type
                                ), span));
                            }

                            is_locked = Some(locked_type.clone());
                        }
                    }

                    if let Some(t) = is_locked {
                        self.define(name.clone(), SymbolType::Locked(t), true);
                    } else {
                        self.define(name.clone(), SymbolType::Dynamic, true);
                    }
                }
            }

            StmtKind::FuncDecl { name, params, body, return_type } => {
                let param_types: Vec<TypeExpr> = params.iter()
                    .map(|p| p.annotation.clone().unwrap_or(TypeExpr::Any))
                    .collect();

                let ret_type = return_type.clone().unwrap_or(TypeExpr::Any);

                self.define(name.clone(), SymbolType::Function {
                    params: param_types,
                    return_type: ret_type.clone(),
                }, true);

                self.enter_scope();

                for param in params {
                    let param_type = param.annotation.clone().unwrap_or(TypeExpr::Any);

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

            StmtKind::If { condition, then, else_b } => {
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

            StmtKind::For { var, iterator, body } => {
                let iter_type = self.infer_type(iterator)?;

                let item_type = match iter_type {
                    TypeExpr::List(inner) => *inner,
                    TypeExpr::Atomic(TokenType::STR) => TypeExpr::Atomic(TokenType::STR),
                    TypeExpr::Atomic(TokenType::RANGE) => TypeExpr::Atomic(TokenType::INT),
                    TypeExpr::Atomic(TokenType::LIST) => TypeExpr::Any,
                    TypeExpr::Any => TypeExpr::Any,

                    _ => return Err(self.error("S002", format!("type error: type {} is not iterable", iter_type), span))
                };

                self.loop_depth += 1;

                self.enter_scope();
                
                self.define(var.clone(), SymbolType::Locked(item_type), true);

                for s in body {
                    self.visit_stmt(s)?;
                }
                self.exit_scope();

                self.loop_depth -= 1;
            }

            StmtKind::While { condition, body } => {
                self.visit_expr(condition)?;

                self.loop_depth += 1;

                self.enter_scope();
                for s in body {
                    self.visit_stmt(s)?;
                }
                self.exit_scope();

                self.loop_depth -= 1;
            }

            StmtKind::ExprStmt(expr) => {
                self.visit_expr(expr)?;
            }

            StmtKind::Return { value, .. } => {
                let actual_type = if let Some(expr) = value {
                    self.infer_type(expr)?
                } else {
                    TypeExpr::Atomic(TokenType::NONE)
                };

                if let Some(expected) = &self.current_return_type {
                    if !self.types_match(expected, &actual_type) {
                        return Err(self.error("S003", format!(
                            "type error: function expected return type {} but got {}",
                            expected, actual_type
                        ), span));
                    }
                }
            }

            StmtKind::Pass => {
                return Ok(())
            }

            StmtKind::Break => {
                if self.loop_depth == 0 {
                    return Err(self.error("S014", "'break' outside loop", span));
                }
            }

            StmtKind::Continue => {
                if self.loop_depth == 0 {
                    return Err(self.error("S015", "'continue' outside loop", span));
                }
            }
        }

        Ok(())
    }

    fn visit_expr(&self, expr: &Expr) -> Result<(), VyprError> {
        self.infer_type(expr)?;
        Ok(())
    }

    fn infer_type(&self, expr: &Expr) -> Result<TypeExpr, VyprError> {
        let span = expr.span;

        match &expr.kind {
            ExprKind::Literal(TokenType::INT_LITERAL(_)) => Ok(TypeExpr::Atomic(TokenType::INT)),
            ExprKind::Literal(TokenType::FLOAT_LITERAL(_)) => Ok(TypeExpr::Atomic(TokenType::FLOAT)),
            ExprKind::Literal(TokenType::STR_LITERAL(_)) => Ok(TypeExpr::Atomic(TokenType::STR)),
            ExprKind::Literal(TokenType::TRUE) | ExprKind::Literal(TokenType::FALSE) => Ok(TypeExpr::Atomic(TokenType::BOOL)),

            ExprKind::Variable(name) => {
                if let Some(sym) = self.resolve(name) {
                    match &sym.kind {
                        SymbolType::Locked(t) => Ok(t.clone()),
                        SymbolType::Dynamic => Ok(TypeExpr::Any), // Unknown type
                        SymbolType::Function { .. } => Ok(TypeExpr::Any),
                    }
                } else {
                    Err(self.error("S004", format!("undefined variable '{}'", name), span))
                }
            },

            ExprKind::Call { callee, args } => {
                let func_name = match &callee.kind {
                    ExprKind::Variable(name) => name,
                    _ => return Err(self.error("S005", "can only call named functions", span))
                };

                // Look up the function symbol
                if let Some(sym) = self.resolve(func_name) {
                    if let SymbolType::Function { params, return_type } = &sym.kind {
                        let is_flexible = params.len() == 1 && params[0] == TypeExpr::Any;

                        if !is_flexible {
                            if args.len() != params.len() {
                                return Err(self.error("S006", format!(
                                    "function '{}' expects {} arguments, got {}", 
                                    func_name, params.len(), args.len()
                                ), span));
                            }

                            for (i, arg) in args.iter().enumerate() {
                                let arg_type = self.infer_type(arg)?;
                                let param_type = &params[i];

                                if !self.types_match(param_type, &arg_type) {
                                    return Err(self.error("S007", format!(
                                        "type error in call to '{}': argument {} expected {}, got {}",
                                        func_name, i + 1, param_type, arg_type
                                    ), span));
                                }
                            }
                        }

                        Ok(return_type.clone())
                    } else {
                        Err(self.error("S008", format!("'{}' is not a function", func_name), span))
                    }
                } else {
                    Err(self.error("S004", format!("undefined function '{}'", func_name), span))
                }
            },

            ExprKind::MethodCall { callee, args, method } => {
                let callee_type = self.infer_type(callee)?;

                match (callee_type, method.as_str()) {
                    (TypeExpr::List(inner_type), "append") => { // check if they are calling .append() on a List
                        if args.len() != 1 {
                            return Err(
                                self.error("S006", "append() takes exactly 1 argument", span)
                                    .with_help("remove the extra argument")
                            );
                        }

                        let arg_type = self.infer_type(&args[0])?;
                        
                        if !self.types_match(&inner_type, &arg_type) {
                            return Err(self.error("S007", format!(
                                "type error: cannot append {} to list[{}]", 
                                arg_type, inner_type
                            ), span));
                        }

                        Ok(TypeExpr::Any)
                    }

                    (TypeExpr::Any, _) => {
                        Ok(TypeExpr::Any)
                    }

                    (t, m) => Err(self.error("S009", format!("type {} has no method '{}'", t, m), span))
                }
            },

            ExprKind::Binary { left, operator, right } => {
                let left_type = self.infer_type(left)?;
                let right_type = self.infer_type(right)?;

                if left_type == TypeExpr::Any || right_type == TypeExpr::Any {
                    return match operator {
                        TokenType::GREATER_THAN | TokenType::LESS_THAN | TokenType::DOUBLE_EQUAL |
                        TokenType::LESS_THAN_EQUAL | TokenType::GREATER_THAN_EQUAL | TokenType::AND |
                        TokenType::OR => Ok(TypeExpr::Atomic(TokenType::BOOL)),
                        _ => Ok(TypeExpr::Any),
                    }
                }

                match operator {
                    TokenType::PLUS => {
                        match (&left_type, &right_type) {
                            (TypeExpr::Atomic(TokenType::INT), TypeExpr::Atomic(TokenType::INT)) => Ok(TypeExpr::Atomic(TokenType::INT)),
                            (TypeExpr::Atomic(TokenType::FLOAT), TypeExpr::Atomic(TokenType::FLOAT)) => Ok(TypeExpr::Atomic(TokenType::FLOAT)),

                            (TypeExpr::Atomic(TokenType::INT), TypeExpr::Atomic(TokenType::FLOAT)) |
                            (TypeExpr::Atomic(TokenType::FLOAT), TypeExpr::Atomic(TokenType::INT)) => Ok(TypeExpr::Atomic(TokenType::FLOAT)),

                            (TypeExpr::Atomic(TokenType::STR), TypeExpr::Atomic(TokenType::STR)) => Ok(TypeExpr::Atomic(TokenType::STR)),
                            
                            _ => Err(self.error("S017", format!("unsupported operand types for +: {} and {}", left_type, right_type), span))
                        }
                    },

                    TokenType::MINUS | TokenType::STAR | TokenType::FSLASH | 
                    TokenType::DOUBLE_STAR | TokenType::MODULO | TokenType::DOUBLE_FSLASH => {
                        if matches!(operator, TokenType::FSLASH | TokenType::MODULO | TokenType::DOUBLE_FSLASH) {
                            if let ExprKind::Literal(TokenType::INT_LITERAL(0)) = right.kind {
                                return Err(self.error("S016", "division by zero", span));
                            }
                        }

                        // These operators ONLY support numeric types
                        match (&left_type, &right_type) {
                            (TypeExpr::Atomic(TokenType::INT), TypeExpr::Atomic(TokenType::INT)) => Ok(TypeExpr::Atomic(TokenType::INT)),
                            (TypeExpr::Atomic(TokenType::FLOAT), TypeExpr::Atomic(TokenType::FLOAT)) => Ok(TypeExpr::Atomic(TokenType::FLOAT)),

                            (TypeExpr::Atomic(TokenType::INT), TypeExpr::Atomic(TokenType::FLOAT)) |
                            (TypeExpr::Atomic(TokenType::FLOAT), TypeExpr::Atomic(TokenType::INT)) => Ok(TypeExpr::Atomic(TokenType::FLOAT)),
                            
                            _ => Err(self.error("S017", format!("unsupported operand types for math operator: {} and {}", left_type, right_type), span))
                        }
                    },

                    TokenType::GREATER_THAN | TokenType::LESS_THAN | TokenType::LESS_THAN_EQUAL | TokenType::GREATER_THAN_EQUAL => {
                        match (&left_type, &right_type) {
                            (TypeExpr::Atomic(TokenType::INT), TypeExpr::Atomic(TokenType::INT)) |
                            (TypeExpr::Atomic(TokenType::FLOAT), TypeExpr::Atomic(TokenType::FLOAT)) |

                            (TypeExpr::Atomic(TokenType::INT), TypeExpr::Atomic(TokenType::FLOAT)) |
                            (TypeExpr::Atomic(TokenType::FLOAT), TypeExpr::Atomic(TokenType::INT)) => Ok(TypeExpr::Atomic(TokenType::BOOL)),

                            _ => Err(self.error("S017", format!("unsupported operand types for comparison: {} and {}", left_type, right_type), span))
                        }
                    },

                    TokenType::DOUBLE_EQUAL | TokenType::AND | TokenType::OR => {
                        Ok(TypeExpr::Atomic(TokenType::BOOL))
                    },

                    _ => Ok(left_type)
                }
            }

            ExprKind::Unary { operator: _, right } => {
                self.infer_type(right)
            },

            ExprKind::Subscript { callee, index } => {
                let list_type = self.infer_type(callee)?;
                let index_type = self.infer_type(index)?;

                if !self.types_match(&TypeExpr::Atomic(TokenType::INT), &index_type) {
                    return Err(self.error("S010", format!("list indices must be integers, not {}", index_type), span));
                }

                match list_type {
                    TypeExpr::List(inner) => Ok(*inner),

                    TypeExpr::Atomic(TokenType::LIST) => {
                        Ok(TypeExpr::Any)
                    }

                    TypeExpr::Any => {
                        Ok(TypeExpr::Any)
                    }

                    _ => Err(self.error("S011", format!("type {} is not subscriptable", list_type), span))
                }
            }

            ExprKind::List(elements) => {
                if elements.is_empty() {
                    Ok(TypeExpr::Atomic(TokenType::LIST))
                } else {
                    let mut list_type = self.infer_type(&elements[0])?;

                    for element in elements.iter().skip(1) {
                        let current_type = self.infer_type(element)?;

                        if !self.types_match(&list_type, &current_type) {
                            list_type = TypeExpr::Union(Box::new(list_type), Box::new(current_type));
                        }
                    }

                    Ok(TypeExpr::List(Box::new(list_type)))
                }
            }

            ExprKind::Grouping(inner) => self.infer_type(inner),

            _ => Ok(TypeExpr::Any)
        }
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
            (TypeExpr::Any, _) => true, 
            (_, TypeExpr::Any) => true,

            (expected_type, TypeExpr::Union(left, right)) => {
                self.types_match(expected_type, left) && self.types_match(expected_type, right)
            }

            (TypeExpr::Union(left, right), actual_type) => {
                self.types_match(left, actual_type) || self.types_match(right, actual_type)
            }

            (TypeExpr::Atomic(t1), TypeExpr::Atomic(t2)) => t1 == t2,

            (TypeExpr::List(inner_expected), TypeExpr::List(inner_actual)) => {
                self.types_match(inner_expected, inner_actual)
            }

            (TypeExpr::Atomic(TokenType::LIST), TypeExpr::List(_)) => true,
            (TypeExpr::List(_), TypeExpr::Atomic(TokenType::LIST)) => true,

            _ => false,
        }
    }
}
