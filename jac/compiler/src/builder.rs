//! Function builder.

use std::collections::{HashMap, HashSet};
use waffle::{Block, Func, FunctionBody, Local, Operator, Signature, Type, Value};

/// The current block and its locals.
struct CurrentBlock {
    /// Mapping of locals to SSA values defined in the current block.
    locals: HashMap<Local, Value>,
    /// The current block.
    current: Block,
}

impl CurrentBlock {
    fn new(block: Block) -> Self {
        Self {
            current: block,
            locals: Default::default(),
        }
    }

    fn equals(&self, block: Block) -> bool {
        self.current == block
    }
}

/// Function Builder.
#[derive(Default)]
pub struct FunctionBuilder {
    /// The current block.
    position: Option<CurrentBlock>,
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
    placeholders: HashMap<Block, Vec<(Local, Value)>>,
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

    /// Set `block` as the current block.
    pub fn current_block(&mut self, block: Block) {
        assert!(self.position.is_none());
        self.position = Some(CurrentBlock::new(block));
    }

    /// Finalizes the current block.
    pub fn finalize_current_block(&mut self) {
        assert!(self.position.is_some());
        // TODO: Handle if `unreachable`, etc.
        self.position = None;
    }

    /// Seal the current block.
    pub fn seal(&mut self, block: Block) {
        assert!(self.sealed.insert(block));
        // Grab all the placeholders for the current block and
        // calculate the operands.
        let placeholders = self.placeholders.remove(&block).unwrap_or_default();
        for (local, value) in placeholders {
            // Shouldn't hit this case just now since we're still not
            // hanlding control flow entirely.
            todo!();
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
        self.unwrap_current_block_mut().locals.insert(local, val);
    }

    fn is_sealed(&self, block: Block) -> bool {
        self.sealed.contains(&block)
    }

    /// Get the SSA value associated to the given `Local`.
    pub fn use_local(&mut self, local: Local) -> Value {
        self.use_local_rec(self.unwrap_current_block().current, local)
    }

    fn unwrap_current_block(&self) -> &CurrentBlock {
        self.position.as_ref().expect("No current block set")
    }

    fn unwrap_current_block_mut(&mut self) -> &mut CurrentBlock {
        self.position.as_mut().expect("No current block set")
    }

    /// Create an [`InstructionBuilder`].
    pub fn instr_builder(&mut self) -> InstructionBuilder<'_> {
        InstructionBuilder {
            block: self.unwrap_current_block().current,
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
        // 1. The local is defined in the current block.
        // 2. The given block is not sealed, therefore, we need to
        //    record the value placeholders.
        // 3. The block is sealed, with a single predecessor,
        //    therefore, we can query the local directly in the
        //    predecessor.
        // 4. General case: the block is sealed, with muliple
        //    predecessors, therefore we need to recursively read the
        //    value in all the predecessors.
        let current = self.unwrap_current_block();
        if current.equals(block) {
            if let Some(&v) = current.locals.get(&local) {
                return v;
            }
        }

        todo!()
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

    pub fn call(&mut self, callee: Func, args: &[Value], ret: Option<Type>) -> Value {
        let op = Operator::Call {
            function_index: callee,
        };
	if let Some(ret) = ret {
	    self.func_builder
		.result
		.add_op(self.block, op, args, &[ret])
	} else {
	    self.func_builder
		.result
		.add_op(self.block, op, args, &[])
	}
    }
}
