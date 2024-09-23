use crate::quickjs::context::Intrinsic;
use crate::quickjs::{
    prelude::{MutFn, Rest},
    qjs, Ctx, Function, Object, Value,
};
use anyhow::Result;
use simon::bjson::BJsonAPI;
use std::ptr::NonNull;

pub struct Simon;

impl Intrinsic for Simon {
    unsafe fn add_intrinsic(cx: NonNull<qjs::JSContext>) {
        register(Ctx::from_raw(cx)).expect("Registering simon to succeed");
    }
}

fn register<'js>(this: Ctx<'js>) -> Result<()> {
    let global = this.globals();
    let simon = Object::new(this.clone())?;

    simon.set(
        "valueAtProp",
        Function::new(
            this.clone(),
            MutFn::new(move |_: Ctx<'js>, args: Rest<Value<'js>>| {
                let scope = &args[0];
                let name = &args[1];

                assert!(scope.is_int());
                assert!(name.is_string());

                // TODO: Handle non-utf8 encoded strings gracefully or add
                // support for this encoding somehow.
                let name = name.as_string().unwrap().to_string().unwrap();

                let api = BJsonAPI;
                let val = api.bjson_value_at_prop(
                    scope.as_int().unwrap() as _,
                    name.as_ptr(),
                    name.len(),
                );

                val as usize
            }),
        ),
    )?;

    simon.set(
        "valueAtIndex",
        Function::new(
            this.clone(),
            MutFn::new(move |_: Ctx<'js>, args: Rest<Value<'js>>| {
                let scope = &args[0];
                let index = &args[1];

                // TODO: Index must be a usize.
                assert!(scope.is_int());

                let api = BJsonAPI;
                let val = api.bjson_value_at_index(
                    scope.as_int().unwrap() as _,
                    index.as_number().unwrap() as usize,
                );

                val as usize
            }),
        ),
    )?;

    simon.set(
        "valueType",
        Function::new(
            this.clone(),
            MutFn::new(move |_: Ctx<'js>, args: Rest<Value<'js>>| {
                let val = &args[0];

                assert!(val.is_int());

                let api = BJsonAPI;
                let ty = api.bjson_value_type(val.as_int().unwrap() as _);

                ty as u8
            }),
        ),
    )?;

    simon.set(
        "valueStrLen",
        Function::new(
            this.clone(),
            MutFn::new(move |_: Ctx<'js>, args: Rest<Value<'js>>| {
                let val = &args[0];

                assert!(val.is_int());

                let api = BJsonAPI;
                api.bjson_value_str_len(val.as_int().unwrap() as _)
            }),
        ),
    )?;

    simon.set(
        "valueArrayLen",
        Function::new(
            this.clone(),
            MutFn::new(move |_: Ctx<'js>, args: Rest<Value<'js>>| {
                let val = &args[0];

                assert!(val.is_int());

                let api = BJsonAPI;
                api.bjson_value_array_len(val.as_int().unwrap() as _)
            }),
        ),
    )?;

    simon.set(
        "valueReadStrBytes",
        Function::new(
            this.clone(),
            MutFn::new(move |_: Ctx<'js>, args: Rest<Value<'js>>| {
                let val = &args[0];

                assert!(val.is_int());

                let api = BJsonAPI;
                let val = val.as_int().unwrap();
                let len = api.bjson_value_str_len(val as _);

                let mut buffer = vec![0; len];
                let ptr = api.bjson_value_read_str_bytes(val as _);
                let slice = unsafe { std::slice::from_raw_parts(ptr as _, len) };
                buffer.copy_from_slice(slice);

                String::from_utf8(buffer).unwrap()
            }),
        ),
    )?;

    simon.set(
        "valueBool",
        Function::new(
            this.clone(),
            MutFn::new(move |cx: Ctx<'js>, args: Rest<Value<'js>>| {
                let val = &args[0];

                assert!(val.is_int());

                let api = BJsonAPI;
                let val = api.bjson_value_bool(val.as_int().unwrap() as _);

                if val == 0 {
                    Value::new_bool(cx.clone(), false)
                } else {
                    Value::new_bool(cx.clone(), true)
                }
            }),
        ),
    )?;

    simon.set(
        "valueInt",
        Function::new(
            this.clone(),
            MutFn::new(move |_: Ctx<'js>, args: Rest<Value<'js>>| {
                let val = &args[0];
                let api = BJsonAPI;

                assert!(val.is_int());

                api.bjson_value_int(val.as_int().unwrap() as _)
            }),
        ),
    )?;

    simon.set(
        "valueFloat",
        Function::new(
            this.clone(),
            MutFn::new(move |_: Ctx<'js>, args: Rest<Value<'js>>| {
                let val = &args[0];

                assert!(val.is_int());

                let api = BJsonAPI;
                api.bjson_value_float(val.as_int().unwrap() as _)
            }),
        ),
    )?;

    global.set("Simon", simon)?;
    Ok(())
}
