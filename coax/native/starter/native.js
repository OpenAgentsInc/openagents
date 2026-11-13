// Firm reason: 1 (WebKit service transport and DOM-host handoff; app decisions stay in Coax).
const nativeFetch = globalThis.fetch.bind(globalThis);
globalThis.coaxNative = {
  call: (op, args = {}) => globalThis.webkit?.messageHandlers?.coaxNative
    ? webkit.messageHandlers.coaxNative.postMessage({op, ...args})
    : Promise.reject(new Error("Native service unavailable")),
};
globalThis.fetch = async (url, options = {}) => {
  const path = String(url);
  if (path.startsWith("/api/") && globalThis.webkit?.messageHandlers?.coaxNative) {
    const r = await coaxNative.call("http", {path, method: options.method || "GET", body: String(options.body || "")});
    return new Response(r.body, {status: r.status, headers: {"Content-Type": r.contentType || "application/json"}});
  }
  return nativeFetch(url, options);
};
addEventListener("coaxial-dom-ready", () => {
  const dom = globalThis.coaxialDom;
  if (!dom) return;
  const note = value => dom.send(20, 0, String(value));
  dom.after = () => {
    const x = dom.exports(), state = dom.state(), action = x.dom_act_kind?.(state);
    if (!action) return;
    x.dom_act_done(state);
    coaxNative.call(action === 1 ? "updates.check" : "updates.status").then(r => note(r.message)).catch(e => note(e.message));
  };
  coaxNative.call("ready").then(r => note(r.message)).catch(() => {});
});
