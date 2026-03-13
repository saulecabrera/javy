use crate::builder::FunctionBuilder;
use crate::frontend::{Frontend, sig};
use crate::{args, crt};
use anyhow::Result;
use jac_translate::{
    FunctionTranslation, Translation,
    quickpars::{FuncIndex, Opcode},
};
use std::collections::{BTreeMap, HashMap};
use waffle::{Export, ExportKind};
use waffle::{
    Func, FuncDecl, Import, ImportKind, Local, Memory, MemoryArg, MemoryData, Module, Signature,
    SignatureData, Table, TableData, Type, declare_entity,
    entity::{EntityRef, EntityVec},
};

/// A function reference index.
declare_entity!(FuncRef, "funcref");

/// Function environment.
/// Regroups mutable and non-mutable borrows of some of the fields
/// owned by the compiler in order to pass it to each compilation
/// unit.
pub(crate) struct FuncEnv<'a, 'data> {
    pub module_translation: &'a Translation<'data>,
    pub function_translation: &'a FunctionTranslation<'data>,
    pub result: &'a mut Module<'data>,
    pub defined_funcs: &'a mut BTreeMap<FuncIndex, (Signature, Func, Option<FuncRef>)>,
    pub imported_funcs: &'a mut HashMap<crt::RuntimeFunction, (Signature, Func)>,
    pub function_table_handle: Table,
    pub function_table: &'a mut EntityVec<FuncRef, Func>,
    pub memory_handle: Memory,
}

/// QuickJS-bytecode-to-Wasm compiler.
pub(crate) struct Compiler<'data> {
    /// QuickJS bytecode in memory representation.
    translation: Translation<'data>,
    /// The resulting WebAssembly module.
    result: Module<'data>,
    /// Translation function index to WebAssembly index mapping.
    defined_funcs: BTreeMap<FuncIndex, (Signature, Func, Option<FuncRef>)>,
    /// Known import function index to WebAssembly index mapping.
    imported_funcs: HashMap<crt::RuntimeFunction, (Signature, Func)>,
    /// Vector of functions to keep track the order in which the
    /// functions that can be invoked by the QuickJS runtime can be
    /// invoked.
    function_table: EntityVec<FuncRef, Func>,
}

impl<'data> Compiler<'data> {
    /// Create a new compiler from the translated QuickJS bytecode.
    pub fn new(translation: Translation<'data>) -> Self {
        let mut function_table: EntityVec<FuncRef, Func> = EntityVec::default();
        Self {
            translation,
            result: Module::empty(),
            defined_funcs: Default::default(),
            imported_funcs: Default::default(),
            function_table,
        }
    }

    /// Perform compilation into Wasm bytes.
    pub fn compile(mut self) -> Result<(Vec<u8>, usize)> {
        let (table_handle, memory_handle) = self.add_runtime_imports();

        for func in &self.translation.module.functions {
            let sig = if self.defined_funcs.contains_key(&func.index) {
                let (sig_handle, _, _) = self.defined_funcs[&func.index];
                sig_handle
            } else {
                let sig_data = sig(&self.translation, func);
                let sig = self.result.signatures.push(sig_data);
                let func_handle =
                    self.result
                        .funcs
                        .push(FuncDecl::Body(sig, "".into(), Default::default()));
                self.defined_funcs
                    .insert(func.index, (sig, func_handle, None));
                sig
            };

            let env = FuncEnv {
                module_translation: &self.translation,
                function_translation: func,
                result: &mut self.result,
                defined_funcs: &mut self.defined_funcs,
                imported_funcs: &mut self.imported_funcs,
                function_table: &mut self.function_table,
                function_table_handle: table_handle,
                memory_handle,
            };
            let body = Frontend::new(env, sig).build()?;
            self.result.funcs[self.defined_funcs[&func.index].1] =
                FuncDecl::Body(sig, "".into(), body);

            if func.is_top_level_eval(&self.translation) {
                let export = Export {
                    name: "_start".into(),
                    kind: ExportKind::Func(self.defined_funcs[&func.index].1),
                };
                self.result.exports.push(export);
            }
        }

        self.patch_functions_table(table_handle, memory_handle);
        Ok((self.result.to_wasm_bytes()?, self.function_table.len()))
    }

