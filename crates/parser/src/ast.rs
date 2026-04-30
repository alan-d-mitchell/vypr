use lexer::token::{Token, TokenType};
use error::error::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Any,
    Atomic(TokenType),
    List(Box<TypeExpr>),
    Union(Box<TypeExpr>, Box<TypeExpr>),
}

impl std::fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeExpr::Atomic(token_type) => match token_type {
                TokenType::INT => write!(f, "'int'"),
                TokenType::FLOAT => write!(f, "'float'"),
                TokenType::STR => write!(f, "'str'"),
                TokenType::BOOL => write!(f, "'bool'"),
                TokenType::NONE => write!(f, "'None'"),
                TokenType::LIST => write!(f, "'list'"),
                TokenType::RANGE => write!(f, "'range'"),
                _ => write!(f, "any")
            },

            TypeExpr::Any => write!(f, "any"),
            TypeExpr::List(inner) => write!(f, "list[{}]", inner),
            TypeExpr::Union(left, right) => write!(f, "{} | {}", left, right),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub annotation: Option<TypeExpr>
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt<'s> {
    pub kind: StmtKind<'s>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind<'s> {
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

    Pass,
    Break,
    Continue,

    ExprStmt(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
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
    },

    ListComp {
        expr: Box<Expr>,
        var: String,
        iterator: Box<Expr>,
        condition: Option<Box<Expr>>,
    }
}
