use crate::{args, builder::FunctionBuilder, crt};
use anyhow::Result;
use waffle::{
    Export, ExportKind, FuncDecl, Local, Module, Signature, SignatureData, Table, TableData, Type,
    entity::EntityRef,
};

/// Responsible for emitting the main linking artifact.
/// The main linking artifact is a module that consists of the following
/// exports:
/// * A function table
/// * An `inv` function, which performs a call indirect.
///
/// (module
///   (type (func (param i32 i64 i32 i32) (result i64)))
///   (table (export "functions") 1 1 funcref)
///   ;; Invoke previously declared functions
///   ;; param 0 = func index
///   ;; param 1 = context
///   ;; param 2 = this
///   ;; param 3 = argc
///   ;; param 3 = argv
///   (func (export "inv") (param i32 i32 i64 i32 i32) (result i64)
///     (local.get 1)
///     (local.get 2)
///     (local.get 3)
///     (local.get 4)
/// 	(call_indirect (type 0) (local.get 0))
///   )
/// )
pub struct Linkage {
    functions_table_len: usize,
    result: Module<'static>,
    builder: FunctionBuilder,
    table: Table,
    call_indirect_sig: Signature,
}

impl Linkage {
    pub fn new(len: usize) -> Self {
        let mut module = Module::empty();
        let (params, ret) = crt::invoke_signature();
        let sig_data = SignatureData {
            params: params.into(),
            returns: vec![ret],
        };
        let sig_handle = module.signatures.push(sig_data.clone());

        let mut builder = FunctionBuilder::new(sig_handle);
        builder.result.n_params = sig_data.params.len();
        builder.result.rets = sig_data.returns.clone();

        for p in &sig_data.params {
            builder.result.locals.push((*p).into());
        }

        let table_len = u64::try_from(len).unwrap();
        let table_data = TableData {
            ty: Type::FuncRef,
            initial: table_len,
            max: Some(table_len),
            func_elements: None,
        };
        let table = module.tables.push(table_data);
        let table_export = Export {
            name: "functions".into(),
            kind: ExportKind::Table(table),
        };
        module.exports.push(table_export);

        let (trampoline_params, trampoline_ret) = crt::trampoline_signature();
        let call_indirect_sig = module.signatures.push(SignatureData {
            params: trampoline_params.into(),
            returns: vec![trampoline_ret],
        });

        Self {
            functions_table_len: len,
            result: module,
            builder,
            table,
            call_indirect_sig,
        }
    }

    pub fn emit(mut self) -> Result<Vec<u8>> {
        let entry = self.builder.result.add_block();
        self.builder.result.entry = entry;
        self.builder.seal(entry);
        self.builder.switch_to_block(entry);

        for (local, ty) in self.builder.result.locals.clone().entries() {
            let val = self.builder.result.add_blockparam(entry, *ty);
            self.builder.declare_local(local, *ty);
            self.builder.def_local(local, val);
        }

        let func_index = self.builder.use_local(args::context());
        let context = self.builder.use_local(Local::new(1));
        let this = self.builder.use_local(Local::new(2));
        let argc = self.builder.use_local(Local::new(3));
        let argv = self.builder.use_local(Local::new(4));

        let ret = self.builder.instr_builder().call_indirect(
            self.call_indirect_sig,
            self.table,
            &[context, this, argc, argv, func_index],
            Some(Type::I64),
        );

        // Exit block.
        let exit = self.builder.result.add_block();
        self.builder.branch(exit);
        self.builder.seal(exit);
        self.builder.exit(exit);
        self.builder.switch_to_block(exit);
        self.builder.instr_builder().ret(&[ret]);

        let sig = self.builder.sig();
        let func = self
            .result
            .funcs
            .push(FuncDecl::Body(sig, "".into(), self.builder.result));
        self.result.exports.push(Export {
            name: "inv".into(),
            kind: ExportKind::Func(func),
        });

        Ok(self.result.to_wasm_bytes()?)
    }
}
