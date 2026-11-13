// Firm reason: 1 (WebKit service transport; app transitions and request bodies are Coax).
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
addEventListener("coaxial-dom-ready", event => {
  if (event.detail?.app !== "app") return;
  const dom = event.detail.host, decoder = new TextDecoder();
  const ops = ["", "auth.state", "auth.emailStart", "auth.emailVerify", "http", "auth.logout", "updates.check", "updates.status"];
  dom.after = () => {
    const x = dom.exports(), state = dom.state(), action = x.dom_act_kind(state);
    if (!action) return;
    const body = decoder.decode(new Uint8Array(dom.memory().buffer, x.app_body(state), x.app_body_len(state)));
    x.dom_act_done(state);
    let args; try { args = JSON.parse(body); } catch { dom.send(200 + action, 0, "{}"); return; }
    coaxNative.call(ops[action], args)
      .then(r => dom.send(100 + action, r.status || 0, action === 4 ? r.body : JSON.stringify(r)))
      .catch(e => dom.send(200 + action, 0, JSON.stringify({error: e.message || "OpenAgents is unavailable. Try again."})));
  };
  // Draw the Coax initial frame before draining its first service request.
  dom.now(); dom.after();
  coaxNative.call("ready").catch(() => {});
});
