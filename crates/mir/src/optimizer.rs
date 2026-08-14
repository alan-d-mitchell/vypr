use std::collections::{HashMap, HashSet, VecDeque};
use crate::mir::*;
use vir::context::Constant;
use vir::vir::{VIRBinOp, VIRUnaryOp};

pub struct Optimizer;

impl Optimizer {
    pub fn optimize(program: &mut MIRProgram) {
        let mut changed = true;
        let mut iterations = 0;

        while changed && iterations < 20 {
            changed = false;
    
            for function in &mut program.functions {
                changed |= Self::simplify_dataflow(function);
                changed |= Self::eliminate_common_subexpressions(function);
                changed |= Self::simplify_branches(function);
                changed |= Self::eliminate_dead_blocks(function); 
                changed |= Self::eliminate_dead_stores(function);
                changed |= Self::merge_blocks(function);
            }

            iterations += 1;
        }
    }

    /// Evaluates expressions after propagation until a fixed point is reached 
    /// within the same function. This chains transitive constants (z = y, y = x, x = 5)
    /// and immediately folds the resulting math.
    fn simplify_dataflow(function: &mut MIRFunction) -> bool {
        let mut overall_changed = false;
        let mut local_changed = true;
        let mut iterations = 0;
        
        while local_changed && iterations < 100 {
            local_changed = false;
            
            local_changed |= Self::propagate_copies(function);
            local_changed |= Self::propagate_constants(function);
            local_changed |= Self::fold_constants(function);
            
            if local_changed {
                overall_changed = true;
            }

            iterations += 1;
        }
        
        overall_changed
    }

    // ========================================================================
    // PHASE 1: LOCAL CLEANUP
    // ========================================================================

    fn merge_blocks(function: &mut MIRFunction) -> bool {
        let mut preds: HashMap<usize, Vec<usize>> = HashMap::new();
        
        for (i, block) in function.basic_blocks.iter().enumerate() {
            match &block.terminator {
                Terminator::Goto { target } => preds.entry(target.0).or_default().push(i),
                Terminator::SwitchInt { true_target, false_target, .. } => {
                    preds.entry(true_target.0).or_default().push(i);
                    preds.entry(false_target.0).or_default().push(i);
                }
                Terminator::Call { target, .. } | Terminator::MethodCall { target, .. } => {
                    preds.entry(target.0).or_default().push(i);
                }
                _ => {}
            }
        }

        for i in 0..function.basic_blocks.len() {
            if let Terminator::Goto { target } = &function.basic_blocks[i].terminator {
                let target_idx = target.0;
                
                if target_idx == i { continue; }

                if let Some(target_preds) = preds.get(&target_idx) {
                    if target_preds.len() == 1 && target_preds[0] == i {
                        let b_stmts = function.basic_blocks[target_idx].statements.clone();
                        let b_term = function.basic_blocks[target_idx].terminator.clone();
                        
                        let block_a = &mut function.basic_blocks[i];
                        block_a.statements.extend(b_stmts);
                        block_a.terminator = b_term;
                        
                        let block_b = &mut function.basic_blocks[target_idx];
                        block_b.statements.clear();
                        block_b.terminator = Terminator::Unreachable;
                        
                        return true; 
                    }
                }
            }
        }
        false
    }

    fn propagate_copies(function: &mut MIRFunction) -> bool {
        let mut assign_counts = HashMap::new();
        let mut copy_values = HashMap::new();

        for block in &function.basic_blocks {
            for stmt in &block.statements {
                if let StatementKind::Assign(place, rval) = &stmt.kind {
                    assign_counts.entry(place.local).and_modify(|c| *c += 1).or_insert(1);
                    
                    if place.projection.is_empty() {
                        if let Rvalue::Use(Operand::Copy(src)) = rval {
                            if src.projection.is_empty() {
                                copy_values.insert(place.local, src.local);
                            }
                        }
                    }
                }
            }
        }

        let safe_copies: HashMap<LocalID, LocalID> = copy_values
            .into_iter()
            .filter(|(dest, src)| {
                let dest_count = assign_counts.get(dest).copied().unwrap_or(0);
                let src_count = assign_counts.get(src).copied().unwrap_or(0);
                dest_count == 1 && src_count <= 1
            })
            .collect();

        if safe_copies.is_empty() { return false; }
        
        let mut changed = false;

        for block in &mut function.basic_blocks {
            for stmt in &mut block.statements {
                if let StatementKind::Assign(_, rval) = &mut stmt.kind {
                    changed |= Self::replace_locals_in_rvalue(rval, &safe_copies);
                }
            }

            match &mut block.terminator {
                Terminator::SwitchInt { discriminant, .. } => {
                    changed |= Self::replace_local(discriminant, &safe_copies);
                }

                Terminator::Call { args, .. } => {
                    for arg in args {
                        changed |= Self::replace_local(arg, &safe_copies);
                    }
                }

                Terminator::MethodCall { object, args, .. } => {
                    changed |= Self::replace_local(object, &safe_copies);
                    for arg in args {
                        changed |= Self::replace_local(arg, &safe_copies);
                    }
                }

                _ => {}
            }
        }

        changed
    }

