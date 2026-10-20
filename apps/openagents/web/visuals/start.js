// Module loading stays in this bridge; Rust owns the slots and their views.
import init, { mount, update, reposition } from "./openagents_web_visuals.js";
var ready = false;
window.addEventListener("oa-visual-slot", function (event) {
  if (ready) {
    var detail = event.detail;
    queueMicrotask(function () { update(detail); });
  }
});
init({ module_or_path: WebAssembly.compileStreaming(fetch(new URL("./openagents_web_visuals_bg.wasm", import.meta.url))) })
  .then(function () {
    mount();
    ready = true;
    window.__oaVisualReady = true;
    Object.values(window.__oaVisualPending || {}).forEach(function (value) { update(value); });
    delete window.__oaVisualPending;
  }).catch(function (error) {
    window.__oaVisualFailed = true;
    console.error("Visuals couldn't start", error);
    window.dispatchEvent(new CustomEvent("oa-visual-failed"));
  });
window.addEventListener("scroll", function () { if (ready) reposition(); }, true);
