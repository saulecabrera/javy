// TODO:
// - Abstract away the usage of the context local.
// - Define types for pointer and JS values instead of having them used directly (e.g., Type::JSValue, Type::JSContext)

//! Function frontend.
//! This modules defines all the functionality to consume the
//! translation layer of QuickJS bytecode and to transform it to a
//! lower level SSA IR.

use crate::{
    builder::FunctionBuilder,
    compiler::{FuncEnv, FuncRef},
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
    SignatureData, Table, TableData, Type, Value, ValueDef, entity::EntityRef,
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

/// Function call builder.
struct CallBuilder<'a, 'data> {
    env: &'a FuncEnv<'a, 'data>,
    builder: &'a mut FunctionBuilder,
    stack: &'a mut Stack,
}

impl<'a, 'data> CallBuilder<'a, 'data> {
    fn new(
        env: &'a FuncEnv<'a, 'data>,
        builder: &'a mut FunctionBuilder,
        stack: &'a mut Stack,
    ) -> Self {
        Self {
            env,
            builder,
            stack,
        }
    }

    /// Build a call instruction.
    /// Taking into account the appropriate invariants.
    fn build(&mut self, callee: Func) -> (Option<Type>, Value) {
        let decl = &self.env.result.funcs[callee];
        let sig = &self.env.result.signatures[decl.sig()];
        let param_count = sig.params.len();
        let args = self.stack.peekn(param_count);
        assert!(param_count == args.len());

        let mut values = vec![];
        for (param, val) in sig.params.iter().zip(args.iter()) {
            assert!(val.ty == Some(*param));
            values.push(val.val);
        }

        let ret = self
            .builder
            .instr_builder()
            .call(callee, &values, sig.returns.first().copied());
        self.stack.drop(param_count);
        (sig.returns.first().copied(), ret)
    }
}

/// Function frontend.
pub(crate) struct Frontend<'a, 'data> {
    /// The function builder.
    builder: FunctionBuilder,
    /// Metadata needed by the function for compilation.
    env: FuncEnv<'a, 'data>,
    /// A mapping from bytecode offsets to blocks.
    offsets_to_blocks: HashMap<usize, Block>,
    /// The abstract value stack.
    stack: Stack,
    /// Whether the function is the top level eval.
    top_level_eval: bool,
}

impl<'a, 'data> Frontend<'a, 'data> {
    pub fn new(env: FuncEnv<'a, 'data>, sig: Signature) -> Self {
        Self {
            env,
            builder: FunctionBuilder::new(sig),
            offsets_to_blocks: Default::default(),
            stack: Default::default(),
            top_level_eval: false,
        }
    }

    pub fn build(mut self) -> Result<FunctionBody> {
        self.top_level_eval = self
            .env
            .function_translation
            .is_top_level_eval(&self.env.module_translation);

        self.prologue()?;
        self.maybe_init_runtime();
        self.handle_operator()?;
        self.epilogue();

        Ok(self.builder.result)
    }

    /// Generic helper to make a function call.
    fn make_call(&mut self, f: Func) -> (Option<Type>, Value) {
        CallBuilder::new(&self.env, &mut self.builder, &mut self.stack).build(f)
    }

    /// Initializes the runtime if currently compiling the top-level
    /// eval function.
    fn maybe_init_runtime(&mut self) {
        if self.top_level_eval {
            let closure_vars_count_val = self
                .builder
                .instr_builder()
                .i32const(u32::try_from(self.env.function_translation.closure_vars.len()).unwrap());

            // Pushing to the abstract stack to make it easier to derive the call args
            // via `Stack::get_op_args` below as opposed having a different code path.
            self.stack.push(StackVal::i32(closure_vars_count_val));

            let builtin = crt::init();
            let (_, func) = self.env.imported_funcs[&builtin];
            let (_, context_val) = self.make_call(func);
            // Local 0 is the context.
            // TODO: Perhaps store the `Local` handle directly in the
            // builder.
            self.builder.def_local(Local::new(0), context_val);
        }
    }

    /// Maybe add a defined a function for the given translation
    /// index, returning the already registered `Func` and `Signature`
    /// if already registered.
    fn maybe_add_defined_function(
        &mut self,
        index: FuncIndex,
    ) -> (Signature, Func, Option<FuncRef>) {
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
            .insert(index, (sig_handle, func_handle, None));

        (sig_handle, func_handle, None)
    }

    fn prologue(&mut self) -> Result<()> {
        let sig_data = &self.env.result.signatures[self.builder.sig()];
        let params = &sig_data.params.clone();
        let params_len = params.len();
        let returns = sig_data.returns.clone();

        self.builder.result.n_params = params_len;
        self.builder.result.rets = returns.clone();

        // We don't allocate a param for the top-level eval,
        // however we still need a local to store the context;
        // it's the responsibility of the top level eval function
        // to initialize the context.
        // Positionally, it's convenient to define the context local
        // at 0, to ensure that we can uniformly access it as local.get 0
        // in all functions.
        if self.top_level_eval {
            self.builder.result.locals.push(Type::I32);
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
            self.builder.result.locals.push((*p).into());
        }

        for (i, cv) in self
            .env
            .function_translation
            .closure_vars
            .iter()
            .enumerate()
        {
            assert!(cv.index as usize == i);
            self.builder.result.locals.push(Type::I64);
        }

        for _ in 0..self.env.function_translation.locals.len() {
            self.builder.result.locals.push(Type::I64);
        }

        // Handle entry block.
        let entry = self.builder.result.add_block();
        self.builder.result.entry = entry;
        self.builder.seal(entry);
        self.builder.switch_to_block(entry);

        // Declare argument locals and add block param values.
        // FIXME: Unfortunate clone below, to avoid borrow checker
        // issues.
        for (local, ty) in self
            .builder
            .result
            .locals
            .clone()
            .entries()
            .take(params_len)
        {
            let v = self.builder.result.add_blockparam(entry, *ty);
            self.builder.declare_local(local, *ty);
            self.builder.def_local(local, v);
        }

        // Declare but not define the rest of the function locals.
        // FIXME: Unfortunate clone below, to avoid borrow checker
        // issues.
        for (local, ty) in self
            .builder
            .result
            .locals
            .clone()
            .entries()
            .skip(params_len)
        {
            self.builder.declare_local(local, *ty);
        }

        // Hanle out block.
        let exit = self.builder.result.add_block();
        self.builder.add_blockparams(exit, &returns);
        self.builder.exit(exit);

        Ok(())
    }

    fn epilogue(&mut self) {}

    fn resolve_funcref(&mut self, func_index: FuncIndex) -> FuncRef {
        let (sig, func, funcref) = self.env.defined_funcs[&func_index];

        if let Some(f) = funcref {
            return f;
        }

        let new_ref = self.env.function_table.push(func);
        self.env
            .defined_funcs
            .insert(func_index, (sig, func, Some(new_ref)));
        new_ref
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
                    let context_val = self.builder.use_local(Local::new(0));
                    let val = self.builder.instr_builder().i32const(val as u32);
                    let (_, new_int32_func) = self.env.imported_funcs[&crt::new_int32()];

                    self.stack.push(StackVal::i32(context_val));
                    self.stack.push(StackVal::i32(val));
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
                    let (_, _, funcref) = self.maybe_add_defined_function(func_index);

                    // Retrieve the target function translation.
                    let target =
                        &self.env.module_translation.module.functions[func_index.as_u32() as usize];

                    let argc_val = self
                        .builder
                        .instr_builder()
                        .i32const(target.header.arg_count);

                    // Magic describes the index in the functions table
                    // where the function reference will live.
                    let funcref = if funcref.is_none() {
                        self.resolve_funcref(func_index)
                    } else {
                        funcref.unwrap()
                    };

                    let magic_val = self
                        .builder
                        .instr_builder()
                        .i32const(u32::try_from(funcref.index()).unwrap());

                    let context_val = self.builder.use_local(Local::new(0));

                    // Prepare the arguments through the abstract value stack.
                    self.stack.push(StackVal::i32(context_val));
                    self.stack.push(StackVal::i32(argc_val));
                    self.stack.push(StackVal::i32(magic_val));

                    // Make the call to create the closure.
                    let (_, closure_func) = self.env.imported_funcs[&crt::closure()];
                    let (call_ty, closure_val) = self.make_call(closure_func);
                    self.stack.push(StackVal::new(closure_val, call_ty));

                    // Initialize the closure variables needed by the
                    // target function.
                    for cv in &target.closure_vars {
                        if cv.is_local {
                            todo!()
                        }

                        let (_, resolve_non_local_var_refs_func) =
                            self.env.imported_funcs[&crt::resolve_non_local_var_ref()];

                        let func_index_val =
                            self.builder.instr_builder().i32const(func_index.as_u32());
                        let cv_index_val = self.builder.instr_builder().i32const(cv.index);

                        // Use the stack to prepare the arguments.
                        self.stack.push(StackVal::i32(context_val));
                        self.stack.push(StackVal::i32(func_index_val));
                        self.stack.push(StackVal::i32(cv_index_val));
                        self.make_call(resolve_non_local_var_refs_func);
                    }
                }

                PutVarRef { index } => {
                    let val = self.stack.pop1();
                    let context_val = self.builder.use_local(Local::new(0));

                    let (_, put_var_ref_func) = self.env.imported_funcs[&crt::put_var_ref()];

                    let cv_index_val = self.builder.instr_builder().i32const(index.as_u32());

                    self.stack.push(StackVal::i32(context_val));
                    self.stack.push(StackVal::i32(cv_index_val));
                    self.stack.push(val);
                    self.make_call(put_var_ref_func);
                }

                GetVarRef { index } => {
                    let context_val = self.builder.use_local(Local::new(0));

                    let (_, get_var_ref_func) = self.env.imported_funcs[&crt::get_var_ref()];

                    let cv_index_val = self.builder.instr_builder().i32const(index.as_u32());
                    self.stack.push(StackVal::i32(context_val));
                    self.stack.push(StackVal::i32(cv_index_val));
                    let (call_ty, val) = self.make_call(get_var_ref_func);
                    self.stack.push(StackVal::new(val, call_ty));
                }

		// Should be `todo!()`; avoiding a panic allows
		// progressive testing of code generation.
                _ => {}
            };
        }
        Ok(())
    }
}
