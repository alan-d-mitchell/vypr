// ------------------------------------------------------------------------
// DISPLAY TRAIT IMPLEMENTATIONS
// ------------------------------------------------------------------------

use crate::mir::*;
use std::fmt;
use vir::context::Constant;

// 1. Program & Functions
impl fmt::Display for MIRProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, func) in self.functions.iter().enumerate() {
            write!(f, "{}", func)?;
            if i < self.functions.len() - 1 {
                writeln!(f, "\n")?;
            }
        }

        let mut strings = Vec::new();
        self.collect_strings(&mut strings);
        
        if !strings.is_empty() {
            writeln!(f, "\n")?;
            for (i, s) in strings.iter().enumerate() {
                let bytes = s.as_bytes();
                writeln!(f, "alloc{} (size: {}, align: 1) {{", i + 1, bytes.len())?;
                
                for chunk in bytes.chunks(16) {
                    let mut hex_part = String::new();
                    let mut ascii_part = String::new();
                    
                    for &b in chunk {
                        hex_part.push_str(&format!("{:02x} ", b));

                        // only print visible ASCII characters, otherwise print a dot
                        if (32..=126).contains(&b) {
                            ascii_part.push(b as char);
                        } else {
                            ascii_part.push('.');
                        }
                    }
                    
                    // pad the hex part out to 48 characters so the ASCII columns align perfectly
                    writeln!(f, "    {:<48} │ {}", hex_part, ascii_part)?;
                }
                writeln!(f, "}}")?;
                
                if i < strings.len() - 1 {
                    writeln!(f)?;
                }
            }
        }

        Ok(())
    }
}

impl fmt::Display for MIRFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "fn {}() {{", self.name)?;

        // Print local variable allocations
        for (i, local) in self.locals.iter().enumerate() {
            write!(f, "    let _{}: {}", i, local.ty)?;
            if let Some(name) = &local.debug_name {
                write!(f, "; // {}", name)?;
            } else {
                write!(f, ";")?;
            }
            writeln!(f)?;
        }
        writeln!(f)?;

        // Print Basic Blocks
        for (i, block) in self.basic_blocks.iter().enumerate() {
            writeln!(f, "    bb{}: {{", i)?;
            for stmt in &block.statements {
                writeln!(f, "        {}", stmt)?;
            }
            writeln!(f, "        {}", block.terminator)?;
            writeln!(f, "    }}")?;
            
            // Add a spacer between blocks unless it's the last one
            if i < self.basic_blocks.len() - 1 {
                writeln!(f)?;
            }
        }

        write!(f, "}}")
    }
}

// 2. Statements & Terminators
impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            StatementKind::Assign(place, rval) => write!(f, "{} = {};", place, rval),
            StatementKind::AssignGlobal(name, rval) => write!(f, "global::{} = {};", name, rval),
            StatementKind::DefineGlobal(name, ty, rval) => write!(f, "global::{} : {} = {};", name, ty, rval),
            StatementKind::AssertType(op, ty) => write!(f, "assert_type {} == {}", op, ty),
        }
    }
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Terminator::Goto { target } => write!(f, "goto -> bb{};", target.0),
            Terminator::SwitchInt { discriminant, true_target, false_target } => {
                write!(f, "switch_int({}) -> [true: bb{}, false: bb{}];", discriminant, true_target.0, false_target.0)
            }

            Terminator::Call { callee, args, kwargs, destination, target } => {
                write!(f, "{} = call {}(", destination, callee)?;

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

                write!(f, ") -> bb{};", target.0)
            }

            Terminator::MethodCall { object, method_name, args, kwargs, destination, target } => {
                write!(f, "{} = call {}.{}(", destination, object, method_name)?;

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

                write!(f, ") -> bb{};", target.0)
            }

            Terminator::Return => write!(f, "return;"),
            Terminator::Unreachable => write!(f, "unreachable;"),
        }
    }
}

