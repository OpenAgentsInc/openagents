// Kiki's playtest flow, as a clickable prototype. Served two ways: as a
// Claude artifact (mock house) and at /new-lander/flow on openagents.com,
// where the house screens run the real Haunted House release.
const REAL = location.pathname.startsWith('/new-lander/flow');
const $ = (id) => document.getElementById(id);
const still = matchMedia('(prefers-reduced-motion: reduce)').matches;

/* ---------- shared WebGPU field (the Coming soon shader, with a tint) ---------- */
const TINTS = {
  cool: [[0.05, 0.055, 0.075], [0.20, 0.21, 0.27]],
  house: [[0.07, 0.04, 0.02], [0.34, 0.18, 0.07]],
  forest: [[0.02, 0.06, 0.04], [0.10, 0.24, 0.14]],
  village: [[0.07, 0.05, 0.03], [0.30, 0.22, 0.12]],
  cemetery: [[0.03, 0.04, 0.06], [0.16, 0.18, 0.22]],
  other: [[0.05, 0.03, 0.07], [0.22, 0.15, 0.30]],
};
let tintFrom = TINTS.cool, tintTo = TINTS.cool, tintT = 1, planetKind = 'forest';
function currentTint() { const k = Math.min(1, tintT); return [0, 1].map((i) => tintFrom[i].map((v, j) => v + (tintTo[i][j] - v) * k)); }
function setTint(name) { tintFrom = currentTint(); tintTo = TINTS[name === 'planet' ? planetKind : name] || TINTS.cool; tintT = 0; }
const SHADER = `
struct U { size: vec2f, time: f32, pad: f32, a: vec4f, b: vec4f }
@group(0) @binding(0) var<uniform> u: U;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2f) -> f32 {
  let i = floor(p); let f = fract(p); let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), s.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), s.x), s.y);
}
fn fbm(q: vec2f) -> f32 {
  var p = q; var v = 0.0; var a = 0.5;
  for (var k = 0; k < 5; k++) { v += a * noise(p); p = p * 2.03 + vec2f(1.7, 9.2); a *= 0.5; }
  return v;
}
@fragment fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let uv = (pos.xy - 0.5 * u.size) / u.size.y;
  let t = u.time * 0.035;
  let w = vec2f(fbm(uv * 1.6 + vec2f(t, -t)), fbm(uv * 1.6 + vec2f(-t, t * 0.7) + 4.0));
  let n = fbm(uv * 2.2 + 2.4 * w + vec2f(0.0, t));
  let glow = exp(-2.2 * dot(uv, uv));
  let tint = mix(u.a.xyz, u.b.xyz, n * n);
  var c = tint * glow * glow * (0.2 + 0.8 * n);
  let star = step(0.9975, hash(floor(pos.xy * 0.5))) * (0.5 + 0.5 * sin(u.time * 1.3 + hash(floor(pos.xy)) * 40.0));
  c += vec3f(star * 0.5 * (1.0 - glow * 0.6));
  return vec4f(c, 1.0);
}`;
async function startField() {
  const canvas = $('field');
  if (!navigator.gpu) return;
  const adapter = await navigator.gpu.requestAdapter(); if (!adapter) return;
  const device = await adapter.requestDevice();
  const context = canvas.getContext('webgpu');
  const format = navigator.gpu.getPreferredCanvasFormat();
  context.configure({ device, format, alphaMode: 'opaque' });
  const module = device.createShaderModule({ code: SHADER });
  const pipeline = device.createRenderPipeline({ layout: 'auto', vertex: { module, entryPoint: 'vs' }, fragment: { module, entryPoint: 'fs', targets: [{ format }] } });
  const uniforms = device.createBuffer({ size: 48, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const group = device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: { buffer: uniforms } }] });
  const begin = performance.now();
  const frame = () => {
    const s = Math.min(devicePixelRatio || 1, 2);
    const w = Math.max(1, Math.floor(canvas.clientWidth * s)), h = Math.max(1, Math.floor(canvas.clientHeight * s));
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
    const time = still ? 20 : 20 + (performance.now() - begin) / 1000;
    tintT = still ? 1 : Math.min(1, tintT + 0.02);
    const [a, b] = currentTint();
    device.queue.writeBuffer(uniforms, 0, new Float32Array([w, h, time, 0, a[0], a[1], a[2], 0, b[0], b[1], b[2], 0]));
    const enc = device.createCommandEncoder();
    const pass = enc.beginRenderPass({ colorAttachments: [{ view: context.getCurrentTexture().createView(), loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 } }] });
    pass.setPipeline(pipeline); pass.setBindGroup(0, group); pass.draw(3); pass.end();
    device.queue.submit([enc.finish()]);
    canvas.classList.add('on');
    requestAnimationFrame(frame);
  };
  frame();
}
startField().catch(() => {});

/* ---------- state, navigation, small helpers ---------- */
const S = { user: null, changes: [], published: false, awarded: false, planet: null, room: 0, editing: false, listening: true, survey: null, greeted: new Set() };
const SCREENS = [['landing', 'Landing'], ['login', 'Login'], ['map', 'Map'], ['house', 'House'], ['setup', 'Setup'], ['planet', 'Planet']];
let current = null;
function go(id) {
  current = id;
  document.querySelectorAll('.screen').forEach((s) => s.classList.toggle('on', s.id === id));
  document.querySelectorAll('#steps button').forEach((b) => b.classList.toggle('on', b.dataset.step === id));
  setTint($(id).dataset.tint);
  galaxy.mode = id === 'map' ? 'map' : id === 'login' ? 'login' : null;
  $('galaxy').classList.toggle('on', !!galaxy.mode);
  $('galaxy').style.opacity = id === 'login' ? '.6' : '1';
  if (galaxy.mode) galaxy.start(); else galaxy.stop();
  if (id === 'landing') orbs.start(); else orbs.stop();
  if (id === 'house') enterHouse();
  if (id === 'planet') walk.start(); else walk.stop();
  if (id === 'map' && S.published && !S.awarded) { S.awarded = true; $('awardOverlay').hidden = false; }
  try { history.replaceState(null, '', '#' + id); } catch (e) { /* the artifact viewer may refuse */ }
}
document.addEventListener('click', (e) => { const t = e.target.closest('[data-go]'); if (t) go(t.dataset.go); });
SCREENS.forEach(([id, name]) => { const b = document.createElement('button'); b.textContent = name; b.dataset.step = id; b.onclick = () => go(id); $('steps').appendChild(b); });
let toastTimer;
function toast(msg) { const t = $('toast'); t.textContent = msg; t.style.opacity = 1; clearTimeout(toastTimer); toastTimer = setTimeout(() => (t.style.opacity = 0), 2400); }
function setWallet() {
  document.querySelectorAll('.wallet').forEach((w) => (w.textContent = 'Wallet · ' + (S.user ? '0 sats' : 'guest')));
  $('who').textContent = S.user || 'Guest';
}
const clock = () => new Date().toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
function say(box, who, text, kind) {
  const m = document.createElement('div'); m.className = 'msg' + (kind ? ' ' + kind : '');
  const w = document.createElement('div'); w.className = 'who'; w.textContent = who;
  const t = document.createElement('time'); t.textContent = clock(); w.appendChild(t);
  const x = document.createElement('div'); x.className = 'txt'; x.textContent = text;
  m.append(w, x); $(box).appendChild(m); $(box).scrollTop = $(box).scrollHeight;
}
const pick = (list) => list[Math.floor(Math.random() * list.length)];

