// Firm reason: 1 (browser host check: tests dom-stream.js multiplexed SSE streams, Last-Event-ID reconnect, IME/caret guards and modal dialog stack against a live HTTP SSE server)
import test from "node:test";
import assert from "node:assert/strict";
import http from "node:http";
import {
  createStreamPool,
  createInputController,
  DOM_STREAM_BASE,
  DOM_STREAM_OPEN,
  DOM_STREAM_CHUNK,
  DOM_STREAM_END,
  DOM_STREAM_ERROR,
  DOM_STREAM_FLAG_SSE,
  DOM_STREAM_FLAG_NO_RECONNECT,
  urlAllowed,
  backoffMs,
  shouldReconnect,
  clampCaret,
} from "./dom-stream.js";

function startServer(handler) {
  return new Promise((resolve) => {
    const server = http.createServer(handler);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      resolve({
        server,
        baseUrl: `http://127.0.0.1:${port}`,
        close: () => new Promise((r) => server.close(() => r())),
      });
    });
  });
}

function waitFor(pred, timeoutMs = 4000) {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    const tick = () => {
      if (pred()) return resolve();
      if (Date.now() - start > timeoutMs) return reject(new Error("timed out waiting for condition"));
      setTimeout(tick, 10);
    };
    tick();
  });
}

test("policy helpers enforce URL safety, backoff curve, reconnect rules, and caret clamping", () => {
  assert.equal(urlAllowed("/api/chat/events/stream/v2alpha1?id=trn_1"), true);
  assert.equal(urlAllowed("//evil.example/x"), false);
  assert.equal(urlAllowed("/\\evil.example/x"), false);
  assert.equal(urlAllowed("https://evil.example/x"), false);
  assert.equal(urlAllowed("/api/bad\nline"), false);

  assert.equal(backoffMs(0, 0), 500);
  assert.equal(backoffMs(1, 0), 1000);
  assert.equal(backoffMs(5, 0), 16000);
  assert.equal(backoffMs(6, 0), 30000);
  assert.equal(backoffMs(0, 3000), 3000);
  assert.equal(backoffMs(3, 3000), 4000);

  assert.equal(shouldReconnect(true, DOM_STREAM_FLAG_SSE, 0, false, 0), true);
  assert.equal(shouldReconnect(true, DOM_STREAM_FLAG_SSE, 503, false, 7), true);
  assert.equal(shouldReconnect(true, DOM_STREAM_FLAG_SSE, 503, false, 8), false);
  assert.equal(shouldReconnect(true, DOM_STREAM_FLAG_SSE, 200, true, 0), true);
  assert.equal(shouldReconnect(true, DOM_STREAM_FLAG_SSE, 204, true, 0), false);
  assert.equal(shouldReconnect(true, DOM_STREAM_FLAG_SSE, 404, false, 0), false);
  assert.equal(shouldReconnect(true, DOM_STREAM_FLAG_SSE | DOM_STREAM_FLAG_NO_RECONNECT, 0, false, 0), false);
  assert.equal(shouldReconnect(true, DOM_STREAM_FLAG_SSE | DOM_STREAM_FLAG_NO_RECONNECT, 200, true, 0), false);
  assert.equal(shouldReconnect(false, DOM_STREAM_FLAG_SSE, 0, false, 0), false);

  assert.equal(clampCaret(-5, 10), 0);
  assert.equal(clampCaret(4, 10), 4);
  assert.equal(clampCaret(15, 10), 10);
  assert.equal(clampCaret(4, 0), 0);
});