    fn propagate_constants(function: &mut MIRFunction) -> bool {
        let mut assign_counts = HashMap::new();
        let mut const_values = HashMap::new();

        for block in &function.basic_blocks {
            for stmt in &block.statements {
                if let StatementKind::Assign(place, rval) = &stmt.kind {
                    if place.projection.is_empty() {
                        let count = assign_counts.entry(place.local).or_insert(0);
                        *count += 1;
                        if let Rvalue::Use(Operand::Const(c)) = rval {
                            const_values.insert(place.local, c.clone());
                        }
                    } else {
                        assign_counts.entry(place.local).and_modify(|c| *c += 1).or_insert(1);
                    }
                }
            }
        }

        let safe_constants: HashMap<LocalID, Constant> = const_values
            .into_iter()
            .filter(|(local, _)| assign_counts.get(local) == Some(&1))
            .collect();

        if safe_constants.is_empty() { return false; }
        let mut changed = false;

        for block in &mut function.basic_blocks {
            for stmt in &mut block.statements {
                if let StatementKind::Assign(_, rval) = &mut stmt.kind {
                    changed |= Self::replace_operands_in_rvalue(rval, &safe_constants);
                }
            }

            match &mut block.terminator {
                Terminator::SwitchInt { discriminant, .. } => {
                    changed |= Self::replace_operand(discriminant, &safe_constants);
                }

                Terminator::Call { args, .. } => {
                    for arg in args {
                        changed |= Self::replace_operand(arg, &safe_constants);
                    }
                }

                Terminator::MethodCall { object, args, .. } => {
                    changed |= Self::replace_operand(object, &safe_constants);
                    for arg in args {
                        changed |= Self::replace_operand(arg, &safe_constants);
                    }
                }

                _ => {}
            }
        }
        changed
    }