/* ---------- glass cards: drag anywhere, close with ×, the agent can open or close them ---------- */
let z = 10;
document.querySelectorAll('[data-card]').forEach((card) => {
  const h = card.querySelector('header'); let sx, sy, ox, oy, on = false;
  h.addEventListener('pointerdown', (e) => {
    if (e.target.closest('.x')) return;
    if (getComputedStyle(card).position !== 'absolute') { const r = card.getBoundingClientRect(); card.style.position = 'fixed'; card.style.left = r.left + 'px'; card.style.top = r.top + 'px'; }
    on = true; h.setPointerCapture(e.pointerId); sx = e.clientX; sy = e.clientY; ox = card.offsetLeft; oy = card.offsetTop; card.style.right = 'auto'; card.style.zIndex = ++z;
  });
  h.addEventListener('pointermove', (e) => {
    if (!on) return;
    card.style.left = Math.min(innerWidth - 60, Math.max(-card.offsetWidth + 60, ox + e.clientX - sx)) + 'px';
    card.style.top = Math.min(innerHeight - 40, Math.max(0, oy + e.clientY - sy)) + 'px';
  });
  h.addEventListener('pointerup', () => (on = false));
  const x = card.querySelector('.x'); if (x) x.onclick = () => showCard(card.dataset.card, false);
});
const CARD_NAMES = { tools: 'tools', voice: 'voice bot', chat: 'chat', inventory: 'inventory', log: 'edits', invite: 'invite', pchat: 'chat', survey: 'survey' };
function showCard(name, on) { const c = document.querySelector(`[data-card="${name}"]`); if (!c) return false; c.hidden = !on; if (on) c.style.zIndex = ++z; return true; }
document.addEventListener('click', async (e) => {
  const b = e.target.closest('[data-copy]'); if (!b) return;
  const inp = $(b.dataset.copy);
  try { await navigator.clipboard.writeText('https://' + inp.value); toast('Link copied'); } catch (err) { inp.select(); toast('Select the link and copy it'); }
  if (b.dataset.copy === 'planetLink') friendJoins();
});

/* ---------- 01 landing: gravity orbs behind the hero ---------- */
function drawOrb(c, x, y, r, alpha) {
  const g = c.createRadialGradient(x, y, 0, x, y, r * 3.2);
  g.addColorStop(0, `rgba(255,255,255,${0.95 * alpha})`); g.addColorStop(0.28, `rgba(237,237,237,${0.55 * alpha})`); g.addColorStop(1, 'rgba(237,237,237,0)');
  c.fillStyle = g; c.beginPath(); c.arc(x, y, r * 3.2, 0, 7); c.fill();
  c.fillStyle = `rgba(255,255,255,${alpha})`; c.beginPath(); c.arc(x, y, r * 0.55, 0, 7); c.fill();
}
const orbs = (() => {
  const cv = $('orbs'), cx = cv.getContext('2d'); let list = [], hold = null, raf = 0;
  const size = () => { const s = Math.min(devicePixelRatio || 1, 2); cv.width = cv.clientWidth * s; cv.height = cv.clientHeight * s; cx.setTransform(s, 0, 0, s, 0, 0); };
  const seed = () => { const w = cv.clientWidth, h = cv.clientHeight; list = [{ x: w * 0.2, y: h * 0.32, vx: 0, vy: 0.3, r: 14 }, { x: w * 0.8, y: h * 0.62, vx: 0, vy: -0.35, r: 10 }, { x: w * 0.7, y: h * 0.2, vx: -0.3, vy: 0, r: 6 }]; };
  const at = (e) => { const r = cv.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; };
  cv.addEventListener('pointerdown', (e) => { const [x, y] = at(e); hold = { x, y, vx: 0, vy: 0, r: 3, t: performance.now() }; cv.setPointerCapture(e.pointerId); });
  cv.addEventListener('pointermove', (e) => { if (hold) [hold.x, hold.y] = at(e); });
  const drop = () => { if (hold) { list.push(hold); hold = null; if (list.length > 40) list.shift(); } };
  cv.addEventListener('pointerup', drop); cv.addEventListener('pointercancel', drop);
  function step() {
    const w = cv.clientWidth, h = cv.clientHeight;
    if (hold) hold.r = Math.min(46, 3 + (performance.now() - hold.t) / 28);
    for (const a of list) for (const b of list) { if (a === b) continue; const dx = b.x - a.x, dy = b.y - a.y, d2 = dx * dx + dy * dy + 400, f = 0.06 * b.r * b.r / d2, d = Math.sqrt(d2); a.vx += f * dx / d; a.vy += f * dy / d; }
    for (const a of list) { a.x += a.vx; a.y += a.vy; if (a.x < a.r || a.x > w - a.r) { a.vx *= -0.8; a.x = Math.max(a.r, Math.min(w - a.r, a.x)); } if (a.y < a.r || a.y > h - a.r) { a.vy *= -0.8; a.y = Math.max(a.r, Math.min(h - a.r, a.y)); } }
    for (let i = 0; i < list.length; i++) for (let j = list.length - 1; j > i; j--) {
      const a = list[i], b = list[j];
      if (Math.hypot(a.x - b.x, a.y - b.y) < (a.r + b.r) * 0.7) { const ma = a.r * a.r, mb = b.r * b.r, m = ma + mb; a.vx = (a.vx * ma + b.vx * mb) / m; a.vy = (a.vy * ma + b.vy * mb) / m; a.x = (a.x * ma + b.x * mb) / m; a.y = (a.y * ma + b.y * mb) / m; a.r = Math.min(60, Math.sqrt(m)); list.splice(j, 1); }
    }
    cx.clearRect(0, 0, w, h);
    for (const a of hold ? [...list, hold] : list) drawOrb(cx, a.x, a.y, a.r, 1);
    raf = requestAnimationFrame(step);
  }
  addEventListener('resize', () => { if (current === 'landing') size(); });
  return { start() { cancelAnimationFrame(raf); requestAnimationFrame(() => { size(); if (!list.length) seed(); step(); }); }, stop() { cancelAnimationFrame(raf); } };
})();

