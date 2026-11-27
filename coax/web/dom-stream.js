// Firm reason: 1 (browser glue: multiplexed keyed SSE streams, Last-Event-ID reconnect, controlled input/IME & modal dialog helpers for dom-host.js)
// Browser-side platform glue for Coax Web Host (#14089).
// Policy bounds mirror coax/web/stream-client-policy.coax (#authority):
//   - 16 concurrent keyed stream slots (0..15), event band 1024 + 4*slot + phase
//   - Same-origin relative path enforcement, GET SSE Last-Event-ID tracking,
//     exponential backoff reconnect (500ms..30000ms, max 8 retries)
//   - Controlled input caret/selection preservation, IME composition guard,
//     and modal <dialog> stack management.

export const DOM_STREAM_SLOTS = 16;
export const DOM_STREAM_BASE = 1024;
export const DOM_STREAM_OPEN = 0;
export const DOM_STREAM_CHUNK = 1;
export const DOM_STREAM_END = 2;
export const DOM_STREAM_ERROR = 3;
export const DOM_DIALOG_CANCEL = 17;

export const DOM_STREAM_FLAG_SSE = 1;
export const DOM_STREAM_FLAG_NO_RECONNECT = 2;

const BASE_BACKOFF_MS = 500;
const MAX_BACKOFF_MS = 30000;
const MAX_RETRIES = 8;
const URL_MAX = 2048;

export function urlAllowed(url) {
  if (typeof url !== "string" || url.length < 1 || url.length > URL_MAX) return false;
  if (!url.startsWith("/") || url.startsWith("//") || url.startsWith("/\\")) return false;
  return !/[\r\n\0]/.test(url);
}

export function statusRetryable(status) {
  return status === 0 || status === 408 || status === 429 || (status >= 500 && status <= 599);
}

export function shouldReconnect(isGet, flags, status, eof, attempts) {
  if (!isGet) return false;
  if ((flags & DOM_STREAM_FLAG_SSE) === 0 || (flags & DOM_STREAM_FLAG_NO_RECONNECT) !== 0) return false;
  if (attempts < 0 || attempts >= MAX_RETRIES) return false;
  if (eof) return status === 200;
  return statusRetryable(status);
}

export function backoffMs(attempt, retryHintMs = 0) {
  const steps = Math.max(0, Math.min(6, attempt | 0));
  let delay = Math.min(MAX_BACKOFF_MS, BASE_BACKOFF_MS * (1 << steps));
  if (retryHintMs > delay) delay = Math.min(MAX_BACKOFF_MS, retryHintMs);
  return delay;
}

export function clampCaret(pos, nextLen) {
  if (nextLen <= 0 || pos <= 0) return 0;
  return pos > nextLen ? nextLen : pos;
}

// Parse SSE id:/retry: metadata lines across arbitrary UTF-8 chunk boundaries
// without modifying the raw bytes delivered to Wasm.
function scanSseMeta(entry, chunkText) {
  const combined = entry.sseTail + chunkText;
  const lines = combined.split("\n");
  entry.sseTail = lines.pop() ?? "";
  for (let raw of lines) {
    if (raw.endsWith("\r")) raw = raw.slice(0, -1);
    if (raw.startsWith("id:")) {
      const val = raw.slice(3).startsWith(" ") ? raw.slice(4) : raw.slice(3);
      if (!val.includes("\0")) entry.lastEventId = val;
    } else if (raw.startsWith("retry:")) {
      const val = (raw.slice(6).startsWith(" ") ? raw.slice(7) : raw.slice(6)).trim();
      if (/^\d+$/.test(val)) {
        const ms = Number(val);
        if (Number.isFinite(ms) && ms >= 0) entry.retryHintMs = Math.min(MAX_BACKOFF_MS, ms);
      }
    }
  }
}

