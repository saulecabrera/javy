//! Metadata of compiler runtime builtins.

use waffle::{SignatureData, Type};

static MODULE: &'static str = "jacrt";
static LINK_MODULE: &'static str = "jacrt-link";

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub struct RuntimeFunction {
    pub module: &'static str,
    pub name: &'static str,
    pub params: &'static [Type],
    pub rets: Option<Type>,
}

/// All the available runtime function imports.
pub const fn function_imports() -> [RuntimeFunction; 6] {
    [
	init(),
	closure(),
	resolve_non_local_var_ref(),
	new_int32(),
	put_var_ref(),
	get_var_ref(),
    ]
}

/// Metadata for context initialization.
pub const fn init() -> RuntimeFunction {
    RuntimeFunction {
	module: MODULE,
	name: "init",
	params: &[Type::I32],
	rets: Some(Type::I32),
    }
}

/// Metadata for closure creation.
pub const fn closure() -> RuntimeFunction {
    RuntimeFunction {
        module: MODULE,
        name: "closure",
        params: &[Type::I32, Type::I32, Type::I32],
        rets: Some(Type::I64),
    }
}

/// Metadata for resolving a non local var ref.
pub const fn resolve_non_local_var_ref() -> RuntimeFunction {
    RuntimeFunction {
	module: MODULE,
	name: "resolve-non-local-var-ref",
	params: &[Type::I32, Type::I32, Type::I32],
	rets: None,
    }
}

/// Metadata for creating a new i32 value.
pub const fn new_int32() -> RuntimeFunction {
    RuntimeFunction {
	module: MODULE,
	name: "new-int32",
	params: &[Type::I32, Type::I32],
	rets: Some(Type::I64),
    }
}

/// Metadata for setting a variable reference.
pub const fn put_var_ref() -> RuntimeFunction {
    RuntimeFunction {
	module: MODULE,
	name: "put-var-ref",
	params: &[Type::I32, Type::I32, Type::I64],
	rets: None,
    }
}

/// Metadata for getting a variable reference.
pub const fn get_var_ref() -> RuntimeFunction { 
    RuntimeFunction {
	module: MODULE,
	name: "get-var-ref",
	params: &[Type::I32, Type::I32],
	rets: Some(Type::I64),
    }
}

/// Metadata about the functions table.
pub const fn func_table() -> (&'static str, &'static str) {
    (LINK_MODULE, "functions")
}
