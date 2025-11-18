use javy_plugin_api::{
    import_namespace,
    javy::quickjs::{qjs, Ctx, Type, Value},
    Config,
};
use std::{
    alloc::{self, Layout},
    ptr::{self, NonNull},
};

mod env;
use env::{CompilerRuntime, FuncEnvHandle};

import_namespace!("jacrt");

// Unlike C's realloc, zero-length allocations need not have
// unique addresses, so a zero-length allocation may be passed
// in and also requested, but it's ok to return anything that's
// non-zero to indicate success.
const ZERO_SIZE_ALLOCATION_PTR: *mut u8 = 1 as _;

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

fn context_from_raw<'js>(raw_context: *mut qjs::JSContext) -> Ctx<'js> {
    unsafe { Ctx::from_raw(NonNull::new(raw_context).unwrap()) }
}

#[no_mangle]
extern "C" fn init(var_ref_slots: usize) -> *mut qjs::JSContext {
    CompilerRuntime::init(var_ref_slots, javy_plugin_api::runtime());
    let runtime = javy_plugin_api::runtime();
    runtime.context().as_raw().as_ptr()
}

#[no_mangle]
unsafe extern "C" fn closure(
    raw_context: *mut qjs::JSContext,
    // _name_ptr: *mut ffi::c_char,
    // _name_len: u32,
    argc: u32,
    magic: u32,
) -> qjs::JSValue {
    // TODO: optimize to have a single call for closure + var_ref init.
    let cx = context_from_raw(raw_context);
    let raw_func = qjs::JS_NewCFunctionData(
        cx.as_raw().as_ptr(),
        Some(callback),
        i32::try_from(argc).unwrap(),
        i32::try_from(magic).unwrap(),
        // Data length and pointer are unused.
        // Could have used `JS_NewCFunctionMagic`, but it's
        // declared as `static inline...`
        0i32,
        ptr::null_mut(),
    );
    let crt = CompilerRuntime::mut_from_context(cx.clone());
    crt.push_default_env(FuncEnvHandle::from_usize(magic as usize));
    // Increase reference count.
    qjs::JS_DupValue(cx.as_raw().as_ptr(), raw_func);
    Value::from_raw(cx.clone(), raw_func).as_raw()
}

#[export_name = "resolve-non-local-var-ref"]
unsafe extern "C" fn resolve_non_local_var_ref(
    context: *mut qjs::JSContext,
    func_index: usize,
    index: usize,
) {
    let cx = context_from_raw(context);
    let crt = CompilerRuntime::mut_from_context(cx.clone());
    crt.resolve_non_local_var_ref(index, FuncEnvHandle::from_usize(func_index));
}

#[export_name = "put-var-ref"]
unsafe extern "C" fn put_var_ref(context: *mut qjs::JSContext, index: usize, val: qjs::JSValue) {
    let cx = context_from_raw(context);
    let crt = CompilerRuntime::mut_from_context(cx.clone());
    crt.put_var_ref(index, Value::from_raw(cx.clone(), val))
}

#[export_name = "get-var-ref"]
unsafe extern "C" fn get_var_ref(context: *mut qjs::JSContext, index: usize) -> qjs::JSValue {
    let cx = context_from_raw(context);
    let crt = CompilerRuntime::mut_from_context(cx.clone());
    crt.get_var_ref_value(index).as_raw()
}

#[export_name = "get-var-ref-check"]
unsafe extern "C" fn get_var_ref_check(context: *mut qjs::JSContext, index: usize) -> qjs::JSValue {
    let cx = context_from_raw(context);
    let crt = CompilerRuntime::mut_from_context(cx.clone());
    let result = crt.get_var_ref_value(index);
    if result.type_of() == Type::Uninitialized {
        // TODO: Exception.
        panic!("Must be initialized");
    }
    result.as_raw()
}

#[export_name = "new-int32"]
unsafe extern "C" fn new_int32(context: *mut qjs::JSContext, raw: i32) -> qjs::JSValue {
    let cx = context_from_raw(context);
    Value::new_int(cx.clone(), raw).clone().as_raw()
}

#[no_mangle]
unsafe extern "C" fn undef(context: *mut qjs::JSContext) -> qjs::JSValue {
    let cx = context_from_raw(context);
    Value::new_undefined(cx.clone()).clone().as_raw()
}

#[no_mangle]
unsafe extern "C" fn call(
    context: *mut qjs::JSContext,
    callee: qjs::JSValue,
    // TODO: add rest of the call arguments
) -> qjs::JSValue {
    let cx = context_from_raw(context);
    let result = qjs::JS_Call(
        cx.as_raw().as_ptr(),
        Value::from_raw(cx.clone(), callee).as_raw().clone(),
        Value::new_undefined(cx.clone()).as_raw(),
        0,
        ptr::null_mut(),
    );

    // TODO:Increase ref count.
    Value::from_raw(cx.clone(), result).clone().as_raw()
}

#[no_mangle]
unsafe extern "C" fn mul(
    context: *mut qjs::JSContext,
    lhs: qjs::JSValue,
    rhs: qjs::JSValue,
) -> qjs::JSValue {
    let cx = context_from_raw(context);
    let lhs = Value::from_raw(cx.clone(), lhs);
    let rhs = Value::from_raw(cx.clone(), rhs);

    if lhs.is_number() && rhs.is_number() {
        let result = Value::new_number(
            cx.clone(),
            lhs.as_number().unwrap() * rhs.as_number().unwrap(),
        );
        result.clone().as_raw()
    } else {
        // TODO: handle all the other cases.
        unreachable!()
    }
}

#[link(wasm_import_module = "jacrt-link")]
extern "C" {
    fn inv(index: usize, context: *mut qjs::JSContext) -> qjs::JSValue;
}
/// Trampoline function to handle JS-to-Wasm calls.
unsafe extern "C" fn callback(
    context: *mut qjs::JSContext,
    _this: qjs::JSValue,
    _argc: i32,
    _argv: *mut qjs::JSValue,
    magic: i32,
    _data: *mut qjs::JSValue,
) -> qjs::JSValue {
    println!("coming to the callback");
    let cx = context_from_raw(context);
    let crt = CompilerRuntime::mut_from_context(cx.clone());
    crt.set_current_env(FuncEnvHandle::from_usize(magic as usize));
    // TODO: Frame
    // TODO: Restore current env handle?
    let res = inv(magic as usize, cx.clone().as_raw().as_ptr());
    let res = Value::from_raw(cx.clone(), res).clone();
    dbg!(&res);
    res.as_raw()
}

#[export_name = "initialize-runtime"]
extern "C" fn initialize_runtime() {
    javy_plugin_api::initialize_runtime(|| Config::default(), |rt| rt)
        .expect("Runtime initialization");
}

// The following functions are not needed by the compiler runtime
// and exist merely to satisfy the plugin interface.

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
