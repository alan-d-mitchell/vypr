use crate::vir::*;
use std::fmt;
use parser::ast::TypeExpr;

impl fmt::Display for VIRFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "def {}(", self.name)?;
        for (i, (var_id, ty)) in self.params.iter().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            
            if *ty != TypeExpr::Any {
                write!(f, "{}: {}", var_id, ty)?;
            } else {
                write!(f, "{}", var_id)?;
            }
        }
        
        if self.return_type != TypeExpr::Any {
            write!(f, ") -> {} ", self.return_type)?;
        } else {
            write!(f, ") ")?;
        }
        
        write!(f, "{}", self.body)
    }
}

impl fmt::Display for VIRBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{{")?;
        for stmt in &self.stmts {
            let stmt_str = format!("{}", stmt);
            for line in stmt_str.lines() {
                writeln!(f, "    {}", line)?;
            }
        }
        write!(f, "}}")
    }
}

impl fmt::Display for VIRStmt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VIRStmt::VarDecl { var_id, init, .. } => {
                write!(f, "{}", var_id)?;
                if let Some(expr) = init {
                    write!(f, " = {};", expr)?;
                } else {
                    write!(f, ";")?;
                }
                Ok(())
            }
            VIRStmt::Expr(expr) => write!(f, "{};", expr),
        }
    }
}

impl fmt::Display for VIRExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind) 
    }
}

impl fmt::Display for VIRExprKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VIRExprKind::VarRef(id) => write!(f, "{}", id),
            VIRExprKind::IntLiteral(val) => write!(f, "{}", val),
            VIRExprKind::FloatLiteral(val) => write!(f, "{}", val),
            VIRExprKind::StringLiteral(val) => write!(f, "\"{}\"", val),
            VIRExprKind::BoolLiteral(val) => write!(f, "{}", val),
            VIRExprKind::NoneLiteral => write!(f, "None"),
            
            VIRExprKind::ListInit { elements } => {
                write!(f, "[")?;
                for (i, val) in elements.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", val)?;
                }
                write!(f, "]")
            }

            VIRExprKind::DictInit { keys, values } => {
                write!(f, "{{")?;
                for (i, (k, v)) in keys.iter().zip(values.iter()).enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }

            VIRExprKind::SubscriptAccess { base, index } => write!(f, "{}[{}]", base, index),
            VIRExprKind::PropertyAccess { object, property } => write!(f, "{}.{}", object, property),
            
            VIRExprKind::Call { callee, args, kwargs } => {
                write!(f, "{}(", callee)?;
                let mut first = true;

                for arg in args {
                    if !first { write!(f, ", ")?; }
                    write!(f, "{}", arg)?;
                    first = false;
                }

                for (name, arg) in kwargs {
                    if !first { write!(f, ", ")?; }
                    write!(f, "{}={}", name, arg)?;
                    first = false;
                }

                write!(f, ")")
            }

            VIRExprKind::MethodCall { object, method_name, args, kwargs } => {
                write!(f, "{}.{}(", object, method_name)?;
                let mut first = true;

                for arg in args {
                    if !first { write!(f, ", ")?; }
                    write!(f, "{}", arg)?;
                    first = false;
                }

                for (name, arg) in kwargs {
                    if !first { write!(f, ", ")?; }
                    write!(f, "{}={}", name, arg)?;
                    first = false;
                }

                write!(f, ")")
            }
            
            VIRExprKind::Binary { op, lhs, rhs } => write!(f, "({} {} {})", lhs, op, rhs),
            VIRExprKind::Unary { op, operand } => write!(f, "{}{}", op, operand),
            VIRExprKind::Assign { target, value } => write!(f, "{} = {}", target, value),
            
            VIRExprKind::If { cond, then_block, else_block } => {
                write!(f, "if {} {}", cond, then_block)?;
                if let Some(els) = else_block {
                    write!(f, " else {}", els)?;
                }

                Ok(())
            }

            VIRExprKind::Loop(block) => write!(f, "loop {}", block),
            VIRExprKind::Break => write!(f, "break"),
            VIRExprKind::Continue => write!(f, "continue"),
            VIRExprKind::Return(Some(expr)) => write!(f, "return {}", expr),
            VIRExprKind::Return(None) => write!(f, "return"),
            VIRExprKind::Block(block) => write!(f, "{}", block),
            
            VIRExprKind::Import(module) => write!(f, "import {}", module),
            VIRExprKind::FunctionDef(function) => write!(f, "def {}()", function),

            VIRExprKind::FormatString(parts) => {
                write!(f, "f\"")?;

                for part in parts {
                    write!(f, "{{{}}}", part)?;
                }

                write!(f, "\"")
            }

            VIRExprKind::ListComp { expr, var_id, iterator, condition } => {
                write!(f, "[{} for {} in {}", expr, var_id, iterator)?;
                if let Some(cond) = condition {
                    write!(f, " if {}", cond)?;
                }
                write!(f, "]")
            }
        }
    }
}

impl fmt::Display for VIRBinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        let op_str = match self {
            VIRBinOp::Add => "+", VIRBinOp::Sub => "-", VIRBinOp::Mul => "*",
            VIRBinOp::Div => "/", VIRBinOp::Mod => "%", VIRBinOp::Eq => "==",
            VIRBinOp::Ne => "!=", VIRBinOp::Lt => "<", VIRBinOp::Le => "<=",
            VIRBinOp::Gt => ">", VIRBinOp::Ge => ">=", VIRBinOp::And => "and",
            VIRBinOp::Or => "or", VIRBinOp::Power => "**", VIRBinOp::FloorDiv => "//"
        };

        write!(f, "{}", op_str)
    }
}

impl fmt::Display for VIRUnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        let op_str = match self {
            VIRUnaryOp::Neg => "-", VIRUnaryOp::Not => "not ",
        };

        write!(f, "{}", op_str)
    }
}

impl fmt::Display for VIRProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        // 1. Dump Global Variables
        for (var_id, _ty, expr) in &self.globals {
            writeln!(f, "global {} = {};", var_id, expr)?;
        }

        if !self.globals.is_empty() && (!self.functions.is_empty() || !self.classes.is_empty()) {
            writeln!(f)?;
        }

        // 2. Dump Classes (For when you add OOP!)
        for (name, fields) in &self.classes {
            writeln!(f, "class {} {{", name)?;
            for (field_name, field_ty) in fields {
                writeln!(f, "    {}: {},", field_name, field_ty)?;
            }
            writeln!(f, "}}\n")?;
        }

        // 3. Dump Functions
        for (i, func) in self.functions.iter().enumerate() {
            write!(f, "{}", func)?;
            if i < self.functions.len() - 1 {
                writeln!(f, "\n")?;
            }
        }

        Ok(())
    }
}
