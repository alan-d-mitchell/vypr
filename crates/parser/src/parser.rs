use lexer::token::{Token, TokenType};
use crate::ast::{Param, TypeExpr};

use super::ast::{Stmt, Expr};

pub struct Parser<'p> {
    tokens: Vec<Token<'p>>,
    current: usize,
}

impl<'p> Parser<'p> {

    pub fn new(tokens: Vec<Token<'p>>) -> Self {
        Self {
            tokens,
            current: 0,
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt<'p>>, String> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            if self.match_token(TokenType::NEWLINE) {
                continue;
            }

            statements.push(self.statement()?);
        }

        Ok(statements)
    }

    fn statement(&mut self) -> Result<Stmt<'p>, String> {
        if self.match_token(TokenType::DEF) {
            return self.function_declaration();
        }

        if self.check(TokenType::IF) {
            return self.if_statement();
        }

        if self.check(TokenType::WHILE) {
            return self.while_statement();
        }

        if self.match_token(TokenType::RETURN) {
            return self.return_statement();
        }

        if let TokenType::IDENTIFIER(name) = self.peek().kind {
            if self.check_identifier() {
                self.advance();
                return self.var_declaration(name);
            }
        }

        self.expression_statement()
    }

    fn parse_type_annotation(&mut self) -> Result<TypeExpr, String> {
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
                        return Err("expected ']' after list type".to_string());
                    }
                    TypeExpr::List(Box::new(inner))
                } else {
                    TypeExpr::Atomic(TokenType::LIST)
                }
            }
            _ => return Err(format!("expected type, found {:?}", token.lexeme)),
        };

        if self.match_token(TokenType::PIPE) {
            let right_side = self.parse_type_annotation()?;
            node = TypeExpr::Union(Box::new(node), Box::new(right_side));
        }

        Ok(node)
    }
    
    fn if_statement(&mut self) -> Result<Stmt<'p>, String> {
        self.consume(TokenType::IF, "expected 'if'")?;
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

        Ok(Stmt::If {
            condition,
            then,
            else_b
        })
    }

    fn if_statement_inner(&mut self) -> Result<Stmt<'p>, String> {
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

        Ok(Stmt::If {
            condition,
            then,
            else_b
        })
    }

    fn while_statement(&mut self) -> Result<Stmt<'p>, String> {
        self.consume(TokenType::WHILE, "Expected 'while'")?;
        let condition = self.expression()?;
        self.consume(TokenType::COLON, "Expected ':' after while condition")?;
        
        let body = self.block()?;
        
        Ok(Stmt::While { 
            condition, 
            body 
        })
    }

    fn return_statement(&mut self) -> Result<Stmt<'p>, String> {
        let keyword = self.previous();
        let mut value = None;

        if !self.check(TokenType::NEWLINE) {
            value = Some(self.expression()?);
        }

        self.match_token(TokenType::NEWLINE);

        Ok(Stmt::Return {
            keyword,
            value
        })
    }

    fn expression_statement(&mut self) -> Result<Stmt<'p>, String> {
        let expr = self.expression()?;
        self.match_token(TokenType::NEWLINE);

        Ok(Stmt::ExprStmt(expr))
    }


    fn var_declaration(&mut self, name: String) -> Result<Stmt<'p>, String> {
        let mut annotation = None;

        if self.match_token(TokenType::COLON) {
            annotation = Some(self.parse_type_annotation()?);
        }

        let mut value = None;
        if self.match_token(TokenType::EQUAL) {
            value = Some(self.expression()?);
        }

        if self.check(TokenType::SEMICOLON) {
            self.advance();
        } else {
            self.match_token(TokenType::NEWLINE);
        }


        Ok(Stmt::VarDecl {
            name,
            value,
            annotation
        })
    }

    fn list_literal(&mut self) -> Result<Expr, String> {
        let mut elements = Vec::new();

        if !self.check(TokenType::RBRACKET) {
            loop {
                elements.push(self.expression()?);

                if !self.match_token(TokenType::COMMA) {
                    break;
                }
            }
        }

        if !self.match_token(TokenType::RBRACKET) {
            return Err("expected ']' after list elements".to_string())
        }

        Ok(Expr::List(elements))
    }
    
    fn function_declaration(&mut self) -> Result<Stmt<'p>, String> {
        let name = match self.advance().kind {
            TokenType::IDENTIFIER(s) => s,
            _ => return Err("expected function name".to_string())
        };

        if !self.match_token(TokenType::LPAREN) {
            return Err("expected '(' after function name".to_string());
        }

        let mut params = Vec::new();
        if !self.check(TokenType::RPAREN) {
            loop {
                let param_name = match self.advance().kind {
                    TokenType::IDENTIFIER(s) => s,
                    _ => return Err("expected parameter name".to_string())
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
            return Err("expected ')' after parameters".to_string());
        }

        let mut return_type = None;
        if self.match_token(TokenType::ARROW) {
            return_type = Some(self.parse_type_annotation()?);
        }

        if !self.match_token(TokenType::COLON) {
            return Err("expected ':' before function body".to_string());
        }

        if !self.match_token(TokenType::NEWLINE) {
            return Err("expected newline before function body".to_string());
        }

        let body = self.block()?;

        Ok(Stmt::FuncDecl {
            name,
            params,
            return_type,
            body
        })
    }

    fn block(&mut self) -> Result<Vec<Stmt<'p>>, String> {
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
            return Err("expected dedent at end of block".to_string());
        }

        Ok(statements)
    }

    pub fn expression(&mut self) -> Result<Expr, String> {
        self.logic_or()
    }

    fn logic_or(&mut self) -> Result<Expr, String> {
        let mut expr = self.logic_and()?;

        while self.match_token(TokenType::OR) {
            let operator = self.previous().kind;
            let right = self.logic_and()?;

            expr = Expr::Binary { 
                left: Box::new(expr), 
                operator, 
                right: Box::new(right) 
            };
        }

        Ok(expr)
    }

    fn logic_and(&mut self) -> Result<Expr, String> {
        let mut expr = self.equality()?;

        while self.match_token(TokenType::AND) {
            let operator = self.previous().kind;
            let right = self.equality()?;

            expr = Expr::Binary { 
                left: Box::new(expr), 
                operator, 
                right: Box::new(right) 
            };
        }

        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.comparison()?;

        while self.match_token(TokenType::DOUBLE_EQUAL) {
            let operator = self.previous().kind;
            let right = self.comparison()?;

            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        let mut expr = self.term()?;

        while self.match_tokens(&[TokenType::LESS_THAN, TokenType::GREATER_THAN, 
            TokenType::LESS_THAN_EQUAL, TokenType::GREATER_THAN_EQUAL]) 
        {
            let operator = self.previous().kind;
            let right = self.term()?;

            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right)
            };
        }

        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut expr = self.factor()?;

        while self.match_tokens(&[TokenType::PLUS, TokenType::MINUS]) {
            let operator = self.previous().kind;
            let right = self.factor()?;

            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right)
            };
        }

        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, String> {
        let mut expr = self.unary()?;

        while self.match_tokens(&[TokenType::STAR, TokenType::FSLASH]) {
            let operator = self.previous().kind;
            let right = self.unary()?;

            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.match_tokens(&[TokenType::MINUS, TokenType::NOT]) {
            let operator = self.previous().kind;
            let right = self.unary()?; // Recursive for --5

            return Ok(Expr::Unary {
                operator,
                right: Box::new(right),
            });
        }

        self.call()
    }

    fn call(&mut self) -> Result<Expr, String> {
        let mut expr = self.primary()?;

        loop {
            if self.match_token(TokenType::LPAREN) {
                expr = self.finish_call(expr)?;
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let token = self.advance();

        match token.kind {
            TokenType::INT_LITERAL(_) | TokenType::FLOAT_LITERAL(_) |
            TokenType::STR_LITERAL(_) | TokenType::TRUE | TokenType::FALSE |
            TokenType::NONE => Ok(Expr::Literal(token.kind)),

            TokenType::IDENTIFIER(name) => Ok(Expr::Variable(name)),
            
            TokenType::LPAREN => {
                let expr = self.expression()?;

                if !self.match_token(TokenType::RPAREN) {
                    return Err("expected ')' after expression.".to_string());
                }

                Ok(Expr::Grouping(Box::new(expr)))
            }

            _ => Err(format!("expected expression, found {:?}", token.lexeme)),
        }
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

    fn finish_call(&mut self, callee: Expr) -> Result<Expr, String> {
        let mut args = Vec::new();

        if !self.check(TokenType::RPAREN) {
            loop {
                args.push(self.expression()?);

                if !self.match_token(TokenType::COMMA) {
                    break;
                }
            }
        }

        self.consume(TokenType::RPAREN, "expected ')' after arguments")?;

        Ok(Expr::Call {
            callee: Box::new(callee),
            args,
        })
    }

    fn check_identifier(&self) -> bool {
        if self.current + 1 >= self.tokens.len() {
            return false;
        }

        let next_kind = &self.tokens[self.current + 1].kind;
        match next_kind {
            TokenType::EQUAL | TokenType::COLON => true,
            _ => false
        }
    }

    fn consume(&mut self, kind: TokenType, message: &str) -> Result<Token<'p>, String> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(message.to_string())
        }
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
