// Firm reason: 1 (it runs in the page: the browser's own WebTransport is what the check tests)
// The in-page half of coax/web/room/wt-check.coax (#13175): with window.__wtArgs = {url, hash,
// count, bytes}, it opens a WebTransport session with serverCertificateHashes, sends `count`
// numbered datagrams paced at about one a millisecond and counts the echoes, round-trips `bytes` on
// a bidirectional stream, and closes the session. It resolves to one JSON line.
(async () => {
  const A = window.__wtArgs, r = { ok: false, transport: typeof WebTransport };
  const pct = (a, p) => { if (!a.length) return -1; const s = a.slice().sort((x, y) => x - y); return Math.round(s[Math.min(s.length - 1, Math.floor((s.length * p) / 100))] * 10) / 10; };
  try {
    const opts = {};
    if (A.hash) opts.serverCertificateHashes = [{ algorithm: 'sha-256', value: Uint8Array.from(A.hash.match(/../g).map((h) => parseInt(h, 16))) }];
    const t0 = performance.now();
    const wt = new WebTransport(A.url, opts);
    await wt.ready;
    r.ready_ms = Math.round(performance.now() - t0);
    r.max_datagram = wt.datagrams.maxDatagramSize;
    const w = wt.datagrams.writable.getWriter(), rd = wt.datagrams.readable.getReader();
    const sent = new Array(A.count), seen = new Set(), rtt = [];
    let big = false;
    const reading = (async () => {
      for (;;) {
        const { value, done } = await rd.read();
        if (done) break;
        const k = new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(0);
        if (k === 0xffffffff) { big = value.byteLength; continue; }
        if (k < A.count && !seen.has(k)) { seen.add(k); rtt.push(performance.now() - sent[k]); }
      }
    })();
    for (let k = 0; k < A.count; k++) {
      const b = new Uint8Array(100); new DataView(b.buffer).setUint32(0, k);
      sent[k] = performance.now(); await w.write(b);
      if (k % 10 === 9) await new Promise((f) => setTimeout(f, 10));
    }
    // the largest payload a room datagram may be (wt.h WT_DATAGRAM_MAX less the 1-byte quarter id
    // and the 2 bytes headroom of an 8-byte one)
    const g = new Uint8Array(Math.min(1190, r.max_datagram)); new DataView(g.buffer).setUint32(0, 0xffffffff); await w.write(g);
    await new Promise((f) => setTimeout(f, 1500));
    r.datagrams_sent = A.count; r.datagrams_echoed = seen.size; r.big_echoed = big;
    r.datagram_rtt_p50 = pct(rtt, 50); r.datagram_rtt_p95 = pct(rtt, 95);
    const s = await wt.createBidirectionalStream(), sw = s.writable.getWriter();
    const msg = new Uint8Array(A.bytes); for (let i = 0; i < msg.length; i++) msg[i] = i * 7 & 255;
    const t1 = performance.now();
    await sw.write(msg); await sw.close();
    const sr = s.readable.getReader(); let total = 0, same = true;
    for (;;) { const { value, done } = await sr.read(); if (done) break; for (let i = 0; i < value.length; i++) if (value[i] !== ((total + i) * 7 & 255)) same = false; total += value.length; }
    r.stream_ms = Math.round(performance.now() - t1); r.stream_bytes = total; r.stream_same = same;
    wt.close({ closeCode: 0, reason: 'done' });
    await wt.closed; await reading.catch(() => {});
    r.closed = true;
    r.ok = r.datagrams_echoed >= A.count * 0.99 && total === A.bytes && same;
  } catch (e) { r.error = String(e && e.message || e); }
  return JSON.stringify(r);
})()
