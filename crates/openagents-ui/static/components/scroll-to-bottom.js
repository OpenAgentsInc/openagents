// ScrollToBottom (openagents_ui::shell::ScrollToBottom). CSP-safe: no
// inline handlers, no eval, no string timers; needs no Alpine.
//
// A button[data-oa-scroll-bottom="<selector>"] follows the scrolling region
// the selector names. An IntersectionObserver watches a sentinel this script
// appends at the region's end, with the region as root and its bottom
// margin grown by a third of its height: the button shows only while the
// region overflows and the sentinel is more than a third of a screen below
// the visible end, so short threads never show it. A link inside the region
// marked [data-oa-scroll-tail] means the newest messages are not loaded:
// the button then shows and clicking it follows that link (HTMX loads the
// tail). Otherwise a click smooth-scrolls to the end. A button marked
// [data-oa-scroll-follow] also opens its region at the end and keeps it
// there as content swaps in, unless the reader scrolled up. Buttons that
// arrive with HTMX content are bound on htmx:load.
(function () {
  "use strict";
  var BOUND = "oaScrollBound";

  function bind(button) {
    if (button.dataset[BOUND]) return;
    var region = document.querySelector(button.getAttribute("data-oa-scroll-bottom"));
    if (!region) return;
    button.dataset[BOUND] = "1";
    var sentinel = document.createElement("div");
    sentinel.className = "oa-scroll-sentinel";
    sentinel.setAttribute("aria-hidden", "true");
    region.appendChild(sentinel);
    var near = true;

    function tail() {
      return region.querySelector("[data-oa-scroll-tail]");
    }
    function update() {
      var overflows = region.scrollHeight > region.clientHeight + 1;
      button.hidden = !(tail() || (overflows && !near));
    }

    if ("IntersectionObserver" in window) {
      var observer = new IntersectionObserver(function (entries) {
        near = entries[entries.length - 1].isIntersecting;
        update();
      }, { root: region, rootMargin: "0px 0px 33% 0px" });
      observer.observe(sentinel);
    } else {
      region.addEventListener("scroll", function () {
        near = region.scrollHeight - region.clientHeight - region.scrollTop < region.clientHeight / 3;
        update();
      }, { passive: true });
    }
    // [data-oa-scroll-follow]: open at the end, and keep the end in view
    // when new content arrives while the reader is near it. `near` still
    // holds the state from before the swap when afterSettle fires.
    var follow = button.hasAttribute("data-oa-scroll-follow");
    function toEnd() {
      region.scrollTop = region.scrollHeight;
    }
    if (follow) toEnd();
    document.addEventListener("htmx:afterSettle", function () {
      if (!region.isConnected) return;
      var stay = follow && near;
      if (sentinel.parentNode !== region || region.lastElementChild !== sentinel) {
        region.appendChild(sentinel);
      }
      if (stay) toEnd();
      update();
    });

    button.addEventListener("click", function () {
      var link = tail();
      if (link) {
        link.click();
        return;
      }
      var reduce = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      region.scrollTo({ top: region.scrollHeight, behavior: reduce ? "auto" : "smooth" });
    });
    update();
  }

  function scan(root) {
    var scope = root && root.querySelectorAll ? root : document;
    var buttons = scope.querySelectorAll("[data-oa-scroll-bottom]");
    for (var i = 0; i < buttons.length; i++) bind(buttons[i]);
    if (scope.matches && scope.matches("[data-oa-scroll-bottom]")) bind(scope);
  }

  document.addEventListener("htmx:load", function (event) { scan(event.target); });
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () { scan(document); });
  } else {
    scan(document);
  }
})();
