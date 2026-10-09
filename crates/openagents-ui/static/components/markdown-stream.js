// A Markdown reply still streaming in (MarkdownRoot::streaming, #11112).
// CSP-safe: no inline handlers, no eval, no string timers.
//
// While a reply streams, the server sends a new render of it over SSE and
// HTMX swaps it in. A reply marked [data-oa-streaming] starts at its old
// height and grows to the new one (.oa-markdown[data-oa-growing] in
// content.css), so the text below it and the page don't jump. A reader at
// the end of the scrolling region stays at the end while it grows. With
// reduced motion the new render shows at once.
(function () {
  "use strict";
  var before = null;

  function owner(root) {
    var node = root.parentElement;
    while (node && !node.id) node = node.parentElement;
    return node;
  }

  function region(root) {
    var node = root.parentElement;
    while (node && node !== document.body) {
      var overflow = window.getComputedStyle(node).overflowY;
      if (overflow === "auto" || overflow === "scroll") return node;
      node = node.parentElement;
    }
    return null;
  }

  document.addEventListener("htmx:sseBeforeMessage", function (event) {
    var scope = event.target;
    if (!scope || !scope.querySelectorAll) return;
    before = [];
    var roots = scope.querySelectorAll(".oa-markdown[data-oa-streaming]");
    for (var i = 0; i < roots.length; i++) {
      var root = roots[i];
      var node = owner(root);
      if (!node) continue;
      var scroller = region(root);
      before.push({
        id: node.id,
        height: root.getBoundingClientRect().height,
        scroller: scroller,
        pinned: !!scroller &&
          scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop < 48,
      });
    }
  });

  function grow(old) {
    var node = document.getElementById(old.id);
    var root = node && node.querySelector(".oa-markdown");
    if (!root) return;
    var height = root.getBoundingClientRect().height;
    if (height <= old.height + 1) return;
    root.style.height = old.height + "px";
    root.setAttribute("data-oa-growing", "");
    root.getBoundingClientRect();
    root.style.height = height + "px";
    var finished = false;
    function done() {
      if (finished) return;
      finished = true;
      root.style.height = "";
      root.removeAttribute("data-oa-growing");
      if (old.pinned) old.scroller.scrollTop = old.scroller.scrollHeight;
    }
    root.addEventListener("transitionend", done, { once: true });
    window.setTimeout(done, 600);
    if (old.pinned) {
      var follow = function () {
        if (finished) return;
        old.scroller.scrollTop = old.scroller.scrollHeight;
        window.requestAnimationFrame(follow);
      };
      window.requestAnimationFrame(follow);
    }
  }

  document.addEventListener("htmx:afterSwap", function () {
    var list = before;
    before = null;
    if (!list || !list.length) return;
    var reduce = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce) return;
    for (var i = 0; i < list.length; i++) grow(list[i]);
  });
})();
