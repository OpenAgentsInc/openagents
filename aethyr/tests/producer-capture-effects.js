// Firm reason: 1 (WebAssembly browser SDK calls and binary fetch transport; Coax supplies imports, allocation, frame order, spans and acceptance).
(() => {
  let pending, module, instance, memory;
  const digest = async bytes => Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)), b => b.toString(16).padStart(2, "0")).join("");
  const raw = value => typeof value === "bigint" ? String(value) : value;
  const start = request => {
    pending = {done: false, error: null};
    (async () => {
      switch (request.operation) {
        case "compile": {
          const bytes = await (await fetch(request.url)).arrayBuffer();
          module = await WebAssembly.compile(bytes);
          return {bytes: bytes.byteLength, sha256: await digest(bytes), imports: WebAssembly.Module.imports(module)};
        }
        case "instantiate": {
          memory = new WebAssembly.Memory({initial: request.initial, maximum: request.maximum});
          const imports = {};
          for (const row of request.imports) {
            const bank = imports[row.module] ||= {};
            if (row.kind === "memory") bank[row.name] = memory;
            else bank[row.name] = (...args) => {
              if (row.effect === "throw") throw Error("producer capture does not provide " + row.module + "." + row.name);
              return row.effect === "bigint" ? BigInt(row.value) : row.effect === "void" ? undefined : row.value;
            };
          }
          instance = await WebAssembly.instantiate(module, imports);
          return {memory_bytes: memory.buffer.byteLength, exports: WebAssembly.Module.exports(module), heap_base: Number(instance.exports.__heap_base?.value ?? 0)};
        }
        case "call": {
          const fn = instance.exports[request.name];
          const args = request.args.map(([low, high]) => {
            const view = new DataView(new ArrayBuffer(8));
            view.setUint32(0, low, true); view.setUint32(4, high, true);
            return view.getFloat64(0, true);
          });
          return {defined: typeof fn === "function", value: typeof fn === "function" ? raw(fn(...args)) ?? null : null};
        }
        case "grow": {
          const before = memory.buffer.byteLength;
          memory.grow(request.pages);
          return {before, after: memory.buffer.byteLength};
        }
        case "write": {
          const bytes = request.url ? new Uint8Array(await (await fetch(request.url)).arrayBuffer()).subarray(request.from, request.from + request.bytes) : Uint8Array.from(atob(request.base64), c => c.charCodeAt(0));
          new Uint8Array(memory.buffer, request.pointer, bytes.length).set(bytes);
          return {bytes: bytes.length, sha256: await digest(bytes)};
        }
        case "read": {
          const bytes = new Uint8Array(memory.buffer, request.pointer, request.bytes).slice();
          const sha256 = await digest(bytes);
          if (request.upload) {
            const response = await fetch(request.upload, {method: "POST", body: bytes});
            return {bytes: bytes.length, sha256, upload_status: response.status, acknowledgement: await response.json()};
          }
          return {bytes: bytes.length, sha256, values: Array.from(bytes)};
        }
      }
      throw Error("unknown WebAssembly SDK operation");
    })().then(value => {pending = {done: true, error: null, value};}, error => {pending = {done: true, error: String(error.stack || error)};});
    return "started";
  };
  globalThis.producerCaptureStart = start;
  globalThis.producerCapturePoll = () => pending;
  return "ready";
})()
