// TODO:
// - Abstract away the usage of the context local.
// - Define types for pointer and JS values instead of having them used directly (e.g., Type::JSValue, Type::JSContext)

//! Function frontend.
//! This modules defines all the functionality to consume the
//! translation layer of QuickJS bytecode and to transform it to a
//! lower level SSA IR.

use crate::{
    args,
    builder::FunctionBuilder,
    compiler::{FuncEnv, FuncRef},
    control::{Cond, CondState, ControlFrame, ControlStack},
    crt,
    stack::{Stack, StackVal},
};
use anyhow::Result;
use jac_translate::{
    FunctionTranslation, Translation,
    quickpars::{BinaryReader, ClosureVarIndex, FuncIndex, Opcode},
};
use std::collections::{HashMap, HashSet};
use waffle::{
    Block, Func, FuncDecl, FunctionBody, Import, ImportKind, Local, Memory, MemoryArg, Module,
    Operator, Signature, SignatureData, Table, TableData, Type, Value, ValueDef, entity::EntityRef,
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
    /// The control frame stack.
    control_stack: ControlStack,
}

impl<'a, 'data> Frontend<'a, 'data> {
    pub fn new(env: FuncEnv<'a, 'data>, sig: Signature) -> Self {
        let top_level_eval = env
            .function_translation
            .is_top_level_eval(&env.module_translation);
        Self {
            env,
            builder: FunctionBuilder::new(sig),
            offsets_to_blocks: Default::default(),
            stack: Default::default(),
            control_stack: ControlStack::new(),
            top_level_eval,
        }
    }

    pub fn build(mut self) -> Result<FunctionBody> {
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
            self.builder.def_local(args::context(), context_val);
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

        // Handle out block.
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

    fn emit_put_var_ref(&mut self, index: ClosureVarIndex) {
        let val = self.stack.pop1();
        let context_val = self.builder.use_local(args::context());

        let (_, put_var_ref_func) = self.env.imported_funcs[&crt::put_var_ref()];

        let cv_index_val = self.builder.instr_builder().i32const(index.as_u32());

        self.stack.push(StackVal::i32(context_val));
        self.stack.push(StackVal::i32(cv_index_val));
        self.stack.push(val);
        self.make_call(put_var_ref_func);
    }

    fn emit_get_var_ref(&mut self, index: ClosureVarIndex) {
        let context_val = self.builder.use_local(args::context());

        let (_, get_var_ref_func) = self.env.imported_funcs[&crt::get_var_ref()];

        let cv_index_val = self.builder.instr_builder().i32const(index.as_u32());
        self.stack.push(StackVal::i32(context_val));
        self.stack.push(StackVal::i32(cv_index_val));
        let (call_ty, val) = self.make_call(get_var_ref_func);
        self.stack.push(StackVal::new(val, call_ty));
    }

    fn emit_get_var_ref_check(&mut self, index: ClosureVarIndex) {
        let context_val = self.builder.use_local(args::context());

        let (_, get_var_ref_check_func) = self.env.imported_funcs[&crt::get_var_ref_check()];

        let cv_index_val = self.builder.instr_builder().i32const(index.as_u32());
        self.stack.push(StackVal::i32(context_val));
        self.stack.push(StackVal::i32(cv_index_val));
        let (call_ty, val) = self.make_call(get_var_ref_check_func);
        self.stack.push(StackVal::new(val, call_ty));
    }

    fn emit_get_arg(&mut self, index: usize) {
        let val = self.builder.use_local(args::at(0, |index| {
            map_local_index(index, self.top_level_eval)
        }));

        self.stack.push(StackVal::new(val, Some(Type::I64)));
    }

    /// Emit a JS function call with the given number of arguments.
    /// The callee and arguments are expected to be on the stack in the order:
    /// [callee, arg0, arg1, ..., argN] (callee at bottom, argN at top)
    fn emit_js_call(&mut self, argc: u32) {
        let context_val = self.builder.use_local(args::context());
        let (_, call_func) = self.env.imported_funcs[&crt::call()];

        // Pop arguments from the stack (in reverse order).
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.stack.pop1());
        }
        args.reverse();

        // Pop the callee.
        let callee = self.stack.pop1();

        // Prepare argc and argv.
        let argc_val = self.builder.instr_builder().i32const(argc);

