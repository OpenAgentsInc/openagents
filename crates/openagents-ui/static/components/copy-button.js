// CopyButton hook for openagents-ui (actions::CopyButton). CSP-safe: one
// delegated listener, no inline handlers, no eval. Load with
// <script src="..." defer>.
(() => {
  "use strict";

  const COPIED_MS = 1300;

  const fallbackCopy = (text) => {
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    let ok = false;
    try {
      ok = document.execCommand("copy");
    } catch (_) {
      ok = false;
    }
    area.remove();
    return ok;
  };

  const copyText = async (text) => {
    if (navigator.clipboard && window.isSecureContext) {
      try {
        await navigator.clipboard.writeText(text);
        return true;
      } catch (_) {
        // Fall through to the textarea path.
      }
    }
    return fallbackCopy(text);
  };

  document.addEventListener("click", async (event) => {
    const target = event.target;
    const button = target instanceof Element ? target.closest("[data-oa-copy]") : null;
    if (!button || button.hasAttribute("data-copied") || button.hasAttribute("data-disabled")) {
      return;
    }

    let text = button.getAttribute("data-oa-copy") || "";
    const fromId = button.getAttribute("data-oa-copy-from");
    if (fromId) {
      const source = document.getElementById(fromId);
      if (source) text = source.textContent || "";
    }

    if (!(await copyText(text))) return;

    button.setAttribute("data-copied", "");
    const status = button.querySelector(".oa-copy-button-status");
    if (status) status.textContent = button.getAttribute("data-oa-copied-label") || "Copied";

    window.setTimeout(() => {
      button.removeAttribute("data-copied");
      if (status) status.textContent = "";
    }, COPIED_MS);
  });
})();