test("multiplexed keyed SSE streams run concurrently without aborting each other", async () => {
  const responses = new Map();
  const srv = await startServer((req, res) => {
    res.writeHead(200, {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "close",
    });
    responses.set(req.url, res);
    res.write(`id: 1\nevent: init\ndata: ${req.url}\n\n`);
  });

  const events = [];
  const decoder = new TextDecoder();
  const pool = createStreamPool({
    fetchImpl: (path, init) => fetch(srv.baseUrl + path, init),
    inboxCap: () => 4096,
    onEvent(kind, status, bytes) {
      const rel = kind - DOM_STREAM_BASE;
      events.push({
        slot: Math.floor(rel / 4),
        phase: rel % 4,
        status,
        text: bytes ? decoder.decode(bytes) : "",
      });
    },
  });

  try {
    assert.equal(pool.open(0, "/stream/sidebar", null, "", DOM_STREAM_FLAG_SSE), 1);
    assert.equal(pool.open(1, "/stream/turn", null, "", DOM_STREAM_FLAG_SSE | DOM_STREAM_FLAG_NO_RECONNECT), 1);

    await waitFor(() =>
      events.some((e) => e.slot === 0 && e.phase === DOM_STREAM_CHUNK && e.text.includes("/stream/sidebar")) &&
      events.some((e) => e.slot === 1 && e.phase === DOM_STREAM_CHUNK && e.text.includes("/stream/turn"))
    );

    // Push interleaved live events on both slots simultaneously.
    responses.get("/stream/sidebar").write("id: 2\nevent: thread\ndata: side-update\n\n");
    responses.get("/stream/turn").write("id: 2\nevent: turn\ndata: tok-1\n\n");

    await waitFor(() =>
      events.some((e) => e.slot === 0 && e.text.includes("side-update")) &&
      events.some((e) => e.slot === 1 && e.text.includes("tok-1"))
    );

    // Closing slot 0 must NOT interrupt slot 1.
    pool.close(0);
    responses.get("/stream/turn").write("id: 3\nevent: turn\ndata: tok-2\n\n");
    responses.get("/stream/turn").end();

    await waitFor(() =>
      events.some((e) => e.slot === 1 && e.text.includes("tok-2")) &&
      events.some((e) => e.slot === 1 && e.phase === DOM_STREAM_END && e.status === 200)
    );
    assert.equal(events.some((e) => e.slot === 0 && e.phase === DOM_STREAM_ERROR), false);
  } finally {
    pool.stopAll();
    for (const res of responses.values()) { try { res.end(); } catch {} }
    await srv.close();
  }
});

test("dropped GET SSE stream automatically reconnects with Last-Event-ID and delivers resumed events", async () => {
  const seenHeaders = [];
  let attempt = 0;
  const srv = await startServer((req, res) => {
    attempt += 1;
    seenHeaders.push(req.headers["last-event-id"] || "");
    if (attempt === 1) {
      res.writeHead(200, { "Content-Type": "text/event-stream", Connection: "close" });
      res.write("retry: 25\n\nid: 1\nevent: turn\ndata: one\n\nid: 2\nevent: turn\ndata: two\n\n");
      // Abruptly destroy TCP socket mid-stream without clean HTTP FIN/chunk terminator.
      setTimeout(() => res.socket.destroy(), 20);
      return;
    }
    res.writeHead(200, { "Content-Type": "text/event-stream", Connection: "close" });
    res.write("id: 3\nevent: turn\ndata: three\n\nid: 4\nevent: turn\ndata: four\n\n");
    res.end();
  });

  const events = [];
  const decoder = new TextDecoder();
  const pool = createStreamPool({
    fetchImpl: (path, init) => fetch(srv.baseUrl + path, init),
    baseBackoffMs: 25,
    inboxCap: () => 4096,
    onEvent(kind, status, bytes) {
      const rel = kind - DOM_STREAM_BASE;
      events.push({
        slot: Math.floor(rel / 4),
        phase: rel % 4,
        status,
        text: bytes ? decoder.decode(bytes) : "",
      });
    },
  });

  try {
    assert.equal(pool.open(2, "/api/chat/events/stream/v2alpha1?id=trn_1", null, "", DOM_STREAM_FLAG_SSE), 1);

    await waitFor(() =>
      events.some((e) => e.slot === 2 && e.phase === DOM_STREAM_CHUNK && e.text.includes("data: four"))
    );
    // Simulate Wasm closing the stream once the terminal event arrives (stops reconnect loop cleanly).
    pool.close(2);

    assert.deepEqual(seenHeaders, ["", "2"]);
    assert.equal(events.some((e) => e.slot === 2 && e.phase === DOM_STREAM_ERROR && e.text.startsWith("reconnecting ")), true);
    const allChunks = events.filter((e) => e.slot === 2 && e.phase === DOM_STREAM_CHUNK).map((e) => e.text).join("");
    assert.ok(allChunks.includes("data: one") && allChunks.includes("data: two"));
    assert.ok(allChunks.includes("data: three") && allChunks.includes("data: four"));
  } finally {
    pool.stopAll();
    await srv.close();
  }
});

