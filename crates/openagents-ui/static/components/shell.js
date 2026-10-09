// App shell behavior for openagents_ui::shell (CSP-safe: no eval, no inline
// handlers, no string timers; needs no Alpine).
//
// Sidebar toggle ([data-oa-sidebar-toggle], SIDEBAR_TOGGLE_ATTR):
// - wide screens (min-width 48rem): a click collapses the left panel to its
//   rail or expands it, by flipping data-sidebar on .oa-layout, and stores
//   the choice in the oa_sidebar cookie (SIDEBAR_COOKIE) so the server
//   renders the next page the same way. The native popover is not opened.
// - narrow screens: the button's own popovertarget opens and closes the
//   drawer; this script stays out of the way.
// - <html data-oa-sidebar-ready> tells the stylesheet the toggle works on
//   wide screens; without this script it is hidden there.
//
// Composer (form[data-oa-composer]): Enter submits, Shift+Enter inserts a
// line, IME composition is left alone. A page adapter that already handled
// the key (event.defaultPrevented) wins. A click on the composer card that
// is not on a control focuses the text box.
(function () {
  "use strict";
  var COOKIE = "oa_sidebar";
  var MAX_AGE = 60 * 60 * 24 * 365;
  var WIDE = "(min-width: 48rem)";
  var root = document.documentElement;
  root.setAttribute("data-oa-sidebar-ready", "");

  function wide() {
    return !window.matchMedia || window.matchMedia(WIDE).matches;
  }

  function sync(layout, button) {
    if (!button) return;
    if (wide()) {
      button.setAttribute("aria-expanded", layout.getAttribute("data-sidebar") === "collapsed" ? "false" : "true");
    } else {
      button.removeAttribute("aria-expanded");
    }
  }

  function syncAll() {
    var layouts = document.querySelectorAll(".oa-layout");
    for (var i = 0; i < layouts.length; i++) {
      sync(layouts[i], layouts[i].querySelector("[data-oa-sidebar-toggle]"));
    }
  }

  document.addEventListener("click", function (event) {
    var target = event.target;
    var button = target && target.closest ? target.closest("[data-oa-sidebar-toggle]") : null;
    if (!button || !wide()) return;
    var layout = button.closest(".oa-layout");
    if (!layout) return;
    event.preventDefault();
    var collapsed = layout.getAttribute("data-sidebar") !== "collapsed";
    layout.setAttribute("data-sidebar", collapsed ? "collapsed" : "expanded");
    var secure = window.location.protocol === "https:" ? "; Secure" : "";
    document.cookie = COOKIE + "=" + (collapsed ? "collapsed" : "expanded") +
      "; Path=/; Max-Age=" + MAX_AGE + "; SameSite=Lax" + secure;
    sync(layout, button);
  });

  if (window.matchMedia) {
    var query = window.matchMedia(WIDE);
    if (query.addEventListener) query.addEventListener("change", syncAll);
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", syncAll);
  } else {
    syncAll();
  }

  document.addEventListener("keydown", function (event) {
    if (event.defaultPrevented || event.key !== "Enter" || event.shiftKey) return;
    if (event.isComposing || event.keyCode === 229) return;
    var input = event.target;
    if (!input || input.tagName !== "TEXTAREA") return;
    var form = input.form;
    if (!form || !form.hasAttribute("data-oa-composer")) return;
    event.preventDefault();
    if (!input.value.trim()) return;
    if (typeof form.requestSubmit === "function") {
      form.requestSubmit();
    } else {
      form.submit();
    }
  });

  document.addEventListener("click", function (event) {
    var target = event.target;
    if (!target || !target.closest) return;
    var body = target.closest("[data-composer-body]");
    if (!body || target.closest("button, a, input, select, textarea, label")) return;
    var input = body.querySelector("textarea");
    if (input && !input.disabled) input.focus();
  });
})();
