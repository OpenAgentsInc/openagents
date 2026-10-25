// NIP-MV presence, interoperable with OpenAgents' verse-bare world.
// Render cadence is independent of networking. No chat/profile subscription.
'use strict';
const GridNet = (() => {
  const WORLD = 'verse-bare', MAX = 128;
  const keyOK = k => typeof k === 'string' && /^[0-9a-f]{64}$/.test(k);
  const finite = (v, n, limit) => Array.isArray(v) && v.length === n && v.every(x => Number.isFinite(x) && Math.abs(x) <= limit);
  const validPose = p => p && p.id === 'avatar' && p.role === 'avatar' && finite(p.p, 3, 264) && finite(p.q, 4, 1.001) && Math.hypot(...p.q) > .9 && Math.hypot(...p.q) < 1.1;
  const yaw = q => Math.atan2(2 * (q[3] * q[1] + q[0] * q[2]), 1 - 2 * (q[1] ** 2 + q[2] ** 2));
  const wrap = x => Math.atan2(Math.sin(x), Math.cos(x));
  const hex = bytes => Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
  const bytes = text => Uint8Array.from(text.match(/../g), s => parseInt(s, 16));
  const read = (k, fallback = null) => { try { return JSON.parse(localStorage.getItem(k)) ?? fallback; } catch (_) { return fallback; } };
  const write = (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch (_) {} };
  const seed = key => parseInt(key.slice(0, 8), 16) & 0x7fffffff;

  let triggers=[],colliders=[];
  function configureLayout(metadata) {
    triggers=metadata.landmarks.filter(l=>l.interaction).map(l=>[l.position[0],l.position[2],l.interaction.leave+2]);
    colliders=metadata.colliders;
  }
  function spawn(key, peers = [], saved = null) {
    const free = p => finite(p, 2, 260) && colliders.every(b=>p[0]<=b.min[0]-.5||p[0]>=b.max[0]+.5||p[1]<=b.min[2]-.5||p[1]>=b.max[2]+.5) &&
      triggers.every(t => Math.hypot(p[0] - t[0], p[1] - t[1]) > t[2]) &&
      peers.every(o => Math.hypot(p[0] - o.p[0], p[1] - o.p[2]) > .95);
    if (saved && free(saved)) return saved;
    let state = seed(key) || 1;
    const random = () => { state = Math.imul(state, 1664525) + 1013904223 | 0; return (state >>> 0) / 4294967296; };
    for (let i = 0; i < 256; i++) { const a = random() * Math.PI * 2, r = 5 + Math.sqrt(random()) * 18; const p = [Math.sin(a) * r, -10 + Math.cos(a) * r]; if (free(p)) return p; }
    return [0, -18];
  }

  class Session {
    constructor({ relay = 'wss://relay.openagents.com', offline = false, name, changed = () => {} } = {}) {
      this.relay = relay; this.offline = offline; this.changed = changed;
      let secret = read('openagents.grid.secret');
      // Earlier OpenAgents builds stored the key as a plain string.
      try { if (!secret) secret = localStorage.getItem('openagents.grid.secret'); } catch (_) {}
      if (!keyOK(secret)) secret = hex(NostrTools.generateSecretKey());
      this.secret = bytes(secret); this.key = NostrTools.getPublicKey(this.secret);
      write('openagents.grid.secret', secret);
      let keptName=read('openagents.grid.name');
      try { if(!keptName) keptName=localStorage.getItem('openagents.grid.name'); } catch (_) {}
      this.name = String(name || keptName || 'Visitor').trim().slice(0, 40);
      write('openagents.grid.name', this.name);
      const lists = read('openagents.grid.blocked', {});
      this.blocked = new Set((lists.blocked || []).filter(keyOK).slice(0, 1000));
      this.muted = new Set((lists.muted || []).filter(keyOK).slice(0, 1000));
      this.peers = new Map(); this.seen = new Set(); this.session = hex(crypto.getRandomValues(new Uint8Array(8)));
      this.sequence = 0; this.joined = false; this.closed = false; this.socket = null;
      this.status = offline ? 'Offline' : 'Watching'; this.lastFrame = 0; this.lastState = 0; this.retry = 0;
      this.lookSeed = seed(this.key); this.pose = null; this.resume = read('aethyr.grid.pose'); this.refusals = 0;
      this.backoffUntil = 0; this.lastByKey = new Map(); this.framesByKey = new Map();
    }
    connect() {
      if (this.offline || this.closed || this.socket) return;
      let url;
      try { url = new URL(this.relay); if (!['ws:', 'wss:'].includes(url.protocol)) throw Error(); }
      catch (_) { this.status = 'Invalid relay address'; this.changed(); return; }
      this.status = 'Connecting'; this.changed();
      const socket = this.socket = new WebSocket(url.href);
      socket.onopen = () => { this.retry = 0; this.status = this.joined ? 'Online' : 'Watching'; this.subscribe(); this.changed(); };
      socket.onmessage = e => { if (typeof e.data === 'string' && e.data.length < 65536) this.receive(e.data); };
      socket.onclose = () => {
        if (this.socket !== socket) return;
        this.socket = null;
        if (this.closed) return;
        this.status = 'Reconnecting'; this.changed();
        this.timer = setTimeout(() => this.connect(), Math.min(30000, 1000 * 2 ** Math.min(this.retry++, 5)));
      };
      socket.onerror = () => { this.status = 'Connection interrupted'; this.changed(); };
    }
    send(message) { if (this.socket?.readyState === 1) this.socket.send(JSON.stringify(message)); }
    subscribe() {
      this.send(['REQ', 'mv-live', { kinds: [23300], '#w': [WORLD], since: Math.floor(Date.now() / 1000) - 15 }]);
      this.send(['REQ', 'mv-state', { kinds: [33301], '#w': [WORLD], '#role': ['avatar'], limit: 500 }]);
      if (typeof GridBuilds !== 'undefined') this.send(['REQ', 'deploy-board', { kinds: [3205,30205], authors: [GridBuilds.ANNOUNCER], limit: 80 }]);
      this.lastState = 0;
    }
    sign(kind, tags, content) { return NostrTools.finalizeEvent({ kind, tags, content: JSON.stringify(content), created_at: Math.floor(Date.now() / 1000) }, this.secret); }
    publish(kind, tags, content) { if (!this.offline && this.socket?.readyState === 1 && Date.now() > this.backoffUntil) this.send(['EVENT', this.sign(kind, tags, content)]); }
    receive(raw) {
      let msg;
      try { msg = JSON.parse(raw); } catch (_) { return; }
      if (!Array.isArray(msg)) return;
      if (msg[0] === 'AUTH' && typeof msg[1] === 'string' && msg[1].length <= 4096) {
        const auth = NostrTools.finalizeEvent({ kind: 22242, created_at: Math.floor(Date.now() / 1000), tags: [['relay', this.relay], ['challenge', msg[1]]], content: '' }, this.secret);
        this.auth = auth.id; this.send(['AUTH', auth]); return;
      }
      if (msg[0] === 'OK') {
        if (msg[1] === this.auth && msg[2]) { this.subscribe(); this.status = this.joined ? 'Online' : 'Watching'; }
        if (!msg[2]) { this.refusals++; if (String(msg[3]).startsWith('rate-limited:')) this.backoffUntil = Date.now() + 10000; }
        this.changed(); return;
      }
      if (msg[0] === 'EVENT' && msg[1] === 'deploy-board' && typeof GridBuilds !== 'undefined') {
        const record = GridBuilds.read(msg[2], this.deployTest || false);
        if(record) this.onBuild?.(record);
        return;
      }
      if (msg[0] !== 'EVENT' || !['mv-live', 'mv-state'].includes(msg[1])) return;
      const e = msg[2], now = Date.now();
      if (!e || !keyOK(e.pubkey) || this.blocked.has(e.pubkey) || ![23300, 33301].includes(e.kind) ||
          !Array.isArray(e.tags) || e.tags.length>64 || !e.tags.every(t=>Array.isArray(t)&&t.length>=2&&t.every(v=>typeof v==='string')) || !e.tags.some(t => t[0] === 'w' && t[1] === WORLD) ||
          typeof e.content !== 'string' || e.content.length > 24000 || !Number.isInteger(e.created_at) ||
          e.created_at * 1000 > now + 10000 || e.created_at * 1000 < now - 120000 || this.seen.has(e.id)) return;
      // Bound signature verification work before doing elliptic curve math.
      const budget = this.framesByKey.get(e.pubkey) || { at: now, n: 0 };
      if (now - budget.at > 1000) { budget.at = now; budget.n = 0; }
      if (++budget.n > 16) return;
      if (!this.framesByKey.has(e.pubkey) && this.framesByKey.size >= 1024) this.framesByKey.delete(this.framesByKey.keys().next().value);
      this.framesByKey.set(e.pubkey, budget);
      try { if (!NostrTools.verifyEvent(e)) return; } catch (_) { return; }
      let content;
      try { content = JSON.parse(e.content); } catch (_) { return; }
      if (content.v !== 1 || !Number.isFinite(content.t) || Math.abs(content.t - now) > 120000) return;
      let pose;
      if (e.kind === 23300) {
        if (!Array.isArray(content.e) || content.e.length > 16 || typeof content.s !== 'string' || content.s.length > 128 || !Number.isSafeInteger(content.n) || content.n < 0) return;
        pose = content.e.find(validPose);
      } else {
        if (!e.tags.some(t => t[0] === 'd' && t[1] === WORLD + '/avatar') || !validPose(content) || typeof content.online !== 'boolean') return;
        pose = content;
      }
      if (!pose) return;
      this.seen.add(e.id); if (this.seen.size > 4096) this.seen.delete(this.seen.values().next().value);
      if (e.pubkey === this.key) {
        if (!this.joined && e.kind === 33301) this.resume = { p: pose.p, yaw: yaw(pose.q) };
        return;
      }
      const previous = this.lastByKey.get(e.pubkey);
      if (previous && (content.t < previous.t || (e.kind === 23300 && content.s === previous.s && content.n <= previous.n))) return;
      if (!this.peers.has(e.pubkey) && this.peers.size >= MAX) return;
      this.lastByKey.set(e.pubkey, { t: content.t, s: content.s, n: content.n });
      if(this.lastByKey.size>1024)this.lastByKey.delete(this.lastByKey.keys().next().value);
      if (e.kind === 33301 && !content.online) { this.peers.delete(e.pubkey); this.changed(); return; }
      let peer = this.peers.get(e.pubkey);
      if (!peer) peer = { key: e.pubkey, name: e.pubkey.slice(0, 8), seed: seed(e.pubkey), samples: [] };
      if (e.kind === 33301 && typeof content.name === 'string' && content.name.trim()) peer.name = content.name.slice(0, 40);
      if (e.kind === 33301 && Number.isInteger(content.aethyr?.seed) && content.aethyr.seed>=0 && content.aethyr.seed<=0x7fffffff) peer.seed=content.aethyr.seed;
      peer.seen = now;
      const previousSample=peer.samples.at(-1);
      const measured=previousSample&&now>previousSample.at?Math.min(12,Math.hypot(pose.p[0]-previousSample.p[0],pose.p[2]-previousSample.p[2])*1000/(now-previousSample.at)):0;
      peer.samples.push({ at: now, p: pose.p, yaw: yaw(pose.q), speed: finite(pose.v, 3, 100) ? Math.hypot(pose.v[0], pose.v[2]) : measured });
      peer.samples = peer.samples.slice(-24); this.peers.set(e.pubkey, peer);
      this.changed();
    }
    join() {
      if (this.joined) return null;
      const p = spawn(this.key, [...this.peers.values()].map(o => o.samples.at(-1)), this.resume?.p && [this.resume.p[0], this.resume.p[2]]);
      this.joined = true; this.status = this.offline ? 'Offline' : this.socket?.readyState === 1 ? 'Online' : 'Connecting'; this.lastState = 0; this.changed();
      return { x: p[0], z: p[1], yaw: Number.isFinite(this.resume?.yaw) ? this.resume.yaw : 0 };
    }
    state(online) {
      if (!this.pose) return;
      const { p, q } = this.pose;
      this.publish(33301, [['d', WORLD + '/avatar'], ['w', WORLD], ['role', 'avatar'], ['c', Math.floor(p[0] / 64) + ',' + Math.floor(p[2] / 64)]],
        { v: 1, id: 'avatar', role: 'avatar', p, q, t: Date.now(), online, name: this.name, aethyr: {seed:this.lookSeed} });
    }
    tick(p, face, speed, velocity) {
      const now = Date.now();
      for (const [key, peer] of this.peers) if (now - peer.seen > 20000) { this.peers.delete(key); this.lastByKey.delete(key); }
      if (!this.joined) return;
      const q = [0, Math.sin(face / 2), 0, Math.cos(face / 2)];
      this.pose = { p, q };
      if (now - this.lastFrame > (speed > .05 ? 100 : 2000)) {
        const entity = { id: 'avatar', role: 'avatar', p, q, v: finite(velocity,3,100)?velocity:[Math.sin(face) * speed, 0, Math.cos(face) * speed] };
        this.publish(23300, [['w', WORLD], ['c', Math.floor(p[0] / 64) + ',' + Math.floor(p[2] / 64)]], { v: 1, s: this.session, n: ++this.sequence, t: now, e: [entity] });
        this.lastFrame = now;
      }
      if (now - this.lastState > 15000) { this.state(true); this.lastState = now; write('aethyr.grid.pose', { p, yaw: face }); }
    }
    figures(now = Date.now()) {
      const at = now - 300;
      return [...this.peers.values()].filter(o => !this.blocked.has(o.key)).map(peer => {
        const samples = peer.samples;
        let a = samples[0], b = a;
        for (const sample of samples) { if (sample.at <= at) a = sample; if (sample.at >= at) { b = sample; break; } b = sample; }
        const blend = b.at > a.at ? Math.max(0, Math.min(1, (at - a.at) / (b.at - a.at))) : 1;
        return { ...peer, p: a.p.map((v, i) => v + (b.p[i] - v) * blend), yaw: a.yaw + wrap(b.yaw - a.yaw) * blend, speed: b.speed };
      });
    }
    moderate(key, kind, on) {
      if (!keyOK(key) || key === this.key || !['blocked', 'muted'].includes(kind)) return;
      const set = this[kind]; if (on && set.size < 1000) set.add(key); else if (!on) set.delete(key);
      write('openagents.grid.blocked', { blocked: [...this.blocked], muted: [...this.muted] });
      if (kind === 'blocked' && on) this.peers.delete(key);
      this.changed();
    }
    rename(name) { this.name = String(name).trim().slice(0, 40) || 'Visitor'; write('openagents.grid.name', this.name); this.lastState = 0; this.changed(); }
    watch() { if (this.joined) this.state(false); this.joined = false; this.status = this.offline ? 'Offline' : 'Watching'; this.changed(); }
    close() { this.watch(); this.closed = true; clearTimeout(this.timer); this.socket?.close(); this.socket = null; }
  }
  return { Session, spawn, validPose, yaw, wrap, read, write, seed, configureLayout };
})();
if (typeof module !== 'undefined') module.exports = GridNet;