/* ---------- 02 login ---------- */
$('googleBtn').onclick = () => { S.user = 'you@gmail.com'; setWallet(); toast('Logged in with Google'); go('map'); };
$('guestBtn').onclick = () => { S.user = null; setWallet(); go('map'); };

/* ---------- 03 the galaxy: behind the login card, and the rotatable map ---------- */
const galaxy = (() => {
  const cv = $('galaxy'), c = cv.getContext('2d'), label = $('label');
  let raf = 0, rot = 0.6, tilt = 0.42, drag = null, hover = null, moved = 0, t0 = performance.now();
  const self = { mode: null };
  function bodies() {
    const t = (performance.now() - t0) / 1000;
    const list = [
      { id: 'oa', name: 'OpenAgents', owner: 'OpenAgents', R: 0, a: 0, size: 0.075, locked: true },
      { id: 'house', name: 'Haunted House', owner: 'OpenAgents', R: 0.15, a: t * 0.3, size: 0.028, warm: true },
      { id: 'blue', name: 'Blue Rush', owner: 'Kiki David', R: 0.42, a: 2.4, size: 0.05, locked: true },
      { id: 'brain', name: 'Big Brain', owner: 'Owner to confirm', R: 0.46, a: 0.5, size: 0.045, locked: true },
      { id: 'grove', name: 'Everglade', owner: 'OpenAgents', R: 0.34, a: 4.2, size: 0.032, locked: true },
      { id: 'grid', name: 'The Grid', owner: 'OpenAgents', R: 0.3, a: 5.4, size: 0.036, locked: true },
    ];
    if (S.awarded) list.push({ id: 'mine', name: S.planet ? S.planet.name : 'New planet', owner: S.user || 'You', R: 0.26, a: 1.6, size: 0.04, mine: true });
    return list;
  }
  function project(p, w, h) {
    const m = Math.min(w, h) * 0.95, cx = w / 2, cy = h * 0.55;
    const x = p.R * Math.cos(p.a + rot), zz = p.R * Math.sin(p.a + rot);
    const depth = zz * Math.cos(tilt);
    const k = 1 / (1 + depth * 0.9);
    return { x: cx + x * m * k, y: cy + zz * Math.sin(tilt) * m * k, r: p.size * m * k, depth };
  }
  function draw() {
    const s = Math.min(devicePixelRatio || 1, 2), w = cv.clientWidth, h = cv.clientHeight;
    if (cv.width !== Math.floor(w * s) || cv.height !== Math.floor(h * s)) { cv.width = w * s; cv.height = h * s; }
    c.setTransform(s, 0, 0, s, 0, 0); c.clearRect(0, 0, w, h);
    if (!drag && !still) rot += self.mode === 'login' ? 0.0016 : 0.0006;
    const m = Math.min(w, h) * 0.95, cx = w / 2, cy = h * 0.55;
    c.strokeStyle = 'rgba(237,237,237,.07)'; c.lineWidth = 1;
    for (const R of [0.15, 0.26, 0.3, 0.34, 0.42, 0.46]) { c.beginPath(); c.ellipse(cx, cy, R * m, R * m * Math.sin(tilt), 0, 0, 7); c.stroke(); }
    const list = bodies().map((p) => ({ p, q: project(p, w, h) })).sort((a, b) => b.q.depth - a.q.depth);
    for (const { p, q } of list) {
      if (p.warm) {
        const g = c.createRadialGradient(q.x, q.y, 0, q.x, q.y, q.r * 3); g.addColorStop(0, 'rgba(232,121,43,.95)'); g.addColorStop(0.35, 'rgba(150,80,30,.5)'); g.addColorStop(1, 'rgba(120,60,20,0)');
        c.fillStyle = g; c.beginPath(); c.arc(q.x, q.y, q.r * 3, 0, 7); c.fill(); c.fillStyle = '#f0d8c0'; c.beginPath(); c.arc(q.x, q.y, q.r * 0.5, 0, 7); c.fill();
      } else drawOrb(c, q.x, q.y, q.r * 0.75, p.locked ? 0.55 : 1);
      if (p.mine) { const k = 0.5 + 0.5 * Math.sin(performance.now() / 300); c.strokeStyle = `rgba(255,255,255,${0.3 + 0.5 * k})`; c.beginPath(); c.arc(q.x, q.y, q.r * 1.6 + k * 4, 0, 7); c.stroke(); }
      if (self.mode === 'map' && hover === p.id) { c.strokeStyle = 'rgba(237,237,237,.6)'; c.beginPath(); c.arc(q.x, q.y, q.r * 1.4, 0, 7); c.stroke(); }
    }
    const hp = self.mode === 'map' && list.find(({ p }) => p.id === hover);
    if (hp) {
      label.style.opacity = 1; label.style.left = hp.q.x + 'px'; label.style.top = hp.q.y - hp.q.r + 'px';
      label.querySelector('.n').textContent = hp.p.name;
      label.querySelector('.o').textContent = hp.p.owner + (hp.p.locked ? ' · Locked' : hp.p.mine ? ' · Yours' : ' · Open');
    } else label.style.opacity = 0;
    raf = requestAnimationFrame(draw);
  }
  function hit(e) {
    const r = cv.getBoundingClientRect(), x = e.clientX - r.left, y = e.clientY - r.top;
    return bodies().map((p) => ({ p, q: project(p, cv.clientWidth, cv.clientHeight) })).sort((a, b) => a.q.depth - b.q.depth).find(({ q }) => Math.hypot(q.x - x, q.y - y) < Math.max(q.r * 1.5, 22))?.p;
  }
  cv.addEventListener('pointerdown', (e) => { if (self.mode !== 'map') return; drag = { x: e.clientX, y: e.clientY, rot, tilt }; moved = 0; cv.setPointerCapture(e.pointerId); });
  cv.addEventListener('pointermove', (e) => {
    if (self.mode !== 'map') return;
    if (drag) { moved += Math.abs(e.movementX) + Math.abs(e.movementY); rot = drag.rot + (e.clientX - drag.x) * 0.006; tilt = Math.max(0.12, Math.min(1.2, drag.tilt + (e.clientY - drag.y) * 0.004)); return; }
    const p = hit(e); hover = p ? p.id : null; cv.style.cursor = p && !p.locked ? 'pointer' : drag ? 'grabbing' : 'grab';
  });
  cv.addEventListener('pointerup', (e) => {
    const wasDrag = drag && moved > 6; drag = null;
    if (self.mode !== 'map' || wasDrag) return;
    const p = hit(e); if (!p) return;
    if (e.pointerType === 'touch' && hover !== p.id) { hover = p.id; return; }
    if (p.locked) { toast(p.name + ' is locked for the playtest'); return; }
    if (p.id === 'house') go('house'); else if (p.id === 'mine') go(S.planet ? 'planet' : 'setup');
  });
  cv.addEventListener('pointerleave', () => (hover = null));
  self.start = () => { cancelAnimationFrame(raf); draw(); };
  self.stop = () => { cancelAnimationFrame(raf); label.style.opacity = 0; };
  self.focus = (id) => { hover = id; };
  return self;
})();
$('awardOk').onclick = () => { $('awardOverlay').hidden = true; galaxy.focus('mine'); };

