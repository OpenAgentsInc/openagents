// Firm reason: 1 (it runs in the page: the browser's own WebTransport against a real room)
// The in-page half of coax/web/room/wt-room-check.coax (#13176): with window.__wtArgs = {url, hash,
// seconds}, it opens a WebTransport session to a room's ticketed relay URL, says its hello on its
// reliable stream (each frame a 2-byte length and its bytes, as coax/web/room-wt.js), sends an input
// datagram every 1/30 s that repeats those the room hasn't acknowledged, and counts what comes back:
// state datagrams (D) and the ticks between them, the frames on the room's streams, and the
// round trip of its inputs (an input's sequence to the first D that acknowledges it).
(async () => {
  const A = window.__wtArgs, r = { ok: false, transport: typeof WebTransport };
  const enc = new TextEncoder(), dec = new TextDecoder();
  const pct = (a, p) => { if (!a.length) return -1; const s = a.slice().sort((x, y) => x - y); return Math.round(s[Math.min(s.length - 1, Math.floor((s.length * p) / 100))]); };
  try {
    const opts = {};
    if (A.hash) opts.serverCertificateHashes = [{ algorithm: 'sha-256', value: Uint8Array.from(A.hash.match(/../g).map((h) => parseInt(h, 16))) }];
    const t0 = performance.now();
    const wt = new WebTransport(A.url, opts);
    await wt.ready;
    r.ready_ms = Math.round(performance.now() - t0);
    const out = (await wt.createUnidirectionalStream()).getWriter();
    const frame = (t) => { const b = enc.encode(t), x = new Uint8Array(b.length + 2); x[0] = b.length >> 8; x[1] = b.length & 255; x.set(b, 2); return out.write(x); };
    await frame('H 6 ranger.0.0.0 srd-ranger 64 Net check');
    let dgrams = 0, ticks = 0, missed = 0, last = -1, acked = 0, streamed = 0, bytes = 0;
    const sentAt = new Map(), rtt = [];
    const dr = wt.datagrams.readable.getReader();
    (async () => {
      for (;;) {
        const { value, done } = await dr.read(); if (done) break;
        dgrams += 1; bytes += value.length;
        const t = dec.decode(value); if (!t.startsWith('D ')) continue;
        const h = t.split(' '), tick = +h[1], ack = +h[2];
        if (last >= 0 && tick > last + 1) missed += tick - last - 1;
        if (tick > last) { last = tick; ticks += 1; }
        if (ack > acked) { for (let s = acked + 1; s <= ack; s++) if (sentAt.has(s)) { rtt.push(performance.now() - sentAt.get(s)); sentAt.delete(s); } acked = ack; }
      }
    })().catch(() => {});
    (async () => {
      const ss = wt.incomingUnidirectionalStreams.getReader();
      for (;;) { const { value, done } = await ss.read(); if (done) break; (async () => { const rd = value.getReader(); for (;;) { const { value: v, done: e } = await rd.read(); if (e) break; streamed += v.length; bytes += v.length; } })().catch(() => {}); }
    })().catch(() => {});
    const dw = wt.datagrams.writable.getWriter(), owed = [];
    const t1 = performance.now();
    for (let seq = 1; performance.now() - t1 < A.seconds * 1000; seq++) {
      owed.push(`I ${seq} 0 0 0 0 0 0 ${last < 0 ? 0 : last}`); sentAt.set(seq, performance.now());
      while (owed.length && +owed[0].split(' ')[1] <= acked) owed.shift();
      while (owed.join('\n').length > 1200) owed.shift();
      dw.write(enc.encode(owed.join('\n'))).catch(() => {});
      await new Promise((f) => setTimeout(f, 33));
    }
    const s = (performance.now() - t1) / 1000;
    r.seconds = Math.round(s); r.state_datagrams = dgrams; r.state_per_second = Math.round(ticks / s); r.ticks_missed = missed;
    r.loss_permille = ticks + missed > 0 ? Math.round(missed * 1000 / (ticks + missed)) : -1;
    r.stream_bytes = streamed; r.bytes_per_second = Math.round(bytes / s);
    r.input_rtt_p50 = pct(rtt, 50); r.input_rtt_p95 = pct(rtt, 95);
    wt.close({ closeCode: 0, reason: 'done' });
    await wt.closed.catch(() => {});
    r.ok = ticks / s >= 25 && streamed > 0;
  } catch (e) { r.error = String(e && e.message || e); }
  return JSON.stringify(r);
})()
