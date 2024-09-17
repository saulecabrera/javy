use crate::quickjs::context::Intrinsic;
use crate::quickjs::{
    prelude::{MutFn, Rest},
    qjs, Ctx, Function, Object, Value,
};
use anyhow::Result;
use simon_defs::def;
use std::ptr::NonNull;

def!();

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

                let val = unsafe {
                    bjson_value_at_prop(scope.as_int().unwrap() as _, name.as_ptr(), name.len())
                };

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

                let val = unsafe {
                    bjson_value_at_index(
                        scope.as_int().unwrap() as _,
                        index.as_number().unwrap() as usize,
                    )
                };

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

                let ty = unsafe { bjson_value_type(val.as_int().unwrap() as _) };

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

                unsafe { bjson_value_str_len(val.as_int().unwrap() as _) }
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

                unsafe { bjson_value_array_len(val.as_int().unwrap() as _) }
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

                let val = val.as_int().unwrap();
                let len = unsafe { bjson_value_str_len(val as _) };
                let mut buf = vec![0u8; len];

                unsafe { bjson_value_read_str_bytes(val as _, buf.as_mut_ptr(), len) };

                String::from_utf8(buf).unwrap()
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

                let val = unsafe { bjson_value_bool(val.as_int().unwrap() as _) };

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

                assert!(val.is_int());

                unsafe { bjson_value_int(val.as_int().unwrap() as _) }
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

                unsafe { bjson_value_float(val.as_int().unwrap() as _) }
            }),
        ),
    )?;

    global.set("Simon", simon)?;
    Ok(())
}
