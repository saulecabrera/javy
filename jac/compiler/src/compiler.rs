use crate::builder::{FunctionBuilder, sig};
use anyhow::Result;
use jac_translate::{
    FunctionTranslation, Translation,
    quickpars::{FuncIndex, Opcode},
};
use std::collections::HashMap;
use waffle::{Func, FuncDecl, Module, Signature};

/// Function environment.
/// Regroups some of the fields owned by the compiler in order to pass
/// it to each compilation unit.
pub(crate) struct FuncEnv<'a, 'data> {
    /// QuickJS module translation.
    pub module_translation: &'a Translation<'data>,
    /// QuickJS function translation.
    pub function_translation: &'a FunctionTranslation<'data>,
    /// The resulting WebAssembly module.
    pub result: &'a mut Module<'data>,
    /// Translation function index to WebAssembly index mapping.
    pub defined_funcs: &'a mut HashMap<FuncIndex, (Signature, Func)>,
    /// Known import function index to WebAssembly index mapping.
    pub imported_funcs: &'a mut HashMap<&'static str, (Signature, Func)>,
}

/// QuickJS-bytecode-to-Wasm compiler.
pub(crate) struct Compiler<'data> {
    /// QuickJS bytecode in memory representation.
    translation: Translation<'data>,
    /// The resulting WebAssembly module.
    result: Module<'data>,
    /// Translation function index to WebAssembly index mapping.
    defined_funcs: HashMap<FuncIndex, (Signature, Func)>,
    /// Known import function index to WebAssembly index mapping.
    imported_funcs: HashMap<&'static str, (Signature, Func)>,
}

impl<'data> Compiler<'data> {
    /// Create a new compiler from the translated QuickJS bytecode.
    pub fn new(translation: Translation<'data>) -> Self {
        Self {
            translation,
            result: Module::empty(),
            defined_funcs: Default::default(),
            imported_funcs: Default::default(),
        }
    }

    // TODO: Before starting each function compilation, create the
    //       known module prelude.  e.g., we could start by adding the
    //       functions table with the known number of functions
    //       (`translation.module.functions.len()`)
    /// Perform compilation into Wasm bytes.
    pub fn compile(mut self) -> Result<Vec<u8>> {
        for func in &self.translation.module.functions {
            let sig = if self.defined_funcs.contains_key(&func.index) {
                let (sig_handle, _) = self.defined_funcs[&func.index];
                sig_handle
            } else {
                let sig_data = sig(&self.translation, func);
                let sig = self.result.signatures.push(sig_data);
                let func_handle =
                    self.result
                        .funcs
                        .push(FuncDecl::Body(sig, "".into(), Default::default()));
                self.defined_funcs.insert(func.index, (sig, func_handle));
                sig
            };

            let env = FuncEnv {
                module_translation: &self.translation,
                function_translation: func,
                result: &mut self.result,
                defined_funcs: &mut self.defined_funcs,
                imported_funcs: &mut self.imported_funcs,
            };
            let body = FunctionBuilder::new(env, sig).build()?;
            self.result.funcs[self.defined_funcs[&func.index].1] =
                FuncDecl::Body(sig, "".into(), body);
        }
        self.result.to_wasm_bytes()
    }
}
