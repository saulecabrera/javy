// TODO:
// - Abstract away the usage of the context local.
// - Define types for pointer and JS values instead of having them used directly (e.g., Type::JSValue, Type::JSContext)

//! Function Builder.
use crate::{
    compiler::FuncEnv,
    crt,
    stack::{Stack, StackVal},
};
use anyhow::Result;
use jac_translate::{
    FunctionTranslation, Translation,
    quickpars::{BinaryReader, FuncIndex, Opcode},
};
use std::collections::{HashMap, HashSet};
use waffle::{
    Block, Func, FuncDecl, FunctionBody, Import, ImportKind, Local, Module, Operator, Signature,
    SignatureData, Type, Value, ValueDef, entity::EntityRef,
};

/// Make a [`SignatureData`] from the given [`FuncEnv`].
pub(crate) fn sig<'data>(
    module_translation: &Translation<'data>,
    func_translation: &FunctionTranslation<'data>,
) -> SignatureData {
    let top_level_eval = func_translation.is_top_level_eval(&module_translation);
    // All functions take as first parameter the JavaScript
    // context except the eval function which is in charge of
    // initializing the context.
    let mut params = if top_level_eval {
        vec![]
    } else {
        vec![Type::I32]
    };

    // All functions return a value.
    let returns = vec![Type::I64];

    for _ in 0..func_translation.header.arg_count {
        params.push(Type::I64);
    }
    let params_len = params.len();

    SignatureData {
        params: params.clone(),
        returns: returns.clone(),
    }
}

/// An IR builder.
pub(crate) struct FunctionBuilder<'a, 'data> {
    /// Metadata needed by the function for compilation.
    env: FuncEnv<'a, 'data>,
    /// The resulting function.
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
    /// Whether the function is the top level eval.
    top_level_eval: bool,
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
    pub fn new(env: FuncEnv<'a, 'data>, sig: Signature) -> Self {
        Self {
            env,
            result: FunctionBody::default(),
            sig,
            offsets_to_blocks: Default::default(),
            declared_locals: Default::default(),
            current_block: None,
            sealed: Default::default(),
            placeholders: Default::default(),
            out: Default::default(),
            stack: Default::default(),
            top_level_eval: false,
        }
    }

    pub fn build(mut self) -> Result<FunctionBody> {
        self.top_level_eval = self
            .env
            .function_translation
            .is_top_level_eval(&self.env.module_translation);

        self.prelude()?;

        self.maybe_init_runtime();

        self.handle_operator()?;

        Ok(self.result)
    }

    /// Generic helper to make a function call.
    fn make_call(&mut self, f: Func) -> (Type, Value) {
        let current_block = self.unwrap_current_block().current;
        let call_op = Operator::Call { function_index: f };
        let (call_args, call_ty) = self.stack.get_op_args(call_op, &self.env);
        (
            call_ty,
            self.result
                .add_op(current_block, call_op, &call_args, &[call_ty]),
        )
    }

    /// Initializes the runtime if currently compiling the top-level
    /// eval function.
    fn maybe_init_runtime(&mut self) {
        if self.top_level_eval {
            let current_block = self.unwrap_current_block().current;

            let operator = Operator::I32Const {
                value: u32::try_from(self.env.function_translation.closure_vars.len()).unwrap(),
            };
            let (const_args, const_ty) = self.stack.get_op_args(operator, &self.env);
            let closure_vars_count_val =
                self.result
                    .add_op(current_block, operator, &const_args, &[const_ty]);
            // Pushing to the abstract stack to make it easier to derive the call args
            // via `Stack::get_op_args` below as opposed having a different code path.
            self.stack
                .push(StackVal::new(closure_vars_count_val, const_ty));

            let (module, name, params, returns) = crt::init();
            let (_, func) = self.maybe_add_function_import(module, name, &params, Some(returns));
            let (_, context_val) = self.make_call(func);
            // Local 0 is the context.
            // TODO: Perhaps store the `Local` handle directly in the
            // builder.
            self.def_local(Local::new(0), context_val);
        }
    }

    /// Creates a known import, if not already defined.
    /// Returning the index of the function in the IR.
    fn maybe_add_function_import(
        &mut self,
        module: &'static str,
        func: &'static str,
        params: &[Type],
        returns: Option<Type>,
    ) -> (Signature, Func) {
        if self.env.imported_funcs.contains_key(func) {
            return self.env.imported_funcs.get(func).cloned().unwrap();
        }

        let signature_data = SignatureData {
            params: params.into(),
            returns: returns.map(|t| vec![t]).unwrap_or_default(),
        };

        let sig_handle = self.env.result.signatures.push(signature_data);
        let func_handle = self
            .env
            .result
            .funcs
            .push(FuncDecl::Import(sig_handle, "".into()));
        self.env.result.imports.push(Import {
            module: module.into(),
            name: func.into(),
            kind: ImportKind::Func(func_handle),
        });
        self.env
            .imported_funcs
            .insert(func, (sig_handle, func_handle));

        (sig_handle, func_handle)
    }

