// Firm reason: 1 (browser SDK calls, capture memory readback, hashing and HTTP byte effects; Coax supplies every option, operation, count, range and verdict).
(() => {
  let pending = { done: true, value: null, error: null };
  const arrays = new Map();
  const number = words => {
    const bytes = new ArrayBuffer(8), view = new DataView(bytes);
    view.setUint32(0, words[0], true); view.setUint32(4, words[1], true);
    return view.getFloat64(0, true);
  };
  const digest = async bytes => [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map(x => x.toString(16).padStart(2, "0")).join("");
  const effect = async request => {
    switch (request.operation) {
      case "step": {
        const rate = number(request.rate_words);
        return request.optional ? globalThis.coaxialClipStep?.(request.count, rate) : globalThis.coaxialClipStep(request.count, rate);
      }
      case "status": return {
        capture: !!globalThis.coaxial?.renderCapture,
        ready: globalThis.coaxial?.goldenStatus?.(),
        render: globalThis.coaxial?.renderStatus,
        loader: document.querySelector("#coaxial-load")?.textContent,
      };
      case "arm": return globalThis.coaxial.captureFrame();
      case "snapshot": {
        const raw = globalThis.coaxial?.renderCapture;
        if (!raw) return { defined: false };
        const capture = { ...raw };
        for (const key of request.drop) delete capture[key];
        for (const entry of request.arrays) {
          const bytes = new Uint8Array(raw[entry.key] ?? []);
          arrays.set(entry.key, bytes);
          capture[entry.key] = { file: entry.file, bytes: bytes.byteLength, sha256: await digest(bytes) };
        }
        return { defined: true, capture, render: globalThis.coaxial?.renderStatus, loader: document.querySelector("#coaxial-load")?.textContent };
      }
      case "upload": {
        const bytes = arrays.get(request.key).subarray(request.from, request.to);
        const response = await fetch(request.url, { method: "POST", headers: { "content-type": "application/octet-stream" }, body: bytes });
        return { status: response.status, ok: response.ok, receipt: await response.json(), sent_bytes: bytes.byteLength };
      }
      default: throw Error("unknown golden SDK operation");
    }
  };
  globalThis.renderGoldenStart = request => {
    pending = { done: false, value: null, error: null };
    Promise.resolve().then(() => effect(request)).then(value => { pending = { done: true, value: value ?? null, error: null }; }, error => { pending = { done: true, value: null, error: String(error?.stack ?? error) }; });
    return "started";
  };
  globalThis.renderGoldenPoll = () => pending;
  return "ready";
})()
