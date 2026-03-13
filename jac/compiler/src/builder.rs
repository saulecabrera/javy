//! Function builder.

use std::collections::{HashMap, HashSet};
use waffle::{
    Block, BlockTarget, Func, FunctionBody, Local, MemoryArg, Operator, Signature, Table,
    Terminator, Type, Value,
};

/// Function Builder.
#[derive(Default)]
pub struct FunctionBuilder {
    /// The current block.
    position: Option<Block>,
    /// The resulting function body.
    pub result: FunctionBody,
    /// The resulting function's signature.
    sig: Signature,
    /// Declaration of function locals.
    declared_locals: HashMap<Local, Type>,
    /// Sealed blocks.
    /// No further predecessors will be added to these blocks.
    sealed: HashSet<Block>,
    /// Phi functions that need to be completed.
    incomplete_phis: HashMap<Block, HashMap<Local, Value>>,
    /// Per-block snapshot of locals state.
    locals: HashMap<Block, HashMap<Local, Value>>,
    /// The exit block.
    exit: Block,
}

impl FunctionBuilder {
    /// Create a new [`FunctionBuilder`].
    pub fn new(sig: Signature) -> Self {
        Self {
            sig,
            ..Default::default()
        }
    }

    /// Add block parameters to the given block.
    pub fn add_blockparams(&mut self, block: Block, params: &[Type]) {
        for &param in params {
            self.result.add_blockparam(block, param);
        }
    }

    /// Get the signature handle.
    pub fn sig(&self) -> Signature {
        self.sig
    }

    /// Switch the current block to the given block.
    /// Records a snapshot of the state of the current block's locals.
    /// The current block is replaced with the target block and a
    /// fresh new state.
    pub fn switch_to_block(&mut self, target: Block) {
        assert!(self.locals.insert(target, HashMap::new()).is_none());
        // Replace the current block.
        self.position = Some(target);
    }

    /// Seal the current block.
    pub fn seal(&mut self, block: Block) {
        assert!(self.sealed.insert(block));
        // Grab all the incomplete phis for the current block and
        // calculate the operands.
        let placeholders = self.incomplete_phis.remove(&block).unwrap_or_default();
        for (local, value) in placeholders {
            self.update_blockparams(block, local, value);
        }
    }

    /// Set the exit block.
    pub fn exit(&mut self, block: Block) {
        self.exit = block;
    }

    /// Declares a function local.
    pub fn declare_local(&mut self, local: Local, ty: Type) {
        // Must be declared only once per function.
        assert!(self.declared_locals.insert(local, ty).is_none());
    }

    /// Define a given local in the current block.
    pub fn def_local(&mut self, local: Local, val: Value) {
        let current = self.unwrap_current_block();
        self.def_local_in(current, local, val);
    }

    /// Generic utility over [`Self::def_local`]
    fn def_local_in(&mut self, block: Block, local: Local, val: Value) {
        let locals = self.locals.entry(block).or_insert_with(|| HashMap::new());

        locals.insert(local, val);
    }

    fn is_sealed(&self, block: Block) -> bool {
        self.sealed.contains(&block)
    }

    /// Get the SSA value associated to the given `Local`.
    pub fn use_local(&mut self, local: Local) -> Value {
        self.use_local_rec(self.unwrap_current_block(), local)
    }

    fn unwrap_current_block(&self) -> Block {
        self.position.expect("No current block set")
    }