export function createStreamPool({
  fetchImpl = globalThis.fetch?.bind(globalThis),
  setTimer = globalThis.setTimeout?.bind(globalThis),
  clearTimer = globalThis.clearTimeout?.bind(globalThis),
  headersForUrl = () => ({}),
  inboxCap = () => 65536,
  onEvent,
  onFlush,
  isAlive = () => true,
  baseBackoffMs = BASE_BACKOFF_MS,
}) {
  const slots = new Array(DOM_STREAM_SLOTS).fill(null);
  const decoder = new TextDecoder();
  const encoder = new TextEncoder();

  function closeSlot(slot) {
    if (!Number.isInteger(slot) || slot < 0 || slot >= DOM_STREAM_SLOTS) return;
    const entry = slots[slot];
    if (!entry) return;
    entry.closed = true;
    slots[slot] = null;
    if (entry.timer !== null) { clearTimer(entry.timer); entry.timer = null; }
    if (entry.controller) { try { entry.controller.abort(); } catch {} entry.controller = null; }
  }

  function stopAll() {
    for (let i = 0; i < DOM_STREAM_SLOTS; i++) closeSlot(i);
  }

  function emit(slot, phase, status, payloadBytes) {
    if (!isAlive()) return;
    onEvent(DOM_STREAM_BASE + slot * 4 + phase, status, payloadBytes || null);
  }

  function computeDelay(attempt, retryHintMs) {
    if (baseBackoffMs === BASE_BACKOFF_MS) return backoffMs(attempt, retryHintMs);
    const steps = Math.max(0, Math.min(6, attempt | 0));
    let delay = Math.min(MAX_BACKOFF_MS, baseBackoffMs * (1 << steps));
    if (retryHintMs > delay) delay = Math.min(MAX_BACKOFF_MS, retryHintMs);
    return delay;
  }

  function connect(entry) {
    const live = () => !entry.closed && slots[entry.slot] === entry && isAlive();
    if (!live()) return;
    if (entry.timer !== null) { clearTimer(entry.timer); entry.timer = null; }
    const controller = new AbortController();
    entry.controller = controller;
    entry.sseTail = "";
    const isSse = (entry.flags & DOM_STREAM_FLAG_SSE) !== 0;

    (async () => {
      let status = 0;
      let sawBody = false;
      let cleanEof = false;
      let errBody = null;
      try {
        const hdrs = { ...headersForUrl(entry.url) };
        if (entry.isGet) {
          if (isSse) {
            hdrs["Accept"] = "text/event-stream";
            hdrs["Cache-Control"] = "no-cache";
          }
          if (entry.lastEventId) hdrs["Last-Event-ID"] = entry.lastEventId;
        } else {
          hdrs["Content-Type"] ||= "application/json";
        }
        const init = entry.isGet
          ? { method: "GET", credentials: "same-origin", cache: "no-store", headers: hdrs, signal: controller.signal }
          : { method: "POST", body: entry.body, credentials: "same-origin", headers: hdrs, signal: controller.signal };
        const res = await fetchImpl(entry.url, init);
        if (!live()) return;
        status = res.status;
        if (res.ok) {
          emit(entry.slot, DOM_STREAM_OPEN, status, null);
          onFlush?.();
          if (res.body && typeof res.body.getReader === "function") {
            const reader = res.body.getReader();
            const cap = Math.max(1, inboxCap());
            for (;;) {
              const { done, value } = await reader.read();
              if (!live()) return;
              if (done) { cleanEof = true; break; }
              if (value && value.length > 0) {
                sawBody = true;
                entry.attempts = 0;
                if (isSse) scanSseMeta(entry, decoder.decode(value, { stream: true }));
                for (let at = 0; at < value.length && live(); at += cap) {
                  emit(entry.slot, DOM_STREAM_CHUNK, status, value.subarray(at, at + cap));
                }
                try { onFlush?.(); } catch (err) { console.error(err); }
              }
            }
          } else {
            const buf = new Uint8Array(await res.arrayBuffer());
            if (!live()) return;
            if (buf.length > 0) {
              sawBody = true;
              entry.attempts = 0;
              if (isSse) scanSseMeta(entry, decoder.decode(buf));
              const cap = Math.max(1, inboxCap());
              for (let at = 0; at < buf.length && live(); at += cap) {
                emit(entry.slot, DOM_STREAM_CHUNK, status, buf.subarray(at, at + cap));
              }
              try { onFlush?.(); } catch (err) { console.error(err); }
            }
            cleanEof = true;
          }
        } else {
          try {
            const cap = Math.max(1, Math.min(4096, inboxCap()));
            const raw = new Uint8Array(await res.arrayBuffer());
            if (raw.length > 0) errBody = raw.length > cap ? raw.subarray(0, cap) : raw;
          } catch {}
        }
      } catch {
        if (!live()) return;
        status = 0;
      }

      if (!live()) return;
      entry.controller = null;

      if (shouldReconnect(entry.isGet, entry.flags, status, cleanEof, entry.attempts)) {
        const delay = computeDelay(entry.attempts, entry.retryHintMs);
        entry.attempts += 1;
        emit(entry.slot, DOM_STREAM_ERROR, status, encoder.encode(`reconnecting ${delay}`));
        onFlush?.();
        entry.timer = setTimer(() => {
          entry.timer = null;
          if (live()) connect(entry);
        }, delay);
        return;
      }

      if (cleanEof && status >= 200 && status < 300) {
        slots[entry.slot] = null;
        entry.closed = true;
        emit(entry.slot, DOM_STREAM_END, status, null);
        onFlush?.();
        return;
      }

      slots[entry.slot] = null;
      entry.closed = true;
      const errBytes = errBody || encoder.encode(sawBody ? "interrupted" : "closed");
      emit(entry.slot, DOM_STREAM_ERROR, status, errBytes);
      onFlush?.();
    })();
  }

  function open(slot, url, bodyBytes = null, lastEventId = "", flags = 0) {
    if (!Number.isInteger(slot) || slot < 0 || slot >= DOM_STREAM_SLOTS) return 0;
    if (!urlAllowed(url)) return 0;
    closeSlot(slot);
    const body = bodyBytes && bodyBytes.length > 0 ? new Uint8Array(bodyBytes) : null;
    const entry = {
      slot,
      url,
      body,
      isGet: !body,
      flags: flags | 0,
      lastEventId: lastEventId || "",
      retryHintMs: 0,
      attempts: 0,
      sseTail: "",
      controller: null,
      timer: null,
      closed: false,
    };
    slots[slot] = entry;
    connect(entry);
    return 1;
  }

  function onVisible() {
    for (let i = 0; i < DOM_STREAM_SLOTS; i++) {
      const entry = slots[i];
      if (entry && !entry.closed && entry.timer !== null && !entry.controller) {
        clearTimer(entry.timer);
        entry.timer = null;
        connect(entry);
      }
    }
  }

  return { open, close: closeSlot, stopAll, onVisible, lastEventIdOf: (s) => slots[s]?.lastEventId ?? "" };
}

