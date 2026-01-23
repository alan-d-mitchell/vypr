use std::collections::HashMap;
use parser::ast::TypeExpr;

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolType {
    Locked(TypeExpr),
    Dynamic
}


#[derive(Debug, Clone)]
pub struct Symbol {
    pub kind: SymbolType,
    pub initialized: bool,
}

pub struct Scope {
    variables: HashMap<String, Symbol>,
}

impl Scope {

    pub fn new() -> Self {
        Self {
            variables: HashMap::new()
        }
    }

    pub fn define(&mut self, name: String, kind: SymbolType, initialized: bool) {
        self.variables.insert(name, Symbol {
            kind,
            initialized,
        });
    }

    pub fn get(&self, name: &str) -> Option<&Symbol> {
        self.variables.get(name)
    }

    pub fn mark_initialized(&mut self, name: &str) {
        if let Some(sym) = self.variables.get_mut(name) {
            sym.initialized = true;
        }
    }
}