        let argv_val = if argc == 0 {
            self.builder.instr_builder().i32const(0)
        } else {
            let value_size: u32 = 8;
            let alloc_size = argc * value_size;

            let (_, cabi_realloc_func) = self.env.imported_funcs[&crt::cabi_realloc()];

            let zero = self.builder.instr_builder().i32const(0);
            let alignment = self.builder.instr_builder().i32const(1);
            let size = self.builder.instr_builder().i32const(alloc_size);

            self.stack.push(StackVal::i32(zero));
            self.stack.push(StackVal::i32(zero));
            self.stack.push(StackVal::i32(alignment));
            self.stack.push(StackVal::i32(size));

            let (_, base_addr) = self.make_call(cabi_realloc_func);

            let mem_arg = MemoryArg {
                align: 0,
                offset: 0,
                memory: self.env.memory_handle,
            };

            // Store each argument at the appropriate offset.
            for (i, arg) in args.iter().enumerate() {
                let offset = u32::try_from(i).unwrap() * value_size;
                let offset_val = self.builder.instr_builder().i32const(offset);
                let addr_val = self.builder.instr_builder().i32add(base_addr, offset_val);
                self.builder
                    .instr_builder()
                    .i64store(mem_arg.clone(), addr_val, arg.val);
            }

            base_addr
        };

        // Prepare call arguments: context, callee, argc, argv.
        self.stack.push(StackVal::i32(context_val));
        self.stack.push(callee);
        let undef = self.builder.instr_builder().mkval(crt::JS_TAG_UNDEFINED, 0);
        self.stack.push(StackVal::i64(undef));
        self.stack.push(StackVal::i32(argc_val));
        self.stack.push(StackVal::i32(argv_val));

