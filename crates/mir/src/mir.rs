use error::error::Span;
use parser::ast::TypeExpr;
use vir::vir::{VIRBinOp, VIRUnaryOp};
use vir::context::Constant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BasicBlockID(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalID(pub usize); // _0 is return value, _1.._n are args, the rest are temporaries

#[derive(Debug, Clone)]
pub struct MIRProgram {
    pub functions: Vec<MIRFunction>,
}

#[derive(Debug, Clone)]
pub struct MIRFunction {
    pub name: String,
    pub arity: usize,
    pub locals: Vec<LocalDecl>,
    pub basic_blocks: Vec<BasicBlock>,
}

#[derive(Debug, Clone)]
pub struct LocalDecl {
    pub ty: TypeExpr,
    pub debug_name: Option<String>, // Helpful for printing dumps!
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub local: LocalID,
    pub projection: Vec<ProjectionElem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionElem {
    Property(String), // obj.prop
    Index(LocalID),   // arr[i]
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    Assign(Place, Rvalue),
    AssignGlobal(String, Rvalue),
    DefineGlobal(String, TypeExpr, Rvalue),
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Rvalue {
    Use(Operand),
    BinaryOp(VIRBinOp, Operand, Operand),
    UnaryOp(VIRUnaryOp, Operand),
    ListInit(Vec<Operand>),
    DictInit(Vec<Operand>, Vec<Operand>),
    Import(String),
    FunctionDef(Box<MIRFunction>),
    FormatString(Vec<Operand>),
}

#[derive(Debug, Clone)]
pub enum Operand {
    Copy(Place),
    Const(Constant),
    Static(String),
}

#[derive(Debug, Clone)]
pub enum Terminator {
    Goto { target: BasicBlockID },
    SwitchInt {
        discriminant: Operand,
        true_target: BasicBlockID,
        false_target: BasicBlockID,
    },
    Call {
        callee: Operand, // Could be a static function ID or a dynamic variable!
        args: Vec<Operand>,
        destination: Place,
        target: BasicBlockID,
    },
    MethodCall {
        object: Operand,
        method_name: String,
        args: Vec<Operand>,
        destination: Place,
        target: BasicBlockID,
    },
    Return,
    Unreachable,
}
