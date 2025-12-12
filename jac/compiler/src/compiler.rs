use crate::crt;
use crate::frontend::{Frontend, sig};
use anyhow::Result;
use jac_translate::{
    FunctionTranslation, Translation,
    quickpars::{FuncIndex, Opcode},
};
use std::collections::{BTreeMap, HashMap};
use waffle::{
    Func, FuncDecl, Import, ImportKind, Module, Signature, SignatureData, Table, TableData, Type,
    declare_entity, entity::EntityVec,
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
    pub imported_funcs: &'a mut HashMap<&'static str, (Signature, Func)>,
    pub function_table_handle: Table,
    pub function_table: &'a mut EntityVec<FuncRef, Func>,
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
    imported_funcs: HashMap<&'static str, (Signature, Func)>,
    /// Trampoline count for QuickJS to Wasm calls.
    trampoline_count: usize,
    /// Vector of functions to keep track the order in which the
    /// functions that can be invoked by the QuickJS runtime can be
    /// invoked.
    function_table: EntityVec<FuncRef, Func>,
}

impl<'data> Compiler<'data> {
    /// Create a new compiler from the translated QuickJS bytecode.
    pub fn new(translation: Translation<'data>) -> Self {
        let mut function_table: EntityVec<FuncRef, Func> = EntityVec::default();
        let trampoline_count = 1;
        for _ in 0..trampoline_count {
            // Invalid to start, will be patched later.
            function_table.push(Func::default());
        }

        Self {
            translation,
            result: Module::empty(),
            defined_funcs: Default::default(),
            imported_funcs: Default::default(),
            trampoline_count,
            function_table,
        }
    }

    /// Perform compilation into Wasm bytes.
    pub fn compile(mut self) -> Result<Vec<u8>> {
        let table_handle = self.import_functions_table();

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
            };
            let body = Frontend::new(env, sig).build()?;
            self.result.funcs[self.defined_funcs[&func.index].1] =
                FuncDecl::Body(sig, "".into(), body);
        }

        self.patch_functions_table(table_handle);
        self.result.to_wasm_bytes()
    }

    /// Imports the functions table, which will contain all the Wasm
    /// function definitions reachable by QuickJS.
    fn import_functions_table(&mut self) -> Table {
        let table_data = TableData {
            ty: Type::FuncRef,
            initial: 0,
            // The number of functions that "escape" the module
            // through the function table equals the number of defined
            // functions, plus the number of trampolines to enable
            // QuickJS <> Wasm calls.
            max: Some(
                u64::try_from(self.translation.module.functions.len() + self.trampoline_count)
                    .unwrap(),
            ),
            // NB: the elements will be patched later on, once
            // all the compiled functions are known.
            // See: patch functions table.
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

    /// Once compilation is finished, patched the functions table with
    /// the indices to all the compiled functions.
    fn patch_functions_table(&mut self, table: Table) {
        let table_data = &mut self.result.tables[table];
        table_data.func_elements = Some(self.function_table.values().cloned().collect());
    }

    /// Defines a trampoline for QuickJS to Wasm functions.
    /// The trampoline has the following signature:
    /// (context: *mut JSContext, this: JSValue, argc: i32, argv: *mut JSValue, index: i32) -> JSValue
    fn define_qjs_to_wasm_trampoline(&mut self) -> Func {
        let sig = SignatureData {
            params: vec![Type::I32, Type::I64, Type::I32, Type::I32, Type::I32],
            returns: vec![Type::I64],
        };
        let sig_handle = self.result.signatures.push(sig);
        todo!()
    }
}
