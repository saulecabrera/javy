(function () {
    const __javy_json_parse = globalThis.__javy_json_parse;
    const __javy_json_stringify = globalThis.__javy_json_stringify;
    const __javy_json_from_stdin = globalThis.__javy_json_from_stdin;
    const __javy_json_to_stdout = globalThis.__javy_json_to_stdout;

    globalThis.Javy.JSON = {
      parse(v) {
        return __javy_json_parse(v);
      },
      stringify(v) {
        return __javy_json_stringify(v);
      },
      fromStdin() {
        return __javy_json_from_stdin();
      },
      toStdout(v) {
        return __javy_json_to_stdout(v);
      }
    };
    

    Reflect.deleteProperty(globalThis, "__javy_json_parse");
    Reflect.deleteProperty(globalThis, "__javy_json_stringify");
    Reflect.deleteProperty(globalThis, "__javy_json_from_stdin");
    Reflect.deleteProperty(globalThis, "__javy_json_to_stdout");
})();

