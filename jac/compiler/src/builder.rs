//! Function Builder.
use crate::stack::{Stack, StackVal};
use anyhow::Result;
use jac_translate::{
    FunctionTranslation, Translation,
    quickpars::{BinaryReader, Opcode},
};
use std::collections::{HashMap, HashSet};
use waffle::{
    Block, Func, FuncDecl, FunctionBody, Local, Module, Operator, Signature, SignatureData, Type,
    Value, ValueDef,
};

/// An IR builder.
pub(crate) struct FunctionBuilder<'a, 'data> {
    /// The QuickJS bytecode function translation.
    translation: &'a FunctionTranslation<'data>,
    /// The parent module of the yet-to-be-consructed function.
    module: &'a mut Module<'data>,
    /// The resulting function body.
    result: FunctionBody,
    /// A handle to the function signature.
    sig: Signature,
    /// A mapping from bytecode offsets to blocks.
    offsets_to_blocks: HashMap<usize, Block>,
    /// Declaration of function locals.
    declared_locals: HashMap<Local, Type>,
    /// The current block and its associated metadata.
    current_block: Option<CurrentBlock>,
    /// Sealed blocks.
    /// No further predecessors will be added to these blocks.
    sealed: HashSet<Block>,
    /// Phi functions that need to be completed.
    placeholders: HashMap<Block, Vec<(Local, Value)>>,
    /// The out block.
    out: Block,
    /// The abstract value stack.
    stack: Stack,
}

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

impl<'a, 'data> FunctionBuilder<'a, 'data> {
    pub fn new(module: &'a mut Module<'data>, translation: &'a FunctionTranslation<'data>) -> Self {
        Self {
            translation,
            module,
            result: FunctionBody::default(),
            sig: Default::default(),
            offsets_to_blocks: Default::default(),
            declared_locals: Default::default(),
            current_block: None,
            sealed: Default::default(),
            placeholders: Default::default(),
            out: Default::default(),
            stack: Default::default(),
        }
    }

    pub fn build(mut self, translation: &Translation<'data>) -> Result<()> {
        self.prelude(translation)?;
        self.handle_operator(translation)?;

        self.module
            .funcs
            .push(FuncDecl::Body(self.sig.clone(), "".into(), self.result));

        Ok(())
    }

    fn prelude(&mut self, translation: &Translation<'data>) -> Result<()> {
        let is_top_level_eval = self.translation.is_top_level_eval(&translation);
        let mut body = FunctionBody::default();
        // All functions take as first parameter the JavaScript
        // context except the eval function which is in charge of
        // initializing the context.
        let mut params = if is_top_level_eval {
            vec![]
        } else {
            vec![Type::I32]
        };

        // All functions return a value.
        let returns = vec![Type::I64];

        for _ in 0..self.translation.header.arg_count {
            params.push(Type::I64);
        }
        let params_len = params.len();
        self.sig = self.module.signatures.push(SignatureData {
            params: params.clone(),
            returns: returns.clone(),
        });

        body.n_params = params_len;
        body.rets = returns.clone();

        // JavaScript locals are stored in the following order in the locals vector:
        // - Locals for arguments: retrieving locals for arguments
        //   implies starting from index 0 of the vector.
        // - Locals for closure vars: retrieving locals for closure vars
        //   implies starting from the length of the local arguments.
        // - Rest of the function's locals: retriving a function
        //   local implies starting from the args.len () +
        //   closure_vars.len() index of the vector.
        for p in &params {
            body.locals.push((*p).into());
        }

        for (i, cv) in self.translation.closure_vars.iter().enumerate() {
            assert!(cv.index as usize == i);
            body.locals.push(Type::I64);
        }

        for _ in 0..self.translation.locals.len() {
            body.locals.push(Type::I64);
        }

        // Handle entry block.
        let entry = body.add_block();
        body.entry = entry;
        self.seal(entry);
        self.current_block(entry);

        // Declare argument locals and add block param values.
        // FIXME: Unfortunate clone below, to avoid borrow checker
        // issues.
        for (local, ty) in body.locals.clone().entries().take(params.len()) {
            let v = body.add_blockparam(entry, *ty);
            self.declare_local(local, *ty);
            self.def_local(local, v);
        }

        // Declare but not define the rest of the function locals.
        // FIXME: Unfortunate clone below, to avoid borrow checker
        // issues.
        for (local, ty) in body.locals.clone().entries().skip(params.len()) {
            self.declare_local(local, *ty);
        }

        // Hanle out block.
        let out = body.add_block();
        self.add_blockparams(out, &returns);
        self.out = out;

        self.result = body;

        Ok(())
    }

    /// Add block parameters to the given block.
    fn add_blockparams(&mut self, block: Block, params: &[Type]) {
        for &param in params {
            self.result.add_blockparam(block, param);
        }
    }

    /// Set `block` as the current block.
    fn current_block(&mut self, block: Block) {
        assert!(self.current_block.is_none());
        self.current_block = Some(CurrentBlock::new(block));
    }

    /// Finalizes the current block.
    fn finalize_current_block(&mut self) {
        assert!(self.current_block.is_some());
        // TODO: Handle if `unreachable`, etc.
        self.current_block = None;
    }

    /// Seal the current block.
    fn seal(&mut self, block: Block) {
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

    /// Declares a function local.
    fn declare_local(&mut self, local: Local, ty: Type) {
        // Must be declared only once per function.
        assert!(self.declared_locals.insert(local, ty).is_none());
    }

    /// Define a given local in the current block.
    fn def_local(&mut self, local: Local, val: Value) {
        self.unwrap_current_block_mut().locals.insert(local, val);
    }

    fn is_sealed(&self, block: Block) -> bool {
        self.sealed.contains(&block)
    }

    /// Get the SSA value associated to the given `Local`.
    fn use_local(&mut self, local: Local) -> Value {
        self.use_local_rec(self.unwrap_current_block().current, local)
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

    fn unwrap_current_block(&self) -> &CurrentBlock {
        self.current_block.as_ref().expect("No current block set")
    }

    fn unwrap_current_block_mut(&mut self) -> &mut CurrentBlock {
        self.current_block.as_mut().expect("No current block set")
    }

    fn handle_operator(&mut self, translation: &Translation) -> Result<()> {
        use Opcode::*;
        let mut reader = self.translation.operators.clone();
        while !reader.done() {
            let (offset, op) = Opcode::from_reader(&mut reader)?;
            match op {
                Drop => {
                    self.stack.pop1();
                }
                PushI8 { val: _ } => {
                    // let val = self
                    //     .result
                    //     .add_value(ValueDef::Operator(Operator::I32Const { value: val as u32 }));
                    // self.stack.push(StackVal::new(val, Type::I32));
                }
                _ => todo!(),
            };
        }
        Ok(())
    }
}