    fn fold_constants(function: &mut MIRFunction) -> bool {
        let mut changed = false;
        for block in &mut function.basic_blocks {
            for stmt in &mut block.statements {
                if let StatementKind::Assign(_place, rval) = &mut stmt.kind {
                    match rval {
                        Rvalue::BinaryOp(op, Operand::Const(c1), Operand::Const(c2)) => {
                            let folded = match (op, c1, c2) {
                                // Integer Arithmetic
                                (VIRBinOp::Add, Constant::Int(i1), Constant::Int(i2)) => Some(Constant::Int(*i1 + *i2)),
                                (VIRBinOp::Sub, Constant::Int(i1), Constant::Int(i2)) => Some(Constant::Int(*i1 - *i2)),
                                (VIRBinOp::Mul, Constant::Int(i1), Constant::Int(i2)) => Some(Constant::Int(*i1 * *i2)),
                                (VIRBinOp::Div, Constant::Int(i1), Constant::Int(i2)) if *i2 != 0 => Some(Constant::Int(*i1 / *i2)),
                                (VIRBinOp::Mod, Constant::Int(i1), Constant::Int(i2)) if *i2 != 0 => Some(Constant::Int(*i1 % *i2)),
                                (VIRBinOp::FloorDiv, Constant::Int(i1), Constant::Int(i2)) if *i2 != 0 => Some(Constant::Int(*i1 / *i2)),
                                (VIRBinOp::Power, Constant::Int(i1), Constant::Int(i2)) => {
                                    if *i2 >= 0 && *i2 <= u32::MAX as i64 { Some(Constant::Int(i1.pow(*i2 as u32))) } else { None }
                                }
                                
                                // Float Arithmetic
                                (VIRBinOp::Add, Constant::Float(f1), Constant::Float(f2)) => Some(Constant::Float(*f1 + *f2)),
                                (VIRBinOp::Sub, Constant::Float(f1), Constant::Float(f2)) => Some(Constant::Float(*f1 - *f2)),
                                (VIRBinOp::Mul, Constant::Float(f1), Constant::Float(f2)) => Some(Constant::Float(*f1 * *f2)),
                                (VIRBinOp::Div, Constant::Float(f1), Constant::Float(f2)) if *f2 != 0.0 => Some(Constant::Float(*f1 / *f2)),
                                (VIRBinOp::Power, Constant::Float(f1), Constant::Float(f2)) => Some(Constant::Float(f1.powf(*f2))),

                                // Integer Comparisons
                                (VIRBinOp::Eq, Constant::Int(i1), Constant::Int(i2)) => Some(Constant::Bool(i1 == i2)),
                                (VIRBinOp::Ne, Constant::Int(i1), Constant::Int(i2)) => Some(Constant::Bool(i1 != i2)),
                                (VIRBinOp::Lt, Constant::Int(i1), Constant::Int(i2)) => Some(Constant::Bool(i1 < i2)),
                                (VIRBinOp::Le, Constant::Int(i1), Constant::Int(i2)) => Some(Constant::Bool(i1 <= i2)),
                                (VIRBinOp::Gt, Constant::Int(i1), Constant::Int(i2)) => Some(Constant::Bool(i1 > i2)),
                                (VIRBinOp::Ge, Constant::Int(i1), Constant::Int(i2)) => Some(Constant::Bool(i1 >= i2)),

                                // Float Comparisons
                                (VIRBinOp::Eq, Constant::Float(f1), Constant::Float(f2)) => Some(Constant::Bool(f1 == f2)),
                                (VIRBinOp::Ne, Constant::Float(f1), Constant::Float(f2)) => Some(Constant::Bool(f1 != f2)),
                                (VIRBinOp::Lt, Constant::Float(f1), Constant::Float(f2)) => Some(Constant::Bool(f1 < f2)),
                                (VIRBinOp::Le, Constant::Float(f1), Constant::Float(f2)) => Some(Constant::Bool(f1 <= f2)),
                                (VIRBinOp::Gt, Constant::Float(f1), Constant::Float(f2)) => Some(Constant::Bool(f1 > f2)),
                                (VIRBinOp::Ge, Constant::Float(f1), Constant::Float(f2)) => Some(Constant::Bool(f1 >= f2)),

                                // Boolean Logic
                                (VIRBinOp::And, Constant::Bool(b1), Constant::Bool(b2)) => Some(Constant::Bool(*b1 && *b2)),
                                (VIRBinOp::Or, Constant::Bool(b1), Constant::Bool(b2)) => Some(Constant::Bool(*b1 || *b2)),
                                (VIRBinOp::Eq, Constant::Bool(b1), Constant::Bool(b2)) => Some(Constant::Bool(b1 == b2)),
                                (VIRBinOp::Ne, Constant::Bool(b1), Constant::Bool(b2)) => Some(Constant::Bool(b1 != b2)),

                                _ => None,
                            };

                            if let Some(new_const) = folded {
                                *rval = Rvalue::Use(Operand::Const(new_const));
                                changed = true;
                            }
                        }

                        Rvalue::UnaryOp(op, Operand::Const(c)) => {
                            let folded = match (op, c) {
                                (VIRUnaryOp::Not, Constant::Bool(b)) => Some(Constant::Bool(!*b)),
                                (VIRUnaryOp::Neg, Constant::Int(i)) => Some(Constant::Int(-*i)),
                                (VIRUnaryOp::Neg, Constant::Float(f)) => Some(Constant::Float(-*f)),
                                _ => None,
                            };
                            
                            if let Some(new_const) = folded {
                                *rval = Rvalue::Use(Operand::Const(new_const));
                                changed = true;
                            }
                        }
                        
                        _ => {}
                    }
                }
            }
        }
        changed
    }

