use std::collections::{HashMap, HashSet, VecDeque};
use crate::mir::*;
use vir::vir::VIRBinOp; // Adjust import based on where your VIRBinOp lives
use vir::context::Constant;

pub struct Optimizer;

impl Optimizer {

    pub fn optimize(program: &mut MIRProgram) {
        for function in &mut program.functions {
            for _ in 0..4 {
                Self::fold_constants(function);
                Self::propagate_constants(function);
            }

            Self::simplify_branches(function);
            Self::eliminate_dead_blocks(function);
            Self::eliminate_dead_stores(function);
        }
    }

    /// PASS 1: Constant Propagation
    /// Finds variables assigned a constant exactly once, and replaces all 
    /// downstream uses of that variable with the raw constant.
    fn propagate_constants(func: &mut MIRFunction) {
        let mut assign_counts = HashMap::new();
        let mut const_values = HashMap::new();

        // Step A: Count assignments and record constants
        for block in &func.basic_blocks {
            for stmt in &block.statements {
                if let StatementKind::Assign(place, rval) = &stmt.kind {
                    // We only care about base locals, not index assignments like arr[0] = 5
                    if place.projection.is_empty() {
                        let count = assign_counts.entry(place.local).or_insert(0);
                        *count += 1;

                        if let Rvalue::Use(Operand::Const(c)) = rval {
                            const_values.insert(place.local, c.clone());
                        }
                    } else {
                        // If it is mutated via index, it's disqualified
                        assign_counts.entry(place.local).and_modify(|c| *c += 1).or_insert(1);
                    }
                }
            }
        }

        // Step B: Filter out variables that were assigned more than once
        let safe_constants: HashMap<LocalID, Constant> = const_values
            .into_iter()
            .filter(|(local, _)| assign_counts.get(local) == Some(&1))
            .collect();

        if safe_constants.is_empty() { return; }

        // Step C: Replace uses of these variables across the entire function
        for block in &mut func.basic_blocks {
            // 1. Replace inside Statements
            for stmt in &mut block.statements {
                if let StatementKind::Assign(_, rval) = &mut stmt.kind {
                    Self::replace_operands_in_rvalue(rval, &safe_constants);
                }
            }

            // 2. Replace inside Terminators (Function calls, Switch blocks)
            match &mut block.terminator {
                Terminator::SwitchInt { discriminant, .. } => {
                    Self::replace_operand(discriminant, &safe_constants);
                }
                Terminator::Call { args, .. } | Terminator::MethodCall { args, .. } => {
                    for arg in args {
                        Self::replace_operand(arg, &safe_constants);
                    }
                }
                _ => {}
            }
        }
    }

    /// PASS 1: Constant Folding
    /// Looks for expressions like `let _1 = const 2 + const 3;` 
    /// and replaces them with `let _1 = const 5;`
    fn fold_constants(function: &mut MIRFunction) {
        for block in &mut function.basic_blocks {
            for stmt in &mut block.statements {
                if let StatementKind::Assign(_place, rval) = &mut stmt.kind {
                    if let Rvalue::BinaryOp(op, Operand::Const(c1), Operand::Const(c2)) = rval {
                        if let (Constant::Int(i1), Constant::Int(i2)) = (c1, c2) {
                            let folded = match op {
                                VIRBinOp::Add => Some(Constant::Int(*i1 + *i2)),
                                VIRBinOp::Sub => Some(Constant::Int(*i1 - *i2)),
                                VIRBinOp::Mul => Some(Constant::Int(*i1 * *i2)),
                                VIRBinOp::Div if *i2 != 0 => Some(Constant::Int(*i1 / *i2)),
                                _ => None,
                            };

                            if let Some(new_const) = folded {
                                *rval = Rvalue::Use(Operand::Const(new_const));
                            }
                        }
                    }
                }
            }
        }
    }

    /// PASS 2: Dead Block Elimination
    /// Traces all reachable blocks from bb0, deletes unreachable ones, 
    /// and remaps all goto/jump targets to match the new block indices.
    fn eliminate_dead_blocks(function: &mut MIRFunction) {
        if function.basic_blocks.is_empty() { return; }

        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();
        
        // Always start tracing from Basic Block 0
        queue.push_back(0); 

        // 1. Trace reachability
        while let Some(bb_idx) = queue.pop_front() {
            if !reachable.insert(bb_idx) {
                continue; // Already visited
            }

            let block = &function.basic_blocks[bb_idx];
            match &block.terminator {
                Terminator::Goto { target } => queue.push_back(target.0),
                Terminator::SwitchInt { true_target, false_target, .. } => {
                    queue.push_back(true_target.0);
                    queue.push_back(false_target.0);
                }

                Terminator::Call { target, .. } | Terminator::MethodCall { target, .. } => {
                    queue.push_back(target.0);
                }

                Terminator::Return | Terminator::Unreachable => {}
            }
        }

        // If everything is reachable, nothing to eliminate!
        if reachable.len() == function.basic_blocks.len() {
            return;
        }

        // 2. Build the remapping dictionary (Old Index -> New Index)
        let mut new_blocks = Vec::new();
        let mut old_to_new = HashMap::new();

        for old_idx in 0..function.basic_blocks.len() {
            if reachable.contains(&old_idx) {
                old_to_new.insert(old_idx, new_blocks.len());
                new_blocks.push(function.basic_blocks[old_idx].clone());
            }
        }

        // 3. Remap all terminators to point to the new, compacted block indices
        for block in &mut new_blocks {
            match &mut block.terminator {
                Terminator::Goto { target } => {
                    target.0 = old_to_new[&target.0];
                }

                Terminator::SwitchInt { true_target, false_target, .. } => {
                    true_target.0 = old_to_new[&true_target.0];
                    false_target.0 = old_to_new[&false_target.0];
                }

                Terminator::Call { target, .. } | Terminator::MethodCall { target, .. } => {
                    target.0 = old_to_new[&target.0];
                }

                _ => {}
            }
        }

        // 4. Overwrite the function's basic blocks
        function.basic_blocks = new_blocks;
    }

