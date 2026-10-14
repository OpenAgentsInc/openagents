// MessageScroller hook for openagents-ui (content::MessageScroller,
// #11621). CSP-safe: no inline handlers, no eval. For each
// [data-oa-message-scroller]:
//  - with data-follow, keeps the view at the newest words while they stream
//    in, but only while the reader is already at the bottom;
//  - when earlier turns load above the first one, keeps the view still.
// The scroll container is the scroller itself when it scrolls, else the
// page.
(() => {
  "use strict";

  const NEAR_PX = 64;

  const scrollerOf = (root) => {
    const style = getComputedStyle(root);
    const scrolls = /(auto|scroll)/.test(style.overflowY) && root.scrollHeight > root.clientHeight;
    return scrolls ? root : document.scrollingElement || document.documentElement;
  };

  const atBottom = (box) => box.scrollHeight - box.scrollTop - box.clientHeight < NEAR_PX;

  const watch = (root) => {
    if (root.dataset.oaMessageScrollerBound) return;
    root.dataset.oaMessageScrollerBound = "1";
    const turns = root.querySelector(".oa-message-scroller__turns");
    if (!turns) return;
    let box = scrollerOf(root);
    let following = atBottom(box);
    let first = turns.firstElementChild;
    let height = box.scrollHeight;
    const remember = () => {
      box = scrollerOf(root);
      following = atBottom(box);
      height = box.scrollHeight;
    };
    (box === root ? root : window).addEventListener("scroll", remember, { passive: true });
    new MutationObserver(() => {
      const grew = box.scrollHeight - height;
      const head = turns.firstElementChild;
      if (head !== first && first && first.isConnected && grew > 0) {
        // Earlier turns arrived above: stay on what the reader was reading.
        box.scrollTop += grew;
      } else if (root.hasAttribute("data-follow") && following) {
        box.scrollTop = box.scrollHeight;
      }
      first = head;
      height = box.scrollHeight;
    }).observe(turns, { childList: true, subtree: true, characterData: true });
  };

  const start = () => document.querySelectorAll("[data-oa-message-scroller]").forEach(watch);
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
  document.addEventListener("htmx:afterSettle", start);

  // A redrawn thread keeps the lines the reader opened: a live chat
  // redraws its turns as they change, and a line someone opened (or
  // closed) stays that way.
  const chosen = new Map();
  document.addEventListener("toggle", (event) => {
    const details = event.target;
    if (!(details instanceof HTMLDetailsElement)) return;
    const line = details.parentElement;
    if (line && line.matches("li.oa-trace-step[id]")) chosen.set(line.id, details.open);
  }, true);
  new MutationObserver((records) => {
    if (chosen.size === 0) return;
    for (const record of records) {
      for (const node of record.addedNodes) {
        if (!(node instanceof Element)) continue;
        const lines = node.matches("li.oa-trace-step[id]") ? [node] : node.querySelectorAll("li.oa-trace-step[id]");
        for (const line of lines) {
          const details = line.querySelector(":scope > details");
          if (details && chosen.has(line.id)) details.open = chosen.get(line.id);
        }
      }
    }
  }).observe(document.documentElement, { childList: true, subtree: true });
})();