export function createInputController({ fit = () => {}, doc = globalThis.document } = {}) {
  const composing = new WeakSet();
  const pendingValues = new WeakMap();
  const modalStack = [];

  function setValue(el, nextValue) {
    if (!el || el.type === "file") return false;
    const text = String(nextValue ?? "");
    if (composing.has(el)) {
      pendingValues.set(el, text);
      return false;
    }
    if (el.value === text) return false;
    const active = doc && doc.activeElement === el;
    let start = null, end = null, dir = "none";
    if (active && typeof el.selectionStart === "number" && typeof el.selectionEnd === "number") {
      start = el.selectionStart;
      end = el.selectionEnd;
      dir = el.selectionDirection || "none";
    }
    el.value = text;
    if (start !== null && typeof el.setSelectionRange === "function") {
      try {
        el.setSelectionRange(clampCaret(start, text.length), clampCaret(end, text.length), dir);
      } catch { /* input type (e.g. range/checkbox) does not support selection */ }
    }
    fit(el);
    return true;
  }

  function onCompositionStart(el) {
    if (el) composing.add(el);
  }

  function onCompositionEnd(el, emitInput) {
    if (!el) return;
    composing.delete(el);
    pendingValues.delete(el);
    fit(el);
    emitInput?.(el);
  }

  function isComposing(el, event) {
    return Boolean((event && event.isComposing) || (el && composing.has(el)));
  }

  function syncDialog(el, wantModal) {
    if (!el || String(el.tagName || "").toUpperCase() !== "DIALOG") return;
    if (wantModal) {
      if (!el.isConnected) return;
      if (!modalStack.includes(el)) modalStack.push(el);
      if (!el.open) {
        if (typeof el.showModal === "function") {
          try { el.showModal(); } catch { el.setAttribute?.("open", ""); }
        } else {
          el.setAttribute?.("open", "");
        }
      }
    } else {
      const idx = modalStack.indexOf(el);
      if (idx >= 0) modalStack.splice(idx, 1);
      if (el.open) {
        if (typeof el.close === "function") { try { el.close(); } catch {} }
        else el.removeAttribute?.("open");
      }
    }
  }

  function onDialogRemoved(el) {
    if (!el) return;
    syncDialog(el, false);
  }

  function topModal() {
    while (modalStack.length > 0 && (!modalStack[modalStack.length - 1].isConnected || !modalStack[modalStack.length - 1].open)) {
      modalStack.pop();
    }
    return modalStack.length > 0 ? modalStack[modalStack.length - 1] : null;
  }

  return { setValue, onCompositionStart, onCompositionEnd, isComposing, syncDialog, onDialogRemoved, topModal };
}
