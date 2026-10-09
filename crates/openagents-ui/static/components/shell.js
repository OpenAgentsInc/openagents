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
// Shortcuts: Control+<letter> (Ctrl only; Cmd+N belongs to the browser on
// macOS) follows the link that declares it in aria-keyshortcuts, such as
// "New chat" (Control+N). Not during IME composition. Chrome on Windows and
// Linux keeps Ctrl+N for a new window and never delivers it to the page.
//
// Send button: disabled (the `disabled` attribute) while the composer's
// text box is empty or whitespace, enabled as soon as it has text; set on
// load, on every input, and after HTMX swaps or requests. Without this
// script it stays enabled and the server rejects an empty message.
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
    if (!event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
    if (event.isComposing || event.keyCode === 229) return;
    if (!event.key || event.key.length !== 1 || !/[a-z]/i.test(event.key)) return;
    var keys = "Control+" + event.key.toUpperCase();
    var links = document.querySelectorAll("a[aria-keyshortcuts]");
    for (var i = 0; i < links.length; i++) {
      if (links[i].getAttribute("aria-keyshortcuts") === keys) {
        event.preventDefault();
        window.location.assign(links[i].href);
        return;
      }
    }
  });

  function composerParts(form) {
    var input = form.querySelector("[data-composer-body] textarea");
    var send = form.querySelector(".oa-composer-footer-end button[type=submit]");
    return { input: input, send: send };
  }

  function syncSend(form) {
    var parts = composerParts(form);
    if (!parts.input || !parts.send) return;
    var empty = !parts.input.value.trim();
    if (empty || parts.input.disabled) {
      parts.send.disabled = true;
    } else if (!form.classList.contains("htmx-request")) {
      parts.send.disabled = false;
    }
  }

  function syncAllSends() {
    var forms = document.querySelectorAll("form[data-oa-composer]");
    for (var i = 0; i < forms.length; i++) {
      syncSend(forms[i]);
      grow(composerParts(forms[i]).input);
    }
  }

  // The text box grows with its text through `field-sizing: content`;
  // where that is missing, set its height from its content (CSSOM, which
  // the style policy allows), up to the stylesheet's max-height.
  var sizing = !!(window.CSS && CSS.supports && CSS.supports("field-sizing", "content"));
  function grow(input) {
    if (sizing || !input) return;
    input.style.height = "auto";
    input.style.height = input.scrollHeight + "px";
  }

  document.addEventListener("input", function (event) {
    var form = event.target && event.target.form;
    if (form && form.hasAttribute("data-oa-composer")) {
      syncSend(form);
      grow(composerParts(form).input);
    }
  });
  // After a composer post succeeds, clear the text box — unless the person
  // has typed something new while it was sending. A failed post keeps the
  // draft. (The Wasm adapter, when loaded, clears it too; doing it twice is
  // harmless.)
  document.addEventListener("htmx:beforeRequest", function (event) {
    var form = event.detail && event.detail.elt;
    if (!form || !form.hasAttribute || !form.hasAttribute("data-oa-composer")) return;
    var input = composerParts(form).input;
    if (input) form._oaSent = input.value;
  });
  document.addEventListener("htmx:afterRequest", function (event) {
    var form = event.detail && event.detail.elt;
    if (!form || !form.hasAttribute || !form.hasAttribute("data-oa-composer")) return;
    if (!event.detail.successful) return;
    var input = composerParts(form).input;
    if (input && form._oaSent !== undefined && input.value === form._oaSent) {
      input.value = "";
      grow(input);
      syncSend(form);
    }
    form._oaSent = undefined;
  });
  ["htmx:afterRequest", "htmx:afterSettle", "htmx:load", "reset"].forEach(function (name) {
    document.addEventListener(name, function () { setTimeout(syncAllSends, 0); });
  });
  window.addEventListener("pageshow", syncAllSends);
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", syncAllSends);
  } else {
    syncAllSends();
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
