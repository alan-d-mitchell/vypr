use error::error::{Span, VyprError};
use lexer::token::{Token, TokenType};
use lexer::lexer::Lexer;
use crate::ast::{Param, TypeExpr, StmtKind, ExprKind};

use super::ast::{Stmt, Expr};

pub struct Parser<'p> {
    tokens: Vec<Token<'p>>,
    current: usize,
    pub errors: Vec<VyprError>,
}

impl<'p> Parser<'p> {

    pub fn new(tokens: Vec<Token<'p>>) -> Self {
        Self {
            tokens,
            current: 0,
            errors: Vec::new(),
        }
    }

    fn make_error(&self, code: &'static str, message: impl Into<String>) -> VyprError {
        let token = self.peek();
        let span = if token.kind == TokenType::EOF && self.current > 0 {
            self.previous().span
        } else {
            token.span
        };

        VyprError::new(code, message, span)
    }

    pub fn parse(&mut self) -> Vec<Stmt<'p>> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            if self.match_token(TokenType::NEWLINE) {
                continue;
            }

            match self.statement() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        statements
    }

    fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            if self.previous().kind == TokenType::NEWLINE || self.previous().kind == TokenType::SEMICOLON {
                return;
            }

            match self.peek().kind {
                TokenType::DEF | TokenType::IF | TokenType::FOR | 
                TokenType::WHILE | TokenType::PASS | TokenType::RETURN => return,
                _ => {}
            }

            self.advance();
        }
    }

    fn statement(&mut self) -> Result<Stmt<'p>, VyprError> {
        if self.match_token(TokenType::DEF) {
            return self.function_declaration();
        }

        if self.check(TokenType::IF) {
            return self.if_statement();
        }

        if self.check(TokenType::FOR) {
            return self.for_statement();
        }

        if self.check(TokenType::WHILE) {
            return self.while_statement();
        }

        if self.match_token(TokenType::RETURN) {
            return self.return_statement();
        }

        if self.match_token(TokenType::PASS) {
            return self.pass_statement();
        }

        if self.match_token(TokenType::BREAK) {
            let span = self.previous().span;
            self.consume_statement_end()?;

            return Ok(Stmt {
                kind: StmtKind::Break,
                span
            });
        }

        if self.match_token(TokenType::CONTINUE) {
            let span = self.previous().span;
            self.consume_statement_end()?;

            return Ok(Stmt {
                kind: StmtKind::Continue,
                span
            });
        }

        if self.match_token(TokenType::IMPORT) {
            return self.import_statement();
        }

        if let TokenType::IDENTIFIER(name) = self.peek().kind {
            if self.check_identifier() {
                let span = self.peek().span;
                self.advance();

                return self.var_declaration(name, span);
            }
        }

        self.expression_statement()
    }

    fn import_statement(&mut self) -> Result<Stmt<'p>, VyprError> {
        let span = self.previous().span;
        let token = self.consume(TokenType::IDENTIFIER("".to_string()), "expected module name")?;

        let module_name = match token.kind {
            TokenType::IDENTIFIER(name) => name,
            _ => return Err(self.make_error("P020", "expected module name")),
        };

        self.match_token(TokenType::SEMICOLON);

        Ok(Stmt {
            kind: StmtKind::Import {
                module: module_name,
            },
            span
        })
    }

    fn parse_type_annotation(&mut self) -> Result<TypeExpr, VyprError> {
        let token = self.advance();

        let mut node = match token.kind {
            TokenType::INT | TokenType::FLOAT | TokenType::STR | 
            TokenType::BOOL | TokenType::NONE => {
                TypeExpr::Atomic(token.kind)
            }

            TokenType::LIST => {
                if self.match_token(TokenType::LBRACKET) {
                    let inner = self.parse_type_annotation()?;

                    if !self.match_token(TokenType::RBRACKET) {
                        return Err(self.make_error("P001", "expected ']' after list type"))
                    }

                    TypeExpr::List(Box::new(inner))
                } else {
                    TypeExpr::Atomic(TokenType::LIST)
                }
            }

            TokenType::DICT => {
                if self.match_token(TokenType::LBRACKET) {
                    let key_type = self.parse_type_annotation()?;
                    self.consume(TokenType::COMMA, "expected ',' between dict key and value types")?;
                    let value_type = self.parse_type_annotation()?;

                    if !self.match_token(TokenType::RBRACKET) {
                        return Err(self.make_error("P001", "expected ']' after dict type"))
                    }

                    TypeExpr::Dict(Box::new(key_type), Box::new(value_type))
                } else {
                    TypeExpr::Atomic(TokenType::DICT)
                }
            }

            _ => return Err(self.make_error("P002", format!("expected type, found {}", token.lexeme))),
        };

        if self.match_token(TokenType::PIPE) {
            let right_side = self.parse_type_annotation()?;
            node = TypeExpr::Union(Box::new(node), Box::new(right_side));
        }

        Ok(node)
    }
    
    fn if_statement(&mut self) -> Result<Stmt<'p>, VyprError> {
        let token = self.consume(TokenType::IF, "expected 'if'")?;
        let span = token.span;

        let condition = self.expression()?;
        self.consume(TokenType::COLON, "expected ':' after if condtion")?;
        
        let then = self.block()?;
        let mut else_b = None;

        if self.check(TokenType::ELIF) {
            self.advance();

            let elif_stmt = self.if_statement_inner()?;
            else_b = Some(vec![elif_stmt]);
        } else if self.match_token(TokenType::ELSE) {
            self.consume(TokenType::COLON, "expected ':' after else")?;
            else_b = Some(self.block()?);
        }

        Ok(Stmt {
            kind: StmtKind::If {
                condition,
                then, else_b
            },
            span
        })
    }

    fn if_statement_inner(&mut self) -> Result<Stmt<'p>, VyprError> {
        let span = self.previous().span;

        let condition = self.expression()?;
        self.consume(TokenType::COLON, "expected ':'")?;
        let then = self.block()?;
    
        let mut else_b = None;

        if self.check(TokenType::ELIF) {
            self.advance();

            let elif_stmt = self.if_statement_inner()?;
            else_b = Some(vec![elif_stmt]);
        } else if self.match_token(TokenType::ELSE) {
            self.consume(TokenType::COLON, "expected ':'")?;
            else_b = Some(self.block()?);
        }

        Ok(Stmt {
            kind: StmtKind::If {
                condition,
                then,
                else_b
            },
            span
        })
    }

    fn for_statement(&mut self) -> Result<Stmt<'p>, VyprError> {
        let start_token = self.consume(TokenType::FOR, "expected 'for'")?;
        let span = start_token.span;

        let token = self.advance();

        let var_name = match token.kind {
            TokenType::IDENTIFIER(s) => s,

            TokenType::INT => "int".to_string(),
            TokenType::FLOAT => "float".to_string(),
            TokenType::STR => "str".to_string(),
            TokenType::BOOL => "bool".to_string(),
            TokenType::LIST => "list".to_string(),

            _ => return Err(self.make_error("P003", "expected variable name after 'for'"))
        };

        self.consume(TokenType::IN, "expected 'in' after variable")?;
        let iterator = self.expression()?;
        self.consume(TokenType::COLON, "expected ':' after for loop iterator")?;

        let body = self.block()?;

        Ok(Stmt { 
            kind: StmtKind::For { var: var_name, iterator, body }, 
            span 
        })
    }

    fn while_statement(&mut self) -> Result<Stmt<'p>, VyprError> {
        let token = self.consume(TokenType::WHILE, "expected 'while'")?;
        let span = token.span;

        let condition = self.expression()?;
        self.consume(TokenType::COLON, "expected ':' after while condition")?;
        
        let body = self.block()?;
        
        Ok(Stmt { 
            kind: StmtKind::While { condition, body }, 
            span 
        })
    }

    fn return_statement(&mut self) -> Result<Stmt<'p>, VyprError> {
        let keyword = self.previous();
        let span = keyword.span;

        let mut value = None;

        if !self.check(TokenType::NEWLINE) && !self.check(TokenType::SEMICOLON) {
            value = Some(self.expression()?);
        }

        self.consume_statement_end()?;

        Ok(Stmt { 
            kind: StmtKind::Return { keyword, value }, 
            span 
        })
    }

    fn pass_statement(&mut self) -> Result<Stmt<'p>, VyprError> {
        let span = self.previous().span;

        self.consume_statement_end()?;

        Ok(Stmt { kind: StmtKind::Pass, span })
    }

    fn expression_statement(&mut self) -> Result<Stmt<'p>, VyprError> {
        let expr = self.expression()?;
        let span = expr.span;

        self.consume_statement_end()?;

        Ok(Stmt { kind: StmtKind::ExprStmt(expr), span })
    }


    fn var_declaration(&mut self, name: String, span: Span) -> Result<Stmt<'p>, VyprError> {
        let mut annotation = None;

        if self.match_token(TokenType::COLON) {
            annotation = Some(self.parse_type_annotation()?);
        }

        let mut value = None;
        if self.match_token(TokenType::EQUAL) {
            value = Some(self.expression()?);
        } else if annotation.is_none() {
            let operator = if self.match_token(TokenType::PLUS_EQUAL) {
                Some(TokenType::PLUS)
            } else if self.match_token(TokenType::MINUS_EQUAL) { 
                Some(TokenType::MINUS) 
            } else if self.match_token(TokenType::STAR_EQUAL) { 
                Some(TokenType::STAR) 
            } else if self.match_token(TokenType::FSLASH_EQUAL) { 
                Some(TokenType::FSLASH) 
            } else if self.match_token(TokenType::DOUBLE_FSLASH_EQUAL) { 
                Some(TokenType::DOUBLE_FSLASH) 
            } else if self.match_token(TokenType::MODULO_EQUAL) { 
                Some(TokenType::MODULO) 
            } else if self.match_token(TokenType::DOUBLE_STAR_EQUAL) { 
                Some(TokenType::DOUBLE_STAR) 
            } else {
                None
            };

            if let Some(op) = operator {
                let right = self.expression()?;
                let var_span = span;

                value = Some(Expr {
                    kind: ExprKind::Binary {
                        left: Box::new(Expr { kind: ExprKind::Variable(name.clone()), span: var_span }),
                        operator: op,
                        right: Box::new(right)
                    },
                    span: var_span
                });
            }
        }

        self.consume_statement_end()?;

        Ok(Stmt { 
            kind: StmtKind::VarDecl { name, value, annotation }, 
            span 
        })
    }
    
    fn fstring_literal(&mut self, s: String) -> Result<ExprKind, VyprError> {
        let mut parts = Vec::new();
        let mut chars = s.chars().peekable();
        let mut current_text = String::new();

        while let Some(c) = chars.next() {
            if c == '{' {
                if !current_text.is_empty() {
                    parts.push(Expr {
                        kind: ExprKind::Literal(TokenType::STR_LITERAL(current_text.clone())),
                        span: self.previous().span, 
                    });
                    current_text.clear();
                }

                let mut expr_str = String::new();
                let mut brace_depth = 1;
                let mut in_string = None;
                
                while let Some(&next_c) = chars.peek() {
                    if let Some(quote) = in_string {
                        if next_c == quote {
                            in_string = None;
                        }
                    } else {
                        if next_c == '"' || next_c == '\'' { 
                            in_string = Some(next_c); 
                        } else if next_c == '{' { 
                            brace_depth += 1; 
                        } else if next_c == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                chars.next();
                                break;
                            }
                        }
                    }

                    expr_str.push(chars.next().unwrap());
                }

                if brace_depth > 0 {
                    return Err(self.make_error("P017", "unterminated '{' in f-string"));
                }

                if expr_str.trim().is_empty() {
                    return Err(self.make_error("P018", "empty expression inside f-string"));
                }

                let mut inner_lexer = Lexer::new(&expr_str);
                let inner_tokens = inner_lexer.tokenize();
                
                if !inner_lexer.errors.is_empty() {
                    return Err(self.make_error("P019", "invalid syntax inside f-string"));
                }

                let mut inner_parser = Parser::new(inner_tokens);
                let inner_expr = inner_parser.expression().map_err(|_| {
                    self.make_error("P019", "invalid syntax inside f-string")
                })?;

                parts.push(inner_expr);
            } else {
                current_text.push(c);
            }
        }

        if !current_text.is_empty() {
            parts.push(Expr {
                kind: ExprKind::Literal(TokenType::STR_LITERAL(current_text)),
                span: self.previous().span,
            });
        }

        Ok(ExprKind::FString(parts))
    }

    fn list_literal(&mut self) -> Result<ExprKind, VyprError> {
        if self.match_token(TokenType::RBRACKET) {
            return Ok(ExprKind::List(Vec::new()));
        }
        
        let first_expr = self.expression()?;
        
        if self.match_token(TokenType::FOR) {
            let var_name = match self.advance().kind {
                TokenType::IDENTIFIER(name) => name,
                _ => return Err(self.make_error("P016", "expected variable name after 'for' in list comprehension")),
            };

            self.consume(TokenType::IN, "expected 'in' after variable")?;

            let iterator = self.expression()?;

            let mut condition = None;
            if self.match_token(TokenType::IF) {
                condition = Some(Box::new(self.expression()?));
            }

            self.consume(TokenType::RBRACKET, "expected ']' at end of list")?;

            return Ok(ExprKind::ListComp {
                expr: Box::new(first_expr),
                var: var_name,
                iterator: Box::new(iterator),
                condition,
            });
        }

        let mut elements = vec![first_expr];

        if self.match_token(TokenType::COMMA) && !self.check(TokenType::RBRACKET) {
            loop {
                if self.check(TokenType::RBRACKET) {
                    break;
                }

                elements.push(self.expression()?);

                if !self.match_token(TokenType::COMMA) {
                    break;
                }
            }
        }

        if !self.match_token(TokenType::RBRACKET) {
            return Err(self.make_error("P004", "expected ']' after list elements"))
        }

        Ok(ExprKind::List(elements))
    }

    fn dict_literal(&mut self) -> Result<ExprKind, VyprError> {
        if self.match_token(TokenType::RBRACE) {
            return Ok(ExprKind::Dict(Vec::new()));
        }

        let mut elements = Vec::new();
        loop {
            if self.check(TokenType::RBRACE) {
                break;
            }

            let key = self.expression()?;
            self.consume(TokenType::COLON, "expected ':' after dictionary key")?;
            let value = self.expression()?;

            elements.push((key, value));

            if !self.match_token(TokenType::COMMA) {
                break;
            }
        }

        self.consume(TokenType::RBRACE, "expected '}' after dictionary")?;

        Ok(ExprKind::Dict(elements))
    }
    
    fn function_declaration(&mut self) -> Result<Stmt<'p>, VyprError> {
        let span = self.previous().span;

        let name = match self.advance().kind {
            TokenType::IDENTIFIER(s) => s,
            _ => return Err(self.make_error("P005", "expected function name"))
        };

        if !self.match_token(TokenType::LPAREN) {
            return Err(self.make_error("P006", "expected '(' after function name"));
        }

        let mut params = Vec::new();
        if !self.check(TokenType::RPAREN) {
            loop {
                let param_name = match self.advance().kind {
                    TokenType::IDENTIFIER(s) => s,
                    _ => return Err(self.make_error("P007", "expected parameter name"))
                };

                let mut annotation = None;
                if self.match_token(TokenType::COLON) {
                    annotation = Some(self.parse_type_annotation()?);
                }

                params.push(Param {
                    name: param_name,
                    annotation
                });
                
                if !self.match_token(TokenType::COMMA) {
                    break;
                }
            }
        }

        if !self.match_token(TokenType::RPAREN) {
            return Err(self.make_error("P008", "expected ')' after parameters"));
        }

        let mut return_type = None;
        if self.match_token(TokenType::ARROW) {
            return_type = Some(self.parse_type_annotation()?);
        }

        if !self.match_token(TokenType::COLON) {
            return Err(self.make_error("P009", "expected ':' before function body"));
        }

        if !self.match_token(TokenType::NEWLINE) {
            return Err(self.make_error("P010", "expected newline before function body"));
        }

        let body = self.block()?;

        Ok(Stmt { 
            kind: StmtKind::FuncDecl { name, params, return_type, body },
            span
        })
    }

    fn block(&mut self) -> Result<Vec<Stmt<'p>>, VyprError> {
        if self.match_token(TokenType::NEWLINE) {
        }

        self.consume(TokenType::INDENT, "expected indent at start of block")?;

        let mut statements = Vec::new();
        while !self.check(TokenType::DEDENT) && !self.is_at_end() {
            if self.match_token(TokenType::NEWLINE) {
                continue;
            }

            statements.push(self.statement()?);
        }

        if !self.match_token(TokenType::DEDENT) {
            return Err(self.make_error("P011", "expected dedent at end of block"));
        }

        Ok(statements)
    }

    fn assignment(&mut self) -> Result<Expr, VyprError> {
        let expr = self.logic_or()?;

        if self.match_token(TokenType::EQUAL) {
            let value = self.assignment()?;
            let span = expr.span;
            
            match &expr.kind {
                ExprKind::Variable(_) | ExprKind::Subscript { .. } => {
                    return Ok(Expr {
                        kind: ExprKind::Assign {
                            target: Box::new(expr),
                            value: Box::new(value)
                        },
                        span
                    });
                }
                _ => return Err(self.make_error("P021", "invalid assignment target"))
            }
        }

        Ok(expr)
    }

    fn arguments(&mut self) -> Result<(Vec<Expr>, Vec<(String, Expr)>), VyprError> {
        let mut args = Vec::new();
        let mut kwargs = Vec::new();

        if !self.check(TokenType::RPAREN) {
            loop {
                let is_kwarg = if let TokenType::IDENTIFIER(_) = self.peek().kind {
                    self.current + 1 < self.tokens.len() && 
                    matches!(self.tokens[self.current + 1].kind, TokenType::EQUAL)
                } else {
                    false
                };

                if is_kwarg {
                    let name = match self.advance().kind {
                        TokenType::IDENTIFIER(n) => n,
                        _ => unreachable!()
                    };

                    self.advance();
                    let value = self.expression()?;
                    kwargs.push((name, value));
                } else {
                    if !kwargs.is_empty() {
                        return Err(self.make_error("P022", "positional argument follows keyword argument"));
                    }
                    args.push(self.expression()?);
                }

                if !self.match_token(TokenType::COMMA) {
                    break;
                }
            }
        }

        Ok((args, kwargs))
    }

    pub fn expression(&mut self) -> Result<Expr, VyprError> {
        self.assignment()
    }

    fn logic_not(&mut self) -> Result<Expr, VyprError> {
        if self.match_token(TokenType::NOT) {
            let operator = self.previous().kind;
            let span = self.previous().span;
            let right = self.logic_not()?;

            return Ok(Expr {
                kind: ExprKind::Unary { operator, right: Box::new(right) },
                span
            });

        }
        
        self.equality()
    }

    fn logic_or(&mut self) -> Result<Expr, VyprError> {
        let mut expr = self.logic_and()?;

        while self.match_token(TokenType::OR) {
            let operator = self.previous().kind;
            let right = self.logic_and()?;
            let span = expr.span;

            expr = Expr { 
                kind: ExprKind::Binary { left: Box::new(expr), operator, right: Box::new(right) }, 
                span 
            };
        }

        Ok(expr)
    }

    fn logic_and(&mut self) -> Result<Expr, VyprError> {
        let mut expr = self.logic_not()?;

        while self.match_token(TokenType::AND) {
            let operator = self.previous().kind;
            let right = self.equality()?;
            let span = expr.span;

            expr = Expr { 
                kind: ExprKind::Binary { left: Box::new(expr), operator, right: Box::new(right) }, 
                span 
            };
        }

        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, VyprError> {
        let mut expr = self.comparison()?;

        while self.match_token(TokenType::DOUBLE_EQUAL) {
            let operator = self.previous().kind;
            let right = self.comparison()?;
            let span = expr.span;

            expr = Expr {
                kind: ExprKind::Binary { left: Box::new(expr), operator, right: Box::new(right) },
                span
            };
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, VyprError> {
        let mut expr = self.term()?;

        while self.match_tokens(&[TokenType::LESS_THAN, TokenType::GREATER_THAN, 
            TokenType::LESS_THAN_EQUAL, TokenType::GREATER_THAN_EQUAL]) 
        {
            let operator = self.previous().kind;
            let right = self.term()?;
            let span = expr.span;

            expr = Expr {
                kind: ExprKind::Binary { left: Box::new(expr), operator, right: Box::new(right) },
                span
            };
        }

        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, VyprError> {
        let mut expr = self.factor()?;

        while self.match_tokens(&[TokenType::PLUS, TokenType::MINUS]) {
            let operator = self.previous().kind;
            let right = self.factor()?;
            let span = expr.span;

            expr = Expr {
                kind: ExprKind::Binary { left: Box::new(expr), operator, right: Box::new(right) },
                span
            };
        }

        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, VyprError> {
        let mut expr = self.unary()?;

        while self.match_tokens(&[
            TokenType::STAR, 
            TokenType::FSLASH,
            TokenType::MODULO,
            TokenType::DOUBLE_FSLASH])
        {
            let operator = self.previous().kind;
            let right = self.unary()?;
            let span = expr.span;

            expr = Expr {
                kind: ExprKind::Binary { left: Box::new(expr), operator, right: Box::new(right) },
                span
            };
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, VyprError> {
        if self.match_tokens(&[TokenType::MINUS]) {
            let operator = self.previous().kind;
            let span = self.previous().span;
            let right = self.unary()?; // Recursive for --5

            return Ok(Expr {
                kind: ExprKind::Unary { operator, right: Box::new(right) },
                span
            });
        }

        self.power()
    }

    fn power(&mut self) -> Result<Expr, VyprError> {
        let mut expr = self.call()?;

        if self.match_token(TokenType::DOUBLE_STAR) {
            let operator = self.previous().kind;
            let right = self.unary()?;
            let span = expr.span;

            expr = Expr {
                kind: ExprKind::Binary { left: Box::new(expr), operator, right: Box::new(right) },
                span
            }
        }

        Ok(expr)
    }

    fn call(&mut self) -> Result<Expr, VyprError> {
        let mut expr = self.primary()?;

        loop {
            if self.match_token(TokenType::LPAREN) {
                expr = self.finish_call(expr)?;
            } else if self.match_token(TokenType::LBRACKET) {
                expr = self.finish_subscript(expr)?;
            } else if self.match_token(TokenType::PERIOD) {
                expr = self.finish_method_call(expr)?;
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, VyprError> {
        let span = self.peek().span;
        let token = self.advance();

        let kind = match token.kind {
            TokenType::INT_LITERAL(_) | TokenType::FLOAT_LITERAL(_) |
            TokenType::STR_LITERAL(_) | TokenType::TRUE | TokenType::FALSE |
            TokenType::NONE => ExprKind::Literal(token.kind),

            TokenType::IDENTIFIER(name) => ExprKind::Variable(name),

            TokenType::INT => ExprKind::Variable("int".to_string()),
            TokenType::FLOAT => ExprKind::Variable("float".to_string()),
            TokenType::STR => ExprKind::Variable("str".to_string()),
            TokenType::FSTRING(s) => self.fstring_literal(s)?,
            TokenType::BOOL => ExprKind::Variable("bool".to_string()),
            TokenType::LIST => ExprKind::Variable("list".to_string()),
            TokenType::RANGE => ExprKind::Variable("range".to_string()),

            TokenType::LBRACKET => self.list_literal()?,

            TokenType::LPAREN => {
                let expr = self.expression()?;
                self.consume(TokenType::RPAREN, "expected ')' after expression")?;
                ExprKind::Grouping(Box::new(expr))
            }

            TokenType::LBRACE => self.dict_literal()?,

            _ => return Err(self.make_error("P013", format!("expected expression, found {}", token.lexeme)))
        };

        Ok(Expr { kind, span })
    }

    fn match_tokens(&mut self, kinds: &[TokenType]) -> bool {
        for kind in kinds {
            if self.check(kind.clone()) {
                self.advance();
                return true;
            }
        }

        false
    }

    fn finish_call(&mut self, callee: Expr) -> Result<Expr, VyprError> {
        let span = callee.span;
        let (args, kwargs) = self.arguments()?;

        self.consume(TokenType::RPAREN, "expected ')' after arguments")?;

        Ok(Expr { kind: ExprKind::Call { callee: Box::new(callee), args, kwargs }, span })
    }

    fn finish_method_call(&mut self, callee: Expr) -> Result<Expr, VyprError> {
        let span = callee.span;

        let method_name = match self.advance().kind {
            TokenType::IDENTIFIER(name) => name,
            _ => return Err(self.make_error("P014", "expected method name after '.'")),
        };

        self.consume(TokenType::LPAREN, "expected '(' after method name")?;

        let (args, kwargs) = self.arguments()?;

        self.consume(TokenType::RPAREN, "expected ')' after arguments")?;

        Ok(Expr { kind: ExprKind::MethodCall { callee: Box::new(callee), method: method_name, args, kwargs }, span })
    }

    fn finish_subscript(&mut self, callee: Expr) -> Result<Expr, VyprError> {
        let span = callee.span;
        let index = self.expression()?;

        self.consume(TokenType::RBRACKET, "expected ']' after subscript")?;

        Ok(Expr { kind: ExprKind::Subscript { callee: Box::new(callee), index: Box::new(index) }, span })
    }

    fn check_identifier(&self) -> bool {
        if self.current + 1 >= self.tokens.len() {
            return false;
        }

        matches!(&self.tokens[self.current + 1].kind, 
            TokenType::EQUAL | TokenType::COLON | TokenType::PLUS_EQUAL | 
            TokenType::MINUS_EQUAL | TokenType::STAR_EQUAL | TokenType::FSLASH_EQUAL | 
            TokenType::DOUBLE_FSLASH_EQUAL | TokenType::MODULO_EQUAL | TokenType::DOUBLE_STAR_EQUAL
        )
    }

    fn consume(&mut self, kind: TokenType, message: &str) -> Result<Token<'p>, VyprError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(self.make_error("P015", message))
        }
    }

     fn consume_statement_end(&mut self) -> Result<(), VyprError> {
        if self.check(TokenType::SEMICOLON) {
            self.advance();
        }

        if self.is_at_end() || self.check(TokenType::DEDENT) {
            return Ok(());
        }

        self.consume(TokenType::NEWLINE, "expected newline after statement")?;

        Ok(())
    }

    fn match_token(&mut self, kind: TokenType) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, kind: TokenType) -> bool {
        if self.is_at_end() {
            return false;
        }

        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(&kind)
    }

    fn advance(&mut self) -> Token<'p> {
        if !self.is_at_end() {
            self.current += 1;
        }

        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenType::EOF
    }

    fn peek(&self) -> Token<'p> {
        self.tokens[self.current].clone()
    }

    fn previous(&self) -> Token<'p> {
        self.tokens[self.current - 1].clone()
    }
}
