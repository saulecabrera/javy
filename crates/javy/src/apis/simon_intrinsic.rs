use crate::hold;
use crate::quickjs::context::Intrinsic;
use crate::quickjs::{
    prelude::{MutFn, Rest},
    qjs, Array, Ctx, Function, Object, String as JSString, Value,
};
use crate::serde::de::Deserializer;
use crate::Args;
use anyhow::Result;
use jsonbb::{Builder, ValueRef};
use simon::bjson::input;
use std::io::Write;
use std::ptr::NonNull;

pub struct Simon;

fn read_input<'a>(args: Args<'a>) -> Result<Value<'a>> {
    let input = input();
    let ctx = args.0;
    // TODO: Wrongly assuming that all inputs are objects. That's potentially
    // true in the correct cases. However we must validate the type of input and
    // act accordingly.

    // TODO: Here we'd need to introduce the artifact handling, or we could,
    // instead of inlining the artifact handling, call the query property method
    // from Simon's bjson API and let all the artifact handling logic live
    // there.
    let result = Object::new(ctx.clone())?;
    if let Some(oref) = input.ref_.as_object() {
        for (k, v) in oref.iter() {
            result.set(k, handle_value(ctx.clone(), v)?)?;
        }
    }

    Ok(Value::from_object(result))
}

fn write_output<'a>(args: Args<'a>) -> Result<Value<'a>> {
    let ctx = args.0;
    let val = args
        .1
        .first()
        .cloned()
        .unwrap_or_else(|| Value::new_undefined(ctx.clone()));

    let mut de = Deserializer::from(val);
    let mut ser = Builder::default();

    serde_transcode::transcode(&mut de, &mut ser)?;

    let val = ser.finish();

    let mut stdout = std::io::stdout();
    stdout.write(val.as_bytes()).expect("write to succeed");
    stdout.flush().expect("flush to succeed");

    Ok(Value::new_undefined(ctx.clone()))
}

fn handle_value<'js>(ctx: Ctx<'js>, value: ValueRef<'_>) -> Result<Value<'js>> {
    match value {
        ValueRef::String(str) => Ok(Value::from_string(JSString::from_str(ctx.clone(), str)?)),
        ValueRef::Null => Ok(Value::new_null(ctx.clone())),
        ValueRef::Bool(b) => Ok(Value::new_bool(ctx.clone(), b)),
        ValueRef::Number(n) => {
            let number = if n.is_u64() {
                n.as_u64().unwrap() as f64
            } else if n.is_i64() {
                n.as_i64().unwrap() as f64
            } else if n.is_f64() {
                n.as_f64().unwrap()
            } else {
                unreachable!()
            };
            Ok(Value::new_number(ctx.clone(), number))
        }
        ValueRef::Object(oref) => {
            let obj = Object::new(ctx.clone())?;
            for (k, v) in oref.iter() {
                obj.set(k, handle_value(ctx.clone(), v)?)?;
            }
            Ok(Value::from_object(obj))
        }
        ValueRef::Array(a) => {
            let result = Array::new(ctx.clone())?;
            for (i, v) in a.iter().enumerate() {
                result.set(i, handle_value(ctx.clone(), v)?)?;
            }
            Ok(Value::from_array(result))
        }
    }
}

impl Intrinsic for Simon {
    unsafe fn add_intrinsic(cx: NonNull<qjs::JSContext>) {
        register(Ctx::from_raw(cx)).expect("Registering simon to succeed");
    }
}

fn register<'js>(this: Ctx<'js>) -> Result<()> {
    let global = this.globals();
    let simon = Object::new(this.clone())?;

    simon.set(
        "readInput",
        Function::new(
            this.clone(),
            MutFn::new(move |this: Ctx<'js>, args: Rest<Value<'js>>| {
                read_input(hold!(this.clone(), args)).unwrap()
            }),
        ),
    )?;

    simon.set(
        "writeOutput",
        Function::new(
            this.clone(),
            MutFn::new(move |this: Ctx<'js>, args: Rest<Value<'js>>| {
                write_output(hold!(this.clone(), args)).unwrap()
            }),
        ),
    )?;

    global.set("Simon", simon)?;
    Ok(())
}