        let (call_ty, result_val) = self.make_call(call_func);
        self.stack.push(StackVal::new(result_val, call_ty));
    }

    /// Getter for value representing the JSContext.
    fn context_val(&mut self) -> Value {
        self.builder.use_local(args::context())
    }

    /// Handles control flow transitions at terminators and offset boundaries.
    ///
    /// When in the `Alt` phase and a terminator is emitted, transitions
    /// to the `Consequent` phase.
    /// When in the `Consequent` phase and the end offset is reached,
    /// branches to the join block and pops the frame.
    fn maybe_handle_control_end(&mut self, offset: u32) {
        if let Some(ControlFrame::Cond(c)) = self.control_stack.peek_mut() {
            match c.state {
                CondState::Alt => {
                    c.state = CondState::Consequent;
                    self.builder.switch_to_block(c.consequent);
                }
                CondState::Consequent => {
                    if let Some(end) = c.end {
                        if offset as usize == end {
                            let join = c
                                .join
                                .expect("consequent with end offset must have a join block");
                            self.builder.branch(join);
                            self.builder.seal(join);
                            self.builder.switch_to_block(join);
                            self.control_stack.pop();
                        }
                    }
                }
            }
        }
    }

    fn handle_operator(&mut self) -> Result<()> {
        use Opcode::*;
        let mut reader = self.env.function_translation.operators.clone();
        let mut in_init_block = false;
        let mut init_guard_end: Option<u32> = None;
        while !reader.done() {
            let (offset, op) = Opcode::from_reader(&mut reader)?;

            if self.top_level_eval {
                if op == PushThis && offset == 0 {
                    continue;
                }

                if let IfFalse8 { offset: target } = op {
                    if offset == 1 {
                        init_guard_end = Some(target);
                        continue;
                    }
                }

                if let Some(end) = init_guard_end {
                    if op == ReturnUndef && offset < end {
                        init_guard_end = None;
                        continue;
                    }
                }
            }

            self.maybe_handle_control_end(offset);

            match op {
                Drop => {
                    self.stack.pop1();
                }
                Undefined => {
                    let val = self.builder.instr_builder().mkval(crt::JS_TAG_UNDEFINED, 0);
                    self.stack.push(StackVal::new(val, Some(Type::I64)));
                }
                PushI8 { val } => {
                    let val = self
                        .builder
                        .instr_builder()
                        .mkval(crt::JS_TAG_INT, val as u64);
                    self.stack.push(StackVal::new(val, Some(Type::I64)));
                }
                // op @ Push1 | Push2 | Push5 => {
                //     let val = match op {
                //         Push1 => self.builder.instr_builder().mkval(crt::JS_TAG_INT, 1u64),
                //         Push2 => self.builder.instr_builder().mkval(crt::JS_TAG_INT, 2u64),
                //         Push5 => self.builder.instr_builder().mkval(crt::JS_TAG_INT, 5u64),
                //     };
                //     self.stack.push(StackVal::new(val, Some(Type::I64)));
                // }

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

                    let context_val = self.builder.use_local(args::context());
                    let func_index_val = self.builder.instr_builder().i32const(func_index.as_u32());

                    // Prepare the arguments through the abstract value stack.
                    self.stack.push(StackVal::i32(context_val));
                    self.stack.push(StackVal::i32(func_index_val));
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

                PutVarRef { index } => self.emit_put_var_ref(index),
                PutVarRef0 => self.emit_put_var_ref(ClosureVarIndex::from_u32(0)),
                PutVarRef1 => self.emit_put_var_ref(ClosureVarIndex::from_u32(1)),
                PutVarRef2 => self.emit_put_var_ref(ClosureVarIndex::from_u32(2)),
                PutVarRef3 => self.emit_put_var_ref(ClosureVarIndex::from_u32(3)),

                GetVarRef { index } => self.emit_get_var_ref(index),
                GetVarRef0 => self.emit_get_var_ref(ClosureVarIndex::from_u32(0)),
                GetVarRef1 => self.emit_get_var_ref(ClosureVarIndex::from_u32(1)),
                GetVarRef2 => self.emit_get_var_ref(ClosureVarIndex::from_u32(2)),
                GetVarRef3 => self.emit_get_var_ref(ClosureVarIndex::from_u32(3)),

                GetVarRefCheck { index } => self.emit_get_var_ref_check(index),

                // TODO: Currently treating `ReturnAsync` as `Return`
                // until we have full async support.
                Return | ReturnAsync => {
                    let val = self.stack.pop1();
                    self.builder.instr_builder().ret(&[val.val]);
                    self.maybe_handle_control_end(offset);
                }

                ReturnUndef => {
                    let undef = self.builder.instr_builder().mkval(crt::JS_TAG_UNDEFINED, 0);
                    self.builder.instr_builder().ret(&[undef]);
                    self.maybe_handle_control_end(offset);
                }

                Mul => {
                    let rhs = self.stack.pop1();
                    let lhs = self.stack.pop1();
                    let context_val = self.builder.use_local(args::context());
                    let (_, mul_func) = self.env.imported_funcs[&crt::mul()];
                    self.stack.push(StackVal::i32(context_val));
                    self.stack.push(lhs);
                    self.stack.push(rhs);
                    let (call_ty, val) = self.make_call(mul_func);
                    self.stack.push(StackVal::new(val, call_ty));
                }

                Call0 => self.emit_js_call(0),
                Call1 => self.emit_js_call(1),
                Call2 => self.emit_js_call(2),
                Call3 => self.emit_js_call(3),
                GetArg0 => self.emit_get_arg(0),
                Lt => {
                    let rhs = self.stack.pop1();
                    let lhs = self.stack.pop1();
                    let context_val = self.context_val();
                    self.stack.push(StackVal::i32(context_val));
                    self.stack.push(lhs);
                    self.stack.push(rhs);
                    let (_, lt_func) = self.env.imported_funcs[&crt::lt()];
                    let (ty, val) = self.make_call(lt_func);
                    self.stack.push(StackVal::new(val, ty));
                }
                IfFalse { offset } | IfFalse8 { offset } => {
                    let cond = self.stack.pop1();
                    let context_val = self.context_val();
                    self.stack.push(StackVal::i32(context_val));
                    self.stack.push(cond);

                    // TODO: the fast path could be inlined in this case.
                    let (_, to_bool_func) = self.env.imported_funcs[&crt::to_bool()];
                    let cond_result = self.make_call(to_bool_func);

                    let alt_block = self.builder.result.add_block();
                    let consequent_block = self.builder.result.add_block();

                    self.control_stack
                        .push(ControlFrame::Cond(Cond::new(alt_block, consequent_block)));

                    let is_false = self.builder.instr_builder().i32eqz(cond_result.1);
                    self.builder
                        .branch_if(is_false, consequent_block, alt_block);
                    self.builder.seal(consequent_block);
                    self.builder.seal(alt_block);
                    self.builder.switch_to_block(alt_block);
                }

                GoTo { offset: target } | GoTo8 { offset: target } | GoTo16 { offset: target } => {
                    let frame = self
                        .control_stack
                        .peek_mut()
                        .expect("expected Cond frame on control stack for GoTo");
                    let ControlFrame::Cond(c) = frame;
                    assert_eq!(c.state, CondState::Alt, "GoTo must occur during Alt phase");

                    let join_block = self.builder.result.add_block();
                    self.builder.branch(join_block);

                    c.state = CondState::Consequent;
                    c.join = Some(join_block);
                    c.end = Some(target as usize);
                    let consequent = c.consequent;
                    self.builder.switch_to_block(consequent);
                }

                op => panic!("Unimplemented {:?} at offset {}", op, offset),
            };
        }
        Ok(())
    }
}

#[inline]
fn map_local_index(original: usize, top_level_eval: bool) -> usize {
    if top_level_eval {
        return original + 1;
    }
    original
}
