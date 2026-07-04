use error::error::Span;
use parser::ast::TypeExpr;
use crate::context::VarID;

#[derive(Debug, Clone)]
pub struct VIRProgram {
    pub functions: Vec<VIRFunction>,
    pub classes: Vec<(String, Vec<(String, TypeExpr)>)>, 
    pub globals: Vec<(VarID, TypeExpr, VIRExpr)>,
}

#[derive(Debug, Clone)]
pub struct VIRFunction {
    pub name: String, 
    pub var_id: VarID, 
    pub params: Vec<(VarID, TypeExpr)>, 
    pub return_type: TypeExpr,
    pub body: VIRBlock,
    pub is_native: bool, // Renamed from is_extern for Vypr's NativeFunctions
}

#[derive(Debug, Clone)]
pub struct VIRExpr {
    pub kind: VIRExprKind,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum VIRExprKind {
    // 1. Value References
    VarRef(VarID),

    // 2. Literals
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    BoolLiteral(bool),
    NoneLiteral, // Vypr-specific

    // 3. Data Structures
    ListInit {
        elements: Vec<VIRExpr>,
    },

    ListComp {
        expr: Box<VIRExpr>,
        var_id: VarID,
        iterator: Box<VIRExpr>,
        condition: Option<Box<VIRExpr>>,
    },

    DictInit {
        keys: Vec<VIRExpr>,
        values: Vec<VIRExpr>,
    },

    SubscriptAccess { // e.g., my_list[0]
        base: Box<VIRExpr>,
        index: Box<VIRExpr>,
    },

    PropertyAccess { // e.g., math.pi
        object: Box<VIRExpr>,
        property: String,
    },

    // 4. Operations
    Call {
        callee: VarID,
        args: Vec<VIRExpr>,
        // kwargs placeholder can go here eventually!
    },

    MethodCall {
        object: Box<VIRExpr>,
        method_name: String,
        args: Vec<VIRExpr>,
    },

    Binary {
        op: VIRBinOp,
        lhs: Box<VIRExpr>,
        rhs: Box<VIRExpr>,
    },

    Unary {
        op: VIRUnaryOp,
        operand: Box<VIRExpr>,
    },

    Assign {
        target: Box<VIRExpr>, 
        value: Box<VIRExpr>,
    },

    // 5. Control Flow Primitives (Desugared)
    If {
        cond: Box<VIRExpr>,
        then_block: Box<VIRBlock>,
        else_block: Option<Box<VIRBlock>>,
    },

    Loop(Box<VIRBlock>), // 'For' loops are desugared into this!
    Break,
    Continue,
    Return(Option<Box<VIRExpr>>),
    Block(VIRBlock),

    Import(String),

    FunctionDef(Box<VIRFunction>),
    FormatString(Vec<VIRExpr>),
}

#[derive(Debug, Clone)]
pub struct VIRBlock {
    pub stmts: Vec<VIRStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum VIRStmt {
    VarDecl {
        var_id: VarID,
        ty: TypeExpr,
        init: Option<VIRExpr>,
        span: Span,
    },
    Expr(VIRExpr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VIRBinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    FloorDiv, Power,
    And, Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VIRUnaryOp {
    Neg, Not,
}
