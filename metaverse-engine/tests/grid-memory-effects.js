// Firm reason: 1 (WebAssembly browser SDK instantiation, scalar export reads and memory.grow effects; Coax supplies every bound and verdict).
(() => {
  let pending;
  let memory;
  const poll = () => pending;
  const start = request => {
    pending = { done: false, error: null };
    (async () => {
      if (request.operation === "instantiate") {
        const bytes = await (await fetch(request.url)).arrayBuffer();
        const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
        const wasm_sha256 = Array.from(digest, byte => byte.toString(16).padStart(2, "0")).join("");
        const module = await WebAssembly.compile(bytes);
        memory = new WebAssembly.Memory({ initial: request.initial, maximum: request.maximum });
        const effects = [], imports = {};
        for (const entry of WebAssembly.Module.imports(module)) {
          const bank = imports[entry.module] ||= {};
          if (entry.kind === "memory") bank[entry.name] = memory;
          else if (entry.kind === "function") bank[entry.name] = () => { effects.push(entry.module + "." + entry.name); throw Error("unexpected platform effect during scalar measurement"); };
          else throw Error("unsupported import kind");
        }
        const instance = await WebAssembly.instantiate(module, imports), x = instance.exports;
        x._initialize?.();
        return { wasm_bytes: bytes.byteLength, wasm_sha256, heap_base: Number(x.__heap_base.value), state_size: Number(x.coaxial_state_size()),
          max_vertices: Number(x.coaxial_max_vertices()), vertex_floats: Number(x.coaxial_vertex_floats()),
          uniform_floats: Number(x.coaxial_uniform_floats()), memory_bytes: memory.buffer.byteLength, effects };
      }
      const before = memory.buffer.byteLength;
      let growth_error = null;
      try { memory.grow(request.grow_pages); } catch (error) { growth_error = String(error); }
      return { before_memory_bytes: before, final_memory_bytes: memory.buffer.byteLength, growth_error };
    })().then(value => { pending = { done: true, error: null, value }; }, error => { pending = { done: true, error: String(error.stack || error) }; });
    return "started";
  };
  globalThis.gridMemoryStart = start;
  globalThis.gridMemoryPoll = poll;
  return "ready";
})()
