use crate::{APIConfig, JSApiSet};
use anyhow::Error;
use javy::{
    hold, hold_and_release, json,
    quickjs::{Function, String as JSString, Value},
    Args,
};
use std::io::{Read, Write};

pub struct Json;

impl JSApiSet for Json {
    fn register(&self, runtime: &javy::Runtime, _: &APIConfig) -> anyhow::Result<()> {
        runtime.context().with(|this| {
            let globals = this.globals();

            globals.set(
                "__javy_json_parse",
                Function::new(this.clone(), |cx, args| {
                    let (cx, args) = hold_and_release!(cx, args);
                    parse(hold!(cx, args))
                }),
            )?;

            globals.set(
                "__javy_json_stringify",
                Function::new(this.clone(), |cx, args| {
                    let (cx, args) = hold_and_release!(cx, args);
                    stringify_value(hold!(cx, args))
                }),
            )?;

            globals.set(
                "__javy_json_from_stdin",
                Function::new(this.clone(), |cx, args| {
                    let (cx, args) = hold_and_release!(cx, args);
                    from_stdin(hold!(cx, args))
                }),
            )?;

            globals.set(
                "__javy_json_to_stdout",
                Function::new(this.clone(), |cx, args| {
                    let (cx, args) = hold_and_release!(cx, args);
                    to_stdout(hold!(cx, args))
                }),
            )?;

            this.eval(include_str!("json.js"))?;
            Ok::<_, Error>(())
        })?;

        Ok(())
    }
}

fn parse(a: Args<'_>) -> Value<'_> {
    let (cx, args) = a.release();

    let string = args[0].as_string().unwrap().to_string().unwrap();
    json::transcode_input(cx.clone(), &string.as_bytes()).unwrap()
}

fn stringify_value(a: Args<'_>) -> Value<'_> {
    let (cx, args) = a.release();

    let bytes = json::transcode_output(args[0].clone()).unwrap();

    let str = std::str::from_utf8(&bytes).unwrap();
    let js_str = JSString::from_str(cx.clone(), str).unwrap();

    Value::from(js_str)
}

fn from_stdin(a: Args<'_>) -> Value<'_> {
    let (cx, _) = a.release();
    let mut bytes = Vec::with_capacity(10000);
    let mut fd = std::io::stdin();
    fd.read_to_end(&mut bytes).unwrap();

    json::transcode_input(cx.clone(), &bytes).unwrap()
}

fn to_stdout(a: Args<'_>) {
    let (_, args) = a.release();
    let mut fd = std::io::stdout();
    let out = json::transcode_output(args[0].clone()).unwrap();
    fd.write_all(&out).unwrap();
}
