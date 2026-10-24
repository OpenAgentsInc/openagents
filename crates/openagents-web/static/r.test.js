// The playtest research capture script (static/r.js): batching, what a
// click sends, and that an input's value is never sent. Run: node --test
// crates/openagents-web/static/r.test.js (the crate's tests run it when
// node is installed). Not served.
"use strict";
var test = require("node:test");
var assert = require("node:assert");
var factory = require("./r.js");

function storage() {
  var map = {};
  return {
    getItem: function (k) { return Object.prototype.hasOwnProperty.call(map, k) ? map[k] : null; },
    setItem: function (k, v) { map[k] = String(v); },
    removeItem: function (k) { delete map[k]; },
  };
}

function element(tag, attributes, extra) {
  var attrs = attributes || {};
  var el = {
    nodeType: 1,
    tagName: tag.toUpperCase(),
    id: attrs.id || "",
    textContent: "",
    parent: null,
    getAttribute: function (name) { return Object.prototype.hasOwnProperty.call(attrs, name) ? attrs[name] : null; },
    hasAttribute: function (name) { return Object.prototype.hasOwnProperty.call(attrs, name); },
    closest: function (selector) {
      var wanted = selector.split(",");
      for (var node = el; node; node = node.parent) {
        for (var i = 0; i < wanted.length; i++) {
          var s = wanted[i];
          var attr = /^\[([a-z-]+)\]$/.exec(s);
          if (attr ? node.hasAttribute(attr[1]) || (attr[1] === "id" && node.id) : node.tagName.toLowerCase() === s) {
            return node;
          }
        }
      }
      return null;
    },
  };
  for (var key in extra || {}) el[key] = extra[key];
  return el;
}

function harness(options) {
  var opts = options || {};
  var handlers = {};
  var sent = [];
  var timers = [];
  function on(target) {
    return function (name, fn) { (handlers[target + ":" + name] = handlers[target + ":" + name] || []).push(fn); };
  }
  var win = {
    innerWidth: 390,
    innerHeight: 844,
    devicePixelRatio: 3,
    scrollY: 0,
    location: { pathname: opts.path || "/playtest", search: opts.search || "", host: "openagents.com" },
    navigator: {
      doNotTrack: opts.dnt ? "1" : null,
      maxTouchPoints: 5,
      sendBeacon: function (url, blob) { sent.push({ url: url, blob: blob }); return true; },
    },
    sessionStorage: storage(),
    localStorage: storage(),
    history: { pushState: function () {}, replaceState: function () {} },
    setTimeout: function (fn) { timers.push(fn); return timers.length; },
    clearTimeout: function () {},
    addEventListener: on("window"),
    crypto: require("node:crypto").webcrypto,
  };
  win.document = {
    cookie: opts.cookie || "",
    title: "Kiki playtest",
    referrer: "",
    visibilityState: "visible",
    documentElement: { scrollHeight: 2000 },
    addEventListener: on("document"),
    querySelectorAll: function () { return []; },
  };
  return {
    win: win,
    sent: sent,
    fire: function (target, name, event) {
      (handlers[target + ":" + name] || []).forEach(function (fn) { fn(event); });
    },
    bodies: async function () {
      var out = [];
      for (var i = 0; i < sent.length; i++) out.push(JSON.parse(await sent[i].blob.text()));
      return out;
    },
  };
}

test("a page view is batched and sent with the playtest session id and campaign", async function () {
  var h = harness({ cookie: "oa_playtest=ps_abcdef123456", search: "?utm_source=reddit&utm_campaign=reddit-gamedev" });
  var api = factory(h.win).start();
  assert.strictEqual(h.sent.length, 0, "nothing is sent before a flush");
  api.flush();
  var bodies = await h.bodies();
  assert.strictEqual(bodies.length, 1);
  assert.strictEqual(h.sent[0].url, "/a/r");
  assert.strictEqual(bodies[0].s, "ps_abcdef123456");
  assert.deepStrictEqual(bodies[0].u, { source: "reddit", campaign: "reddit-gamedev" });
  assert.strictEqual(bodies[0].e[0].k, "page_view");
  assert.strictEqual(bodies[0].e[0].p, "/playtest");
  assert.strictEqual(bodies[0].e[0].x.class, "phone");
});