    fn eliminate_dead_stores(func: &mut MIRFunction) {
        let mut loop_changed = true;
        
        while loop_changed {
            loop_changed = false;
            let mut read_counts: HashMap<LocalID, usize> = HashMap::new();

            // 1. Tally up every time a variable is read
            for block in &func.basic_blocks {
                for stmt in &block.statements {
                    if let StatementKind::Assign(place, rval) = &stmt.kind {
                        Self::count_reads_in_rvalue(rval, &mut read_counts);

                        for proj in &place.projection {
                            if let ProjectionElem::Index(idx_local) = proj {
                                *read_counts.entry(*idx_local).or_insert(0) += 1;
                            }
                        }
                    }
                }

                match &block.terminator {
                    Terminator::SwitchInt { discriminant, .. } => Self::count_reads(discriminant, &mut read_counts),
                    Terminator::Call { args, .. } | Terminator::MethodCall { args, .. } => {
                        for arg in args { Self::count_reads(arg, &mut read_counts); }
                    }
                    _ => {}
                }
            }

            // 2. Delete assignments if the variable is never read!
            for block in &mut func.basic_blocks {
                let original_len = block.statements.len();
                
                block.statements.retain(|stmt| {
                    if let StatementKind::Assign(place, rval) = &stmt.kind {
                        // always keep returns, subscripts and imports
                        if place.local.0 == 0 || !place.projection.is_empty() || matches!(rval, Rvalue::Import(_)) 
                        {
                            return true;
                        }

                        // otherwise, only keep the assignment if it gets read later
                        read_counts.get(&place.local).unwrap_or(&0) > &0
                    } else {
                        true
                    }
                });

                // if we deleted something, loop again because downstream variables might now be dead
                if block.statements.len() < original_len {
                    loop_changed = true;
                }
            }
        }
    }

    fn simplify_branches(function: &mut MIRFunction) {
        for block in &mut function.basic_blocks {
            if let Terminator::SwitchInt { discriminant, true_target, false_target } = &block.terminator {
                if let Operand::Const(Constant::Bool(val)) = discriminant {
                    let definitive_target = if *val { *true_target } else { *false_target };
                    block.terminator = Terminator::Goto { target: definitive_target };
                }
            }
        }
    }

    
    // --- HELPER: Recursively replaces variables with constants in Rvalues ---
    fn replace_operands_in_rvalue(rval: &mut Rvalue, safe_constants: &HashMap<LocalID, Constant>) {
        match rval {
            Rvalue::Use(op) => Self::replace_operand(op, safe_constants),
            Rvalue::BinaryOp(_, lhs, rhs) => {
                Self::replace_operand(lhs, safe_constants);
                Self::replace_operand(rhs, safe_constants);
            }
            Rvalue::UnaryOp(_, op) => Self::replace_operand(op, safe_constants),
            Rvalue::ListInit(ops) | Rvalue::FormatString(ops) => {
                for op in ops { Self::replace_operand(op, safe_constants); }
            }
            Rvalue::DictInit(keys, vals) => {
                for k in keys { Self::replace_operand(k, safe_constants); }
                for v in vals { Self::replace_operand(v, safe_constants); }
            }
            Rvalue::ListAppend(list, item) => {
                Self::replace_operand(list, safe_constants);
                Self::replace_operand(item, safe_constants);
            }
            Rvalue::Length(op) => Self::replace_operand(op, safe_constants),
            _ => {}
        }
    }

    // --- HELPER: Checks if an Operand is a variable that should be replaced ---
    fn replace_operand(op: &mut Operand, safe_constants: &HashMap<LocalID, Constant>) {
        if let Operand::Copy(place) = op {
            if place.projection.is_empty() {
                if let Some(c) = safe_constants.get(&place.local) {
                    *op = Operand::Const(c.clone());
                }
            }
        }
    }

    fn count_reads_in_rvalue(rval: &Rvalue, counts: &mut HashMap<LocalID, usize>) {
        match rval {
            Rvalue::Use(op) | Rvalue::UnaryOp(_, op) | Rvalue::Length(op) => Self::count_reads(op, counts),
            Rvalue::BinaryOp(_, lhs, rhs) | Rvalue::ListAppend(lhs, rhs) => {
                Self::count_reads(lhs, counts);
                Self::count_reads(rhs, counts);
            }
            Rvalue::ListInit(ops) | Rvalue::FormatString(ops) => {
                for op in ops { Self::count_reads(op, counts); }
            }
            Rvalue::DictInit(keys, vals) => {
                for k in keys { Self::count_reads(k, counts); }
                for v in vals { Self::count_reads(v, counts); }
            }
            _ => {}
        }
    }

    fn count_reads(op: &Operand, counts: &mut HashMap<LocalID, usize>) {
        if let Operand::Copy(place) = op {
            *counts.entry(place.local).or_insert(0) += 1;

            for proj in &place.projection {
                if let ProjectionElem::Index(idx_local) = proj {
                    *counts.entry(*idx_local).or_insert(0) += 1;
                }
            }
        }
    }
}
