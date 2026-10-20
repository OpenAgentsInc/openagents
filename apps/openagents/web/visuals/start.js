// Module loading stays in this bridge; Rust owns the slots and their views.
import { loadRenderer } from "./renderer.js";
var appModule;
var ready = false;
window.addEventListener("oa-visual-slot", function (event) {
  if (ready) {
    var detail = event.detail;
    queueMicrotask(function () { appModule.update(detail); });
  }
});
loadRenderer(new URL("./openagents_web_visuals.js", import.meta.url), new URL("./openagents_web_visuals_bg.wasm", import.meta.url), new URL("./openagents_web_visuals_webgl.js", import.meta.url), new URL("./openagents_web_visuals_webgl_bg.wasm", import.meta.url))
  .then(function (module) {
    appModule = module;
    appModule.mount();
    ready = true;
    window.__oaVisualReady = true;
    Object.values(window.__oaVisualPending || {}).forEach(function (value) { appModule.update(value); });
    delete window.__oaVisualPending;
  }).catch(function (error) {
    window.__oaVisualFailed = true;
    console.error("Visuals couldn't start", error);
    window.dispatchEvent(new CustomEvent("oa-visual-failed"));
  });
window.addEventListener("scroll", function () { if (ready) appModule.reposition(); }, true);
