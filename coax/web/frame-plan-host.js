// Firm reason: 1 (browser glue: load the frame plan's reactor and copy a page's counts in, its operations out).
// The frame plan for a page whose engine exports no coaxial_frame_plan_v1 (GitHub #13451):
// build/frame-plan.wasm is metaverse-engine/render/graph/frame-plan.coax's frame_plan_host. host.js reads the
// page's counts from its exports, asks here, and walks the operations as it walks a page's own plan.
const wasmURL = new URL("../build/frame-plan.wasm", import.meta.url);
let opened;
export function openFramePlan() {
  opened ??= (async () => {
    const response = await fetch(wasmURL, { cache: "no-store" });
    if (!response.ok) throw Error(`frame plan reactor: ${response.status}`);
    const module = await WebAssembly.compile(await response.arrayBuffer());
    const memory = new WebAssembly.Memory({ initial: 32, maximum: 256 });
    const imports = { env: { memory } };
    for (const imp of WebAssembly.Module.imports(module))
      if (imp.kind === "function") (imports[imp.module] ||= {})[imp.name] ||= imp.name === "proc_exit" ? (code) => { throw Error(`frame plan reactor exited ${code}`); } : () => 8;
    const x = new WebAssembly.Instance(module, imports).exports;
    x._initialize?.();
    const mem = x.memory || memory, askWords = x.frame_plan_ask_words(), capacity = x.frame_plan_capacity();
    // ask(words): the page's counts (frame-plan.coax FPA: askWords of them) in; the plan's words out, or null when refused
    return Object.freeze({ askWords,
      ask(words) {
        new Uint32Array(mem.buffer, x.frame_plan_ask_ptr(), askWords).set(words.subarray(0, askWords));
        const wrote = x.frame_plan_write();
        return wrote >= 8 && wrote <= capacity ? new Uint32Array(mem.buffer, x.frame_plan_ptr(), wrote) : null;
      } });
  })();
  opened.catch(() => { opened = undefined; });
  return opened;
}
