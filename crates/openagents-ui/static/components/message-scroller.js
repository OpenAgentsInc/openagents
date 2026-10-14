// MessageScroller hook for openagents-ui (content::MessageScroller,
// #11621, #11639). CSP-safe: no inline handlers, no eval. For each
// [data-oa-message-scroller]:
//  - with data-follow, it opens at the newest turn and keeps the view
//    there while words stream in, a turn is redrawn, or the region
//    resizes (a phone's toolbar or keyboard), but only while the reader is
//    at the bottom; scrolling up stops following, and coming back to the
//    end starts it again;
//  - otherwise (the reader scrolled up, or earlier turns loaded above),
//    it keeps the turn the reader was looking at where it was, even when
//    the turns are redrawn from scratch.
// The scroll container is the nearest scrolling ancestor (the scroller
// itself when it scrolls), else the page.
(() => {
  "use strict";

  const NEAR_PX = 64;
  // Turns the view anchors to: anything in the scroller with an id that
  // stays the same across redraws (`#chat-message-3`).
  const TURNS = "article[id], .oa-turn[id], .oa-marker[id]";

  const scrollerOf = (root) => {
    for (let node = root; node && node !== document.body && node !== document.documentElement; node = node.parentElement) {
      if (/(auto|scroll)/.test(getComputedStyle(node).overflowY)) return node;
    }
    return document.scrollingElement || document.documentElement;
  };

  const page = (box) => box === document.scrollingElement || box === document.documentElement;
  const atBottom = (box) => box.scrollHeight - box.scrollTop - box.clientHeight < NEAR_PX;
  const boxTop = (box) => (page(box) ? 0 : box.getBoundingClientRect().top);

  const watch = (root) => {
    if (root.dataset.oaMessageScrollerBound) return;
    const turns = root.querySelector(".oa-message-scroller__turns");
    if (!turns) return;
    root.dataset.oaMessageScrollerBound = "1";
    const follows = root.hasAttribute("data-follow");
    const box = scrollerOf(root);
    // A link to one turn (#chat-message-3) opens there, not at the end.
    const linked = location.hash.length > 1 && root.querySelector("#" + CSS.escape(location.hash.slice(1)));
    let following = follows && !linked;
    let anchor = null; // { id, top }: the first turn in view
    let first = null; // the first turn's id
    let opened = false; // the reader opened or closed a line

    const firstTurn = () => {
      const head = turns.querySelector(TURNS);
      return head ? head.id : null;
    };
    const pick = () => {
      const top = boxTop(box);
      for (const turn of turns.querySelectorAll(TURNS)) {
        const rect = turn.getBoundingClientRect();
        if (rect.bottom > top + 1) return { id: turn.id, top: rect.top - top };
      }
      return null;
    };
    const toEnd = () => {
      box.scrollTop = box.scrollHeight;
    };
    const remember = () => {
      anchor = pick();
      first = firstTurn();
    };
    const settle = () => {
      if (!root.isConnected) return;
      const head = firstTurn();
      // Earlier turns arrived above the ones shown: stay on what the
      // reader was reading, even at the end.
      const earlier = first && head !== first && turns.querySelector("#" + CSS.escape(first)) && head;
      // A line the reader opened grows in place; the view doesn't run
      // to the end of it.
      const still = earlier || opened;
      opened = false;
      if (following && !still) {
        toEnd();
      } else if (anchor) {
        const turn = turns.querySelector("#" + CSS.escape(anchor.id));
        if (turn) {
          const moved = turn.getBoundingClientRect().top - boxTop(box) - anchor.top;
          if (Math.abs(moved) > 0.5) box.scrollTop += moved;
        }
      }
      if (still && follows) following = atBottom(box);
      remember();
    };

    (page(box) ? window : box).addEventListener("scroll", () => {
      if (follows) following = atBottom(box);
      remember();
    }, { passive: true });
    turns.addEventListener("click", (event) => {
      if (!(event.target instanceof Element) || !event.target.closest("summary")) return;
      opened = true;
      // Only for the resize that opening causes.
      requestAnimationFrame(() => requestAnimationFrame(() => { opened = false; }));
    }, true);
    new MutationObserver(settle).observe(turns, { childList: true, subtree: true, characterData: true });
    if ("ResizeObserver" in window) {
      const resized = new ResizeObserver(settle);
      resized.observe(turns);
      if (!page(box)) resized.observe(box);
    }
    if (following) toEnd();
    remember();
  };

  const start = () => document.querySelectorAll("[data-oa-message-scroller]").forEach(watch);
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
  document.addEventListener("htmx:afterSettle", start);
  document.addEventListener("htmx:load", start);

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