/* ---------- NPCs: a small text dataset with simple capabilities ---------- */
// Each NPC can greet, give a hint about the next step, answer when named
// in chat (@name), and emote. Lines are the dataset; nothing is generated.
const NPCS = {
  ghost: { name: 'Ghost', room: 1, greet: ['Oooh, a visitor. Mind the cold spots.', 'Welcome to the parlor. Nobody stays long.'], hint: ['Open Edit and try the lanterns. I like them orange.', 'Publish when you like it. Then the whole Verse can visit.'], answer: ['I only remember this house.', 'Ask the witch about the cauldron.'], emote: ['*floats through the wall*', '*the candles flicker*'] },
  cat: { name: 'Black Cat', room: 3, greet: ['Mrrp.'], hint: ['*stares at the cauldron, then at you*'], answer: ['Mrrrow.', '*ignores you*'], emote: ['*knocks a candle off the table*'] },
  witch: { name: 'Witch', room: 3, greet: ['Come in, come in. Mind the cat.'], hint: ['Place something from the inventory. One thing at a time if you are a guest.', 'Revert lets you try another idea.'], answer: ['My cauldron takes requests.', 'The ghost talks too much.'], emote: ['*stirs the cauldron*'] },
};
const PLANET_NPC = { forest: ['Ranger', 'Welcome to your forest. The path loops back here.'], village: ['Baker', 'Fresh bread at the square. Your village looks good.'], cemetery: ['Groundskeeper', 'Quiet tonight. Watch your step between the stones.'], other: ['Wanderer', 'I have never seen a place quite like this one.'] };
function npcFromText(text) { const m = text.toLowerCase().match(/@(\w+)/); if (!m) return null; return Object.entries(NPCS).find(([k, n]) => k.startsWith(m[1]) || n.name.toLowerCase().startsWith(m[1])); }
function npcSpeak(key, cap) { const n = NPCS[key]; say('houseWorld', n.name, pick(n[cap] || n.answer), 'npc'); }