    fn eliminate_dead_blocks(function: &mut MIRFunction) -> bool {
        if function.basic_blocks.is_empty() { return false; }

        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(0); 

        while let Some(bb_idx) = queue.pop_front() {
            if !reachable.insert(bb_idx) { continue; }
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

        if reachable.len() == function.basic_blocks.len() {
            return false;
        }

        let mut new_blocks = Vec::new();
        let mut old_to_new = HashMap::new();

        for old_idx in 0..function.basic_blocks.len() {
            if reachable.contains(&old_idx) {
                old_to_new.insert(old_idx, new_blocks.len());
                new_blocks.push(function.basic_blocks[old_idx].clone());
            }
        }

        for block in &mut new_blocks {
            match &mut block.terminator {
                Terminator::Goto { target } => target.0 = old_to_new[&target.0],
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

        function.basic_blocks = new_blocks;
        true
    }

    fn eliminate_dead_stores(function: &mut MIRFunction) -> bool {
        let mut loop_changed = false;
        let mut overall_changed = false;
        
        loop {
            loop_changed = false;
            let mut read_counts: HashMap<LocalID, usize> = HashMap::new();

            for block in &function.basic_blocks {
                for stmt in &block.statements {
                    match &stmt.kind {
                        StatementKind::Assign(place, rval) => {
                            Self::count_reads_in_rvalue(rval, &mut read_counts);
                            for proj in &place.projection {
                                if let ProjectionElem::Index(idx_local) = proj {
                                    *read_counts.entry(*idx_local).or_insert(0) += 1;
                                }
                            }
                        }
                        StatementKind::DefineGlobal(_, _, rval) | StatementKind::AssignGlobal(_, rval) => {
                            Self::count_reads_in_rvalue(rval, &mut read_counts);
                        }

                        StatementKind::AssertType(op, _) => {
                            Self::count_reads(op, &mut read_counts);
                        }
                    }
                }

                match &block.terminator {
                    Terminator::SwitchInt { discriminant, .. } => Self::count_reads(discriminant, &mut read_counts),
                    Terminator::Call { args, destination, .. } => {
                        for arg in args { Self::count_reads(arg, &mut read_counts); }
                        for proj in &destination.projection {
                            if let ProjectionElem::Index(idx_local) = proj {
                                *read_counts.entry(*idx_local).or_insert(0) += 1;
                            }
                        }
                    }

                    Terminator::MethodCall { object, args, destination, .. } => {
                        Self::count_reads(object, &mut read_counts);

                        for arg in args {
                            Self::count_reads(arg, &mut read_counts);
                        }

                        for projection in &destination.projection {
                            if let ProjectionElem::Index(idx_local) = projection {
                                *read_counts.entry(*idx_local).or_insert(0) += 1;
                            }
                        }
                    }

                    _ => {}
                }
            }

            for block in &mut function.basic_blocks {
                let original_len = block.statements.len();
                
                block.statements.retain(|stmt| {
                    match &stmt.kind {
                        StatementKind::Assign(place, rval) => {
                            // ALWAYS keep the return value, subscript assignments, and imports!
                            if place.local.0 == 0 || !place.projection.is_empty() || matches!(rval, Rvalue::Import(_)) { 
                                return true; 
                            }
                            read_counts.get(&place.local).unwrap_or(&0) > &0
                        }

                        StatementKind::DefineGlobal(_, _, _) | StatementKind::AssignGlobal(_, _) => {
                            true
                        }

                        StatementKind::AssertType(_, _) => {
                            true
                        }
                    }
                });

                if block.statements.len() < original_len {
                    loop_changed = true;
                    overall_changed = true;
                }
            }
            
            if !loop_changed { break; }
        }

        overall_changed
    }

    fn simplify_branches(function: &mut MIRFunction) -> bool {
        let mut changed = false;
        for block in &mut function.basic_blocks {
            if let Terminator::SwitchInt { discriminant, true_target, false_target } = &block.terminator {
                if let Operand::Const(Constant::Bool(val)) = discriminant {
                    let definitive_target = if *val { *true_target } else { *false_target };
                    block.terminator = Terminator::Goto { target: definitive_target };
                    changed = true;
                }
            }
        }
        changed
    }

    fn eliminate_common_subexpressions(func: &mut MIRFunction) -> bool {
        let mut changed = false;

        for block in &mut func.basic_blocks {
            let mut available: Vec<(Rvalue, LocalID)> = Vec::new();

            for stmt in &mut block.statements {
                if let StatementKind::Assign(place, rval) = &mut stmt.kind {
                    if Self::is_cse_candidate(rval) {
                        
                        let mut found = None;
                        for (avail_rval, avail_local) in &available {
                            if Self::is_rvalue_eq(rval, avail_rval) {
                                found = Some(*avail_local);
                                break;
                            }
                        }

                        if let Some(cached_local) = found {
                            *rval = Rvalue::Use(Operand::Copy(Place {
                                local: cached_local,
                                projection: vec![],
                            }));
                            changed = true;
                        } else {
                            if place.projection.is_empty() {
                                available.push((rval.clone(), place.local));
                            }
                        }
                    }

                    let mutated = place.local;
                    available.retain(|(avail_rval, avail_local)| {
                        *avail_local != mutated && !Self::rvalue_uses_local(avail_rval, mutated)
                    });
                }
            }
        }
        changed
    }

    // ========================================================================
    // HELPERS
    // ========================================================================

    fn replace_operands_in_rvalue(rval: &mut Rvalue, safe_constants: &HashMap<LocalID, Constant>) -> bool {
        let mut changed = false;
        match rval {
            Rvalue::Use(op) | Rvalue::UnaryOp(_, op) | Rvalue::Length(op) => {
                changed |= Self::replace_operand(op, safe_constants);
            }
            Rvalue::BinaryOp(_, lhs, rhs) | Rvalue::ListAppend(lhs, rhs) => {
                changed |= Self::replace_operand(lhs, safe_constants);
                changed |= Self::replace_operand(rhs, safe_constants);
            }
            Rvalue::ListInit(ops) | Rvalue::FormatString(ops) => {
                for op in ops { changed |= Self::replace_operand(op, safe_constants); }
            }
            Rvalue::DictInit(keys, vals) => {
                for k in keys { changed |= Self::replace_operand(k, safe_constants); }
                for v in vals { changed |= Self::replace_operand(v, safe_constants); }
            }
            Rvalue::Import(_) | Rvalue::FunctionDef(_) => {}
        }
        changed
    }

    fn replace_operand(op: &mut Operand, safe_constants: &HashMap<LocalID, Constant>) -> bool {
        if let Operand::Copy(place) = op {
            if place.projection.is_empty() {
                if let Some(c) = safe_constants.get(&place.local) {
                    *op = Operand::Const(c.clone());
                    return true;
                }
            }
        }
        false
    }

    fn replace_locals_in_rvalue(rval: &mut Rvalue, safe_copies: &HashMap<LocalID, LocalID>) -> bool {
        let mut changed = false;
        match rval {
            Rvalue::Use(op) | Rvalue::UnaryOp(_, op) | Rvalue::Length(op) => {
                changed |= Self::replace_local(op, safe_copies);
            }
            Rvalue::BinaryOp(_, lhs, rhs) | Rvalue::ListAppend(lhs, rhs) => {
                changed |= Self::replace_local(lhs, safe_copies);
                changed |= Self::replace_local(rhs, safe_copies);
            }
            Rvalue::ListInit(ops) | Rvalue::FormatString(ops) => {
                for op in ops { changed |= Self::replace_local(op, safe_copies); }
            }
            Rvalue::DictInit(keys, vals) => {
                for k in keys { changed |= Self::replace_local(k, safe_copies); }
                for v in vals { changed |= Self::replace_local(v, safe_copies); }
            }
            Rvalue::Import(_) | Rvalue::FunctionDef(_) => {}
        }
        changed
    }

    fn replace_local(op: &mut Operand, safe_copies: &HashMap<LocalID, LocalID>) -> bool {
        if let Operand::Copy(place) = op {
            if place.projection.is_empty() {
                if let Some(&src_local) = safe_copies.get(&place.local) {
                    place.local = src_local;
                    return true;
                }
            }
        }
        false
    }

    fn count_reads_in_rvalue(rval: &Rvalue, counts: &mut HashMap<LocalID, usize>) {
        match rval {
            Rvalue::Use(op) | Rvalue::UnaryOp(_, op) | Rvalue::Length(op) => {
                Self::count_reads(op, counts);
            }
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
            Rvalue::Import(_) | Rvalue::FunctionDef(_) => {}
        }
    }

    fn count_reads(op: &Operand, counts: &mut HashMap<LocalID, usize>) {
        if let Operand::Copy(place) = op {
            *counts.entry(place.local).or_insert(0) += 1;
            for proj in &place.projection {
                if let ProjectionElem::Index(idx_local) = proj {
                    *counts.entry(idx_local.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    fn remap_locals_and_blocks(blocks: &mut Vec<crate::mir::BasicBlock>, local_offset: usize, block_offset: usize, destination_local: LocalID) {
        let shift_local = |local: &mut LocalID| {
            if local.0 == 0 {
                *local = destination_local;
            } else {
                local.0 += local_offset - 1; 
            }
        };

        for block in blocks.iter_mut() {
            for stmt in &mut block.statements {
                if let StatementKind::Assign(place, rval) = &mut stmt.kind {
                    shift_local(&mut place.local);
                    Self::shift_locals_in_rvalue(rval, &shift_local);
                }
            }

            match &mut block.terminator {
                Terminator::Goto { target } => target.0 += block_offset,
                Terminator::SwitchInt { discriminant, true_target, false_target } => {
                    Self::shift_locals_in_operand(discriminant, &shift_local);
                    true_target.0 += block_offset;
                    false_target.0 += block_offset;

                }
                Terminator::Call { args, destination, target, .. } => {
                    for arg in args { Self::shift_locals_in_operand(arg, &shift_local); }
                    shift_local(&mut destination.local);
                    target.0 += block_offset;
                }

                Terminator::MethodCall { object, args, destination, target, .. } => {
                    Self::shift_locals_in_operand(object, &shift_local);
                    for arg in args { Self::shift_locals_in_operand(arg, &shift_local); }
                    shift_local(&mut destination.local);
                    target.0 += block_offset;
                }

                _ => {}
            }
        }
    }

    fn shift_locals_in_rvalue<F: Fn(&mut LocalID)>(rval: &mut Rvalue, shift: &F) {
        match rval {
            Rvalue::Use(op) | Rvalue::UnaryOp(_, op) | Rvalue::Length(op) => Self::shift_locals_in_operand(op, shift),
            Rvalue::BinaryOp(_, lhs, rhs) | Rvalue::ListAppend(lhs, rhs) => {
                Self::shift_locals_in_operand(lhs, shift);
                Self::shift_locals_in_operand(rhs, shift);
            }
            Rvalue::ListInit(ops) | Rvalue::FormatString(ops) => {
                for op in ops { Self::shift_locals_in_operand(op, shift); }
            }
            Rvalue::DictInit(keys, vals) => {
                for k in keys { Self::shift_locals_in_operand(k, shift); }
                for v in vals { Self::shift_locals_in_operand(v, shift); }
            }
            Rvalue::Import(_) | Rvalue::FunctionDef(_) => {}
        }
    }

    fn shift_locals_in_operand<F: Fn(&mut LocalID)>(op: &mut Operand, shift: &F) {
        if let Operand::Copy(place) = op {
            shift(&mut place.local);
        }
    }

    fn is_cse_candidate(rval: &Rvalue) -> bool {
        matches!(rval, Rvalue::BinaryOp(_, _, _) | Rvalue::UnaryOp(_, _))
    }

    fn is_rvalue_eq(a: &Rvalue, b: &Rvalue) -> bool {
        match (a, b) {
            (Rvalue::BinaryOp(op_a, l_a, r_a), Rvalue::BinaryOp(op_b, l_b, r_b)) => {
                op_a == op_b && Self::is_operand_eq(l_a, l_b) && Self::is_operand_eq(r_a, r_b)
            }
            (Rvalue::UnaryOp(op_a, o_a), Rvalue::UnaryOp(op_b, o_b)) => {
                op_a == op_b && Self::is_operand_eq(o_a, o_b)
            }
            _ => false,
        }
    }

    fn is_operand_eq(a: &Operand, b: &Operand) -> bool {
        match (a, b) {
            (Operand::Copy(p_a), Operand::Copy(p_b)) => p_a == p_b,
            (Operand::Const(c_a), Operand::Const(c_b)) => c_a == c_b,
            (Operand::Static(s_a), Operand::Static(s_b)) => s_a == s_b,
            _ => false,
        }
    }

    fn rvalue_uses_local(rval: &Rvalue, target: LocalID) -> bool {
        match rval {
            Rvalue::BinaryOp(_, l, r) | Rvalue::ListAppend(l, r) => {
                Self::operand_uses_local(l, target) || Self::operand_uses_local(r, target)
            }
            Rvalue::UnaryOp(_, op) | Rvalue::Use(op) | Rvalue::Length(op) => {
                Self::operand_uses_local(op, target)
            }
            Rvalue::ListInit(ops) | Rvalue::FormatString(ops) => {
                ops.iter().any(|op| Self::operand_uses_local(op, target))
            }
            Rvalue::DictInit(keys, vals) => {
                keys.iter().any(|op| Self::operand_uses_local(op, target)) || 
                vals.iter().any(|op| Self::operand_uses_local(op, target))
            }
            Rvalue::Import(_) | Rvalue::FunctionDef(_) => false
        }
    }

    fn operand_uses_local(op: &Operand, target: LocalID) -> bool {
        match op {
            Operand::Copy(p) => p.local == target,
            Operand::Const(_) | Operand::Static(_) => false,
        }
    }
}
