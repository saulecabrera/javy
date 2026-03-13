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

/// Signature for the trampoline functions stored in the functions table.
/// (context: *mut JSContext, this: JSValue, argc: i32, argv: *mut JSValue) -> JSValue
pub const fn trampoline_signature() -> ([Type; 4], Type) {
    ([Type::I32, Type::I64, Type::I32, Type::I32], Type::I64)
}

/// Signature for the invoke function, exported from the linking artifact.
pub const fn invoke_signature() -> ([Type; 5], Type) {
    (
        [Type::I32, Type::I32, Type::I64, Type::I32, Type::I32],
        Type::I64,
    )
}

/// All the available runtime function imports.
pub const fn function_imports() -> [RuntimeFunction; 12] {
    [
        init(),
        closure(),
        resolve_non_local_var_ref(),
        new_number(),
        put_var_ref(),
        get_var_ref(),
        get_var_ref_check(),
        mul(),
        lt(),
        call(),
        cabi_realloc(),
        to_bool(),
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
        params: &[Type::I32, Type::I32, Type::I32, Type::I32],
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
pub const fn new_number() -> RuntimeFunction {
    RuntimeFunction {
        module: MODULE,
        name: "JS_NewNumber",
        params: &[Type::I32, Type::F64],
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

/// Metadata for multiplication.
pub const fn mul() -> RuntimeFunction {
    RuntimeFunction {
        module: MODULE,
        name: "JS_Mul",
        params: &[Type::I32, Type::I64, Type::I64],
        rets: Some(Type::I64),
    }
}

/// Metadata for multiplication.
pub const fn lt() -> RuntimeFunction {
    RuntimeFunction {
        module: MODULE,
        name: "JS_Lt",
        params: &[Type::I32, Type::I64, Type::I64],
        rets: Some(Type::I64),
    }
}

/// Metadata for getting a variable reference with an initialization check.
pub const fn get_var_ref_check() -> RuntimeFunction {
    RuntimeFunction {
        module: MODULE,
        name: "get-var-ref-check",
        params: &[Type::I32, Type::I32],
        rets: Some(Type::I64),
    }
}

/// Metadata for calling a JS function.
/// Arguments:
/// - context: pointer to JSContext
/// - callee: the JS function to call (JSValue)
/// - this_obj: the caller context (JSValue)
/// - argc: number of arguments
/// - argv: pointer to an array of JSValues in the runtime's memory
pub const fn call() -> RuntimeFunction {
    RuntimeFunction {
        module: MODULE,
        name: "JS_Call",
        params: &[Type::I32, Type::I64, Type::I64, Type::I32, Type::I32],
        rets: Some(Type::I64),
    }
}

/// Metadata for converting a JSValue to a boolean.
pub const fn to_bool() -> RuntimeFunction {
    RuntimeFunction {
        module: MODULE,
        name: "JS_ToBool",
        params: &[Type::I32, Type::I64],
        rets: Some(Type::I32),
    }
}

/// Metadata for memory allocation (cabi_realloc).
/// Arguments:
/// - original_ptr: pointer to original memory (0 for new allocation)
/// - original_size: size of original memory (0 for new allocation)
/// - alignment: alignment requirement
/// - new_size: size of new allocation
/// Returns: pointer to allocated memory
pub const fn cabi_realloc() -> RuntimeFunction {
    RuntimeFunction {
        module: MODULE,
        name: "cabi_realloc",
        params: &[Type::I32, Type::I32, Type::I32, Type::I32],
        rets: Some(Type::I32),
    }
}

/// QuickJS tag for integer values.
pub const JS_TAG_INT: u64 = 0;

/// QuickJS tag for undefined values.
pub const JS_TAG_UNDEFINED: u64 = 3;

/// Metadata about the functions table.
pub const fn func_table() -> (&'static str, &'static str) {
    (LINK_MODULE, "functions")
}

/// Metadata about the runtime's memory.
pub const fn memory() -> (&'static str, &'static str) {
    (MODULE, "memory")
}
