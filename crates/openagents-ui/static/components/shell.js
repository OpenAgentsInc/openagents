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
// Phone keyboard: below 48rem, --oa-viewport-height on <html> follows the
// visual viewport's height (set through the CSSOM, which the style policy
// allows), so when the on-screen keyboard opens the shell shrinks to the
// space above it and the docked composer stays in view (iOS Safari resizes
// neither dvh nor the layout viewport for the keyboard). Without this
// script the shell keeps 100dvh.
//
// Shortcuts: Control+<letter> (Ctrl only; Cmd+N belongs to the browser on
// macOS) follows the link that declares it in aria-keyshortcuts, such as
// "New chat" (Control+N). Not during IME composition. Chrome on Windows and
// Linux keeps Ctrl+N for a new window and never delivers it to the page.
//
// Send button: disabled (the `disabled` attribute) while the composer's
// text box is empty or whitespace, enabled as soon as it has text; set on
// load, on every input, and after HTMX swaps or requests; also disabled
// while the page marks an answer as being written for that composer
// ([data-oa-composer-busy="<form id>"]). Without this script it stays
// enabled and the server rejects an empty message.
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

  var viewport = window.visualViewport;
  if (viewport) {
    var fitViewport = function () {
      if (wide()) {
        root.style.removeProperty("--oa-viewport-height");
        return;
      }
      root.style.setProperty("--oa-viewport-height", Math.round(viewport.height) + "px");
      // The keyboard can leave the page scrolled; the shell never scrolls.
      if (window.scrollY) window.scrollTo(0, 0);
    };
    viewport.addEventListener("resize", fitViewport);
    fitViewport();
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
        // A click, so a boosted page (hx-boost) swaps instead of loading.
        links[i].click();
        return;
      }
    }
  });

  function composerParts(form) {
    var input = form.querySelector("[data-composer-body] textarea");
    var send = form.querySelector(".oa-composer-footer-end button[type=submit]");
    return { input: input, send: send };
  }

  // A page marks an answer still being written for a composer with
  // data-oa-composer-busy="<form id>" (a live stream updates it): its send
  // button stays off until the mark goes.
  function busy(form) {
    if (!form.id) return false;
    var marks = document.querySelectorAll("[data-oa-composer-busy]");
    for (var i = 0; i < marks.length; i++) {
      if (marks[i].getAttribute("data-oa-composer-busy") === form.id) return true;
    }
    return false;
  }

  // A refused send is answered with its error status and this header; its
  // body puts the reason in the composer's status region (out of band), so
  // HTMX is let swap it. The draft stays.
  function refused(event) {
    var xhr = event.detail && event.detail.xhr;
    return !!(xhr && xhr.getResponseHeader && xhr.getResponseHeader("X-OpenAgents-Refused"));
  }
  document.addEventListener("htmx:beforeSwap", function (event) {
    if (refused(event)) {
      event.detail.shouldSwap = true;
      event.detail.isError = false;
    }
  });

  function syncSend(form) {
    var parts = composerParts(form);
    if (!parts.input || !parts.send) return;
    var empty = !parts.input.value.trim();
    if (empty || parts.input.disabled || busy(form)) {
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
    if (!event.detail.successful || refused(event)) {
      form._oaSent = undefined;
      return;
    }
    var input = composerParts(form).input;
    if (input && form._oaSent !== undefined && input.value === form._oaSent) {
      input.value = "";
      grow(input);
      syncSend(form);
    }
    form._oaSent = undefined;
  });
  ["htmx:afterRequest", "htmx:afterSettle", "htmx:load", "htmx:sseMessage", "reset"].forEach(function (name) {
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
    // Nothing while a send is in flight or an answer is being written.
    var send = composerParts(form).send;
    if (send && send.disabled) return;
    if (typeof form.requestSubmit === "function") {
      form.requestSubmit();
    } else {
      form.submit();
    }
  });

  // Chat list (ChatSearch, RowRename, ChatList's [data-oa-chat-rows]):
  // - Cmd+K / Ctrl+K focuses the search box (opening the panel first when
  //   it is a closed drawer or collapsed to its rail).
  // - In the search box: ArrowDown moves to the first row, Enter opens the
  //   first row, Escape clears the search. In the rows, ArrowUp/ArrowDown
  //   move between rows and back up to the box.
  // - Ctrl+Shift+[ and Ctrl+Shift+] open the previous and next chat.
  // - Renaming: the field's text is selected when it appears; Escape
  //   follows its Cancel link.
  function chatRows() {
    var box = document.querySelector("[data-oa-chat-rows]");
    return box ? Array.prototype.slice.call(box.querySelectorAll("a.oa-nav-item")) : [];
  }

  function searchBox() {
    return document.querySelector("[data-oa-chat-search] input[type=search]");
  }

  function visible(element) {
    return !!(element && element.getClientRects().length);
  }

  function openPanel() {
    var toggle = document.querySelector("[data-oa-sidebar-toggle]");
    if (toggle) toggle.click();
  }

  document.addEventListener("keydown", function (event) {
    if (event.defaultPrevented || event.isComposing || event.keyCode === 229) return;
    var target = event.target;
    var key = event.key;

    if ((event.metaKey || event.ctrlKey) && !event.altKey && !event.shiftKey &&
        key && key.toLowerCase() === "k") {
      var box = searchBox();
      if (!box) return;
      event.preventDefault();
      if (!visible(box)) openPanel();
      box.focus();
      box.select();
      return;
    }

    if (event.ctrlKey && event.shiftKey && !event.metaKey && !event.altKey &&
        (event.code === "BracketLeft" || event.code === "BracketRight")) {
      var rows = chatRows();
      if (!rows.length) return;
      event.preventDefault();
      var at = -1;
      for (var i = 0; i < rows.length; i++) {
        if (rows[i].getAttribute("aria-current") === "page") at = i;
      }
      var next = event.code === "BracketLeft" ? at - 1 : at + 1;
      if (at < 0) next = event.code === "BracketLeft" ? rows.length - 1 : 0;
      if (next >= 0 && next < rows.length) rows[next].click();
      return;
    }

    if (!target || !target.closest) return;

    if (key === "Escape" && target.closest("[data-oa-rename]")) {
      var cancel = target.closest("[data-oa-rename]").querySelector("[data-oa-rename-cancel]");
      if (cancel) {
        event.preventDefault();
        cancel.click();
      }
      return;
    }

    if (target.matches && target.matches("[data-oa-chat-search] input[type=search]")) {
      if (key === "Escape") {
        if (!target.value) return;
        event.preventDefault();
        target.value = "";
        target.dispatchEvent(new Event("input", { bubbles: true }));
      } else if (key === "ArrowDown") {
        var first = chatRows()[0];
        if (first) {
          event.preventDefault();
          first.focus();
        }
      } else if (key === "Enter") {
        var top = chatRows()[0];
        event.preventDefault();
        if (top && target.value.trim()) top.click();
      }
      return;
    }

    if ((key === "ArrowDown" || key === "ArrowUp") && target.matches &&
        target.matches("[data-oa-chat-rows] a.oa-nav-item")) {
      var list = chatRows();
      var index = list.indexOf(target);
      if (index < 0) return;
      event.preventDefault();
      if (key === "ArrowDown" && index + 1 < list.length) {
        list[index + 1].focus();
      } else if (key === "ArrowUp") {
        if (index > 0) {
          list[index - 1].focus();
        } else if (searchBox()) {
          searchBox().focus();
        }
      }
    }
  });

  function selectRename() {
    var input = document.querySelector("[data-oa-rename] input[name=title]");
    if (input && !input._oaSelected) {
      input._oaSelected = true;
      input.focus();
      input.select();
    }
  }
  document.addEventListener("htmx:afterSettle", selectRename);
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", selectRename);
  } else {
    selectRename();
  }

  // Project groups remember being closed in this browser: the ids of the
  // closed ones go in the oa_project_groups cookie, which the server reads
  // when it draws the sidebar.
  document.addEventListener("toggle", function (event) {
    var group = event.target;
    if (!group || !group.matches || !group.matches("details[data-oa-project]")) return;
    var closed = [];
    document.querySelectorAll("details[data-oa-project]").forEach(function (each) {
      var id = each.getAttribute("data-oa-project");
      if (!each.open && /^prj_[0-9a-f]{16}$/.test(id) && closed.indexOf(id) < 0) closed.push(id);
    });
    document.cookie = "oa_project_groups=" + closed.slice(0, 50).join(".") +
      "; Path=/; Max-Age=31536000; SameSite=Lax";
  }, true);

  // "Projects" and "Chats" fold from their headings; the folded ones are
  // remembered in this browser (localStorage, when it is allowed) and put
  // back after every HTMX swap of the list.
  var SECTIONS = "oa_sidebar_sections";
  function foldedSections() {
    try {
      return JSON.parse(window.localStorage.getItem(SECTIONS) || "[]") || [];
    } catch (e) {
      return [];
    }
  }
  function setSection(group, folded) {
    var button = group.querySelector("[data-oa-section-toggle]");
    if (folded) group.setAttribute("data-collapsed", "");
    else group.removeAttribute("data-collapsed");
    if (button) button.setAttribute("aria-expanded", folded ? "false" : "true");
  }
  function applySections() {
    var folded = foldedSections();
    document.querySelectorAll("[data-oa-section]").forEach(function (group) {
      setSection(group, folded.indexOf(group.getAttribute("data-oa-section")) >= 0);
    });
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", applySections);
  } else {
    applySections();
  }
  document.addEventListener("htmx:load", applySections);
  document.addEventListener("click", function (event) {
    var button = event.target && event.target.closest && event.target.closest("[data-oa-section-toggle]");
    var group = button && button.closest("[data-oa-section]");
    if (!group) return;
    var key = group.getAttribute("data-oa-section");
    var folded = foldedSections().filter(function (each) { return each !== key; });
    var fold = button.getAttribute("aria-expanded") !== "false";
    if (fold) folded.push(key);
    setSection(group, fold);
    try {
      window.localStorage.setItem(SECTIONS, JSON.stringify(folded));
    } catch (e) { /* storage blocked: the fold lasts for this page */ }
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
