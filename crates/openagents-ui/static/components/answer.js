// Answer tabs (content/answer.rs): start a tab set on the tab named for the
// reader's system (data-oa-os="windows" on Windows). The tabs work without
// this script; it only picks the first one shown. CSP-safe: no inline
// handlers, no eval. Load with <script src="..." defer>.
(() => {
  "use strict";

  const platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
  const os = /win/i.test(platform) ? "windows" : "unix";

  const pick = (root) => {
    const scope = root instanceof Element || root instanceof Document ? root : document;
    for (const input of scope.querySelectorAll(".oa-tabs__input[data-oa-os]:not([data-oa-picked])")) {
      input.setAttribute("data-oa-picked", "");
      if (input.getAttribute("data-oa-os") === os) input.checked = true;
    }
  };

  document.addEventListener("DOMContentLoaded", () => pick(document));
  // A streamed or swapped reply brings new tabs.
  document.addEventListener("htmx:afterSettle", (event) => pick(event.target));
  document.addEventListener("htmx:sseMessage", () => pick(document));
})();