/* ---------- 04-07 the Haunted House ---------- */
const ROOMS = ['Entry hall', 'Parlor', 'Long hall', 'Kitchen'];
const CHANGES = { candles: 'Warmer candlelight', lanterns: 'Lanterns glow orange', moon: 'Moonlit window', fog: 'Low fog' };
const ITEMS = [['🎃', 'Pumpkin'], ['🕯', 'Candles'], ['🏮', 'Lantern'], ['📚', 'Bookcase'], ['🧰', 'Chest'], ['🖼', 'Portrait'], ['🦇', 'Bat'], ['⚱', 'Urn']];
let houseLoaded = false;
function enterHouse() {
  if (REAL) {
    $('roomView').hidden = true; $('houseFrame').hidden = false;
    if (!houseLoaded) { houseLoaded = true; $('houseFrame').src = '/new-lander/house'; }
  }
  renderHouse();
  greetRoom();
}
// On openagents.com the house page is same-origin: hide its own controls so
// only the flow's controls show. CSSOM changes are allowed under its CSP.
$('houseFrame').addEventListener('load', () => {
  const hideHostControls = () => {
    try {
      const d = $('houseFrame').contentDocument;
      for (const id of ['world-dock', 'house-hud', 'house-editor', 'world-voice']) { const el = d.getElementById(id); if (el) el.style.display = 'none'; }
    } catch (e) { /* not same-origin: leave the house as it is */ }
  };
  hideHostControls(); setTimeout(hideHostControls, 1500); setTimeout(hideHostControls, 5000);
});
function roomSVG(i) {
  const has = (k) => S.changes.some((c) => c.key === k);
  const lantern = has('lanterns') ? '#ff8a2a' : '#7a4a22';
  const candle = has('candles') ? '#ffc070' : '#d07a30';
  const depth = i === 2 ? 0.16 : 0.3; // the long hall goes further
  const W = 1000, H = 600, bw = W * depth, bh = H * depth * 1.3, bx = (W - bw) / 2, by = (H - bh) / 2 + 10;
  const placed = S.changes.filter((c) => c.key.startsWith('place:') && c.room === i);
  let s = `<svg viewBox="0 0 ${W} ${H}" preserveAspectRatio="xMidYMid slice" aria-label="${ROOMS[i]}, first person">`;
  s += `<rect width="${W}" height="${H}" fill="#140b05"/>`;
  s += `<polygon points="0,0 ${W},0 ${bx + bw},${by} ${bx},${by}" fill="#24150a"/>`; // ceiling
  s += `<polygon points="0,${H} ${W},${H} ${bx + bw},${by + bh} ${bx},${by + bh}" fill="#3a2412"/>`; // floor
  s += `<polygon points="0,0 ${bx},${by} ${bx},${by + bh} 0,${H}" fill="#4a2c14"/>`; // left wall
  s += `<polygon points="${W},0 ${bx + bw},${by} ${bx + bw},${by + bh} ${W},${H}" fill="#5a3518"/>`; // right wall
  s += `<rect x="${bx}" y="${by}" width="${bw}" height="${bh}" fill="#2e1b0c"/>`; // back wall
  for (let k = 1; k < 6; k++) { const t = k / 6; s += `<line x1="${t * W}" y1="${H}" x2="${bx + t * bw}" y2="${by + bh}" stroke="#2a180b" stroke-width="3"/>`; }
  if (i < ROOMS.length - 1) s += `<g class="door" data-door="${i + 1}"><rect x="${W / 2 - bw * 0.18}" y="${by + bh * 0.3}" width="${bw * 0.36}" height="${bh * 0.7}" fill="#0b0603"/><text x="${W / 2}" y="${by + bh * 0.3 - 10}" fill="#d9b58a" font-size="16" text-anchor="middle" font-family="Geist,system-ui">${ROOMS[i + 1]} →</text></g>`;
  if (i > 0) s += `<g class="door" data-door="${i - 1}"><polygon points="40,${H * 0.3} 150,${H * 0.36} 150,${H * 0.86} 40,${H * 0.98}" fill="#0b0603"/><text x="95" y="${H * 0.28}" fill="#d9b58a" font-size="16" text-anchor="middle" font-family="Geist,system-ui">← ${ROOMS[i - 1]}</text></g>`;
  // wall lanterns
  for (const [x, y] of [[bx - 60, by + 40], [bx + bw + 60, by + 40]]) s += `<circle cx="${x}" cy="${y}" r="${has('lanterns') ? 34 : 18}" fill="${lantern}" opacity="${has('lanterns') ? 0.35 : 0.2}"/><rect x="${x - 9}" y="${y - 14}" width="18" height="28" fill="${lantern}"/>`;
  if (has('moon') && (i === 0 || i === 1)) s += `<rect x="${bx + bw * 0.1}" y="${by + 20}" width="${bw * 0.22}" height="${bh * 0.35}" fill="#c9d4e8" opacity=".55"/><polygon points="${bx + bw * 0.1},${by + bh} ${bx + bw * 0.32},${by + bh} ${W * 0.5},${H} ${W * 0.2},${H}" fill="#c9d4e8" opacity=".08"/>`;
  if (i === 1) s += `<rect x="${W * 0.66}" y="${H * 0.6}" width="140" height="90" rx="14" fill="#2a170a"/><rect x="${bx + bw * 0.62}" y="${by + 26}" width="${bw * 0.22}" height="${bh * 0.3}" fill="#1a0f07" stroke="#8a5a2a" stroke-width="4"/>`;
  if (i === 3) s += `<ellipse cx="${W * 0.62}" cy="${H * 0.78}" rx="80" ry="22" fill="#e8792b" opacity=".35"/><path d="M${W * 0.62 - 70},${H * 0.7} q70,90 140,0 z" fill="#0a0603"/>`;
  s += `<rect x="${W * 0.24}" y="${H * 0.74}" width="10" height="26" fill="${candle}"/><circle cx="${W * 0.24 + 5}" cy="${H * 0.73}" r="${has('candles') ? 12 : 6}" fill="${candle}" opacity=".9"/>`;
  placed.forEach((c, k) => { s += `<text x="${W * 0.36 + k * 70}" y="${H * 0.9}" font-size="48" text-anchor="middle">${c.icon}</text>`; });
  for (const [key, n] of Object.entries(NPCS)) {
    if (n.room !== i) continue;
    if (key === 'ghost') s += `<g class="npc" data-npc="ghost"><path d="M${W * 0.32},${H * 0.62} q0,-120 60,-120 q60,0 60,120 l-15,-14 -15,14 -15,-14 -15,14 -15,-14 z" fill="#ededed" opacity=".8"/><circle cx="${W * 0.32 + 42}" cy="${H * 0.46}" r="6" fill="#140b05"/><circle cx="${W * 0.32 + 78}" cy="${H * 0.46}" r="6" fill="#140b05"/></g>`;
    if (key === 'cat') s += `<g class="npc" data-npc="cat"><ellipse cx="${W * 0.8}" cy="${H * 0.86}" rx="40" ry="24" fill="#050302"/><circle cx="${W * 0.8 + 36}" cy="${H * 0.8}" r="18" fill="#050302"/><polygon points="${W * 0.8 + 24},${H * 0.77} ${W * 0.8 + 28},${H * 0.73} ${W * 0.8 + 34},${H * 0.77}" fill="#050302"/><circle cx="${W * 0.8 + 42}" cy="${H * 0.8}" r="3" fill="#e8792b"/></g>`;
    if (key === 'witch') s += `<g class="npc" data-npc="witch"><polygon points="${W * 0.44},${H * 0.42} ${W * 0.47},${H * 0.26} ${W * 0.5},${H * 0.42}" fill="#050302"/><rect x="${W * 0.43}" y="${H * 0.42}" width="${W * 0.08}" height="6" fill="#050302"/><polygon points="${W * 0.44},${H * 0.44} ${W * 0.5},${H * 0.44} ${W * 0.53},${H * 0.86} ${W * 0.41},${H * 0.86}" fill="#0c0704"/></g>`;
  }
  if (has('fog')) s += `<rect x="0" y="${H * 0.72}" width="${W}" height="${H * 0.28}" fill="#c9b8a4" opacity=".16"/><rect x="0" y="${H * 0.82}" width="${W}" height="${H * 0.18}" fill="#c9b8a4" opacity=".14"/>`;
  return s + '</svg>';
}
function renderHouse() {
  if (!REAL) $('roomView').innerHTML = roomSVG(S.room);
  $('editCards').hidden = !S.editing;
  $('playActions').hidden = S.editing;
  $('editActions').hidden = !S.editing;
  $('house').classList.toggle('dim-world', S.editing);
  $('houseState').textContent = (REAL ? 'Haunted House' : ROOMS[S.room]) + (S.editing ? ' · Editing' : '');
  document.querySelectorAll('#toolChips .chip').forEach((c) => c.classList.toggle('on', S.changes.some((x) => x.key === c.dataset.change)));
  $('revertBtn').disabled = !S.changes.length;
  $('publishBtn').disabled = !S.changes.length;
  const rows = [{ who: 'OpenAgents', what: 'Built the Haunted House', amt: '0 sats' }]
    .concat(S.changes.map((c) => ({ who: S.user || 'You (guest)', what: c.label + (S.published ? '' : ' · draft'), amt: '0 sats' })));
  $('log').innerHTML = '';
  for (const r of rows) {
    const e = document.createElement('div'); e.className = 'e';
    e.innerHTML = '<span></span><span class="amt"></span><span class="muted"></span><span></span>';
    e.children[0].textContent = r.what; e.children[1].textContent = r.amt; e.children[2].textContent = r.who; $('log').appendChild(e);
  }
}
function greetRoom() {
  for (const [key, n] of Object.entries(NPCS)) {
    if ((REAL ? true : n.room === S.room) && !S.greeted.has(key)) { S.greeted.add(key); setTimeout(() => npcSpeak(key, 'greet'), 400); if (REAL) break; }
  }
}
$('roomView').addEventListener('click', (e) => {
  const d = e.target.closest('[data-door]'); if (d) { S.room = +d.dataset.door; renderHouse(); greetRoom(); return; }
  const n = e.target.closest('[data-npc]'); if (n) { npcSpeak(n.dataset.npc, Math.random() < 0.5 ? 'hint' : 'emote'); if (S.editing) showCard('chat', true); else toast(NPCS[n.dataset.npc].name + ' says something in the chat'); }
});
addEventListener('keydown', (e) => {
  if (current !== 'house' || REAL || e.target.closest('input,textarea')) return;
  if ((e.key === 'ArrowUp' || e.key === 'w') && S.room < ROOMS.length - 1) { S.room++; renderHouse(); greetRoom(); }
  if ((e.key === 'ArrowDown' || e.key === 's') && S.room > 0) { S.room--; renderHouse(); greetRoom(); }
});
$('editBtn').onclick = () => { S.editing = true; ['tools', 'voice', 'chat', 'inventory', 'log'].forEach((c) => showCard(c, true)); renderHouse(); };
$('playBtn').onclick = () => { S.editing = false; renderHouse(); toast('Playing your version. Only you see it until you publish.'); };
$('compileBtn').onclick = () => toast(S.changes.length ? 'Compiled. Your change is in the world.' : 'Nothing to compile yet.');
$('revertBtn').onclick = () => revert();
function revert() { const c = S.changes.pop(); renderHouse(); if (c) say('houseAgent', 'Agent', 'Reverted: ' + c.label.toLowerCase() + '.', ''); toast('Reverted'); }
$('revertBig').onclick = () => { $('revertOverlay').hidden = true; revert(); };
$('revertKeep').onclick = () => { $('revertOverlay').hidden = true; };
function applyChange(change) {
  if (S.changes.some((c) => c.key === change.key)) return;
  if (!S.user && S.changes.length) { $('revertOverlay').hidden = false; return; }
  S.changes.push(change); renderHouse();
  say('houseAgent', 'Agent', 'Done: ' + change.label.toLowerCase() + '. Play it, or publish when you like it.');
}
$('toolChips').addEventListener('click', (e) => { const c = e.target.closest('[data-change]'); if (c) applyChange({ key: c.dataset.change, label: CHANGES[c.dataset.change] }); });
ITEMS.forEach(([icon, name]) => {
  const b = document.createElement('button'); b.innerHTML = '<span class="ico"></span><span></span>';
  b.children[0].textContent = icon; b.children[1].textContent = name;
  b.onclick = () => applyChange({ key: 'place:' + name, label: 'Placed a ' + name.toLowerCase() + ' in the ' + (REAL ? 'house' : ROOMS[S.room].toLowerCase()), icon, room: S.room });
  $('inv').appendChild(b);
});
document.querySelectorAll('.tabs button').forEach((b) => (b.onclick = () => {
  document.querySelectorAll('.tabs button').forEach((x) => x.classList.toggle('on', x === b));
  $('houseWorld').hidden = b.dataset.tab !== 'world'; $('houseAgent').hidden = b.dataset.tab !== 'agent';
  $('houseChatIn').placeholder = b.dataset.tab === 'world' ? 'Message the world (try @ghost)' : 'Ask the agent to change something';
}));
say('houseAgent', 'Agent', 'Tell me what to change, or pick a tool. I can also open or close any card: "close inventory", "show tools".');
say('houseWorld', 'Haunted House', 'You and three residents are here. Say hi, or name one: @ghost, @witch, @cat.', 'sys');
// The agent understands a small, fixed set of requests in this prototype.
function agentHandle(text) {
  const t = text.toLowerCase();
  const card = Object.keys(CARD_NAMES).find((k) => t.includes(CARD_NAMES[k]) || t.includes(k));
  if (card && /(close|hide|remove)/.test(t)) { showCard(card, false); return 'Closed the ' + CARD_NAMES[card] + '.'; }
  if (card && /(show|open|bring)/.test(t)) { showCard(card, true); return 'Opened the ' + CARD_NAMES[card] + '.'; }
  const k = t.includes('lantern') || t.includes('orange') ? 'lanterns' : t.includes('fog') ? 'fog' : t.includes('moon') || t.includes('window') ? 'moon' : t.includes('candle') || t.includes('light') ? 'candles' : null;
  if (k) { applyChange({ key: k, label: CHANGES[k] }); return null; }
  if (t.includes('revert') || t.includes('undo')) { revert(); return null; }
  if (t.includes('publish')) { $('publishBtn').click(); return null; }
  return 'In this prototype I can change the lanterns, candles, moon or fog, revert, publish, and open or close cards.';
}
$('houseChat').onsubmit = (e) => {
  e.preventDefault(); const i = $('houseChatIn'), text = i.value.trim(); if (!text) return; i.value = '';
  const agentTab = !$('houseAgent').hidden;
  say(agentTab ? 'houseAgent' : 'houseWorld', S.user || 'You', text, 'me');
  if (agentTab) { const r = agentHandle(text); if (r) setTimeout(() => say('houseAgent', 'Agent', r), 300); return; }
  const n = npcFromText(text);
  setTimeout(() => n ? npcSpeak(n[0], /hint|help|what|how/.test(text.toLowerCase()) ? 'hint' : 'answer') : npcSpeak(pick(Object.keys(NPCS)), 'emote'), 450);
};
$('listenBtn').onclick = () => {
  S.listening = !S.listening; $('wave').classList.toggle('off', !S.listening);
  $('listenState').textContent = S.listening ? 'Listening' : 'Not listening';
  $('listenBtn').textContent = S.listening ? 'Stop listening' : 'Start listening';
};
$('publishBtn').onclick = () => { if (!S.user) { $('loginOverlay').hidden = false; return; } publish(); };
$('pubGoogle').onclick = () => { S.user = 'you@gmail.com'; setWallet(); $('loginOverlay').hidden = true; toast('Logged in. Your change is still here.'); publish(); };
$('pubCancel').onclick = () => ($('loginOverlay').hidden = true);
function publish() { S.published = true; S.editing = false; renderHouse(); $('shareOverlay').hidden = false; }
$('toMap').onclick = () => { $('shareOverlay').hidden = true; go('map'); };