    /// Maybe add a defined a function for the given translation
    /// index, returning the already registered `Func` and `Signature`
    /// if already registered.
    fn maybe_add_defined_function(&mut self, index: FuncIndex) -> (Signature, Func) {
        if self.env.defined_funcs.contains_key(&index) {
            return self.env.defined_funcs.get(&index).cloned().unwrap();
        }

        let target = &self.env.module_translation.module.functions[index.as_u32() as usize];
        let sig = sig(&self.env.module_translation, &target);
        let sig_handle = self.env.result.signatures.push(sig);
        let func_handle =
            self.env
                .result
                .funcs
                .push(FuncDecl::Body(sig_handle, "".into(), Default::default()));
        self.env
            .defined_funcs
            .insert(index, (sig_handle, func_handle));

        (sig_handle, func_handle)
    }

    fn prelude(&mut self) -> Result<()> {
        let mut body = FunctionBody::default();

        let sig_data = &self.env.result.signatures[self.sig];
        let params = &sig_data.params.clone();
        let params_len = params.len();
        let returns = sig_data.returns.clone();
        body.n_params = params_len;
        body.rets = returns.clone();

        // We don't allocate a param for the top-level eval,
        // however we still need a local to store the context;
        // it's the responsibility of the top level eval function
        // to initialize the context.
        // Positionally, it's convenient to define the context local
        // at 0, to ensure that we can uniformly access it as local.get 0
        // in all functions.
        if self.top_level_eval {
            body.locals.push(Type::I32);
        }
        // JavaScript locals are stored in the following order in the locals vector:
        // - Locals for arguments: retrieving locals for arguments
        //   implies starting from index 0 of the vector.
        // - Locals for closure vars: retrieving locals for closure vars
        //   implies starting from the length of the local arguments.
        // - Rest of the function's locals: retriving a function
        //   local implies starting from the args.len () +
        //   closure_vars.len() index of the vector.
        for p in params {
            body.locals.push((*p).into());
        }

        for (i, cv) in self
            .env
            .function_translation
            .closure_vars
            .iter()
            .enumerate()
        {
            assert!(cv.index as usize == i);
            body.locals.push(Type::I64);
        }

        for _ in 0..self.env.function_translation.locals.len() {
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
        for (local, ty) in body.locals.clone().entries().take(params_len) {
            let v = body.add_blockparam(entry, *ty);
            self.declare_local(local, *ty);
            self.def_local(local, v);
        }

        // Declare but not define the rest of the function locals.
        // FIXME: Unfortunate clone below, to avoid borrow checker
        // issues.
        for (local, ty) in body.locals.clone().entries().skip(params_len) {
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

    fn handle_operator(&mut self) -> Result<()> {
        use Opcode::*;
        let mut reader = self.env.function_translation.operators.clone();
        let mut in_init_block = false;
        while !reader.done() {
            let (offset, op) = Opcode::from_reader(&mut reader)?;

            match op {
                PushThis => {
                    // TODO: Handle the rest of the intruction.
                    // TODO: Add docs.
                    // TODO: This heuristic might not be the best one though
                    //       could be made better once introducing support for
                    //       control flow.
                    if offset == 0 {
                        in_init_block = true;
                        continue;
                    }

                    todo!()
                }
                ReturnUndef => {
                    if in_init_block {
                        in_init_block = false;
                        continue;
                    }
                    todo!()
                }
                Drop => {
                    self.stack.pop1();
                }
                PushI8 { val } => {
                    let context_val = self.use_local(Local::new(0));
                    let current_block = self.unwrap_current_block().current;
                    let operator = Operator::I32Const { value: val as u32 };
                    let (args, ty) = self.stack.get_op_args(operator, &self.env);
                    let val = self.result.add_op(current_block, operator, &args, &[ty]);
                    let (module, name, params, returns) = crt::new_int32();
                    let (_, new_int32_func) =
                        self.maybe_add_function_import(module, name, &params, Some(returns));

                    self.stack.push(StackVal::new(context_val, Type::I32));
                    self.stack.push(StackVal::new(val, ty));
                    let (call_ty, val) = self.make_call(new_int32_func);
                    self.stack.push(StackVal::new(val, call_ty));
                }
                FClosure8 { index } => {
                    let func_index = self
                        .env
                        .module_translation
                        .resolve_func_index(self.env.function_translation.index, index);
                    // Ensure that we have a Wasm function defined,
                    // prior to creating the closure for it.
                    let (_, _) = self.maybe_add_defined_function(func_index);

                    // Retrieve the target function translation.
                    let target =
                        &self.env.module_translation.module.functions[func_index.as_u32() as usize];

                    let current_block = self.unwrap_current_block().current;

                    let argc_op = Operator::I32Const {
                        value: target.header.arg_count,
                    };
                    let (argc_args, argc_ty) = self.stack.get_op_args(argc_op, &self.env);
                    let argc_val =
                        self.result
                            .add_op(current_block, argc_op, &argc_args, &[argc_ty]);

                    // Magic describes the index in the functions table
                    // where the function reference will live.
                    let magic_op = Operator::I32Const {
                        value: func_index.as_u32(),
                    };
                    let (magic_args, magic_ty) = self.stack.get_op_args(magic_op, &self.env);
                    let magic_val =
                        self.result
                            .add_op(current_block, magic_op, &magic_args, &[magic_ty]);

                    let context_val = self.use_local(Local::new(0));

                    // Prepare the arguments through the abstract value stack.
                    self.stack.push(StackVal::new(context_val, Type::I32));
                    self.stack.push(StackVal::new(argc_val, argc_ty));
                    self.stack.push(StackVal::new(magic_val, magic_ty));

                    // Make the call to create the closure.
                    let (module, name, params, returns) = crt::closure();
                    let (_, closure_func) =
                        self.maybe_add_function_import(module, name, &params, Some(returns));
                    let (call_ty, closure_val) = self.make_call(closure_func);
                    self.stack.push(StackVal::new(closure_val, call_ty));

                    // Initialize the closure variables needed by the
                    // target function.
                    for cv in &target.closure_vars {
                        if cv.is_local {
                            todo!()
                        }

                        let (module, name, params) = crt::resolve_non_local_var_ref();
                        let (_, resolve_non_local_var_refs_func) =
                            self.maybe_add_function_import(module, name, &params, None);

                        let func_index_op = Operator::I32Const {
                            value: func_index.as_u32(),
                        };
                        let (func_index_args, func_index_ty) =
                            self.stack.get_op_args(func_index_op, &self.env);
                        let func_index_val = self.result.add_op(
                            current_block,
                            func_index_op,
                            &func_index_args,
                            &[func_index_ty],
                        );

                        let cv_index_op = Operator::I32Const { value: cv.index };
                        let (cv_index_args, cv_index_ty) =
                            self.stack.get_op_args(cv_index_op, &self.env);
                        let cv_index_val = self.result.add_op(
                            current_block,
                            cv_index_op,
                            &cv_index_args,
                            &[cv_index_ty],
                        );

                        // Use the stack to prepare the arguments.
                        self.stack.push(StackVal::new(context_val, Type::I32));
                        self.stack
                            .push(StackVal::new(func_index_val, func_index_ty));
                        self.stack.push(StackVal::new(cv_index_val, cv_index_ty));
                        self.make_call(resolve_non_local_var_refs_func);
                    }
                }

                PutVarRef { index } => {
                    let current_block = self.unwrap_current_block().current;
                    let val = self.stack.pop1();
                    let context_val = self.use_local(Local::new(0));

                    let (module, name, args) = crt::put_var_ref();
                    let (_, put_var_ref_func) =
                        self.maybe_add_function_import(module, name, &args, None);

                    let cv_index_op = Operator::I32Const {
                        value: index.as_u32(),
                    };
                    let (cv_index_args, cv_index_type) =
                        self.stack.get_op_args(cv_index_op, &self.env);
                    let cv_index_val = self.result.add_op(
                        current_block,
                        cv_index_op,
                        &cv_index_args,
                        &[cv_index_type],
                    );

                    self.stack.push(StackVal::new(context_val, Type::I32));
                    self.stack.push(StackVal::new(cv_index_val, Type::I32));
                    self.stack.push(val);
                    self.make_call(put_var_ref_func);
                }

                GetVarRef { index } => {
                    let current_block = self.unwrap_current_block().current;
                    let context_val = self.use_local(Local::new(0));

                    let (module, name, args, returns) = crt::get_var_ref();
                    let (_, get_var_ref_func) =
                        self.maybe_add_function_import(module, name, &args, Some(returns));
                    let cv_index_op = Operator::I32Const {
                        value: index.as_u32(),
                    };
                    let (cv_index_args, cv_index_ty) =
                        self.stack.get_op_args(cv_index_op, &self.env);
                    let cv_index_val = self.result.add_op(
                        current_block,
                        cv_index_op,
                        &cv_index_args,
                        &[cv_index_ty],
                    );

                    self.stack.push(StackVal::new(context_val, Type::I32));
                    self.stack.push(StackVal::new(cv_index_val, cv_index_ty));
                    let (call_ty, val) = self.make_call(get_var_ref_func);
                    self.stack.push(StackVal::new(val, call_ty));
                }

                _ => todo!(),
            };
        }
        Ok(())
    }
}
