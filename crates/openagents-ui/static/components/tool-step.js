// Step rows (openagents-ui shell::Step, Message::thinking): a tool step or
// the model's thinking opens and closes its detail. CSP-safe: one delegated
// listener on `button[data-oa-tool-step]`, which names its detail in
// aria-controls and says whether it is open in aria-expanded.
(() => {
  "use strict";

  document.addEventListener("click", (event) => {
    const target = event.target;
    const button = target instanceof Element ? target.closest("[data-oa-tool-step]") : null;
    if (!button) return;
    const detail = document.getElementById(button.getAttribute("aria-controls") || "");
    if (!detail) return;
    const open = button.getAttribute("aria-expanded") !== "true";
    button.setAttribute("aria-expanded", open ? "true" : "false");
    detail.hidden = !open;
  });
})();
