// Playtest research capture for the launch funnel (privacy policy,
// "Playtest research"). First-party: batches go only to this site's /a/r
// with navigator.sendBeacon. It never reads what anyone types: for a
// field it sends the field's name or label, never its value.
//
// Opted out (the playtest intro's toggle sets localStorage
// oa_research_optout=1), or Do Not Track / Global Privacy Control: no id,
// label, or detail is sent, only the kind of event and the page.
//
// Pages report their own milestones with window.oaResearch.track(name,
// props) or a "oa:research" CustomEvent ({detail: {name, props}}), and
// engine or bridge state with a data-engine-state or data-bridge-state
// attribute on any element.
(function (root, factory) {
  if (typeof module === "object" && module.exports) {
    module.exports = factory;
  } else if (root && root.document) {
    root.oaResearch = factory(root).start();
  }
})(typeof window === "undefined" ? null : window, function (win) {
  "use strict";
  var doc = win.document;
  var nav = win.navigator || {};
  var loc = win.location;
  var BEACON = "/a/r";
  var COOKIE = "oa_playtest";
  var OPT_OUT = "oa_research_optout";
  var MOST_BATCH = 40;
  var MOST_QUEUE = 400;
  var FLUSH_MS = 5000;
  var queue = [];
  var timer = null;
  var started = false;
  var shownAt = Date.now();
  var activeMs = 0;
  var maxScroll = 0;
  var lastPath = "";

  function store(kind) {
    try {
      return win[kind] || null;
    } catch (_) {
      return null;
    }
  }

  function quiet() {
    return nav.doNotTrack === "1" || win.doNotTrack === "1" || nav.globalPrivacyControl === true;
  }

  function optedOut() {
    var local = store("localStorage");
    try {
      if (local && local.getItem(OPT_OUT) === "1") return true;
    } catch (_) {}
    return /(?:^|;\s*)oa_research_optout=1(?:;|$)/.test(doc.cookie || "");
  }

  function aggregate() {
    return quiet() || optedOut();
  }

  function hex(n) {
    var out = "";
    var bytes = new Uint8Array(n);
    if (win.crypto && win.crypto.getRandomValues) win.crypto.getRandomValues(bytes);
    else for (var i = 0; i < n; i++) bytes[i] = Math.floor(Math.random() * 256);
    for (var j = 0; j < n; j++) out += (bytes[j] < 16 ? "0" : "") + bytes[j].toString(16);
    return out;
  }

  function sessionId() {
    var match = /(?:^|;\s*)oa_playtest=([A-Za-z0-9_-]{8,64})(?:;|$)/.exec(doc.cookie || "");
    if (match) return match[1];
    var tab = store("sessionStorage");
    try {
      var id = tab && tab.getItem("oa_research_tab");
      if (id && /^[A-Za-z0-9_-]{8,64}$/.test(id)) return id;
      id = "t_" + hex(12);
      if (tab) tab.setItem("oa_research_tab", id);
      return id;
    } catch (_) {
      return "t_" + hex(12);
    }
  }

  function utm() {
    var tab = store("sessionStorage");
    var saved = null;
    try {
      saved = tab && JSON.parse(tab.getItem("oa_research_utm") || "null");
    } catch (_) {}
    var params;
    try {
      params = new URLSearchParams(loc.search || "");
    } catch (_) {
      return saved || {};
    }
    var source = params.get("utm_source");
    var campaign = params.get("utm_campaign");
    if (source || campaign) {
      saved = { source: (source || "").slice(0, 48), campaign: (campaign || "").slice(0, 48) };
      try {
        if (tab) tab.setItem("oa_research_utm", JSON.stringify(saved));
      } catch (_) {}
    }
    return saved || {};
  }

  function clean(text, most) {
    return String(text || "").replace(/\s+/g, " ").trim().slice(0, most);
  }

  function path() {
    return String(loc.pathname || "/").slice(0, 128);
  }

  // What a click landed on, as a label: never an input's value.
  var TARGETS = "[data-aethyr-id],a,button,summary,label,select,textarea,input,[role],[id]";
  function describe(el) {
    var target = el.closest ? el.closest(TARGETS) || el : el;
    var tag = (target.tagName || "").toLowerCase();
    var field = tag === "input" || tag === "textarea" || tag === "select" || target.isContentEditable;
    var aethyr = target.getAttribute && target.getAttribute("data-aethyr-id");
    var role = (target.getAttribute && target.getAttribute("role")) || "";
    var name = field ? (target.getAttribute("name") || target.getAttribute("aria-label") || "") : "";
    var text = field
      ? ""
      : clean(target.getAttribute && target.getAttribute("aria-label") || target.textContent, 60);
    var label = aethyr || target.id || name || text || tag;
    var props = { tag: tag };
    if (aethyr) props.aethyr = aethyr;
    if (target.id) props.id = target.id;
    if (role) props.role = role;
    if (text && label !== text) props.text = text;
    if (field) props.field = target.getAttribute("type") || tag;
    if (tag === "a" && target.getAttribute("href")) {
      props.href = String(target.getAttribute("href")).split(/[?#]/)[0].slice(0, 120);
    }
    return { label: clean(label, 80), props: props };
  }

  function push(kind, label, props) {
    if (queue.length >= MOST_QUEUE) return;
    var event = { k: kind, t: Date.now(), p: path() };
    if (!aggregate()) {
      if (label) event.l = clean(label, 80);
      if (props) event.x = props;
    }
    queue.push(event);
    if (queue.length >= MOST_BATCH) flush();
    else if (!timer) timer = win.setTimeout(flush, FLUSH_MS);
  }

  function send(body) {
    try {
      if (typeof nav.sendBeacon === "function") {
        return nav.sendBeacon(BEACON, new Blob([JSON.stringify(body)], { type: "text/plain" }));
      }
    } catch (_) {
      // Recording never gets in the way.
    }
    return false;
  }

  function flush(extra) {
    if (timer) {
      win.clearTimeout(timer);
      timer = null;
    }
    while (queue.length || extra) {
      var events = queue.splice(0, MOST_BATCH);
      var body = { v: 1, n: Date.now(), e: events };
      if (aggregate()) body.a = 1;
      else {
        body.s = sessionId();
        body.u = utm();
      }
      if (extra) {
        for (var key in extra) body[key] = extra[key];
        extra = null;
      }
      send(body);
    }
  }

  function viewport() {
    var w = win.innerWidth || 0;
    return {
      width: w,
      height: win.innerHeight || 0,
      dpr: Math.round((win.devicePixelRatio || 1) * 100) / 100,
      touch: "ontouchstart" in win || (nav.maxTouchPoints || 0) > 0,
      class: w < 600 ? "phone" : w < 1024 ? "tablet" : "desktop",
    };
  }

  function pageView() {
    var current = path();
    if (current === lastPath) return;
    var kind = lastPath ? "route" : "page_view";
    lastPath = current;
    maxScroll = 0;
    var props = viewport();
    if (doc.referrer) {
      try {
        var from = new URL(doc.referrer);
        props.referrer = from.host === loc.host ? from.pathname.slice(0, 120) : from.host;
      } catch (_) {}
    }
    push(kind, doc.title ? clean(doc.title, 80) : "", props);
  }

  function onClick(event) {
    var el = event.target;
    if (!el || el.nodeType !== 1) return;
    var what = describe(el);
    push("click", what.label, what.props);
  }

  function onSubmit(event) {
    var form = event.target;
    if (!form || !form.elements) return;
    var names = [];
    for (var i = 0; i < form.elements.length && names.length < 20; i++) {
      var name = form.elements[i].getAttribute && form.elements[i].getAttribute("name");
      if (name && names.indexOf(name) < 0) names.push(name);
    }
    var label = form.getAttribute("data-aethyr-id") || form.id || form.getAttribute("name") || "form";
    var action = String(form.getAttribute("action") || "").split(/[?#]/)[0].slice(0, 120);
    push("submit", label, { fields: names.join(",").slice(0, 120), action: action });
    flush();
  }

  function onScroll() {
    var el = doc.documentElement || {};
    var height = (el.scrollHeight || 0) - (win.innerHeight || 0);
    var depth = height <= 0 ? 100 : Math.min(100, Math.round((100 * (win.scrollY || 0)) / height));
    var steps = [25, 50, 75, 100];
    for (var i = 0; i < steps.length; i++) {
      if (depth >= steps[i] && maxScroll < steps[i]) {
        maxScroll = steps[i];
        push("scroll", String(steps[i]), { depth: steps[i] });
      }
    }
  }

  function onVisibility() {
    var hidden = doc.visibilityState === "hidden";
    if (hidden) {
      activeMs += Date.now() - shownAt;
      push("visibility", "hidden", { active_ms: activeMs });
      push("session", "", { active_ms: activeMs, scroll: maxScroll });
      flush();
    } else {
      shownAt = Date.now();
      push("visibility", "visible", null);
    }
  }

  function onError(event) {
    var source = "";
    try {
      source = event.filename ? new URL(event.filename).pathname.slice(0, 120) : "";
    } catch (_) {}
    push("error", clean(event.message || "error", 120), { source: source, line: event.lineno || 0 });
  }

  function onRejection(event) {
    var reason = event.reason;
    var message = reason && reason.message ? reason.message : String(reason || "rejection");
    push("error", clean(message, 120), { source: "promise" });
  }

  function track(name, props) {
    if (!/^[a-z][a-z0-9_]{0,47}$/.test(String(name || ""))) return;
    var clean_props = {};
    var count = 0;
    for (var key in props || {}) {
      if (count >= 12 || !/^[a-z][a-z0-9_]{0,47}$/.test(key)) continue;
      var value = props[key];
      if (typeof value === "string") clean_props[key] = clean(value, 120);
      else if (typeof value === "number" || typeof value === "boolean") clean_props[key] = value;
      else continue;
      count++;
    }
    push("milestone", name, clean_props);
  }

  function watchEngine() {
    if (typeof win.MutationObserver !== "function" || !doc.documentElement) return;
    var seen = {};
    var report = function (el, attribute) {
      var state = clean(el.getAttribute(attribute), 40);
      var key = attribute + ":" + (el.id || el.getAttribute("data-aethyr-id") || "");
      if (!state || seen[key] === state) return;
      seen[key] = state;
      push("engine", attribute.replace("data-", "").replace("-state", "") + ":" + state, {
        state: state,
        element: el.id || el.getAttribute("data-aethyr-id") || "",
      });
    };
    var attributes = ["data-engine-state", "data-bridge-state"];
    new win.MutationObserver(function (records) {
      for (var i = 0; i < records.length; i++) {
        if (records[i].type === "attributes") report(records[i].target, records[i].attributeName);
      }
    }).observe(doc.documentElement, { subtree: true, attributes: true, attributeFilter: attributes });
    var present = doc.querySelectorAll("[data-engine-state],[data-bridge-state]");
    for (var i = 0; i < present.length; i++) {
      for (var j = 0; j < attributes.length; j++) {
        if (present[i].hasAttribute(attributes[j])) report(present[i], attributes[j]);
      }
    }
  }

  function wrapHistory(name) {
    var original = win.history && win.history[name];
    if (typeof original !== "function") return;
    win.history[name] = function () {
      var out = original.apply(this, arguments);
      pageView();
      return out;
    };
  }

  function optOut(on) {
    var local = store("localStorage");
    var id = sessionId();
    try {
      if (local) {
        if (on) local.setItem(OPT_OUT, "1");
        else local.removeItem(OPT_OUT);
      }
    } catch (_) {}
    doc.cookie = OPT_OUT + "=" + (on ? "1" : "0") + "; path=/; max-age=" + (on ? 31536000 : 0) + "; samesite=lax";
    if (on) {
      queue.length = 0;
      send({ v: 1, s: id, o: 1, n: Date.now(), e: [] });
    }
  }

  function start() {
    if (started) return api;
    started = true;
    pageView();
    doc.addEventListener("click", onClick, true);
    doc.addEventListener("submit", onSubmit, true);
    doc.addEventListener("visibilitychange", onVisibility);
    win.addEventListener("scroll", onScroll, { passive: true });
    win.addEventListener("error", onError);
    win.addEventListener("unhandledrejection", onRejection);
    win.addEventListener("popstate", pageView);
    win.addEventListener("hashchange", pageView);
    win.addEventListener("pagehide", function () {
      flush();
    });
    win.addEventListener("oa:research", function (event) {
      var detail = event.detail || {};
      track(detail.name, detail.props);
    });
    wrapHistory("pushState");
    wrapHistory("replaceState");
    watchEngine();
    return api;
  }

  var api = {
    start: start,
    track: track,
    flush: flush,
    optOut: optOut,
    optedOut: aggregate,
    sessionId: sessionId,
    _queue: queue,
  };
  return api;
});