    /// Create an [`InstructionBuilder`].
    pub fn instr_builder(&mut self) -> InstructionBuilder<'_> {
        InstructionBuilder {
            block: self.unwrap_current_block(),
            func_builder: self,
        }
    }

    /// Recursively reads a local, starting at the given block.
    /// The algorithm is base on Simple and Efficient
    /// Construction of Static Single Assignment Form (2013). Drawing
    /// further inspiration from Waffle's frontend
    /// (https://github.com/bytecodealliance/waffle/blob/main/src/frontend.rs).
    fn use_local_rec(&mut self, block: Block, local: Local) -> Value {
        // According to the algorithm, we need to handle 4 cases:
        // 1. The local is defined at the given block.
        // 2. The given block is not sealed, therefore, we need to
        //    record the value placeholders.
        // 3. The block is sealed, with a single predecessor,
        //    therefore, we can query the local directly in the
        //    predecessor.
        // 4. General case: the block is sealed, with muliple
        //    predecessors, therefore we need to recursively read the
        //    value in all the predecessors.
        if let Some(locals) = self.locals.get(&block) {
            if let Some(&v) = locals.get(&local) {
                return v;
            }
        }

        let local_type = self.declared_locals[&local];

        // Not all predecessors are known.
        if !self.is_sealed(block) {
            let placeholder = self.result.add_placeholder(local_type);
            self.def_local_in(block, local, placeholder);
            let incomplete_phis_for_block = self
                .incomplete_phis
                .entry(block)
                .or_insert_with(|| HashMap::new());

            incomplete_phis_for_block.insert(local, placeholder);
            return placeholder;
        }

        // All predecessors are known.
        let block_def = &self.result.blocks[block];

        if block_def.preds.len() == 1 {
            return self.use_local_rec(block_def.preds[0], local);
        }

        // Define the placeholder before recursing(in `update_blockparams`) to avoid
        // infinite loops.
        let placeholder = self.result.add_placeholder(local_type);
        self.def_local_in(block, local, placeholder);

        // Multiple predecessors.
        self.update_blockparams(block, local, placeholder);

        placeholder
    }

    /// Update the blockparams for the given block's predecessors.
    /// Equivalent to `add_phi_operands` in the paper (see
    /// [`Self::use_local_rec`]).  but using block params instead to
    /// match Waffle's IR.
    fn update_blockparams(&mut self, block: Block, local: Local, val: Value) {
        let block_def = &self.result.blocks[block];
        let preds = block_def.preds.clone();

        let mut resolved_values = vec![];
        for pred in &preds {
            let resolved_value = self.use_local_rec(*pred, local);
            resolved_values.push(resolved_value);
        }

        match self.resolve_trivial_alias(val, &resolved_values) {
            Some(alias) => {
                self.result.set_alias(val, alias);
            }
            None => {
                // Swap the placeholder.
                self.result.replace_placeholder_with_blockparam(block, val);
                // Update predecessor targets to account for the resolved values.
                for (pred_index, (resolved_val, pred)) in
                    resolved_values.iter().zip(preds).enumerate()
                {
                    // Retrieve the position of the target block in
                    // the successors of the predecessor.
                    let terminator_target_index =
                        self.result.blocks[block].pos_in_pred_succ[pred_index];
                    // Update the target args.
                    self.result.blocks[pred].terminator.update_target(
                        terminator_target_index,
                        |target| {
                            target.args.push(*resolved_val);
                        },
                    );
                }
            }
        }
    }

    /// Resolves trivial alias, to keep the SSA minimal.
    /// According to the paper, a phi is trivial iff:
    ///   just references itself and one other value v any number of
    ///   times.
    fn resolve_trivial_alias(&mut self, val: Value, results: &[Value]) -> Option<Value> {
        let same = None;
        for result in results {
            // Self-reference or same value.
            if *result == val || same.is_some_and(|v| v == *result) {
                continue;
            }

            // Non-trivial: a different value exists.
            if same.is_some() {
                return None;
            }

            let same = Some(result);
        }

        if let Some(alias) = same {
            if self.result.resolve_alias(alias) != val {
                return same;
            }
            return None;
        }

        return None;
    }

    /// Sets an unconditional branch terminator for the current block.
    pub fn branch(&mut self, target: Block) {
        let terminator = Terminator::Br {
            target: BlockTarget {
                block: target,
                args: vec![],
            },
        };
        self.result
            .set_terminator(self.unwrap_current_block(), terminator);
    }

    /// Sets a conditional branch terminator for the current block.
    pub fn branch_if(&mut self, cond: Value, then: Block, alt: Block) {
        let terminator = Terminator::CondBr {
            cond,
            if_true: BlockTarget {
                block: then,
                args: vec![],
            },
            if_false: BlockTarget {
                block: alt,
                args: vec![],
            },
        };
        self.result
            .set_terminator(self.unwrap_current_block(), terminator);
    }
}

