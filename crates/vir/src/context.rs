use std::collections::HashMap;
use std::fmt;
use error::error::Span;
use parser::ast::TypeExpr; 

// Holds the unique ID for the compiler, and the String name purely for clean debugging dumps!
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VarID(pub usize, pub String);

impl fmt::Display for VarID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id_{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeID(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    None,
}

impl fmt::Display for Constant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constant::Int(v) => write!(f, "{}", v),
            Constant::Float(v) => write!(f, "{}", v),
            Constant::Bool(v) => write!(f, "{}", v),
            Constant::String(v) => write!(f, "\"{}\"", v),
            Constant::None => write!(f, "None"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SymbolKind {
    Variable {
        ty: TypeExpr,
        is_mutable: bool,
    },

    Function {
        params: Vec<TypeExpr>,
        return_type: TypeExpr,
    },

    Class {
        fields: Vec<(String, TypeExpr)>, // placeholder for when you add Object-Oriented features
    },

    Constant {
        ty: TypeExpr,
        value: Constant,
    },
}

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub span: Span,
    pub module_path: Vec<String>,
    pub kind: SymbolKind,
}

#[derive(Default)]
pub struct VIRContext {
    pub definitions: HashMap<usize, SymbolInfo>, // Keyed by the usize inside VarID
    pub types: HashMap<TypeID, TypeExpr>,
    next_var_id: usize,
    next_type_id: usize,
}

impl VIRContext {

    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_def(&mut self, info: SymbolInfo) -> VarID {
        let id = self.next_var_id;
        let name = info.name.clone();
        
        self.next_var_id += 1;
        self.definitions.insert(id, info);

        VarID(id, name)
    }

    pub fn get_def(&self, id: &VarID) -> Option<&SymbolInfo> {
        self.definitions.get(&id.0)
    }

    pub fn update_def(&mut self, id: &VarID, info: SymbolInfo) {
        self.definitions.insert(id.0, info);
    }
}
