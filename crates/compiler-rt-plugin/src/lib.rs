// Single-threaded scenario.
#![allow(static_mut_refs)]

use anyhow::anyhow;
use javy_plugin_api::import_namespace;
use javy_plugin_api::javy::quickjs::{qjs, Ctx, Value};
use std::{
    alloc::{self, Layout},
    cell::OnceCell,
    ffi,
    ptr::{self, NonNull},
};

mod env;
use env::CompilerRuntime;

import_namespace!("javy-compiler-rt");

// Unlike C's realloc, zero-length allocations need not have
// unique addresses, so a zero-length allocation may be passed
// in and also requested, but it's ok to return anything that's
// non-zero to indicate success.
const ZERO_SIZE_ALLOCATION_PTR: *mut u8 = 1 as _;

/// Runtime.
static mut RT: OnceCell<CompilerRuntime> = OnceCell::new();

/// Allocates memory in instance.
///
/// 1. Allocate memory of new_size with alignment.
/// 2. If original_ptr != 0.  
///    a. copy min(new_size, original_size) bytes from original_ptr to new memory.  
///    b. de-allocate original_ptr.
/// 3. Return new memory ptr.
///
/// # Safety
///
/// * `original_ptr` must be 0 or a valid pointer.
/// * If `original_ptr` is not 0, it must be valid for reads of `original_size`
///   bytes.
/// * If `original_ptr` is not 0, it must be properly aligned.
/// * If `original_size` is not 0, it must match the `new_size` value provided
///   in the original `cabi_realloc` call that returned `original_ptr`.
#[export_name = "cabi_realloc"]
unsafe extern "C" fn cabi_realloc(
    original_ptr: *mut u8,
    original_size: usize,
    alignment: usize,
    new_size: usize,
) -> *mut std::ffi::c_void {
    assert!(new_size >= original_size);

    let new_mem = match new_size {
        0 => ZERO_SIZE_ALLOCATION_PTR,
        // this call to `alloc` is safe since `new_size` must be > 0
        _ => alloc::alloc(Layout::from_size_align(new_size, alignment).unwrap()),
    };

    if !original_ptr.is_null() && original_size != 0 {
        ptr::copy_nonoverlapping(original_ptr, new_mem, original_size);
        alloc::dealloc(
            original_ptr,
            Layout::from_size_align(original_size, alignment).unwrap(),
        );
    }
    new_mem as _
}

#[no_mangle]
extern "C" fn init(_var_ref_slots: u32) -> *mut qjs::JSContext {
    let mut runtime = CompilerRuntime::new();
    let _ = runtime.new_env();

    unsafe {
        RT.set(runtime)
            .map_err(|_| anyhow!("Could not initialize the runtime"))
            .unwrap();

        RT.get()
            .expect("Runtime to be initialized")
            .inner
            .context()
            .as_raw()
            .as_ptr()
    }
}

#[export_name = "closure"]
unsafe extern "C" fn closure(
    context: *mut qjs::JSContext,
    _name_ptr: *mut ffi::c_char,
    _name_len: u32,
    argc: u32,
    magic: u32,
) -> qjs::JSValue {
    // TODO: Could use `JS_NewCFunctionMagic`, but it's declared as
    // `static inline ...`
    let func = qjs::JS_NewCFunctionData(
        context,
        Some(callback),
        i32::try_from(argc).unwrap(),
        i32::try_from(magic).unwrap(),
        0i32,
        ptr::null_mut(),
    );

    // TODO: duplicate
    func
}

/// Trampoline function to handle JS-to-Wasm calls.
unsafe extern "C" fn callback(
    context: *mut qjs::JSContext,
    _this: qjs::JSValue,
    _argc: i32,
    _argv: *mut qjs::JSValue,
    _magic: i32,
    _data: *mut qjs::JSValue,
) -> qjs::JSValue {
    println!("Invoked!");
    unsafe { Value::new_undefined(Ctx::from_raw(NonNull::new(context as _).unwrap())).as_raw() }
}

// The following functions are not needed by the compiler runtime
// and exist merely to satisfy the plugin interface.

#[export_name = "initialize-runtime"]
extern "C" fn initialize_runtime() {
    unreachable!()
}

#[export_name = "compile-src"]
extern "C" fn compile_src(_src_ptr: *const u8, _src_len: usize) -> *const u32 {
    unreachable!()
}

#[export_name = "invoke"]
extern "C" fn invoke(
    _bytecode_ptr: *const u8,
    _bytecode_len: usize,
    _fn_name_discriminator: u32,
    _fn_name_ptr: *const u8,
    _fn_name_len: usize,
) {
    unreachable!()
}
