use lexer::token::{Token, TokenType};

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Atomic(TokenType),
    List(Box<TypeExpr>),
    Union(Box<TypeExpr>, Box<TypeExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub annotation: Option<TypeExpr>
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt<'s> {
    VarDecl {
        name: String,
        value: Option<Expr>,
        annotation: Option<TypeExpr>
    },

    FuncDecl {
        name: String,
        params: Vec<Param>,
        return_type: Option<TypeExpr>,
        body: Vec<Stmt<'s>>
    },

    Return {
        keyword: Token<'s>,
        value: Option<Expr>
    },

    If {
        condition: Expr,
        then: Vec<Stmt<'s>>,
        else_b: Option<Vec<Stmt<'s>>>,
    },

    While {
        condition: Expr,
        body: Vec<Stmt<'s>>,
    },

    For {
        var: String,
        iterator: Expr,
        body: Vec<Stmt<'s>>
    },

    ExprStmt(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(TokenType),
    Variable(String),
    List(Vec<Expr>),
    Grouping(Box<Expr>),

    Call {
        callee: Box<Expr>,
        args: Vec<Expr>
    },

    Subscript {
        callee: Box<Expr>,
        index: Box<Expr>
    },

    Binary {
        left: Box<Expr>,
        operator: TokenType,
        right: Box<Expr>
    },

    Unary {
        operator: TokenType,
        right: Box<Expr>
    },

    MethodCall {
        callee: Box<Expr>,
        args: Vec<Expr>,
        method: String,
    }
}