test("clicks are batched forty at a time", async function () {
  var h = harness();
  var api = factory(h.win).start();
  var button = element("button", { "data-aethyr-id": "start-playtest" });
  for (var i = 0; i < 45; i++) h.fire("document", "click", { target: button });
  assert.strictEqual(h.sent.length, 1, "a full batch goes at once");
  api.flush();
  var bodies = await h.bodies();
  assert.strictEqual(bodies[0].e.length, 40);
  assert.strictEqual(bodies[1].e.length, 6);
  var click = bodies[0].e[1];
  assert.strictEqual(click.k, "click");
  assert.strictEqual(click.l, "start-playtest");
  assert.strictEqual(click.x.aethyr, "start-playtest");
  assert.match(bodies[0].s, /^t_[0-9a-f]{24}$/, "without the cookie, a per-tab id");
});

test("an input's value is never sent", async function () {
  var h = harness();
  var api = factory(h.win).start();
  var secret = "hunter2-my-secret-answer";
  var input = element("input", { name: "email", type: "email" }, { value: secret, textContent: secret });
  var area = element("textarea", { name: "feedback" }, { value: secret, textContent: secret });
  h.fire("document", "click", { target: input });
  h.fire("document", "click", { target: area });
  var form = element("form", { id: "survey" });
  form.elements = [input, area];
  h.fire("document", "submit", { target: form });
  api.flush();
  var text = (await h.bodies()).map(function (b) { return JSON.stringify(b); }).join("\n");
  assert.ok(!text.includes(secret), "the value appears nowhere");
  assert.ok(text.includes("\"fields\":\"email,feedback\""), "a submit sends field names");
  assert.ok(text.includes("\"l\":\"email\""), "a field click is labeled by its name");
});

test("Do Not Track sends only the kind of event and the page", async function () {
  var h = harness({ dnt: true, cookie: "oa_playtest=ps_abcdef123456" });
  var api = factory(h.win).start();
  h.fire("document", "click", { target: element("button", { id: "go" }) });
  api.flush();
  var body = (await h.bodies())[0];
  assert.strictEqual(body.a, 1);
  assert.strictEqual(body.s, undefined);
  assert.strictEqual(body.u, undefined);
  body.e.forEach(function (e) {
    assert.deepStrictEqual(Object.keys(e).sort(), ["k", "p", "t"]);
  });
});

test("opting out tells the server once, then sends counts only", async function () {
  var h = harness({ cookie: "oa_playtest=ps_abcdef123456" });
  var api = factory(h.win).start();
  api.optOut(true);
  h.fire("document", "click", { target: element("button", { id: "go" }) });
  api.flush();
  var bodies = await h.bodies();
  assert.strictEqual(bodies[0].o, 1);
  assert.strictEqual(bodies[0].s, "ps_abcdef123456");
  assert.strictEqual(bodies[0].e.length, 0, "the queued events are dropped");
  assert.strictEqual(bodies[1].a, 1);
  assert.strictEqual(bodies[1].s, undefined);
});

test("milestones keep only small, named properties", async function () {
  var h = harness();
  var api = factory(h.win).start();
  api.track("tutorial_moved", { seconds: 4, how: "keys", nested: { no: 1 }, "Bad Key": "x" });
  api.track("Not A Name", {});
  api.flush();
  var events = (await h.bodies())[0].e;
  var milestone = events.filter(function (e) { return e.k === "milestone"; });
  assert.strictEqual(milestone.length, 1);
  assert.deepStrictEqual(milestone[0].x, { seconds: 4, how: "keys" });
});
