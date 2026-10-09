// /live (#10197): the route map with OpenAgents' real traffic. It reads
// the pay host's public flow events (openagents.flow-event.v1, section 7 of
// docs/payments/2026-10-02-central-receive-and-splits.md) through this
// site's /api/flow/snapshot and /api/flow/stream, draws the snapshot's
// topology (positions from the same route_map layout the desktop uses), and
// animates each streamed event the way the desktop deck's `routes-live`
// scene does (crates/openagents-desktop/src/route_live.rs):
//
//   call, run  neutral, from the router out to the node
//   payment    warning, from the node back to the router
//   share      warning, from the router out past the node to its author
//   payout     warning, from the router straight to the author's wallet
//   bonus      warning with a ring, out to the author
//
// Events on one node wait for the one before to land. Nothing is made up:
// the snapshot's events are history (counted and listed, not animated), and
// with no stream the page says so and draws no dots. After a drop it
// reconnects, backing off, and skips events it has already seen.
//
// The mapping is plain functions, exported for `node --test` under
// static/flow.test.js; the page wiring runs only in a browser.
(function (root) {
  "use strict";

  // A Coder Noir role as the page's design token (openagents-ui), resolved
  // to a color the canvas accepts. Tokens are `light-dark()` values, so a
  // hidden probe element computes them for the current theme.
  var TOKENS = {
    canvas: "--color-surface",
    content: "--color-text",
    "content-secondary": "--color-text-secondary",
    "stroke-subtle": "--color-border-subtle",
    info: "--color-text-info",
    success: "--color-text-success",
    warning: "--color-text-warning"
  };
  function theme(role) {
    var name = TOKENS[role] || "--" + role;
    if (typeof document === "undefined" || typeof root.getComputedStyle !== "function" || !document.body) {
      return "var(" + name + ")";
    }
    var probe = document.createElement("span");
    probe.hidden = true;
    probe.style.color = "var(" + name + ")";
    document.body.appendChild(probe);
    var color = root.getComputedStyle(probe).color;
    document.body.removeChild(probe);
    return color;
  }
  var REQUEST = theme("content");
  var PAYMENT = theme("warning");
  var TRIP = 1.6;
  var SHARE = 2.0;
  var PAYOUT = 1.4;
  var MAX_WAIT = 3.0;
  var AUTHOR = 34;
  var WALLET = 64;

  // Application theme roles preserve the map's distinct node kinds.
  var KIND_COLOR = {
    front: theme("content"),
    family: theme("content-secondary"),
    route: theme("info"),
    answer: theme("terminal-ansi-6"),
    knowledge: theme("success"),
    model: theme("content-secondary"),
    coder: theme("terminal-ansi-5"),
    engine: theme("terminal-ansi-13"),
    plugin: theme("warning"),
    screen: theme("warning")
  };
  // Base radii in world units (route_map::layout::radius at weight 0.5).
  var KIND_RADIUS = {
    front: 34, family: 22, route: 16, coder: 20, model: 14,
    plugin: 12, engine: 10, answer: 7, knowledge: 7, screen: 7
  };

  // The snapshot's topology as {nodes, index}: each node {id, kind,
  // parent (an index or -1), x, y}. Accepts a bare array or {nodes}, a
  // parent by id or index, and a position as {position:{x,y}} or x, y.
  function topology(raw) {
    var list = Array.isArray(raw) ? raw : (raw && raw.nodes) || [];
    var index = {};
    var nodes = list.map(function (n, i) {
      index[n.id] = i;
      var p = n.position || n;
      return { id: String(n.id), kind: String(n.kind || ""), parentRef: n.parent,
        x: Number(p.x) || 0, y: Number(p.y) || 0, parent: -1 };
    });
    nodes.forEach(function (n) {
      var ref = n.parentRef;
      if (typeof ref === "number" && ref >= 0 && ref < nodes.length) n.parent = ref;
      else if (typeof ref === "string" && ref in index) n.parent = index[ref];
      delete n.parentRef;
    });
    return { nodes: nodes, index: index };
  }

  function find(map, id) {
    return id in map.index ? map.index[id] : -1;
  }

  // The node an event's dot runs to: its node by id; a short plugin name
  // (`plugin:explain-error`) finds `plugin:crates/plugin-explain-error`;
  // otherwise Coder for plugins, hosted resources, and runs, else the router.
  function target(map, event) {
    var node = String(event.node || "");
    var at = find(map, node);
    if (at >= 0) return at;
    var name = node.indexOf("plugin:") === 0 ? node.slice(7) : (event.plugin || "");
    if (name) {
      for (var i = 0; i < map.nodes.length; i++) {
        var n = map.nodes[i];
        if (n.kind !== "plugin" || n.id.indexOf("plugin:") !== 0) continue;
        var dir = n.id.slice(7);
        var last = dir.split("/").pop();
        if (last === name || last === "plugin-" + name) return i;
      }
    }
    var near = -1;
    var resource = event.resource;
    if (resource === "plugin" || resource === "hosted_resource" || resource === "coder" ||
        event.type === "run") near = find(map, "coder");
    if (near < 0) near = find(map, "front");
    return near < 0 ? 0 : near;
  }

  function pathTo(map, leaf) {
    var path = [leaf];
    var at = map.nodes[leaf].parent;
    var guard = 0;
    while (at >= 0 && guard++ < 64) {
      path.push(at);
      at = map.nodes[at].parent;
    }
    return path.reverse();
  }

  // A stop: a node index, or {past: index, distance}.
  function legs(map, event) {
    if (!map.nodes.length) return [];
    var leaf = target(map, event);
    var out = pathTo(map, leaf);
    var back = out.slice().reverse();
    var toAuthor = out.concat([{ past: leaf, distance: AUTHOR }]);
    function leg(stops, color, ring, seconds) {
      return { stops: stops, color: color, ring: ring, seconds: seconds };
    }
    switch (event.type) {
      case "call":
      case "run": return [leg(out, REQUEST, false, TRIP)];
      case "payment": return [leg(back, PAYMENT, false, TRIP)];
      case "share": return [leg(toAuthor, PAYMENT, false, SHARE)];
      case "bonus": return [leg(toAuthor, PAYMENT, true, SHARE)];
      case "payout": return [leg([out[0], { past: leaf, distance: WALLET }], PAYMENT, false, PAYOUT)];
      default: return [];
    }
  }

  function place(map, stop) {
    if (typeof stop === "number") return { x: map.nodes[stop].x, y: map.nodes[stop].y };
    var n = map.nodes[stop.past];
    var from = n.parent >= 0 ? map.nodes[n.parent] : { x: 0, y: 0 };
    var dx = n.x - from.x, dy = n.y - from.y;
    var length = Math.sqrt(dx * dx + dy * dy);
    if (length < 0.001) return { x: n.x + stop.distance, y: n.y };
    return { x: n.x + dx / length * stop.distance, y: n.y + dy / length * stop.distance };
  }

  function along(map, stops, t) {
    if (!stops.length) return { x: 0, y: 0 };
    if (stops.length === 1) return place(map, stops[0]);
    var hops = stops.length - 1;
    var at = Math.min(Math.max(t, 0) * hops, hops - 0.0001);
    var hop = Math.floor(at);
    var a = place(map, stops[hop]), b = place(map, stops[hop + 1]);
    var f = at - hop;
    return { x: a.x + (b.x - a.x) * f, y: a.y + (b.y - a.y) * f };
  }

  // Schedules events into flights on one clock, in seconds.
  function Schedule() {
    this.ready = {};
    this.flights = [];
  }
  Schedule.prototype.push = function (map, event, now) {
    var leaf = target(map, event);
    var self = this;
    legs(map, event).forEach(function (leg) {
      var ready = leaf in self.ready ? self.ready[leaf] : now;
      var start = Math.min(Math.max(ready, now), now + MAX_WAIT);
      self.ready[leaf] = start + leg.seconds;
      self.flights.push({ seq: event.seq, leg: leg, start: start });
    });
  };
  Schedule.prototype.land = function (now) {
    this.flights = this.flights.filter(function (f) { return f.start + f.leg.seconds > now; });
  };
  Schedule.prototype.pulses = function (map, now, still) {
    var out = [];
    this.flights.forEach(function (f) {
      var t = (now - f.start) / Math.max(f.leg.seconds, 0.01);
      if (t < 0 || t >= 1) return;
      var at = along(map, f.leg.stops, still ? 1 : t);
      out.push({ x: at.x, y: at.y, color: f.leg.color, ring: f.leg.ring,
        radius: f.leg.color === PAYMENT ? 3.2 : 3.0 });
    });
    return out;
  };

  // Counts one event into the totals.
  function count(totals, event) {
    var amount = Number(event.amount_sats) || 0;
    // Sum in whole millisatoshis so fractional sats never drift.
    function plus(sats) { return (Math.round(sats * 1000) + Math.round(amount * 1000)) / 1000; }
    if (event.type === "call") totals.calls += 1;
    else if (event.type === "payment") totals.received_sats = plus(totals.received_sats);
    else if (event.type === "payout") totals.paid_out_sats = plus(totals.paid_out_sats);
  }

  function grouped(n) {
    return String(Math.round(n)).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  }

  // Money has millisatoshi precision; call counts remain integral.
  function money(n) {
    return Number(n).toLocaleString("en-US", { minimumFractionDigits: 0, maximumFractionDigits: 3 });
  }

  function utc(ms) {
    var d = new Date(Number(ms) || 0);
    function two(n) { return n < 10 ? "0" + n : String(n); }
    return d.getUTCFullYear() + "-" + two(d.getUTCMonth() + 1) + "-" + two(d.getUTCDate()) +
      " " + two(d.getUTCHours()) + ":" + two(d.getUTCMinutes()) + ":" + two(d.getUTCSeconds()) + " UTC";
  }

  // One event as a recent-list line: public fields only.
  function describe(event) {
    var what = event.plugin || event.node || "";
    var words = { call: "call", payment: "payment", share: "author share",
      payout: "payout", bonus: "bonus", run: "Coder run" }[event.type] || event.type;
    var amount = event.amount_sats != null ? " · " + money(event.amount_sats) + " sats" : "";
    return utc(event.at).slice(11, 19) + "  " + words + amount + (what ? " · " + what : "");
  }

  var api = { topology: topology, target: target, pathTo: pathTo, legs: legs, place: place,
    along: along, Schedule: Schedule, count: count, grouped: grouped, money: money, utc: utc,
    describe: describe, REQUEST: REQUEST, PAYMENT: PAYMENT, TRIP: TRIP, SHARE: SHARE,
    PAYOUT: PAYOUT, MAX_WAIT: MAX_WAIT, AUTHOR: AUTHOR, WALLET: WALLET };
  if (typeof module === "object" && module.exports) module.exports = api;
  if (typeof document === "undefined") return;

  // The page.
  var box = document.getElementById("flow");
  var canvas = document.getElementById("flow-map");
  var statusLine = document.getElementById("flow-status");
  var recent = document.getElementById("flow-recent");
  if (!box || !canvas || !canvas.getContext || !window.fetch) return;
  var context = canvas.getContext("2d");
  var still = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  var map = { nodes: [], index: {} };
  var schedule = new Schedule();
  var totals = { received_sats: 0, paid_out_sats: 0, calls: 0 };
  var lastSeq = 0;
  var lastAt = null;
  var state = "connecting";
  var source = null;
  var wait = 2000;
  var started = performance.now();
  var shown = [];

  function clock() { return (performance.now() - started) / 1000; }

  function say() {
    var last = lastAt != null ? "last event " + utc(lastAt) : "no events yet";
    statusLine.textContent =
      state === "live" ? "Live · " + last :
      state === "down" ? "The flow stream is unreachable · " + last + ". Retrying." :
      state === "again" ? "The flow stream dropped · " + last + ". Reconnecting." :
      "Connecting to the flow stream.";
    document.getElementById("flow-received").textContent = money(totals.received_sats) + " sats";
    document.getElementById("flow-paid").textContent = money(totals.paid_out_sats) + " sats";
    document.getElementById("flow-calls").textContent = grouped(totals.calls);
  }

  function remember(event) {
    shown.unshift(describe(event));
    shown = shown.slice(0, 12);
    recent.textContent = "";
    shown.forEach(function (text) {
      var li = document.createElement("li");
      li.textContent = text;
      recent.appendChild(li);
    });
  }

  function take(event, animate) {
    if (!event || typeof event !== "object") return;
    var seq = Number(event.seq) || 0;
    if (seq && seq <= lastSeq) return;
    lastSeq = Math.max(lastSeq, seq);
    lastAt = event.at;
    remember(event);
    if (!animate) return;
    count(totals, event);
    schedule.push(map, event, clock());
  }

  function snapshot() {
    return fetch(box.getAttribute("data-snapshot"), { headers: { Accept: "application/json" } })
      .then(function (response) {
        if (!response.ok) throw new Error("snapshot " + response.status);
        return response.json();
      })
      .then(function (snap) {
        if (snap.topology) map = topology(snap.topology);
        var t = snap.totals || {};
        totals = { received_sats: Number(t.received_sats) || 0,
          paid_out_sats: Number(t.paid_out_sats) || 0, calls: Number(t.calls) || 0 };
        (snap.events || []).slice().sort(function (a, b) { return a.seq - b.seq; })
          .forEach(function (event) { take(event, false); });
      });
  }

  function down() {
    state = "down";
    schedule.flights = [];
    if (source) { source.close(); source = null; }
    say();
    setTimeout(connect, wait);
    wait = Math.min(wait * 2, 30000);
  }

  function connect() {
    state = state === "down" ? "down" : "connecting";
    say();
    snapshot().then(function () {
      source = new EventSource(box.getAttribute("data-stream"));
      source.onopen = function () { state = "live"; wait = 2000; say(); };
      source.onmessage = function (message) {
        var event;
        try { event = JSON.parse(message.data); } catch (e) { return; }
        state = "live";
        take(event, true);
        say();
      };
      // The browser retries a dropped stream by itself, sending the last
      // id; one it gave up on is closed, so start over.
      source.onerror = function () {
        if (!source || source.readyState === 2) down();
        else { state = "again"; say(); }
      };
    }).catch(down);
  }

  // Drawing.
  var view = { scale: 1, x: 0, y: 0, ratio: 1, w: 0, h: 0 };
  function fit() {
    var ratio = window.devicePixelRatio || 1;
    var w = canvas.clientWidth, h = canvas.clientHeight;
    if (canvas.width !== Math.round(w * ratio) || canvas.height !== Math.round(h * ratio)) {
      canvas.width = Math.round(w * ratio);
      canvas.height = Math.round(h * ratio);
    }
    var minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    map.nodes.forEach(function (n) {
      var r = (KIND_RADIUS[n.kind] || 8) + WALLET;
      minX = Math.min(minX, n.x - r); maxX = Math.max(maxX, n.x + r);
      minY = Math.min(minY, n.y - r); maxY = Math.max(maxY, n.y + r);
    });
    if (minX > maxX) { minX = -1; maxX = 1; minY = -1; maxY = 1; }
    var margin = 12;
    var scale = Math.min((w - 2 * margin) / (maxX - minX), (h - 2 * margin) / (maxY - minY));
    view = { scale: scale, ratio: ratio, w: w, h: h,
      x: w / 2 - (minX + maxX) / 2 * scale, y: h / 2 - (minY + maxY) / 2 * scale };
  }

  function screen(p) { return { x: view.x + p.x * view.scale, y: view.y + p.y * view.scale }; }

  function dot(p, r, color, alpha) {
    context.globalAlpha = alpha;
    context.fillStyle = color;
    context.beginPath();
    context.arc(p.x, p.y, r, 0, Math.PI * 2);
    context.fill();
  }

  function draw() {
    fit();
    context.setTransform(view.ratio, 0, 0, view.ratio, 0, 0);
    context.clearRect(0, 0, view.w, view.h);
    context.fillStyle = theme("canvas");
    context.fillRect(0, 0, view.w, view.h);
    // As the map shrinks, nodes grow a little so they read (as the deck).
    var boost = Math.min(Math.max(Math.sqrt(0.55 / Math.max(view.scale, 0.01)), 1), 2.2);
    context.lineWidth = 1;
    context.strokeStyle = theme("stroke-subtle");
    context.globalAlpha = 1;
    map.nodes.forEach(function (n) {
      if (n.parent < 0) return;
      var a = screen(map.nodes[n.parent]), b = screen(n);
      context.beginPath();
      context.moveTo(a.x, a.y);
      context.lineTo(b.x, b.y);
      context.stroke();
    });
    map.nodes.forEach(function (n) {
      var r = Math.max((KIND_RADIUS[n.kind] || 8) * view.scale * boost, 1.5);
      dot(screen(n), r, KIND_COLOR[n.kind] || theme("content-secondary"), 1);
    });
    var now = clock();
    schedule.land(now);
    if (state === "live" || state === "again") {
      schedule.pulses(map, now, still).forEach(function (p) {
        var at = screen(p);
        dot(at, p.radius * 2.6, p.color, 0.18);
        dot(at, p.radius, p.color, 1);
        if (p.ring) {
          context.globalAlpha = 1;
          context.strokeStyle = p.color;
          context.beginPath();
          context.arc(at.x, at.y, p.radius * 2.4, 0, Math.PI * 2);
          context.stroke();
        }
      });
    }
    context.globalAlpha = 1;
    window.requestAnimationFrame(draw);
  }

  say();
  connect();
  window.requestAnimationFrame(draw);
})(this);