test("non-retryable 404 SSE response terminates without reconnecting", async () => {
  let calls = 0;
  const srv = await startServer((_req, res) => {
    calls += 1;
    res.writeHead(404, { "Content-Type": "application/json" });
    res.end('{"error":"not found"}');
  });

  const events = [];
  const decoder = new TextDecoder();
  const pool = createStreamPool({
    fetchImpl: (path, init) => fetch(srv.baseUrl + path, init),
    baseBackoffMs: 20,
    onEvent(kind, status, bytes) {
      const rel = kind - DOM_STREAM_BASE;
      events.push({ slot: Math.floor(rel / 4), phase: rel % 4, status, text: bytes ? decoder.decode(bytes) : "" });
    },
  });

  try {
    pool.open(3, "/api/chat/events/stream/v2alpha1?id=trn_missing", null, "", DOM_STREAM_FLAG_SSE);
    await waitFor(() => events.some((e) => e.slot === 3 && e.phase === DOM_STREAM_ERROR && e.status === 404));
    await new Promise((r) => setTimeout(r, 80));
    assert.equal(calls, 1);
    const errEv = events.find((e) => e.slot === 3 && e.phase === DOM_STREAM_ERROR);
    assert.equal(errEv.text, '{"error":"not found"}');
  } finally {
    pool.stopAll();
    await srv.close();
  }
});

test("controlled input preserves focused caret/selection, guards IME composition, and manages modal dialog stack", () => {
  const input = {
    tagName: "TEXTAREA",
    type: "textarea",
    value: "hello world",
    selectionStart: 5,
    selectionEnd: 5,
    selectionDirection: "forward",
    selectionCalls: [],
    setSelectionRange(s, e, d) {
      this.selectionStart = s;
      this.selectionEnd = e;
      this.selectionDirection = d;
      this.selectionCalls.push([s, e, d]);
    },
  };
  const fakeDoc = { activeElement: input };
  let fits = 0;
  const ctrl = createInputController({ fit: () => { fits += 1; }, doc: fakeDoc });

  // Identical value write is a no-op (never resets selection).
  assert.equal(ctrl.setValue(input, "hello world"), false);
  assert.equal(input.selectionCalls.length, 0);

  // Updating controlled value on focused input preserves caret position.
  assert.equal(ctrl.setValue(input, "hello world!"), true);
  assert.equal(input.value, "hello world!");
  assert.deepEqual(input.selectionCalls.at(-1), [5, 5, "forward"]);
  assert.equal(fits, 1);

  // Shrinking value clamps caret cleanly.
  input.selectionStart = 10; input.selectionEnd = 12;
  assert.equal(ctrl.setValue(input, "hi"), true);
  assert.deepEqual(input.selectionCalls.at(-1), [2, 2, "forward"]);

  // IME composition blocks controlled overwrites until compositionend commits final text.
  ctrl.onCompositionStart(input);
  assert.equal(ctrl.isComposing(input, { isComposing: false }), true);
  input.value = "nihon";
  assert.equal(ctrl.setValue(input, "CLOBBERED"), false);
  assert.equal(input.value, "nihon");
  let committed = null;
  input.value = "日本";
  ctrl.onCompositionEnd(input, (el) => { committed = el.value; });
  assert.equal(ctrl.isComposing(input, { isComposing: false }), false);
  assert.equal(committed, "日本");

  // Modal <dialog> stack open/close/remove lifecycle.
  const makeDialog = (name) => ({
    name,
    tagName: "DIALOG",
    isConnected: true,
    open: false,
    shown: 0,
    closed: 0,
    showModal() { this.open = true; this.shown += 1; },
    close() { this.open = false; this.closed += 1; },
  });
  const d1 = makeDialog("settings");
  const d2 = makeDialog("confirm");
  ctrl.syncDialog(d1, true);
  ctrl.syncDialog(d2, true);
  assert.equal(d1.open, true);
  assert.equal(d2.open, true);
  assert.equal(ctrl.topModal(), d2);
  ctrl.onDialogRemoved(d2);
  assert.equal(d2.open, false);
  assert.equal(d2.closed, 1);
  assert.equal(ctrl.topModal(), d1);
  ctrl.syncDialog(d1, false);
  assert.equal(d1.open, false);
  assert.equal(ctrl.topModal(), null);
});
