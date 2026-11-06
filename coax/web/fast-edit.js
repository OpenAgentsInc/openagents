// Firm reason: 1 (browser SDK glue: instantiate a sealed scalar Wasm module
// and publish its callable between complete JavaScript/Wasm frames).
export function fastEditBoundary(notice = () => {}) {
  let base, active, revision = 0n, requested = 0, timer, calls = 0, publishedCalls = 0, probing = false, lastInput = 0, lastValue = 0;
  const local = ["127.0.0.1", "localhost", "[::1]"].includes(location.hostname);
  const identity = x => Array.from({length: 64}, (_, i) => String.fromCharCode(x.coax_dev_identity_byte?.(i) ?? x.coax_dev_patch_identity_byte?.(i) ?? 0)).join("");
  const info = (sample = 0.25) => {
    probing = true;
    try { return {revision: Number(revision), identity: base ? identity(base) : "", value: base ? Number(base.coax_dev_observe(sample)) : null, calls, game_calls_since_publish: calls - publishedCalls, last_input: lastInput, last_value: lastValue}; }
    finally { probing = false; }
  };
  function dispatch(value) {
    if (!base?.coax_dev_baseline) throw new Error("fast-edit: baseline is not bound");
    const result = active ? active(value) : base.coax_dev_baseline(value);
    if (!probing) { calls++;lastInput = value;lastValue = result; }
    return result;
  }
  async function install(bytes, offer) {
    const request = ++requested, original = base;
    if (!local || !base?.fe_publish_offer) throw new Error("fast-edit: no local sealed baseline");
    const module = await WebAssembly.compile(bytes);
    const allowed = new Set(["fd_write", "fd_seek", "fd_close", "proc_exit", "environ_get", "environ_sizes_get"]);
    let candidateMemory;
    const wasi = Object.fromEntries([...allowed].map(name => [name, () => { throw new Error(`fast-edit: candidate trapped (${name})`); }]));
    wasi.environ_sizes_get = (count, size) => { const view = new DataView(candidateMemory.buffer);view.setUint32(count, 0, true);view.setUint32(size, 0, true);return 0; };
    wasi.environ_get = () => 0;
    for (const imp of WebAssembly.Module.imports(module)) {
      if (imp.module !== "wasi_snapshot_preview1" || imp.kind !== "function" || !allowed.has(imp.name)) throw new Error("fast-edit: unsupported candidate import");
    }
    const candidate = (await WebAssembly.instantiate(module, {wasi_snapshot_preview1: wasi})).exports;
    candidateMemory = candidate.memory;
    candidate._initialize?.();
    const next = candidate.coax_dev_patch_revision?.();
    const same = offer.identity === identity(original) && identity(candidate) === offer.identity;
    const worker = globalThis.coaxial?.roomHost;
    const safe = request === requested && original === base && !(worker?.transport === "wasm-worker" && !worker.stopped);
    if (typeof next !== "bigint" || Number(next) !== offer.revision || typeof candidate.fast_edit_candidate !== "function" ||
        !base.fe_publish_offer(64n, same ? 1 : 0, next, revision, safe ? 1 : 0)) throw new Error("fast-edit: identity, revision or host activity refused");
    // Promise continuations run with no old Wasm frame on the main JS stack.
    await new Promise(requestAnimationFrame);
    const currentWorker = globalThis.coaxial?.roomHost;
    const publishSafe = request === requested && original === base && !(currentWorker?.transport === "wasm-worker" && !currentWorker.stopped);
    if (!base?.fe_publish_offer(64n, same ? 1 : 0, next, revision, publishSafe ? 1 : 0)) throw new Error("fast-edit: baseline or activity changed before publication");
    active = candidate.fast_edit_candidate; revision = next; publishedCalls = calls;
    await new Promise(requestAnimationFrame);
    return info();
  }
  function bind(exports) {
    stop(); base = exports; active = undefined; revision = 0n; calls = publishedCalls = 0; lastInput = lastValue = 0;
    // Only an explicitly instrumented development build has this export.
    // The default endpoint also lets the ordinary drift driver exercise it.
    const url = new URL(location.href).searchParams.get("fast-edit") || (base.coax_dev_baseline ? "/build/fast-edit/latest.json" : "");
    if (!local || !url || !base.coax_dev_baseline || !base.fe_publish_offer) return;
    const endpoint = new URL(url, location.href);
    if (endpoint.origin !== location.origin) throw new Error("fast-edit: offer must be same-origin");
    let busy = false, failedRevision = -1;
    timer = setInterval(async () => {
      if (busy) return; busy = true;
      try {
        const response = await fetch(endpoint, {cache: "no-store"});
        if (!response.ok) return;
        const offer = await response.json();
        const checkResponse = await fetch(new URL("verification.json", endpoint), {cache: "no-store"});
        if (checkResponse.ok) {
          api.verification = await checkResponse.json();
          if (api.verification.revision === Number(revision) && api.verification.state === "failed" && failedRevision !== Number(revision)) {
            failedRevision = Number(revision);api.error = `fast-edit: revision ${revision} failed checks; commit is blocked`;notice(api.error);
          }
        }
        if (offer.revision <= Number(revision)) return;
        const address = new URL(`candidate-${offer.revision}.wasm`, endpoint);
        const binary = await fetch(address, {cache: "no-store"});
        if (!binary.ok) throw new Error("fast-edit: candidate unavailable");
        await install(await binary.arrayBuffer(), offer);
        api.error = "";
      } catch (error) { api.error = String(error.message || error); }
      finally { busy = false; }
    }, 100);
  }
  function stop() { clearInterval(timer); timer = undefined; requested++; base = undefined; active = undefined; }
  const api = {dispatch, bind, install, info, stop, error: "", verification: null};
  return api;
}