    /// Adds all the runtime imports to the module.
    fn add_runtime_imports(&mut self) -> (Table, Memory) {
        for builtin in crt::function_imports() {
            let signature_data = SignatureData {
                params: builtin.params.into(),
                returns: builtin.rets.map(|t| vec![t]).unwrap_or_default(),
            };

            let sig_handle = self.result.signatures.push(signature_data);
            let func_handle = self
                .result
                .funcs
                .push(FuncDecl::Import(sig_handle, builtin.name.into()));
            self.result.imports.push(Import {
                module: builtin.module.into(),
                name: builtin.name.into(),
                kind: ImportKind::Func(func_handle),
            });
            self.imported_funcs
                .insert(builtin, (sig_handle, func_handle));
        }

        (self.import_functions_table(), self.import_memory())
    }

    /// Imports the functions table, which will contain all the Wasm
    /// function definitions reachable by QuickJS.
    fn import_functions_table(&mut self) -> Table {
        let table_data = TableData {
            ty: Type::FuncRef,
            // NB: the table details will be patched later on, once
            // all the compiled functions are known.
            // See: `patch_functions_table`
            initial: 0,
            max: None,
            func_elements: None,
        };

        let table_handle = self.result.tables.push(table_data);
        let (module, name) = crt::func_table();
        self.result.imports.push(Import {
            module: module.into(),
            name: name.into(),
            kind: ImportKind::Table(table_handle),
        });

        table_handle
    }

    /// Imports the runtime's memory.
    fn import_memory(&mut self) -> Memory {
        let memory_data = MemoryData {
            initial_pages: 0,
            maximum_pages: None,
            segments: vec![],
        };

        let memory_handle = self.result.memories.push(memory_data);
        let (module, name) = crt::memory();
        self.result.imports.push(Import {
            module: module.into(),
            name: name.into(),
            kind: ImportKind::Memory(memory_handle),
        });

        memory_handle
    }

    /// Once compilation is finished, patch the functions table with
    /// the indices of the trampoline generated for each compiled
    /// function.
    fn patch_functions_table(&mut self, table: Table, memory: Memory) {
        let mut defined_trampolines = vec![];

        for (table_idx, func_handle) in self.function_table.clone().entries() {
            defined_trampolines.push(self.wrap(*func_handle, table, memory));
        }

        let table_data = &mut self.result.tables[table];
        let elem_count: u64 = u64::try_from(self.function_table.len()).unwrap();
        table_data.initial = elem_count;
        table_data.max = Some(elem_count);
        table_data.func_elements = Some(defined_trampolines);
    }

