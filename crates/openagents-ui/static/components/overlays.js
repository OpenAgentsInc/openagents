// Overlay behavior for crates/openagents-ui (UI-05): Popover, Menu, Tooltip,
// SelectControl and Dialog. The native Popover API and <dialog> do show,
// hide, Escape and light dismiss; this file adds roving focus, arrow keys,
// type-ahead, focus return, data-state=open|closed, aria-expanded, and a
// placement fallback where CSS anchor positioning is missing.
//
// Alpine.js CSP build: each component is registered with Alpine.data and
// wires its own listeners in init(), so markup carries only names such as
// x-data="oaMenu" and never an expression (the site CSP is script-src
// 'self' with no 'unsafe-eval'). Load this script before Alpine, both
// deferred. Without JavaScript the markup still works: popovertarget opens
// panels, links navigate, the native select submits, and the dialog's
// close form closes it.
//
// The pure helpers below are exported for node --test
// (tests/js/overlays.test.js, run by the crate's tests when node
// is installed).
(function () {
  "use strict";

  // ---------------------------------------------------------------------
  // Pure helpers
  // ---------------------------------------------------------------------

  // The index an arrow, Home or End key moves to among `count` items,
  // wrapping at the ends; -1 for "nothing yet" starts at the near end.
  function step(index, count, key) {
    if (count <= 0) return -1;
    switch (key) {
      case "ArrowDown":
        return index < 0 ? 0 : (index + 1) % count;
      case "ArrowUp":
        return index < 0 ? count - 1 : (index - 1 + count) % count;
      case "Home":
      case "PageUp":
        return 0;
      case "End":
      case "PageDown":
        return count - 1;
      default:
        return index;
    }
  }

  function normalize(text) {
    return String(text == null ? "" : text).replace(/\s+/g, " ").trim().toLowerCase();
  }

  // Type-ahead: the first label starting with `query`, searching from the
  // current item. Repeating one letter cycles through the labels starting
  // with it; a longer query keeps the current item while it still matches.
  function typeahead(labels, current, query) {
    var q = normalize(query);
    var n = labels.length;
    if (!q || !n) return -1;
    var repeated = q.split("").every(function (c) { return c === q[0]; });
    if (repeated) q = q[0];
    var start = repeated ? current + 1 : Math.max(current, 0);
    for (var i = 0; i < n; i++) {
      var idx = (((start + i) % n) + n) % n;
      if (normalize(labels[idx]).indexOf(q) === 0) return idx;
    }
    return -1;
  }

  // Search filter: case-insensitive substring; an empty query matches all.
  function matches(label, query) {
    var q = normalize(query);
    return !q || normalize(label).indexOf(q) !== -1;
  }

  // Fallback placement of a `size` panel next to a `rect` trigger inside a
  // `viewport`, on `side` with `align`, flipping to the other side when it
  // does not fit and keeping an 8px margin from the edges.
  function position(rect, size, viewport, side, align, gap) {
    var edge = 8;
    var top;
    var left;
    if (side === "left" || side === "right") {
      var after = rect.right + gap;
      var before = rect.left - gap - size.width;
      if (side === "right") {
        left = after + size.width > viewport.width - edge && before >= edge ? before : after;
      } else {
        left = before < edge && after + size.width <= viewport.width - edge ? after : before;
      }
      top = align === "center"
        ? rect.top + (rect.height - size.height) / 2
        : align === "end" ? rect.bottom - size.height : rect.top;
    } else {
      var below = rect.bottom + gap;
      var above = rect.top - gap - size.height;
      if (side === "top") {
        top = above < edge && below + size.height <= viewport.height - edge ? below : above;
      } else {
        top = below + size.height > viewport.height - edge && above >= edge ? above : below;
      }
      left = align === "center"
        ? rect.left + (rect.width - size.width) / 2
        : align === "end" ? rect.right - size.width : rect.left;
    }
    return {
      top: clamp(top, edge, viewport.height - size.height - edge),
      left: clamp(left, edge, viewport.width - size.width - edge)
    };
  }

  function clamp(value, low, high) {
    return Math.max(low, Math.min(value, Math.max(low, high)));
  }

  var helpers = { step: step, typeahead: typeahead, matches: matches, position: position };
  if (typeof module === "object" && module.exports) {
    module.exports = helpers;
    return;
  }

  // ---------------------------------------------------------------------
  // Shared DOM behavior
  // ---------------------------------------------------------------------

  var anchored = !!(window.CSS && CSS.supports && CSS.supports("position-area", "block-end"));
  var FOCUSABLE = "a[href],button:not([disabled]),input:not([disabled]),select:not([disabled])," +
    "textarea:not([disabled]),[tabindex]:not([tabindex='-1'])";

  function isOpen(panel) {
    try {
      return panel.matches(":popover-open");
    } catch (e) {
      return false;
    }
  }

  function show(panel, trigger) {
    if (!panel.showPopover || isOpen(panel)) return;
    try {
      panel.showPopover({ source: trigger });
    } catch (e) {
      try {
        panel.showPopover();
      } catch (e2) { /* detached or unsupported */ }
    }
  }

  function hide(panel) {
    if (!panel.hidePopover || !isOpen(panel)) return;
    try {
      panel.hidePopover();
    } catch (e) { /* already hidden */ }
  }

  function place(panel, trigger) {
    if (anchored || !trigger) return;
    var side = panel.getAttribute("data-side") || "bottom";
    var align = panel.getAttribute("data-align") || "start";
    var gap = panel.classList.contains("oa-tooltip") ? 6 : 4;
    var rect = trigger.getBoundingClientRect();
    var style = panel.style;
    if (panel.hasAttribute("data-match-width")) style.minWidth = rect.width + "px";
    var root = document.documentElement;
    var at = position(
      rect,
      { width: panel.offsetWidth, height: panel.offsetHeight },
      { width: root.clientWidth, height: root.clientHeight },
      side,
      align,
      gap
    );
    style.position = "fixed";
    style.inset = "auto";
    style.margin = "0";
    style.top = at.top + "px";
    style.left = at.left + "px";
  }

  function setState(elements, open) {
    elements.forEach(function (el) {
      if (el) el.setAttribute("data-state", open ? "open" : "closed");
    });
  }

  // Wires a popover panel to its trigger: state attributes, placement while
  // open, and focus back to the trigger when it closes with focus inside.
  function bindPanel(ctx) {
    var panel = ctx.panel;
    var trigger = ctx.trigger;
    var reflow = function () { if (isOpen(panel)) place(panel, trigger); };
    if (ctx.expanded) trigger.setAttribute("aria-expanded", "false");
    setState([ctx.root, trigger, panel], false);
    panel.addEventListener("beforetoggle", function (e) {
      if (e.newState === "open" && !anchored) panel.setAttribute("data-oa-placing", "");
    });
    panel.addEventListener("toggle", function (e) {
      var open = e.newState === "open";
      setState([ctx.root, trigger, panel], open);
      if (ctx.expanded) trigger.setAttribute("aria-expanded", open ? "true" : "false");
      if (open) {
        place(panel, trigger);
        panel.removeAttribute("data-oa-placing");
        if (!anchored) {
          window.addEventListener("resize", reflow);
          window.addEventListener("scroll", reflow, true);
        }
        if (ctx.onOpen) ctx.onOpen();
      } else {
        panel.removeAttribute("data-oa-placing");
        window.removeEventListener("resize", reflow);
        window.removeEventListener("scroll", reflow, true);
        if (ctx.onClose) ctx.onClose();
        var active = document.activeElement;
        if (ctx.returnFocus !== false && (!active || active === document.body || panel.contains(active))) {
          trigger.focus({ preventScroll: true });
        }
      }
    });
  }

  function Typeahead() {
    this.query = "";
    this.timer = 0;
  }

  Typeahead.prototype.push = function (key) {
    var self = this;
    clearTimeout(this.timer);
    this.query += key;
    this.timer = setTimeout(function () { self.query = ""; }, 500);
    return this.query;
  };

  // A printable key that should feed type-ahead (Space only mid-query).
  function typeaheadKey(e, buffer) {
    if (e.ctrlKey || e.metaKey || e.altKey || e.key.length !== 1) return false;
    return e.key !== " " || buffer.query !== "";
  }

  function labelOf(el) {
    return el.getAttribute("data-label") || el.textContent || "";
  }

  // ---------------------------------------------------------------------
  // Components
  // ---------------------------------------------------------------------

  // Popover: a panel of arbitrary content (role=dialog). Focus moves to the
  // first focusable child, or the panel itself.
  function popover() {
    return {
      init: function () {
        var root = this.$el;
        var trigger = root.querySelector("[data-oa-trigger]");
        var panel = root.querySelector("[data-oa-panel]");
        if (!trigger || !panel) return;
        bindPanel({
          root: root,
          trigger: trigger,
          panel: panel,
          expanded: true,
          onOpen: function () {
            var first = panel.querySelector("[autofocus]") || panel.querySelector(FOCUSABLE);
            (first || panel).focus({ preventScroll: true });
          }
        });
      }
    };
  }

  // Menu: role=menu of menuitem links and buttons. Arrow keys, Home/End and
  // type-ahead move focus; pointer hover highlights; Tab closes; choosing
  // an item closes the menu and fires "oa-menu-select".
  function menu() {
    return {
      init: function () {
        var root = this.$el;
        var trigger = root.querySelector("[data-oa-trigger]");
        var panel = root.querySelector("[data-oa-panel]");
        if (!trigger || !panel) return;
        var buffer = new Typeahead();
        var pending = "first";
        var items = function () {
          return Array.prototype.filter.call(panel.querySelectorAll("[role^='menuitem']"), function (el) {
            return el.getAttribute("aria-disabled") !== "true" && !el.hidden;
          });
        };
        var highlight = function (item) {
          Array.prototype.forEach.call(panel.querySelectorAll("[data-highlighted]"), function (el) {
            el.removeAttribute("data-highlighted");
          });
          if (item) {
            item.setAttribute("data-highlighted", "");
            item.focus({ preventScroll: false });
          }
        };
        bindPanel({
          root: root,
          trigger: trigger,
          panel: panel,
          expanded: true,
          onOpen: function () {
            var list = items();
            if (pending === "panel" || !list.length) {
              highlight(null);
              panel.focus({ preventScroll: true });
            } else {
              highlight(pending === "last" ? list[list.length - 1] : list[0]);
            }
            pending = "first";
          },
          onClose: function () {
            highlight(null);
            pending = "first";
          }
        });
        trigger.addEventListener("pointerdown", function () { pending = "panel"; });
        trigger.addEventListener("keydown", function (e) {
          if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
          e.preventDefault();
          pending = e.key === "ArrowUp" ? "last" : "first";
          show(panel, trigger);
        });
        panel.addEventListener("keydown", function (e) {
          var list = items();
          var index = list.indexOf(document.activeElement);
          if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Home" || e.key === "End") {
            e.preventDefault();
            highlight(list[step(index, list.length, e.key)]);
          } else if (e.key === "Tab") {
            hide(panel);
          } else if (e.key === " " && buffer.query === "" && index >= 0 && list[index].tagName === "A") {
            e.preventDefault();
            list[index].click();
          } else if (typeaheadKey(e, buffer)) {
            e.preventDefault();
            var next = typeahead(list.map(labelOf), index, buffer.push(e.key));
            if (next >= 0) highlight(list[next]);
          }
        });
        panel.addEventListener("pointermove", function (e) {
          var item = e.target.closest && e.target.closest("[role^='menuitem']");
          if (item && item !== document.activeElement && items().indexOf(item) !== -1) highlight(item);
        });
        panel.addEventListener("pointerleave", function () {
          if (isOpen(panel)) {
            highlight(null);
            panel.focus({ preventScroll: true });
          }
        });
        panel.addEventListener("click", function (e) {
          var item = e.target.closest && e.target.closest("[role^='menuitem']");
          if (!item || !panel.contains(item)) return;
          if (item.getAttribute("aria-disabled") === "true") {
            e.preventDefault();
            return;
          }
          if (item.getAttribute("role") === "menuitemcheckbox") {
            var checked = item.getAttribute("aria-checked") !== "true";
            item.setAttribute("aria-checked", checked ? "true" : "false");
          }
          item.dispatchEvent(new CustomEvent("oa-menu-select", {
            bubbles: true,
            detail: {
              value: item.getAttribute("data-value"),
              checked: item.getAttribute("aria-checked") === "true"
            }
          }));
          if (!item.hasAttribute("data-keep-open")) hide(panel);
        });
      }
    };
  }

  // Tooltip: role=tooltip, shown after a short delay on mouse hover and at
  // once on keyboard focus; hidden on leave, blur, press and Escape. The
  // first focusable element in the trigger slot is described by it.
  function tooltip() {
    return {
      init: function () {
        var root = this.$el;
        var panel = root.querySelector("[data-oa-panel]");
        var slot = root.firstElementChild;
        if (!panel || !slot || slot === panel) return;
        var trigger = slot.matches(FOCUSABLE) ? slot : slot.querySelector(FOCUSABLE) || slot;
        var ids = (trigger.getAttribute("aria-describedby") || "").split(/\s+/).filter(Boolean);
        if (ids.indexOf(panel.id) === -1) ids.push(panel.id);
        trigger.setAttribute("aria-describedby", ids.join(" "));
        var delay = parseInt(root.getAttribute("data-delay") || "150", 10);
        var timer = 0;
        var open = function (wait) {
          clearTimeout(timer);
          timer = setTimeout(function () { show(panel, trigger); }, wait);
        };
        var close = function () {
          clearTimeout(timer);
          hide(panel);
        };
        bindPanel({ root: root, trigger: trigger, panel: panel, expanded: false, returnFocus: false });
        root.addEventListener("pointerenter", function (e) { if (e.pointerType === "mouse") open(delay); });
        root.addEventListener("pointerleave", close);
        trigger.addEventListener("focusin", function () {
          var visible = true;
          try {
            visible = trigger.matches(":focus-visible");
          } catch (e) { /* older engines */ }
          if (visible) open(0);
        });
        trigger.addEventListener("focusout", close);
        trigger.addEventListener("pointerdown", close);
        trigger.addEventListener("keydown", function (e) {
          if (e.key === "Escape" && isOpen(panel)) {
            e.preventDefault();
            e.stopPropagation();
            close();
          }
        });
      }
    };
  }

  // SelectControl: enhances a native <select> into a trigger plus popover
  // listbox with optional search and multiple selection. The native select
  // stays in the form (hidden) and receives input and change events.
  function select() {
    return {
      init: function () {
        var root = this.$el;
        var nativeWrap = root.querySelector("[data-oa-native]");
        var native = nativeWrap && nativeWrap.querySelector("select");
        var trigger = root.querySelector("[data-oa-trigger]");
        var panel = root.querySelector("[data-oa-panel]");
        var listbox = panel && panel.querySelector("[role='listbox']");
        if (!native || !trigger || !panel || !listbox) return;
        var search = panel.querySelector("[data-oa-search]");
        var empty = panel.querySelector("[data-oa-empty]");
        var text = trigger.querySelector("[data-oa-text]");
        var holder = search || listbox;
        var multiple = native.multiple;
        var placeholder = trigger.getAttribute("data-placeholder") || "";
        var buffer = new Typeahead();
        var active = null;

        nativeWrap.hidden = true;
        trigger.hidden = false;
        trigger.disabled = native.disabled;
        if (native.labels && native.labels.length) {
          trigger.id = trigger.id || native.id + "-trigger";
          Array.prototype.forEach.call(native.labels, function (label) { label.htmlFor = trigger.id; });
        }

        var options = function () {
          return Array.prototype.slice.call(listbox.querySelectorAll("[role='option']"));
        };
        var visible = function () {
          return options().filter(function (o) {
            return !o.hidden && o.getAttribute("aria-disabled") !== "true";
          });
        };
        var nativeOption = function (o) {
          var value = o.getAttribute("data-value");
          return Array.prototype.filter.call(native.options, function (n) { return n.value === value; })[0];
        };
        var sync = function () {
          var labels = [];
          options().forEach(function (o) {
            var n = nativeOption(o);
            var selected = !!(n && n.selected);
            o.setAttribute("aria-selected", selected ? "true" : "false");
            if (selected) labels.push(labelOf(o).trim());
          });
          if (text) text.textContent = labels.length ? labels.join(", ") : placeholder;
          trigger.setAttribute("data-selected", labels.length ? "true" : "false");
        };
        var activate = function (o, scroll) {
          if (active) active.removeAttribute("data-highlighted");
          active = o || null;
          if (active) {
            active.setAttribute("data-highlighted", "");
            holder.setAttribute("aria-activedescendant", active.id);
            if (scroll && active.scrollIntoView) active.scrollIntoView({ block: "nearest" });
          } else {
            holder.removeAttribute("aria-activedescendant");
          }
        };
        var choose = function (o) {
          if (!o || o.getAttribute("aria-disabled") === "true") return;
          var n = nativeOption(o);
          if (!n) return;
          if (multiple) n.selected = !n.selected;
          else native.value = n.value;
          sync();
          native.dispatchEvent(new Event("input", { bubbles: true }));
          native.dispatchEvent(new Event("change", { bubbles: true }));
          if (multiple) holder.focus({ preventScroll: true });
          else hide(panel);
        };
        var filter = function () {
          if (!search) return;
          var shown = 0;
          options().forEach(function (o) {
            var hit = matches(labelOf(o), search.value);
            o.hidden = !hit;
            if (hit) shown++;
          });
          if (empty) empty.hidden = shown > 0;
          if (!active || active.hidden) activate(visible()[0], true);
        };

        bindPanel({
          root: root,
          trigger: trigger,
          panel: panel,
          expanded: true,
          onOpen: function () {
            if (search) {
              search.value = "";
              filter();
            }
            var list = visible();
            var selected = list.filter(function (o) { return o.getAttribute("aria-selected") === "true"; })[0];
            activate(selected || list[0], true);
            holder.focus({ preventScroll: true });
          },
          onClose: function () { activate(null); }
        });
        trigger.addEventListener("keydown", function (e) {
          if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
          e.preventDefault();
          show(panel, trigger);
        });
        holder.addEventListener("keydown", function (e) {
          var list = visible();
          var index = list.indexOf(active);
          if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Home" || e.key === "End") {
            if (search && (e.key === "Home" || e.key === "End")) return;
            e.preventDefault();
            activate(list[step(index, list.length, e.key)], true);
          } else if (e.key === "Enter" || (e.key === " " && !search && buffer.query === "")) {
            e.preventDefault();
            choose(active);
          } else if (e.key === "Tab") {
            hide(panel);
          } else if (!search && typeaheadKey(e, buffer)) {
            e.preventDefault();
            var next = typeahead(list.map(labelOf), index, buffer.push(e.key));
            if (next >= 0) activate(list[next], true);
          }
        });
        if (search) search.addEventListener("input", filter);
        listbox.addEventListener("pointermove", function (e) {
          var o = e.target.closest && e.target.closest("[role='option']");
          if (o && o !== active && o.getAttribute("aria-disabled") !== "true") activate(o, false);
        });
        // Keep focus in the search box while the pointer picks an option.
        listbox.addEventListener("mousedown", function (e) { if (search) e.preventDefault(); });
        listbox.addEventListener("click", function (e) {
          var o = e.target.closest && e.target.closest("[role='option']");
          if (o) choose(o);
        });
        native.addEventListener("change", sync);
        sync();
      }
    };
  }

  // Dialog: native <dialog> opened with showModal(). Invoker buttons use
  // command="show-modal"; where that is missing, a delegated click on
  // [data-oa-dialog] opens it. closedby="any" adds backdrop dismissal,
  // emulated where unsupported. Focus returns to the opener on close.
  var commandSupported = typeof HTMLButtonElement !== "undefined" &&
    "command" in HTMLButtonElement.prototype;
  var closedBySupported = typeof HTMLDialogElement !== "undefined" &&
    "closedBy" in HTMLDialogElement.prototype;
  var lastOutside = null;
  var delegated = false;

  function delegateDialogs() {
    if (delegated) return;
    delegated = true;
    document.addEventListener("focusin", function (e) {
      if (!e.target.closest || !e.target.closest("dialog[open]")) lastOutside = e.target;
    }, true);
    // A link with data-oa-dialog opens its dialog when one is on the page
    // and goes to its own page otherwise (no script, or no dialog). Buttons
    // open natively where invoker commands ship.
    document.addEventListener("click", function (e) {
      var opener = e.target.closest && e.target.closest("[data-oa-dialog]");
      if (!opener) return;
      if (commandSupported && opener.tagName !== "A") return;
      var d = document.getElementById(opener.getAttribute("data-oa-dialog"));
      if (d && d.showModal && !d.open) {
        e.preventDefault();
        lastOutside = opener;
        d.showModal();
      }
    });
  }

  function dialog() {
    return {
      init: function () {
        var d = this.$el;
        var returnTo = null;
        delegateDialogs();
        var openers = function () {
          return Array.prototype.slice.call(document.querySelectorAll("[data-oa-dialog]")).filter(function (el) {
            return el.getAttribute("data-oa-dialog") === d.id;
          });
        };
        var state = function () {
          var open = d.open;
          setState([d].concat(openers()), open);
          if (open && !returnTo) returnTo = lastOutside;
        };
        d.addEventListener("command", function (e) { if (e.source) returnTo = e.source; });
        new MutationObserver(state).observe(d, { attributes: true, attributeFilter: ["open"] });
        d.addEventListener("close", function () {
          state();
          var active = document.activeElement;
          if (returnTo && returnTo.isConnected && (!active || active === document.body || d.contains(active))) {
            returnTo.focus({ preventScroll: true });
          }
          returnTo = null;
        });
        if (!closedBySupported && d.getAttribute("closedby") === "any") {
          d.addEventListener("click", function (e) {
            if (e.target !== d) return;
            var r = d.getBoundingClientRect();
            if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) d.close();
          });
        }
        // data-oa-open: a dialog added to the page already open (loaded on
        // demand, such as a form with fresh tickets); it leaves when closed.
        if (d.hasAttribute("data-oa-open")) {
          d.addEventListener("close", function () { d.remove(); });
          if (!d.open && d.showModal) d.showModal();
        }
        state();
      }
    };
  }

  var components = {
    oaPopover: popover,
    oaMenu: menu,
    oaTooltip: tooltip,
    oaSelect: select,
    oaDialog: dialog
  };

  function register(Alpine) {
    Object.keys(components).forEach(function (name) { Alpine.data(name, components[name]); });
  }

  if (window.Alpine) register(window.Alpine);
  else document.addEventListener("alpine:init", function () { register(window.Alpine); });
})();
