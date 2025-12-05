//! Metadata of compiler runtime builtins.

use waffle::{SignatureData, Type};

static MODULE: &'static str = "jacrt";

/// Metadata for context initialization.
pub const fn init() -> (&'static str, &'static str, [Type; 1], Type) {
    (MODULE, "init", [Type::I32], Type::I32)
}

/// Metadata for closure creation.
pub const fn closure() -> (&'static str, &'static str, [Type; 3], Type) {
    (
        MODULE,
        "closure",
        [Type::I32, Type::I32, Type::I32],
        Type::I64,
    )
}

/// Metadata for resolving a non local var ref.
pub const fn resolve_non_local_var_ref() -> (&'static str, &'static str, [Type; 3]) {
    (
        MODULE,
        "resolve-non-local-var-ref",
        [Type::I32, Type::I32, Type::I32],
    )
}

/// Metadata for creating a new i32 value.
pub const fn new_int32() -> (&'static str, &'static str, [Type; 2], Type) {
    (MODULE, "new-int32", [Type::I32, Type::I32], Type::I64)
}

/// Metadata for setting a variable reference.
pub const fn put_var_ref() -> (&'static str, &'static str, [Type; 3]) {
    (MODULE, "put-var-ref", [Type::I32, Type::I32, Type::I64])
}

/// Metadata for getting a variable reference.
pub const fn get_var_ref() -> (&'static str, &'static str, [Type; 2], Type) {
    (MODULE, "get-var-ref", [Type::I32, Type::I32], Type::I64)
}