    /// Defines a trampoline for QuickJS to Wasm functions.
    /// The trampoline has the following signature:
    /// (context: *mut JSContext, this: JSValue, argc: i32, argv: *mut JSValue) -> JSValue
    fn wrap(&mut self, inner: Func, table: Table, memory: Memory) -> Func {
        let (trampoline_params, trampoline_ret) = crt::trampoline_signature();
        let sig = SignatureData {
            params: trampoline_params.into(),
            returns: vec![trampoline_ret],
        };
        let sig_handle = self.result.signatures.push(sig.clone());
        let mut builder = FunctionBuilder::new(sig_handle);
        builder.result.n_params = sig.params.len();
        builder.result.rets = sig.returns.clone();

        for p in &sig.params {
            builder.result.locals.push((*p).into());
        }

        let entry = builder.result.add_block();
        builder.result.entry = entry;
        builder.seal(entry);
        builder.switch_to_block(entry);

        for (local, ty) in builder.result.locals.clone().entries() {
            let val = builder.result.add_blockparam(entry, *ty);
            builder.declare_local(local, *ty);
            builder.def_local(local, val);
        }

        // Declare locals for each of the callee's params.
        let callee = &self.result.funcs[inner];
        let callee_sig = callee.sig();
        let callee_sig_data = &self.result.signatures[callee_sig];
        let arg_locals_index = builder.result.locals.len();

        // Declare 1 local per callee argument, except for the first
        // one which is reserved for the JSContext and which we
        // already have access to.
        for p in &callee_sig_data.params[1..] {
            let ty: Type = (*p).into();
            let local = builder.result.locals.push(ty).into();
            builder.declare_local(local, ty);
        }

        // The first parameter is reserved for the `JSContext`.
        let args_len = callee_sig_data.params.len().checked_sub(1).unwrap_or(0);
        // All values are represented as i64.
        let value_size = 8;

        // Check if argc >= args_len (all arguments provided).
        // argc is at local index 2.
        let argc = builder.use_local(Local::new(2));
        let args_len_const = builder
            .instr_builder()
            .i32const(u32::try_from(args_len).unwrap());
        let has_all_args = builder.instr_builder().i32ge_u(argc, args_len_const);

        let fast_path = builder.result.add_block();
        let slow_path = builder.result.add_block();
        let call_block = builder.result.add_block();
        builder.branch_if(has_all_args, fast_path, slow_path);

        // Fast path: load all arguments from argv.
        builder.seal(fast_path);
        builder.switch_to_block(fast_path);
        let argv_ptr = builder.use_local(Local::new(3));
        for i in 0..args_len {
            let offset_val = builder
                .instr_builder()
                .i32const(u32::try_from(i * value_size).unwrap());
            let addr_val = builder.instr_builder().i32add(argv_ptr, offset_val);
            let loaded_val = builder.instr_builder().i64load(
                MemoryArg {
                    align: 0,
                    offset: 0,
                    memory,
                },
                addr_val,
            );
            let arg_local = Local::new(arg_locals_index + i);
            builder.def_local(arg_local, loaded_val);
        }
        builder.branch(call_block);

        builder.seal(slow_path);
        builder.switch_to_block(slow_path);
        let slow_argv_ptr = builder.use_local(Local::new(3));
        let slow_argc = builder.use_local(Local::new(2));
        for i in 0..args_len {
            let arg_local = Local::new(arg_locals_index + i);
            let idx = builder.instr_builder().i32const(u32::try_from(i).unwrap());
            // i >= argc means this argument was not provided.
            let is_missing = builder.instr_builder().i32ge_u(idx, slow_argc);

            let load_block = builder.result.add_block();
            let undef_block = builder.result.add_block();
            let merge_block = builder.result.add_block();
            builder.branch_if(is_missing, undef_block, load_block);

            // Load from argv.
            builder.seal(load_block);
            builder.switch_to_block(load_block);
            let offset_val = builder
                .instr_builder()
                .i32const(u32::try_from(i * value_size).unwrap());
            let addr_val = builder.instr_builder().i32add(slow_argv_ptr, offset_val);
            let loaded_val = builder.instr_builder().i64load(
                MemoryArg {
                    align: 0,
                    offset: 0,
                    memory,
                },
                addr_val,
            );
            builder.def_local(arg_local, loaded_val);
            builder.branch(merge_block);

            // Use undefined.
            builder.seal(undef_block);
            builder.switch_to_block(undef_block);
            let undef_val = builder.instr_builder().mkval(crt::JS_TAG_UNDEFINED, 0);
            builder.def_local(arg_local, undef_val);
            builder.branch(merge_block);

            builder.seal(merge_block);
            builder.switch_to_block(merge_block);
        }
        builder.branch(call_block);

        builder.seal(call_block);
        builder.switch_to_block(call_block);
        let mut args = vec![builder.use_local(args::context())];
        for i in 0..args_len {
            let arg_local = Local::new(arg_locals_index + i);
            args.push(builder.use_local(arg_local));
        }
        let return_val =
            builder
                .instr_builder()
                .call(inner, &args, callee_sig_data.returns.first().copied());

        // Exit
        let exit = builder.result.add_block();
        builder.branch(exit);
        builder.seal(exit);
        builder.exit(exit);
        builder.switch_to_block(exit);
        builder.instr_builder().ret(&[return_val]);
        self.result
            .funcs
            .push(FuncDecl::Body(sig_handle, "".into(), builder.result))
    }
}