/// Instruction builder.
/// Provides convenience methods to add instructions to the current
/// [`FunctionBuilder`].
pub struct InstructionBuilder<'a> {
    /// The current block.
    block: Block,
    /// The function builder.
    func_builder: &'a mut FunctionBuilder,
}

impl<'a> InstructionBuilder<'a> {
    pub fn i32const(&mut self, value: u32) -> Value {
        let op = Operator::I32Const { value };
        self.func_builder
            .result
            .add_op(self.block, op, &[], &[Type::I32])
    }

    /// Inline a constant JSValue using the NaN boxing `JS_MKVAL` formula:
    /// `(tag << 32) | val`
    pub fn mkval(&mut self, tag: u64, val: u64) -> Value {
        self.i64const((tag << 32) | val)
    }

    pub fn i64const(&mut self, value: u64) -> Value {
        let op = Operator::I64Const { value };
        self.func_builder
            .result
            .add_op(self.block, op, &[], &[Type::I64])
    }

    pub fn i32eqz(&mut self, val: Value) -> Value {
        self.func_builder
            .result
            .add_op(self.block, Operator::I32Eqz, &[val], &[Type::I32])
    }

    pub fn i32ge_u(&mut self, lhs: Value, rhs: Value) -> Value {
        self.func_builder
            .result
            .add_op(self.block, Operator::I32GeU, &[lhs, rhs], &[Type::I32])
    }

    pub fn i32mul(&mut self, lhs: Value, rhs: Value) -> Value {
        self.func_builder
            .result
            .add_op(self.block, Operator::I32Mul, &[lhs, rhs], &[Type::I32])
    }

    pub fn i32add(&mut self, lhs: Value, rhs: Value) -> Value {
        self.func_builder
            .result
            .add_op(self.block, Operator::I32Add, &[lhs, rhs], &[Type::I32])
    }

    pub fn f64const(&mut self, value: u64) -> Value {
        self.func_builder
            .result
            .add_op(self.block, Operator::F64Const { value }, &[], &[Type::F64])
    }

    pub fn i64load(&mut self, mem: MemoryArg, addr: Value) -> Value {
        self.func_builder.result.add_op(
            self.block,
            Operator::I64Load { memory: mem },
            &[addr],
            &[Type::I64],
        )
    }

    pub fn i64store(&mut self, mem: MemoryArg, addr: Value, value: Value) {
        self.func_builder.result.add_op(
            self.block,
            Operator::I64Store { memory: mem },
            &[addr, value],
            &[],
        );
    }

    pub fn call(&mut self, callee: Func, args: &[Value], ret: Option<Type>) -> Value {
        let op = Operator::Call {
            function_index: callee,
        };
        if let Some(ret) = ret {
            self.func_builder
                .result
                .add_op(self.block, op, args, &[ret])
        } else {
            self.func_builder.result.add_op(self.block, op, args, &[])
        }
    }

    pub fn call_indirect(
        &mut self,
        sig: Signature,
        table: Table,
        args: &[Value],
        ret: Option<Type>,
    ) -> Value {
        let op = Operator::CallIndirect {
            sig_index: sig,
            table_index: table,
        };

        if let Some(ret) = ret {
            self.func_builder
                .result
                .add_op(self.block, op, args, &[ret])
        } else {
            self.func_builder.result.add_op(self.block, op, args, &[])
        }
    }

    pub fn ret(&mut self, values: &[Value]) {
        self.func_builder.result.set_terminator(
            self.block,
            Terminator::Return {
                values: values.into(),
            },
        );
    }
}
