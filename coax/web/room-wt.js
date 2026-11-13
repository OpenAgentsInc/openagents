// Firm reason: 1 (the browser's own WebTransport and WebSocket are what a page's room runs on)
// The page host's WebTransport (#13176): the transport under metaverse-engine/net/transport.coax for a room
// URL the app opens as "wt+ws://HOST/PATH" or "wt+wss://HOST/PATH". It returns a WebSocket-shaped
// object (readyState, onopen, onmessage, onerror, onclose, send, close), as room-worker.js does, so
// host.js's sockets carry it like any other.
//   - It opens a WebTransport session to https://HOST/PATH (the room's UDP port has the TCP port's
//     number). It falls back to the WebSocket at ws(s)://HOST/PATH where WebTransport is missing
//     (Safari before 18, older browsers), fails, or isn't ready within WT_OPEN_MS; after one that
//     wasn't ready in time (UDP blocked), this page goes straight to the WebSocket. The two are never open at once: each would take the
//     same seat.
//   - __wt is true once the session is ready, and host.js says "wt" in the socket's opened event.
//   - What the app sends begins with a channel mark (NET_MARK + channel, metaverse-engine/net/transport.coax):
//     channel 0 goes as one datagram, the others on one reliable stream, each frame its length (2
//     bytes, big-endian) and its bytes. On the WebSocket fallback the mark is taken off.
//   - What the room sends: each datagram is one frame; each stream the room opens carries frames as
//     above. Every frame reaches the app as a text message.
//   - ?wthash=HEX (a local room's self-signed certificate, coax/web/room/wt-cert.sh) is passed as
//     serverCertificateHashes; a room with a CA certificate needs none.
// globalThis.coaxial.wt counts what went each way; a check's globalThis.__netTap(way, data) sees each frame
// (coax/web/everglade2-latency.js).
const WT_OPEN_MS = 2500, WT_MARK = 1, WT_CHANNELS = 5;
const enc = new TextEncoder(), dec = new TextDecoder();
let wtFailed = false;
const counts = () => (globalThis.coaxial ||= {}).wt ||= { sessions: 0, fallbacks: 0, dgIn: 0, dgOut: 0, bytesIn: 0, bytesOut: 0, framesIn: 0, framesOut: 0 };
function hashes() {
  const h = new URLSearchParams(location.search).get('wthash');
  return h && /^[0-9a-f]{64}$/i.test(h) ? [{ algorithm: 'sha-256', value: Uint8Array.from(h.match(/../g).map((x) => parseInt(x, 16))) }] : null;
}
export function wtSocket(url) {
  const wsUrl = url.slice(3), c = counts();
  const s = { readyState: 0, __wt: false, onopen: null, onmessage: null, onerror: null, onclose: null };
  let ws = null, wt = null, dg = null, out = null, closed = false;
  const done = () => { if (s.readyState === 3) return; s.readyState = 3; s.onclose?.({}); };
  const message = (bytes) => { c.framesIn++; c.bytesIn += bytes.length; const data = dec.decode(bytes); globalThis.__netTap?.('in', data); s.onmessage?.({ data }); };
  const useWs = () => {
    if (closed) return;
    c.fallbacks++;
    ws = new WebSocket(wsUrl);
    ws.onopen = () => { s.readyState = 1; s.onopen?.({}); };
    ws.onmessage = (m) => { if (typeof m.data === 'string') { c.framesIn++; c.bytesIn += m.data.length; } s.onmessage?.(m); };
    ws.onerror = () => s.onerror?.({});
    ws.onclose = () => done();
  };
  s.send = (text) => {
    const mark = text.length ? text.charCodeAt(0) - WT_MARK : -1, marked = mark >= 0 && mark < WT_CHANNELS;
    if (ws) { if (ws.readyState === 1) { const t = marked ? text.slice(1) : text; c.framesOut++; c.bytesOut += t.length; ws.send(t); } return; }
    if (!wt || s.readyState !== 1) return;
    const body = enc.encode(marked ? text.slice(1) : text);
    c.framesOut++; c.bytesOut += body.length; globalThis.__netTap?.('out', body.length);
    if (marked && mark === 0) { c.dgOut++; dg.write(body).catch(() => {}); return; }
    if (body.length > 65535) return;
    const b = new Uint8Array(body.length + 2); b[0] = body.length >> 8; b[1] = body.length & 255; b.set(body, 2);
    out.write(b).catch(() => {});
  };
  s.close = () => {
    if (closed) return;
    closed = true;
    if (ws) { ws.close(); return; }
    try { wt?.close({ closeCode: 0, reason: '' }); } catch {}
    setTimeout(done);
  };
  if (wtFailed || typeof WebTransport !== 'function' || !/^wss?:\/\//.test(wsUrl)) { useWs(); return s; }
  // (a local room's localhost is its IPv4 loopback: the room's UDP socket is IPv4, coax/web/room/wt.c)
  const https = 'https://' + wsUrl.replace(/^wss?:\/\//, '').replace(/^localhost(?=[:/])/, '127.0.0.1');
  (async () => {
    let timer = 0, late = false;
    try {
      const h = hashes();
      wt = new WebTransport(https, h ? { serverCertificateHashes: h } : {});
      await Promise.race([wt.ready, new Promise((_, no) => { timer = setTimeout(() => { late = true; no(new Error('timeout')); }, WT_OPEN_MS); })]);
      clearTimeout(timer);
      dg = wt.datagrams.writable.getWriter();
      out = (await wt.createUnidirectionalStream()).getWriter();
      if (closed) { wt.close(); return; }
      c.sessions++;
      s.__wt = true; s.readyState = 1; s.onopen?.({});
      (async () => {
        const r = wt.datagrams.readable.getReader();
        for (;;) { const { value, done: end } = await r.read(); if (end) break; c.dgIn++; message(value); }
      })().catch(() => {});
      (async () => {
        const streams = wt.incomingUnidirectionalStreams.getReader();
        for (;;) {
          const { value, done: end } = await streams.read();
          if (end) break;
          (async () => {
            const r = value.getReader(); let buf = new Uint8Array(0);
            for (;;) {
              const { value: v, done: e } = await r.read();
              if (e) break;
              const b = new Uint8Array(buf.length + v.length); b.set(buf); b.set(v, buf.length); buf = b;
              let at = 0;
              while (buf.length - at >= 2) {
                const n = (buf[at] << 8) | buf[at + 1];
                if (buf.length - at - 2 < n) break;
                message(buf.subarray(at + 2, at + 2 + n)); at += 2 + n;
              }
              buf = buf.slice(at);
            }
          })().catch(() => {});
        }
      })().catch(() => {});
      wt.closed.then(done, done);
    } catch (e) {
      clearTimeout(timer); c.error = String(e && e.message || e);
      try { wt?.close(); } catch {}
      wt = null; if (late || typeof WebTransport !== 'function') wtFailed = true;   // a refused session tries again next time
      useWs();
    }
  })();
  return s;
}
