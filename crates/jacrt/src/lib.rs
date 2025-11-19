use javy_plugin_api::{
    Config, import_namespace,
    javy::quickjs::{Ctx, Type, Value, qjs},
};
use std::ptr::{self, NonNull};

mod env;
use env::{CompilerRuntime, FuncEnvHandle};

import_namespace!("jacrt");

fn context_from_raw<'js>(raw_context: *mut qjs::JSContext) -> Ctx<'js> {
    // SAFETY
    // The documentation for `Ctx::from_raw` states:
    //
    // User must ensure that a lock was acquired over the runtime and
    // that invariant is a unique lifetime which can’t be coerced to a
    // lifetime outside the scope of the lock of to the lifetime of
    // another runtime.
    //
    // Note that in our use-case:
    // There's 1:1 relationship between Runtime and Context, which
    // prevents the Runtime from getting acquired elsewhere; which by
    // consequence also enforces a 1:1 relationship between Runtime
    // and Context in terms of lifetime.
    unsafe { Ctx::from_raw(NonNull::new(raw_context).unwrap()) }
}

#[unsafe(no_mangle)]
extern "C" fn init(var_ref_slots: usize) -> *mut qjs::JSContext {
    CompilerRuntime::init(var_ref_slots, javy_plugin_api::runtime());
    let runtime = javy_plugin_api::runtime();
    runtime.context().as_raw().as_ptr()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn closure(
    raw_context: *mut qjs::JSContext,
    // _name_ptr: *mut ffi::c_char,
    // _name_len: u32,
    argc: u32,
    magic: u32,
) -> qjs::JSValue {
    // TODO: optimize to have a single call for closure + var_ref init.
    let cx = context_from_raw(raw_context);
    let raw_func = unsafe {
        qjs::JS_NewCFunctionData(
            cx.as_raw().as_ptr(),
            Some(callback),
            i32::try_from(argc).unwrap(),
            i32::try_from(magic).unwrap(),
            // Data length and pointer are unused.
            // Could have used `JS_NewCFunctionMagic`, but it's
            // declared as `static inline...`
            0i32,
            ptr::null_mut(),
        )
    };
    let crt = CompilerRuntime::mut_from_context(cx.clone());
    crt.push_default_env(FuncEnvHandle::from_usize(magic as usize));
    // Increase reference count.
    unsafe { qjs::JS_DupValue(cx.as_raw().as_ptr(), raw_func) };
    unsafe { Value::from_raw(cx.clone(), raw_func).as_raw() }
}

#[unsafe(export_name = "resolve-non-local-var-ref")]
unsafe extern "C" fn resolve_non_local_var_ref(
    context: *mut qjs::JSContext,
    func_index: usize,
    index: usize,
) {
    let cx = context_from_raw(context);
    let crt = CompilerRuntime::mut_from_context(cx.clone());
    crt.resolve_non_local_var_ref(index, FuncEnvHandle::from_usize(func_index));
}

#[unsafe(export_name = "put-var-ref")]
unsafe extern "C" fn put_var_ref(context: *mut qjs::JSContext, index: usize, val: qjs::JSValue) {
    let cx = context_from_raw(context);
    let crt = CompilerRuntime::mut_from_context(cx.clone());
    crt.put_var_ref(index, unsafe { Value::from_raw(cx.clone(), val) })
}

#[unsafe(export_name = "get-var-ref")]
unsafe extern "C" fn get_var_ref(context: *mut qjs::JSContext, index: usize) -> qjs::JSValue {
    let cx = context_from_raw(context);
    let crt = CompilerRuntime::mut_from_context(cx.clone());
    crt.get_var_ref_value(index).as_raw()
}

#[unsafe(export_name = "get-var-ref-check")]
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

#[unsafe(export_name = "new-int32")]
unsafe extern "C" fn new_int32(context: *mut qjs::JSContext, raw: i32) -> qjs::JSValue {
    let cx = context_from_raw(context);
    Value::new_int(cx.clone(), raw).clone().as_raw()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn undef(context: *mut qjs::JSContext) -> qjs::JSValue {
    let cx = context_from_raw(context);
    Value::new_undefined(cx.clone()).clone().as_raw()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn call(
    context: *mut qjs::JSContext,
    callee: qjs::JSValue,
    // TODO: add rest of the call arguments
) -> qjs::JSValue {
    let cx = context_from_raw(context);
    let result = unsafe {
        qjs::JS_Call(
            cx.as_raw().as_ptr(),
            Value::from_raw(cx.clone(), callee).as_raw().clone(),
            Value::new_undefined(cx.clone()).as_raw(),
            0,
            ptr::null_mut(),
        )
    };

    // TODO:Increase ref count.
    unsafe { Value::from_raw(cx.clone(), result).clone().as_raw() }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mul(
    context: *mut qjs::JSContext,
    lhs: qjs::JSValue,
    rhs: qjs::JSValue,
) -> qjs::JSValue {
    let cx = context_from_raw(context);
    let lhs = unsafe { Value::from_raw(cx.clone(), lhs) };
    let rhs = unsafe { Value::from_raw(cx.clone(), rhs) };

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
unsafe extern "C" {
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
    let cx = context_from_raw(context);
    let crt = CompilerRuntime::mut_from_context(cx.clone());
    crt.set_current_env(FuncEnvHandle::from_usize(magic as usize));
    // TODO: Frame
    // TODO: Restore current env handle?
    let res = unsafe { inv(magic as usize, cx.clone().as_raw().as_ptr()) };
    let res = unsafe { Value::from_raw(cx.clone(), res).clone() };
    res.as_raw()
}

#[unsafe(export_name = "initialize-runtime")]
extern "C" fn initialize_runtime() {
    javy_plugin_api::initialize_runtime(|| Config::default(), |rt| rt)
        .expect("Runtime initialization");
}
