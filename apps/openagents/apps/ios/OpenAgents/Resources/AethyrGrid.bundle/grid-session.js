// NIP-MV presence, interoperable with OpenAgents' verse-bare world.
// Session/crowd behavior ported from OpenAgents verse::session and verse-core::crowd.
// Render cadence is independent of networking. No chat/profile subscription.
'use strict';
const GridNet = (() => {
  const WORLD = 'verse-bare', MAX = 128;
  const MOVING = 200, IDLE = 5000, STATE = 30000, DELAY = 500, STALE = 10000, JOIN_RETRY = 5000;
  const cleanName = value => [...(typeof value === 'string' ? value : '').replace(/[\p{Cc}\p{Cf}]/gu, '').replace(/\s+/g, ' ').trim()].slice(0, 40).join('');
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
      peers.every(o => Math.hypot(p[0] - o.p[0], p[1] - o.p[2]) > 2.1);
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
      try { if (!keyOK(secret)) throw Error(); NostrTools.getPublicKey(bytes(secret)); }
      catch (_) { secret = hex(NostrTools.generateSecretKey()); }
      this.secret = bytes(secret); this.key = NostrTools.getPublicKey(this.secret);
      write('openagents.grid.secret', secret);
      let keptName=read('openagents.grid.name');
      try { if(!keptName) keptName=localStorage.getItem('openagents.grid.name'); } catch (_) {}
      this.name = cleanName(name || keptName || 'Visitor') || 'Visitor';
      write('openagents.grid.name', this.name);
      const lists = read('openagents.grid.blocked', {});
      this.blocked = new Set((Array.isArray(lists?.blocked) ? lists.blocked : []).filter(keyOK).slice(0, 1000));
      this.muted = new Set((Array.isArray(lists?.muted) ? lists.muted : []).filter(keyOK).slice(0, 1000));
      this.peers = new Map(); this.seen = new Set(); this.session = hex(crypto.getRandomValues(new Uint8Array(8)));
      this.sequence = 0; this.joined = false; this.closed = false; this.socket = null;
      this.status = offline ? 'Offline' : 'Watching'; this.lastFrame = 0; this.lastState = 0; this.retry = 0;
      this.lookSeed = seed(this.key); this.pose = null; this.resume = read('aethyr.grid.pose'); this.refusals = 0;
      this.lastSaved = this.lastStateAttempt = this.lastStateCreated = 0;
      this.backoffUntil = 0; this.backoff = 5000; this.lastRefusal = null;
      this.lastByKey = new Map(); this.framesByKey = new Map(); this.states = new Map();
      this.liveReady = false; this.stateReady = false; this.resumeReady = offline;
      this.auth = null; this.challenge = null; this.authAccepted = false; this.joinAt = 0;
      this.pendingSpawn = false; this.spawnDeadline = 0; this.framesPublished = 0;
    }
    get ready() { return this.liveReady && this.stateReady && (!this.auth || this.authAccepted); }
    updateStatus() {
      this.status = this.offline ? 'Offline' : Date.now() < this.backoffUntil && this.lastRefusal?.includes('world is full') ? 'Grid full · Waiting for room' :
        this.ready ? (this.joined ? 'Online' : 'Watching') : this.socket?.readyState === 1 ? 'Connecting' : 'Reconnecting';
      this.changed();
    }
    connect() {
      if (this.offline || this.closed || this.socket) return;
      let url;
      try { url = new URL(this.relay); if (!['ws:', 'wss:'].includes(url.protocol)) throw Error(); }
      catch (_) { this.status = 'Invalid relay address'; this.changed(); return; }
      this.status = 'Connecting'; this.changed();
      const socket = this.socket = new WebSocket(url.href);
      socket.onopen = () => {
        if (this.socket !== socket || this.closed) return;
        this.retry = 0; this.auth = null; this.challenge = null; this.authAccepted = false;
        this.session = hex(crypto.getRandomValues(new Uint8Array(8))); this.sequence = 0;
        this.lastFrame = this.lastState = 0; this.subscribe(); this.updateStatus();
      };
      socket.onmessage = e => { if (this.socket === socket && typeof e.data === 'string' && e.data.length < 65536) this.receive(e.data); };
      socket.onclose = () => {
        if (this.socket !== socket) return;
        this.socket = null; this.liveReady = this.stateReady = false;
        if (this.closed) return;
        this.updateStatus();
        this.timer = setTimeout(() => this.connect(), Math.min(30000, 1000 * 2 ** Math.min(this.retry++, 5)));
      };
      socket.onerror = () => { if (this.socket === socket) { this.status = 'Connection interrupted'; this.changed(); } };
    }
    send(message) {
      if (this.socket?.readyState !== 1) return false;
      try { this.socket.send(JSON.stringify(message)); return true; } catch (_) { return false; }
    }
    request(id) {
      if (id === 'mv-live') this.send(['REQ', id, { kinds: [23300], '#w': [WORLD] }]);
      if (id === 'mv-state') this.send(['REQ', id, { kinds: [33301], '#w': [WORLD], limit: 500 }]);
      if (id === 'mv-me') this.send(['REQ', id, { kinds: [33301], authors: [this.key], '#d': [WORLD + '/avatar'], limit: 1 }]);
    }
    subscribe() {
      this.liveReady = this.stateReady = false; this.joinAt = Date.now();
      this.request('mv-live'); this.request('mv-state'); this.request('mv-me');
      if (typeof GridBuilds !== 'undefined') this.send(['REQ', 'deploy-board', { kinds: [3205,30205], authors: [GridBuilds.ANNOUNCER], limit: 80 }]);
      this.lastFrame = this.lastState = 0;
    }
    answerChallenge() {
      const auth = NostrTools.finalizeEvent({ kind: 22242, created_at: Math.floor(Date.now() / 1000), tags: [['relay', this.relay], ['challenge', this.challenge]], content: '' }, this.secret);
      this.auth = auth.id; this.joinAt = Date.now(); this.send(['AUTH', auth]);
    }
    retryJoin(now) {
      if (this.offline || this.ready || this.socket?.readyState !== 1 || now - this.joinAt < JOIN_RETRY) return;
      if (this.auth && !this.authAccepted && this.challenge) this.answerChallenge();
      else { if (!this.liveReady) this.request('mv-live'); if (!this.stateReady) this.request('mv-state'); this.joinAt = now; }
    }
    sign(kind, tags, content, created = Math.floor(Date.now() / 1000)) { return NostrTools.finalizeEvent({ kind, tags, content: JSON.stringify(content), created_at: created }, this.secret); }
    publish(kind, tags, content, created) { return !this.offline && this.ready && Date.now() >= this.backoffUntil && this.send(['EVENT', this.sign(kind, tags, content, created)]); }
    receive(raw) {
      let msg;
      try { msg = JSON.parse(raw); } catch (_) { return; }
      if (!Array.isArray(msg)) return;
      if (msg[0] === 'AUTH' && typeof msg[1] === 'string' && msg[1].length <= 4096) {
        this.challenge = msg[1]; this.authAccepted = false; this.liveReady = this.stateReady = false;
        this.answerChallenge(); this.updateStatus(); return;
      }
      if (msg[0] === 'EOSE') {
        if (msg[1] === 'mv-live') this.liveReady = true;
        if (msg[1] === 'mv-state') this.stateReady = true;
        if (msg[1] === 'mv-me') { this.resumeReady = true; this.send(['CLOSE', 'mv-me']); }
        this.updateStatus(); return;
      }
      if (msg[0] === 'CLOSED' && ['mv-live', 'mv-state'].includes(msg[1])) {
        console.warn('Grid relay subscription', msg[1], String(msg[2]).slice(0,200));
        if (msg[1] === 'mv-live') this.liveReady = false; else this.stateReady = false;
        this.joinAt = Date.now(); this.updateStatus();
        if (!String(msg[2]).startsWith('auth-required:')) { this.status = 'World updates interrupted · Retrying'; this.changed(); }
        return;
      }
      if (msg[0] === 'OK') {
        if (msg[1] === this.auth) {
          if (msg[2] === true) { this.authAccepted = true; this.subscribe(); }
          else { this.status = 'Relay rejected authentication · Retrying'; this.changed(); return; }
        }
        if (msg[2] === false) {
          this.refusals++; this.lastRefusal = String(msg[3]).slice(0, 200);
          if (this.lastRefusal.startsWith('rate-limited:')) {
            const now = Date.now(); this.backoff = now < this.backoffUntil ? Math.min(60000, this.backoff * 2) : 5000;
            this.backoffUntil = now + this.backoff;
          }
        }
        this.updateStatus(); return;
      }
      if (msg[0] === 'EVENT' && msg[1] === 'deploy-board' && typeof GridBuilds !== 'undefined') {
        const record = GridBuilds.read(msg[2], this.deployTest || false);
        if(record) this.onBuild?.(record);
        return;
      }
      if (msg[0] !== 'EVENT' || !['mv-live', 'mv-state', 'mv-me'].includes(msg[1])) return;
      const e = msg[2], now = Date.now();
      if (!e || !keyOK(e.pubkey) || this.blocked.has(e.pubkey) || ![23300, 33301].includes(e.kind) ||
          !Array.isArray(e.tags) || e.tags.length>64 || !e.tags.every(t=>Array.isArray(t)&&t.length>=2&&t.every(v=>typeof v==='string')) || e.tags.filter(t => t[0] === 'w').length !== 1 || !e.tags.some(t => t[0] === 'w' && t[1] === WORLD) ||
          typeof e.content !== 'string' || e.content.length > 24000 || !Number.isInteger(e.created_at) ||
          e.created_at * 1000 > now + 10000 || (e.kind === 23300 && e.created_at * 1000 < now - STALE) || this.seen.has(e.id)) return;
      // Bound signature verification work before doing elliptic curve math.
      const budget = this.framesByKey.get(e.pubkey) || { at: now, n: 0 };
      if (now - budget.at > 1000) { budget.at = now; budget.n = 0; }
      if (++budget.n > 16) return;
      if (!this.framesByKey.has(e.pubkey) && this.framesByKey.size >= 1024) this.framesByKey.delete(this.framesByKey.keys().next().value);
      this.framesByKey.set(e.pubkey, budget);
      try { if (!NostrTools.verifyEvent(e)) return; } catch (_) { return; }
      let content;
      try { content = JSON.parse(e.content); } catch (_) { return; }
      if (!content || content.v !== 1 || !Number.isSafeInteger(content.t) || content.t < 0 || content.t > now + 10000 || (e.kind === 23300 && content.t < now - STALE)) return;
      let pose;
      if (e.kind === 23300) {
        if (!Array.isArray(content.e) || content.e.length > 16 || typeof content.s !== 'string' || !content.s.length || content.s.length > 128 || !Number.isSafeInteger(content.n) || content.n < 0) return;
        pose = content.e.find(validPose);
      } else {
        if (!e.tags.some(t => t[0] === 'd' && t[1] === WORLD + '/avatar') || !validPose(content) || typeof content.online !== 'boolean') return;
        pose = content;
      }
      if (!pose) return;
      this.seen.add(e.id); if (this.seen.size > 4096) this.seen.delete(this.seen.values().next().value);
      if (e.pubkey === this.key) {
        if ((!this.joined || this.pendingSpawn) && e.kind === 33301 && (!this.resume?.t || content.t > this.resume.t)) this.resume = { p: pose.p, yaw: yaw(pose.q), t: content.t };
        return;
      }
      if (e.kind === 33301) {
        const old = this.states.get(e.pubkey);
        if (old && content.t <= old.t) return;
        const state = { t: content.t, online: content.online, name: cleanName(content.name || ''), seed: content.aethyr?.seed };
        this.states.set(e.pubkey, state);
        if (this.states.size > 1024) this.states.delete(this.states.keys().next().value);
        const peer = this.peers.get(e.pubkey);
        if (peer) {
          if (!state.online && content.t >= peer.time) this.peers.delete(e.pubkey);
          else this.profile(peer, state);
        }
        this.changed(); return;
      }
      // Durable state is metadata, never a motion sample or sequence reset.
      if (this.states.get(e.pubkey)?.online === false && content.t <= this.states.get(e.pubkey).t) return;
      let previous = this.lastByKey.get(e.pubkey);
      if (previous) {
        if (previous.s === content.s) { if (content.n <= previous.n) return; }
        else {
          if (previous.retired.has(content.s) || content.t < previous.t || previous.retired.size >= 16) return;
          previous.retired.add(previous.s);
        }
      } else previous = { retired: new Set() };
      previous.t = Math.max(previous.t || 0, content.t); previous.s = content.s; previous.n = content.n;
      this.lastByKey.set(e.pubkey, previous);
      if (this.lastByKey.size > 1024) this.lastByKey.delete(this.lastByKey.keys().next().value);
      if (!this.peers.has(e.pubkey) && this.peers.size >= MAX) return;
      let peer = this.peers.get(e.pubkey);
      if (!peer) peer = { key: e.pubkey, name: e.pubkey.slice(0, 8), seed: seed(e.pubkey), samples: [] };
      this.profile(peer, this.states.get(e.pubkey));
      peer.seen = now; peer.time = content.t;
      const last = peer.samples.at(-1);
      const measured = last && now > last.at ? Math.min(15, Math.hypot(pose.p[0]-last.p[0],pose.p[2]-last.p[2])*1000/(now-last.at)) : 0;
      const norm = Math.hypot(...pose.q);
      peer.samples.push({ at: now, p: pose.p, yaw: yaw(pose.q.map(v=>v/norm)), speed: finite(pose.v, 3, 100) ? Math.hypot(pose.v[0],pose.v[2]) : measured });
      peer.samples = peer.samples.slice(-32); this.peers.set(e.pubkey, peer); this.changed();
    }
    profile(peer, state) {
      if (!state) return;
      if (state.name) peer.name = state.name;
      if (Number.isInteger(state.seed) && state.seed >= 0 && state.seed <= 0x7fffffff) peer.seed = state.seed;
    }
    join() {
      if (this.joined) return null;
      this.joined = true; this.pendingSpawn = true; this.spawnDeadline = Date.now() + 1500;
      this.lastFrame = this.lastState = 0; this.updateStatus();
      return this.pollSpawn();
    }
    pollSpawn() {
      if (!this.pendingSpawn || (!this.offline && !this.resumeReady && Date.now() < this.spawnDeadline)) return null;
      this.pendingSpawn = false;
      const p = spawn(this.key, [...this.peers.values()].map(o => o.samples.at(-1)), this.resume?.p && [this.resume.p[0], this.resume.p[2]]);
      return { x: p[0], z: p[1], yaw: Number.isFinite(this.resume?.yaw) ? this.resume.yaw : Math.atan2(-p[0], -p[1]) };
    }
    state(online) {
      if (!this.pose || online && Date.now()-this.lastStateAttempt < 1500) return false;
      const created = Math.max(Math.floor(Date.now()/1000), this.lastStateCreated+1);
      const { p, q } = this.pose;
      const sent = this.publish(33301, [['d', WORLD + '/avatar'], ['w', WORLD], ['role', 'avatar'], ['c', Math.floor(p[0] / 64) + ',' + Math.floor(p[2] / 64)]],
        { v: 1, id: 'avatar', role: 'avatar', p, q, t: Date.now(), online, name: this.name, aethyr: {seed:this.lookSeed} }, created);
      if (sent) { this.lastStateAttempt = Date.now(); this.lastStateCreated = created; }
      return sent;
    }
    tick(p, face, speed, velocity) {
      const now = Date.now();
      this.retryJoin(now);
      for (const [key, peer] of this.peers) if (now - peer.seen >= STALE) this.peers.delete(key);
      if (!this.joined || this.pendingSpawn) return;
      const q = [0, Math.sin(face / 2), 0, Math.cos(face / 2)];
      this.pose = { p, q };
      const moving = speed > .05 || this.lastPosition && Math.hypot(...p.map((v,i)=>v-this.lastPosition[i])) > .005 || Math.abs(velocity?.[1] || 0) > .05 || this.lastYaw !== undefined && Math.abs(wrap(face - this.lastYaw)) > .01;
      if (now - this.lastFrame >= (moving ? MOVING : IDLE)) {
        const entity = { id: 'avatar', role: 'avatar', p, q, v: finite(velocity,3,100)?velocity:[Math.sin(face) * speed, 0, Math.cos(face) * speed] };
        if (this.publish(23300, [['w', WORLD], ['c', Math.floor(p[0] / 64) + ',' + Math.floor(p[2] / 64)]], { v: 1, s: this.session, n: ++this.sequence, t: now, e: [entity] })) { this.lastFrame = now; this.lastYaw = face; this.lastPosition = [...p]; this.framesPublished++; }
      }
      if (now - this.lastState >= STATE && this.state(true)) this.lastState = now;
      if (now-this.lastSaved >= 15000) { write('aethyr.grid.pose', {p,yaw:face,t:now}); this.lastSaved=now; }
    }
    figures(now = Date.now()) {
      const at = now - DELAY;
      return [...this.peers.values()].filter(o => !this.blocked.has(o.key) && o.samples.length && (o.seen === undefined || now-o.seen < STALE)).map(peer => {
        const samples = peer.samples;
        let a = samples[0], b = a;
        for (const sample of samples) { if (sample.at <= at) a = sample; if (sample.at >= at) { b = sample; break; } b = sample; }
        const blend = b.at > a.at ? Math.max(0, Math.min(1, (at-a.at)/(b.at-a.at))) : 1;
        let p = a.p.map((v,i)=>v+(b.p[i]-v)*blend);
        let speed = b.at>a.at ? Math.min(15,Math.hypot(b.p[0]-a.p[0],b.p[2]-a.p[2])*1000/(b.at-a.at)) : 0;
        const last = samples.at(-1), prev = samples.at(-2);
        if (at > last.at && prev && last.at > prev.at) {
          const span = last.at-prev.at, velocity = last.p.map((v,i)=>(v-prev.p[i])*1000/span);
          if (Math.hypot(...velocity) <= 15) { p = last.p.map((v,i)=>v+velocity[i]*Math.min(at-last.at,span)/1000); speed=at-last.at<span?Math.hypot(velocity[0],velocity[2]):0; }
        }
        return {...peer, p, yaw:a.yaw+wrap(b.yaw-a.yaw)*blend, speed};
      });
    }
    metrics() {
      return {key:this.key,ready:this.ready,status:this.status,pendingSpawn:this.pendingSpawn,framesPublished:this.framesPublished,refusals:this.refusals,lastRefusal:this.lastRefusal,players:this.figures().map(({key,name,p})=>({key,name,p}))};
    }
    moderate(key, kind, on) {
      if (!keyOK(key) || key === this.key || !['blocked', 'muted'].includes(kind)) return;
      const set = this[kind]; if (on && set.size < 1000) set.add(key); else if (!on) set.delete(key);
      write('openagents.grid.blocked', { blocked: [...this.blocked], muted: [...this.muted] });
      if (kind === 'blocked' && on) this.peers.delete(key);
      this.changed();
    }
    rename(name) { this.name = cleanName(name) || 'Visitor'; write('openagents.grid.name', this.name); this.lastState = 0; this.changed(); }
    watch() { if (this.joined) this.state(false); this.joined = false; this.pendingSpawn = false; if (this.pose) write('aethyr.grid.pose', {p:this.pose.p,yaw:yaw(this.pose.q),t:Date.now()}); this.updateStatus(); }
    close() { this.watch(); this.closed = true; clearTimeout(this.timer); this.socket?.close(); this.socket = null; }
  }
  return { Session, spawn, validPose, yaw, wrap, read, write, seed, configureLayout, cleanName, MOVING, IDLE, STATE, DELAY, STALE };
})();
if (typeof module !== 'undefined') module.exports = GridNet;