/* ---------- 08 planet setup ---------- */
document.querySelectorAll('[data-pick]').forEach((g) => g.addEventListener('click', (e) => {
  const c = e.target.closest('.chip'); if (!c) return; const was = c.classList.contains('on');
  g.querySelectorAll('.chip').forEach((x) => x.classList.remove('on')); if (!was) c.classList.add('on');
}));
document.querySelector('[data-for=env]').addEventListener('click', (e) => { const c = e.target.closest('.chip'); if (c) $('env').value = c.textContent; });
const chosen = (k) => document.querySelector(`[data-pick=${k}] .chip.on`)?.textContent || null;
$('generate').onclick = () => {
  const env = $('env').value.trim() || 'Forest', t = env.toLowerCase();
  planetKind = t.includes('forest') || t.includes('wood') ? 'forest' : t.includes('village') || t.includes('town') ? 'village' : t.includes('cemet') || t.includes('grave') ? 'cemetery' : 'other';
  S.planet = { name: env.replace(/\b\w/g, (m) => m.toUpperCase()), env, art: chosen('art') || 'Low poly', cam: chosen('cam') || 'First person', cycle: chosen('cycle') || 'Day/night cycle' };
  walk.build();
  go('planet');
};

/* ---------- 09 your planet: a small walkable world ---------- */
const walk = (() => {
  const cv = $('walk'), c = cv.getContext('2d'), off = document.createElement('canvas'), oc = off.getContext('2d');
  const P = { x: 0, z: -6, yaw: 0 }, keys = new Set(); let things = [], raf = 0, last = 0, look = null, stickStart = null, greetedNpc = false, t0 = performance.now();
  function rand(seed) { let s = seed; return () => ((s = (s * 16807) % 2147483647) / 2147483647); }
  function build() {
    if (!S.planet) S.planet = { name: 'Forest', env: 'Forest', art: 'Low poly', cam: 'First person', cycle: 'Day/night cycle' };
    const r = rand([...S.planet.env].reduce((a, ch) => a + ch.charCodeAt(0), 7));
    const type = { forest: 'tree', village: 'house', cemetery: 'grave', other: 'crystal' }[planetKind];
    things = [];
    for (let i = 0; i < 90; i++) { const a = r() * 6.28, d = 4 + r() * 36; things.push({ x: Math.cos(a) * d, z: Math.sin(a) * d, type, s: 0.7 + r() * 0.8 }); }
    const [npcName] = PLANET_NPC[planetKind]; things.push({ x: 0, z: 3, type: 'npc', s: 1, name: npcName });
    P.x = 0; P.z = -6; P.yaw = 0; greetedNpc = false;
    $('planetTitle').textContent = S.planet.name;
    $('planetDesc').textContent = [S.planet.art, S.planet.cam, S.planet.cycle].join(' · ');
    $('planetLink').value = 'openagents.com/v/' + S.planet.name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
    $('planetMsgs').innerHTML = '';
    say('planetMsgs', S.planet.name, 'You are on your planet. Share the link and friends appear here.', 'sys');
  }
  function sky(t) {
    if (S.planet.cycle === 'Always day') return 1; if (S.planet.cycle === 'Always night') return 0;
    return 0.5 + 0.5 * Math.sin(t / 20); // a day lasts about two minutes here
  }
  const PAL = { forest: ['#173622', '#0d1f14', '#2b5a36'], village: ['#4a321a', '#2a1d10', '#e8792b'], cemetery: ['#262b33', '#14171d', '#3a404a'], other: ['#2e2140', '#1a1222', '#7a5aa8'] };
  function drawThing(g, th, sx, sy, size, art) {
    const [fg, , hi] = PAL[planetKind];
    g.lineWidth = Math.max(1, size * 0.04);
    const ink = art === 'Ink';
    g.fillStyle = ink ? '#0a0a0a' : fg; g.strokeStyle = ink ? '#ededed' : 'rgba(0,0,0,.35)';
    const poly = (pts) => { g.beginPath(); pts.forEach(([x, y], i) => (i ? g.lineTo(sx + x * size, sy + y * size) : g.moveTo(sx + x * size, sy + y * size))); g.closePath(); g.fill(); if (ink || art === 'Painted') g.stroke(); };
    if (th.type === 'tree') { poly([[-0.5, 0], [0, -1.6], [0.5, 0]]); poly([[-0.4, -0.7], [0, -2.1], [0.4, -0.7]]); }
    else if (th.type === 'house') { poly([[-0.6, 0], [-0.6, -0.8], [0.6, -0.8], [0.6, 0]]); g.fillStyle = ink ? '#0a0a0a' : '#111'; poly([[-0.75, -0.75], [0, -1.35], [0.75, -0.75]]); g.fillStyle = ink ? '#ededed' : hi; poly([[-0.15, -0.55], [0.15, -0.55], [0.15, -0.3], [-0.15, -0.3]]); }
    else if (th.type === 'grave') { poly([[-0.3, 0], [-0.3, -0.6], [-0.2, -0.75], [0.2, -0.75], [0.3, -0.6], [0.3, 0]]); }
    else if (th.type === 'crystal') { g.fillStyle = ink ? '#0a0a0a' : hi; poly([[-0.3, 0], [-0.15, -1.1], [0.1, -1.4], [0.3, 0]]); }
    else if (th.type === 'npc') { g.fillStyle = '#ededed'; poly([[-0.22, 0], [-0.22, -0.7], [0.22, -0.7], [0.22, 0]]); g.beginPath(); g.arc(sx, sy - 0.85 * size, 0.18 * size, 0, 7); g.fill(); g.fillStyle = 'rgba(237,237,237,.85)'; g.font = `${Math.max(10, size * 0.16)}px Geist, system-ui`; g.textAlign = 'center'; g.fillText(th.name, sx, sy - 1.15 * size); }
  }
  function frame(now) {
    const dt = Math.min(0.05, (now - last) / 1000 || 0); last = now;
    const sp = 6 * dt, fx = Math.sin(P.yaw), fz = Math.cos(P.yaw);
    let mx = 0, mz = 0;
    if (keys.has('w') || keys.has('arrowup')) mz += 1; if (keys.has('s') || keys.has('arrowdown')) mz -= 1;
    if (keys.has('a')) mx -= 1; if (keys.has('d')) mx += 1;
    if (keys.has('arrowleft')) P.yaw -= 1.8 * dt; if (keys.has('arrowright')) P.yaw += 1.8 * dt;
    if (stickStart) { mz += -stickStart.dy / 60; mx += stickStart.dx / 60; }
    P.x = Math.max(-40, Math.min(40, P.x + (fx * mz + fz * mx) * sp));
    P.z = Math.max(-40, Math.min(40, P.z + (fz * mz - fx * mx) * sp));
    const art = S.planet.art, pixel = art === 'Pixel';
    const W = cv.clientWidth, H = cv.clientHeight, s = Math.min(devicePixelRatio || 1, 2);
    const rw = pixel ? Math.ceil(W / 5) : Math.floor(W * s), rh = pixel ? Math.ceil(H / 5) : Math.floor(H * s);
    if (off.width !== rw || off.height !== rh) { off.width = rw; off.height = rh; }
    if (cv.width !== Math.floor(W * s) || cv.height !== Math.floor(H * s)) { cv.width = W * s; cv.height = H * s; }
    const g = oc, w = rw, h = rh, t = (now - t0) / 1000, day = sky(t);
    const top = `rgb(${10 + 90 * day},${12 + 120 * day},${24 + 150 * day})`, bot = `rgb(${20 + 160 * day},${16 + 120 * day},${30 + 90 * day})`;
    const gr = g.createLinearGradient(0, 0, 0, h / 2); gr.addColorStop(0, top); gr.addColorStop(1, bot); g.fillStyle = gr; g.fillRect(0, 0, w, h / 2);
    const [, ground] = PAL[planetKind]; g.fillStyle = ground; g.fillRect(0, h / 2, w, h / 2);
    g.fillStyle = `rgba(0,0,0,${0.55 * (1 - day)})`; g.fillRect(0, h / 2, w, h / 2);
    if (S.planet.cam === 'Top down') {
      const sc = Math.min(w, h) / 90;
      g.fillStyle = ground; g.fillRect(0, 0, w, h);
      for (const th of things) { g.fillStyle = th.type === 'npc' ? '#ededed' : PAL[planetKind][0]; g.beginPath(); g.arc(w / 2 + (th.x - P.x) * sc, h / 2 + (th.z - P.z) * sc, (th.type === 'npc' ? 0.8 : 1.1 * th.s) * sc, 0, 7); g.fill(); }
      g.fillStyle = '#ededed'; g.beginPath(); g.arc(w / 2, h / 2, sc, 0, 7); g.fill();
    } else {
      const third = S.planet.cam === 'Third person';
      const cx = P.x - (third ? fx * 3 : 0), cz = P.z - (third ? fz * 3 : 0), f = h * 0.9;
      const vis = [];
      for (const th of things) {
        const dx = th.x - cx, dz = th.z - cz, rz = dx * fx + dz * fz, rx = dx * fz - dz * fx;
        if (rz > 0.4) vis.push({ th, rz, sx: w / 2 + (rx / rz) * f, sy: h / 2 + (1.6 / rz) * f, size: (f / rz) * th.s });
      }
      vis.sort((a, b) => b.rz - a.rz).forEach((v) => { g.globalAlpha = Math.max(0.15, Math.min(1, 1.6 - v.rz / 30)); drawThing(g, v.th, v.sx, v.sy, v.size, art); });
      g.globalAlpha = 1;
      if (third) { g.fillStyle = '#ededed'; g.fillRect(w / 2 - h * 0.025, h * 0.7, h * 0.05, h * 0.12); g.beginPath(); g.arc(w / 2, h * 0.68, h * 0.03, 0, 7); g.fill(); }
    }
    c.setTransform(1, 0, 0, 1, 0, 0); c.imageSmoothingEnabled = !pixel; c.drawImage(off, 0, 0, cv.width, cv.height);
    const npc = things.find((th) => th.type === 'npc');
    if (!greetedNpc && Math.hypot(npc.x - P.x, npc.z - P.z) < 4) { greetedNpc = true; say('planetMsgs', npc.name, PLANET_NPC[planetKind][1], 'npc'); }
    raf = requestAnimationFrame(frame);
  }
  addEventListener('keydown', (e) => { if (current === 'planet' && !e.target.closest('input,textarea')) { keys.add(e.key.toLowerCase()); if (e.key.startsWith('Arrow')) e.preventDefault(); } });
  addEventListener('keyup', (e) => keys.delete(e.key.toLowerCase()));
  cv.addEventListener('pointerdown', (e) => {
    cv.focus(); cv.setPointerCapture(e.pointerId);
    if (e.pointerType === 'touch' && e.clientX < innerWidth / 2) stickStart = { x: e.clientX, y: e.clientY, dx: 0, dy: 0 };
    else look = { x: e.clientX, yaw: P.yaw };
    $('walkHint').style.opacity = 0;
  });
  cv.addEventListener('pointermove', (e) => {
    if (stickStart) { stickStart.dx = Math.max(-60, Math.min(60, e.clientX - stickStart.x)); stickStart.dy = Math.max(-60, Math.min(60, e.clientY - stickStart.y)); }
    if (look) P.yaw = look.yaw + (e.clientX - look.x) * 0.006;
  });
  const end = () => { look = null; stickStart = null; };
  cv.addEventListener('pointerup', end); cv.addEventListener('pointercancel', end);
  return { build, start() { if (!things.length) build(); cancelAnimationFrame(raf); last = performance.now(); raf = requestAnimationFrame(frame); }, stop() { cancelAnimationFrame(raf); keys.clear(); } };
})();
let friendTimer;
function friendJoins() {
  clearTimeout(friendTimer);
  friendTimer = setTimeout(() => { say('planetMsgs', S.planet?.name || 'Planet', 'Sam joined from your link.', 'sys'); setTimeout(() => say('planetMsgs', 'Sam', 'whoa, this is your planet? walking over to you'), 900); }, 2500);
}
$('pChat').onsubmit = (e) => { e.preventDefault(); const i = $('pChatIn'), text = i.value.trim(); if (!text) return; say('planetMsgs', S.user || 'You', text, 'me'); i.value = ''; };