// 3. Values & Operands
impl fmt::Display for Rvalue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rvalue::Use(op) => write!(f, "{}", op),
            Rvalue::BinaryOp(op, lhs, rhs) => write!(f, "{} {} {}", lhs, op, rhs),
            Rvalue::UnaryOp(op, operand) => write!(f, "{}{}", op, operand),
            Rvalue::ListInit(elements) => {
                write!(f, "[")?;
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", elem)?;
                }
                write!(f, "]")
            }

            Rvalue::Import(module) => {
                write!(f, "import {}", module)
            }

            Rvalue::FunctionDef(function) => {
                let func_str = function.to_string();
                let mut lines = func_str.lines();

                if let Some(first) = lines.next() {
                    write!(f, "{}", first)?;
                }

                for line in lines {
                    write!(f, "\n        {}", line)?;
                }

                Ok(())
            }        

            Rvalue::FormatString(ops) => {
                write!(f, "format_string(")?;

                for (i, op) in ops.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", op)?;
                }

                write!(f, ")")
            }

            Rvalue::DictInit(keys, values) => {
                write!(f, "{{")?;
                for (i, (k, v)) in keys.iter().zip(values.iter()).enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }

            Rvalue::Length(op) => write!(f, "length({})", op),
            Rvalue::ListAppend(list, item) => write!(f, "{}.append({})", list, item),
        }
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operand::Copy(place) => write!(f, "{}", place),
            Operand::Const(c) => write!(f, "const {}", c),
            Operand::Static(name) => write!(f, "global::{}", name),
        }
    }
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_{}", self.local.0)?;
        for proj in &self.projection {
            match proj {
                ProjectionElem::Property(name) => write!(f, ".{}", name)?,
                ProjectionElem::Index(local) => write!(f, "[_{}]", local.0)?,
            }
        }
        Ok(())
    }
}

// ------------------------------------------------------------------------
// STRING COLLECTION TRAIT FOR ALLOC DUMPS
// ------------------------------------------------------------------------

/// Private helper trait to scan the MIR tree and find all strings that 
/// the Compiler is going to allocate into the chunk constant pool.
trait CollectStrings {
    fn collect_strings(&self, out: &mut Vec<String>);
}

impl CollectStrings for MIRProgram {
    fn collect_strings(&self, out: &mut Vec<String>) {
        for f in &self.functions {
            f.collect_strings(out);
        }
    }
}

impl CollectStrings for MIRFunction {
    fn collect_strings(&self, out: &mut Vec<String>) {
        for bb in &self.basic_blocks {
            for stmt in &bb.statements {
                stmt.kind.collect_strings(out);
            }
            bb.terminator.collect_strings(out);
        }
    }
}

impl CollectStrings for StatementKind {
    fn collect_strings(&self, out: &mut Vec<String>) {
        match self {
            StatementKind::Assign(_, rval)
            | StatementKind::AssignGlobal(_, rval)
            | StatementKind::DefineGlobal(_, _, rval) => rval.collect_strings(out),
            StatementKind::AssertType(op, _) => op.collect_strings(out),
        }
    }
}

impl CollectStrings for Rvalue {
    fn collect_strings(&self, out: &mut Vec<String>) {
        match self {
            Rvalue::Use(op) | Rvalue::UnaryOp(_, op) | Rvalue::Length(op) => op.collect_strings(out),
            Rvalue::BinaryOp(_, lhs, rhs) | Rvalue::ListAppend(lhs, rhs) => {
                lhs.collect_strings(out);
                rhs.collect_strings(out);
            }
            Rvalue::ListInit(ops) | Rvalue::FormatString(ops) => {
                for op in ops { op.collect_strings(out); }
            }
            Rvalue::DictInit(keys, vals) => {
                for op in keys { op.collect_strings(out); }
                for op in vals { op.collect_strings(out); }
            }
            Rvalue::Import(module) => {
                if !out.contains(module) {
                    out.push(module.clone());
                }
            }
            Rvalue::FunctionDef(f) => f.collect_strings(out),
        }
    }
}

impl CollectStrings for Terminator {
    fn collect_strings(&self, out: &mut Vec<String>) {
        match self {
            Terminator::SwitchInt { discriminant, .. } => discriminant.collect_strings(out),
            Terminator::Call { callee, args, kwargs, .. } => {
                callee.collect_strings(out);
                for op in args { op.collect_strings(out); }
                // Kwarg keys are allocated as string constants by the compiler!
                for (name, op) in kwargs {
                    if !out.contains(name) { out.push(name.clone()); }
                    op.collect_strings(out);
                }
            }
            Terminator::MethodCall { object, method_name, args, kwargs, .. } => {
                object.collect_strings(out);
                // Method names are allocated as string constants by the compiler!
                if !out.contains(method_name) { out.push(method_name.clone()); }
                for op in args { op.collect_strings(out); }
                for (name, op) in kwargs {
                    if !out.contains(name) { out.push(name.clone()); }
                    op.collect_strings(out);
                }
            }
            _ => {}
        }
    }
}

impl CollectStrings for Operand {
    fn collect_strings(&self, out: &mut Vec<String>) {
        // Find raw string literals
        if let Operand::Const(Constant::String(s)) = self {
            if !out.contains(s) {
                out.push(s.clone());
            }
        }
    }
}