/* ---------- the survey ---------- */
document.querySelectorAll('.scale').forEach((sc) => {
  for (let n = 1; n <= 5; n++) { const l = document.createElement('label'); l.innerHTML = `<input type="radio" name="${sc.dataset.name}" value="${n}"> ${n}`; sc.appendChild(l); }
  const lo = document.createElement('span'); lo.className = 'small muted'; lo.textContent = '1 = not at all · 5 = very'; sc.appendChild(lo);
});
$('openSurvey').onclick = () => ($('surveyOverlay').hidden = false);
$('surveyCancel').onclick = () => ($('surveyOverlay').hidden = true);
$('survey').onsubmit = (e) => {
  e.preventDefault();
  const fd = new FormData($('survey')); S.survey = { easy: fd.get('easy'), place: fd.get('place'), stuck: fd.getAll('stuck'), make: fd.get('make'), broke: fd.get('broke'), share: fd.get('share') };
  if (!S.survey.easy || !S.survey.share) { toast('Answer the first question and the last one, then send.'); return; }
  $('surveyOverlay').hidden = true;
  const card = $('surveyCard').querySelector('.b');
  card.innerHTML = '<h3>Thank you.</h3><div class="muted">' + 'In this prototype your answers stay on this page.' + ' Share your planet with a friend while you are here.</div>';
  toast('Answers saved on this page');
};

/* ---------- boot ---------- */
setWallet();
const startAt = (location.hash || '').slice(1);
go(SCREENS.some((s) => s[0] === startAt) ? startAt : 'landing');
