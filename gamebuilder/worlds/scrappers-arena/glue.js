// Firm reason: 1 (thin browser HUD, keyboard/mouse input, and WebAudio synthesis glue for Scrappers Arena;
// all arena physics, bots, modes, scoring, and WebGPU draw calls are in Coax).
const $ = (id) => document.getElementById(id);
const ACT_EVENT = 90;
const MSG = { start: 120, lobby: 121, keys: 122, fire: 123, super: 124, view: 125, touch: 126, pause: 127 };

const MODES = [
  { name: "Knockouts", goal: "First to 10", desc: "Knock rivals out. First to 10 wins." },
  { name: "Last Stand", goal: "First to 3 rounds", desc: "No respawns per round. Last team standing takes it." },
  { name: "Scrapball", goal: "First to 3 goals", desc: "Grab the ball, carry it, and fire to kick into their goal." },
  { name: "Hoops", goal: "First to 5 baskets", desc: "Grab the ball and fire to lob it through their hoop." },
];
const CLASSES = ["Bolt", "Boulder", "Lumen"];

// Exact SOUND_SHAPES from gamebuilder/core/play.coax (id 1..12)
const SOUND_SHAPES = [
  null,
  [950, 320, 0.09, 0.2],   // 1 Bolt shot
  [240, 70, 0.20, 0.6],    // 2 Boulder blast
  [1600, 520, 0.24, 0.08], // 3 Lumen rifle
  [720, 1080, 0.05, 0.0],  // 4 Hit confirm
  [170, 90, 0.13, 0.5],    // 5 Hurt
  [520, 70, 0.42, 0.3],    // 6 Knockout
  [280, 1250, 0.36, 0.1],  // 7 Super
  [520, 1040, 0.70, 0.0],  // 8 Win / Score
  [420, 140, 0.70, 0.1],   // 9 Lose / Rival score
  [660, 660, 0.10, 0.0],   // 10 Countdown beep
  [990, 990, 0.28, 0.0],   // 11 GO!
  [300, 520, 0.08, 0.0],   // 12 Jump
];

let audioCtx = null;
let sfxBuffers = null;
let muted = false;

function ensureAudio() {
  if (muted) return null;
  if (!audioCtx) {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return null;
    audioCtx = new AC();
  }
  if (audioCtx.state === "suspended") audioCtx.resume().catch(() => {});
  if (!sfxBuffers) {
    const sr = audioCtx.sampleRate || 44100;
    sfxBuffers = [null];
    for (let id = 1; id < SOUND_SHAPES.length; id++) {
      const [from, to, sec, noise] = SOUND_SHAPES[id];
      const n = Math.max(1, Math.floor(sec * sr));
      const buf = audioCtx.createBuffer(1, n, sr);
      const out = buf.getChannelData(0);
      let phase = 0;
      let seed = 12345 + id;
      for (let i = 0; i < n; i++) {
        const t = i / n;
        phase += (from + (to - from) * t) / sr;
        if (phase > 1) phase -= 1;
        seed = (seed * 1103515245 + 12345) % 2147483648;
        const hiss = seed / 1073741824 - 1;
        const fade = (1 - t) * (1 - t) * Math.min(t * 100, 1);
        out[i] = (Math.sin(phase * 6.2831853) * (1 - noise) + hiss * noise) * fade * 0.32;
      }
      sfxBuffers.push(buf);
    }
  }
  return audioCtx;
}

function playSfxList(spec) {
  if (muted || !spec) return;
  const ctx = ensureAudio();
  if (!ctx || !sfxBuffers) return;
  for (const item of spec.split(",")) {
    const [idStr, loudStr] = item.split(":");
    const id = Number(idStr) | 0;
    const loud = Math.max(0, Math.min(100, Number(loudStr) || 0));
    const buf = sfxBuffers[id];
    if (!buf || loud <= 0) continue;
    const src = ctx.createBufferSource();
    src.buffer = buf;
    const gain = ctx.createGain();
    gain.gain.value = (loud / 100) * 0.65;
    src.connect(gain);
    gain.connect(ctx.destination);
    src.start();
  }
}

// ---- messaging to Coax engine

const queue = [];
const isReady = () => !!globalThis.coaxial?.ready && typeof globalThis.coaxial.send === "function";
function send(kind, text) {
  if (!isReady()) { queue.push([kind, String(text)]); return; }
  try { globalThis.coaxial.send(kind, String(text)); } catch {}
}
function drainQueue() {
  if (!isReady()) { requestAnimationFrame(drainQueue); return; }
  while (queue.length) { const [k, t] = queue.shift(); send(k, t); }
}
requestAnimationFrame(drainQueue);

// ---- UI state

let pickKind = 0;
let pickMode = 0;
let pickSize = 3;
let touchUI = matchMedia("(pointer:coarse)").matches || ((navigator.maxTouchPoints || 0) > 1 && /iPad|iPhone|iPod|Macintosh|Android/i.test(navigator.userAgent));
let hasMovedTouch = false;
let cur = { lb: 1, pa: 0, lk: 0, ov: 0, wn: -1, md: 0, sz: 3, kd: 0, hp: 100, sp: 0, s0: 0, s1: 0, tm: 1500, rs: 0, th: 0, mk: 0, ht: 0, od: 0, gl: 10, ct: 0, sh: 0, ft: 1, ko: 0, fl: 0, bl: 0, a0: 3, a1: 3, bt: 1, tc: 0, st: 0, sox: 0, soy: 0, skx: 0, sky: 0, fr: 0, fd: "" };
let held = 0, sentHeld = -1;
// Local low-latency mirror of active touch stick & buttons (mirrors gamebuilder/core/play.coax geometry)
const PLAY_STICK = 60;
const touchState = { stickId: null, ox: 0, oy: 0, dx: 0, dy: 0, fireId: null, jumpId: null };

function setTouchUI(on) {
  touchUI = !!on;
  document.body.classList.toggle("touch-ui", touchUI);
}

function hitCircle(x, y, cx, cy, r) {
  const dx = x - cx, dy = y - cy;
  return dx * dx + dy * dy <= r * r * 1.3;
}

function updateStickVisual() {
  const stick = $("sa-stick"), knob = $("sa-stick-knob");
  if (!stick || !knob) return;
  const localOn = touchState.stickId !== null;
  const active = localOn || cur.st === 1;
  stick.classList.toggle("active", active);
  if (localOn) {
    stick.style.left = `${touchState.ox}px`;
    stick.style.top = `${touchState.oy}px`;
    knob.style.transform = `translate(calc(-50% + ${touchState.dx}px), calc(-50% + ${touchState.dy}px))`;
  } else if (cur.st === 1 && (cur.sox > 0 || cur.soy > 0)) {
    stick.style.left = `${cur.sox}px`;
    stick.style.top = `${cur.soy}px`;
    knob.style.transform = `translate(calc(-50% + ${(cur.skx / 100) * PLAY_STICK}px), calc(-50% + ${(cur.sky / 100) * PLAY_STICK}px))`;
  } else {
    stick.style.left = "";
    stick.style.top = "";
    knob.style.transform = "translate(-50%, -50%)";
  }
  $("sa-btn-fire")?.classList.toggle("on", touchState.fireId !== null || cur.fr === 1);
  $("sa-btn-jump")?.classList.toggle("on", touchState.jumpId !== null);
}

function clearTouchVisuals() {
  touchState.stickId = null;
  touchState.dx = 0; touchState.dy = 0;
  touchState.fireId = null;
  touchState.jumpId = null;
  updateStickVisual();
}

const BITS = {
  KeyW: 1, ArrowUp: 1,
  KeyS: 2, ArrowDown: 2,
  KeyA: 4, ArrowLeft: 4,
  KeyD: 8, ArrowRight: 8,
  Space: 16,
  KeyQ: 32, KeyE: 32, KeyF: 32,
  KeyV: 64,
  KeyR: 128,
};

function flushKeys() {
  if (held === sentHeld) return;
  sentHeld = held;
  send(MSG.keys, held);
}

function updatePickerUI() {
  document.querySelectorAll("[data-kind]").forEach((el) => {
    const on = Number(el.dataset.kind) === pickKind;
    el.classList.toggle("on", on);
    el.setAttribute("aria-pressed", String(on));
  });
  document.querySelectorAll("[data-mode]").forEach((el) => {
    const on = Number(el.dataset.mode) === pickMode;
    el.classList.toggle("on", on);
    el.setAttribute("aria-pressed", String(on));
  });
  document.querySelectorAll("[data-size]").forEach((el) => {
    const on = Number(el.dataset.size) === pickSize;
    el.classList.toggle("on", on);
    el.setAttribute("aria-pressed", String(on));
  });
  const m = MODES[pickMode] || MODES[0];
  const note = $("sa-mode-desc");
  if (note) note.textContent = m.desc;
}

function startMatch() {
  ensureAudio();
  held = 0; sentHeld = -1;
  hasMovedTouch = false;
  clearTouchVisuals();
  send(MSG.start, `${pickKind} ${pickMode} ${pickSize} ${touchUI ? 1 : 0}`);
}

function openLobby() {
  held = 0; sentHeld = -1;
  clearTouchVisuals();
  try { if (document.pointerLockElement) document.exitPointerLock(); } catch {}
  send(MSG.lobby, "1");
}

function setPaused(p) {
  held = 0; sentHeld = -1;
  clearTouchVisuals();
  if (p) { try { if (document.pointerLockElement) document.exitPointerLock(); } catch {} }
  send(MSG.pause, p ? "1" : "0");
}

function fmtClock(tenths) {
  const s = Math.max(0, Math.ceil(tenths / 10));
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${m}:${r < 10 ? "0" : ""}${r}`;
}

function renderHud() {
  if (cur.tc === 1 && !touchUI) setTouchUI(true);

  const inLobby = cur.lb === 1;
  const paused = !inLobby && cur.pa === 1;
  const over = !inLobby && cur.ov === 1;
  document.body.classList.toggle("in-match", !inLobby);

  $("sa-lobby")?.classList.toggle("hide", !inLobby);
  $("sa-hud")?.classList.toggle("hide", inLobby);
  $("sa-pause")?.classList.toggle("hide", !paused);
  $("sa-over")?.classList.toggle("hide", !over);

  if (inLobby) return;

  const m = MODES[cur.md] || MODES[0];
  $("sa-s0").textContent = String(cur.s0);
  $("sa-s1").textContent = String(cur.s1);
  $("sa-clock").textContent = fmtClock(cur.tm);
  $("sa-mode-pill").textContent = cur.md === 1
    ? `${m.name.toUpperCase()} · RD ${cur.bt} · TO ${cur.gl}`
    : `${m.name.toUpperCase()} · ${cur.sz}v${cur.sz} · TO ${cur.gl}`;

  // Last Stand standing dots
  const standEl = $("sa-standing");
  if (standEl) {
    standEl.classList.toggle("hide", cur.md !== 1);
    if (cur.md === 1) {
      const dots = (alive, total) => "●".repeat(Math.max(0, alive)) + "○".repeat(Math.max(0, total - alive));
      standEl.innerHTML = `<span class="mine">${dots(cur.a0, cur.sz)}</span><span>ALIVE</span><span class="rival">${dots(cur.a1, cur.sz)}</span>`;
    }
  }

  // Ball carrier indicator
  const ballEl = $("sa-ball");
  if (ballEl) {
    ballEl.classList.toggle("hide", cur.bl !== 1);
    if (cur.bl === 1) {
      ballEl.textContent = touchUI
        ? (cur.md === 3 ? "HOLDING BALL — TAP THROW AT HOOP" : "HOLDING BALL — TAP KICK INTO GOAL")
        : (cur.md === 3 ? "HOLDING BALL — CLICK TO THROW AT HOOP" : "HOLDING BALL — CLICK TO KICK INTO GOAL");
    }
  }

  // Countdown (3, 2, 1)
  const countEl = $("sa-count");
  if (countEl) {
    if (cur.ct > 0 && !over) {
      countEl.classList.remove("hide");
      countEl.textContent = String(Math.ceil(cur.ct / 10));
    } else {
      countEl.classList.add("hide");
    }
  }

  // Feed banner (knockout / goal / basket / round)
  const feedEl = $("sa-feed");
  if (feedEl) {
    if (cur.fd) {
      feedEl.classList.remove("hide");
      feedEl.classList.toggle("mine", cur.ft === 1);
      feedEl.classList.toggle("rival", cur.ft !== 1);
      feedEl.textContent = cur.fd;
    } else {
      feedEl.classList.add("hide");
    }
  }

  // Respawn banner
  const respEl = $("sa-respawn");
  if (respEl) {
    if (cur.rs > 0 && !over) {
      respEl.classList.remove("hide");
      respEl.textContent = `KNOCKED OUT · BACK IN ${(cur.rs / 10).toFixed(1)}s`;
    } else {
      respEl.classList.add("hide");
    }
  }

  // Crosshair & hitmarker
  const xh = $("sa-crosshair");
  if (xh) {
    xh.classList.toggle("hide", over || paused || cur.rs > 0);
    xh.classList.toggle("hit", cur.mk > 5);
  }

  // Mouse lock prompt
  const lockEl = $("sa-lock-hint");
  if (lockEl) {
    lockEl.classList.toggle("hide", touchUI || cur.lk === 1 || paused || over);
  }

  // Touch HUD labels & states
  const fireBtn = $("sa-btn-fire");
  if (fireBtn) {
    fireBtn.textContent = cur.bl === 1 ? (cur.md === 3 ? "THROW" : "KICK") : "FIRE";
  }
  const superBtn = $("sa-btn-super");
  const superPct = $("sa-btn-super-pct");
  if (superBtn) superBtn.classList.toggle("ready", cur.sp >= 100 || cur.od === 1);
  if (superPct) superPct.textContent = cur.od === 1 ? "ON!" : cur.sp >= 100 ? "GO!" : `${Math.max(0, Math.min(100, cur.sp))}%`;
  const touchView = $("sa-btn-view");
  if (touchView) touchView.textContent = cur.th ? "3RD" : "1ST";
  const touchPill = $("sa-touch-pill");
  if (touchPill) touchPill.classList.toggle("hide", hasMovedTouch || over || paused || cur.rs > 0 || cur.bl === 1);
  updateStickVisual();

  // Health & Super bars
  const hp = Math.max(0, Math.min(100, cur.hp));
  const sp = Math.max(0, Math.min(100, cur.sp));
  const hpFill = $("sa-hp-fill");
  if (hpFill) {
    hpFill.style.width = `${hp}%`;
    hpFill.classList.toggle("low", hp <= 30);
    hpFill.classList.toggle("mid", hp > 30 && hp <= 60);
  }
  $("sa-hp-val").textContent = `${hp}%`;

  const spFill = $("sa-sp-fill");
  if (spFill) {
    spFill.style.width = `${cur.od ? 100 : sp}%`;
    spFill.classList.toggle("ready", sp >= 100 || cur.od === 1);
  }
  const spLabel = $("sa-sp-label");
  if (spLabel) {
    if (cur.od === 1) spLabel.textContent = "OVERDRIVE ACTIVE!";
    else if (sp >= 100) spLabel.textContent = "SUPER READY [Q / E / R-CLICK]";
    else spLabel.textContent = `SUPER ${sp}%`;
  }
  $("sa-class-tag").textContent = `${(CLASSES[cur.kd] || "Bolt").toUpperCase()}${cur.sh ? " · SHIELD" : ""}`;
  $("sa-stats-tag").textContent = `KO ${cur.ko} · FALLS ${cur.fl}`;
  const viewBtn = $("sa-view-btn");
  if (viewBtn) viewBtn.textContent = cur.th ? "3RD (V)" : "1ST (V)";

  if (over) {
    $("sa-over-title").textContent = cur.wn === 0 ? "VICTORY" : cur.wn === 1 ? "DEFEAT" : "DRAW";
    $("sa-over-title").className = cur.wn === 0 ? "win" : cur.wn === 1 ? "lose" : "draw";
    $("sa-over-score").textContent = `${cur.s0} — ${cur.s1}`;
    $("sa-over-meta").textContent = `${m.name} · ${CLASSES[cur.kd] || "Bolt"} · ${cur.ko} knockouts · ${cur.fl} falls`;
  }
}

// ---- receive HUD & sound events from Coax

addEventListener("coaxial", (e) => {
  const { code, text } = e.detail || {};
  if (code !== ACT_EVENT || typeof text !== "string" || !text.startsWith("hud|")) return;
  const next = { ...cur, fd: "" };
  let sfx = "";
  for (const part of text.slice(4).split(";")) {
    const i = part.indexOf("=");
    if (i < 1) continue;
    const k = part.slice(0, i);
    const v = part.slice(i + 1);
    if (k === "fd") next.fd = v;
    else if (k === "sx") sfx = v;
    else next[k] = Number(v);
  }
  // Sync picker selections when hash pre-selected them
  if (next.lb === 1 && cur.lb === 1 && next.md !== cur.md) { pickMode = next.md; updatePickerUI(); }
  if (next.lb === 1 && cur.lb === 1 && next.kd !== cur.kd) { pickKind = next.kd; updatePickerUI(); }
  if (next.lb === 1 && cur.lb === 1 && next.sz !== cur.sz) { pickSize = next.sz; updatePickerUI(); }
  cur = next;
  if (sfx) playSfxList(sfx);
  renderHud();
});

// ---- DOM wiring

document.querySelectorAll("[data-kind]").forEach((el) => {
  el.addEventListener("click", () => { pickKind = Number(el.dataset.kind) | 0; updatePickerUI(); });
});
document.querySelectorAll("[data-mode]").forEach((el) => {
  el.addEventListener("click", () => { pickMode = Number(el.dataset.mode) | 0; updatePickerUI(); });
});
document.querySelectorAll("[data-size]").forEach((el) => {
  el.addEventListener("click", () => { pickSize = Number(el.dataset.size) | 0; updatePickerUI(); });
});
$("sa-play-btn")?.addEventListener("click", startMatch);
$("sa-rematch-btn")?.addEventListener("click", startMatch);
$("sa-over-lobby-btn")?.addEventListener("click", openLobby);
$("sa-resume-btn")?.addEventListener("click", () => setPaused(false));
$("sa-pause-lobby-btn")?.addEventListener("click", openLobby);
$("sa-menu-btn")?.addEventListener("click", () => setPaused(true));
$("sa-view-btn")?.addEventListener("click", () => send(MSG.view, "1"));
$("sa-mute-btn")?.addEventListener("click", () => {
  muted = !muted;
  const b = $("sa-mute-btn");
  if (b) b.textContent = muted ? "SOUND OFF" : "SOUND ON";
});

// Keyboard handling
addEventListener("keydown", (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  if (e.code === "Enter" && cur.lb === 1) {
    e.preventDefault();
    startMatch();
    return;
  }
  if (e.code === "Escape") {
    if (cur.lb === 0) {
      e.preventDefault();
      if (cur.ov === 1) openLobby();
      else setPaused(cur.pa !== 1);
    }
    return;
  }
  if (cur.lb === 1 || cur.pa === 1) return;
  const b = BITS[e.code] || 0;
  if (!b) return;
  e.preventDefault();
  if (e.repeat) return;
  ensureAudio();
  held |= b;
  flushKeys();
});

addEventListener("keyup", (e) => {
  const b = BITS[e.code] || 0;
  if (!b) return;
  held &= ~b;
  flushKeys();
});

addEventListener("blur", () => { held = 0; sentHeld = -1; clearTouchVisuals(); send(MSG.keys, 0); send(MSG.fire, 0); });
addEventListener("contextmenu", (e) => { if (cur.lb === 0) e.preventDefault(); });

// Mirror live touch stick & right-side button states at display frame rate (matches play.coax zones)
addEventListener("pointerdown", (e) => {
  if (e.pointerType !== "mouse") {
    if (!touchUI) setTouchUI(true);
    ensureAudio();
  }
  if (e.pointerType === "mouse" || cur.lb === 1 || cur.pa === 1 || cur.ov === 1) return;
  if (e.target?.closest?.("[data-coaxial-dock]")) return;
  const x = e.clientX, y = e.clientY, w = innerWidth, h = innerHeight;
  if (hitCircle(x, y, w - 44, 112, 22)) return;
  if (hitCircle(x, y, w - 66, h - 214, 30)) {
    if (touchState.jumpId === null) touchState.jumpId = e.pointerId;
    updateStickVisual();
    return;
  }
  if (hitCircle(x, y, w - 196, h - 76, 34)) return;
  if (hitCircle(x, y, w - 92, h - 112, 46)) {
    if (touchState.fireId === null) touchState.fireId = e.pointerId;
    updateStickVisual();
    return;
  }
  if (x < w * 0.5 && touchState.stickId === null) {
    touchState.stickId = e.pointerId;
    touchState.ox = x;
    touchState.oy = y;
    touchState.dx = 0;
    touchState.dy = 0;
    updateStickVisual();
  }
}, { passive: true });

addEventListener("pointermove", (e) => {
  if (touchState.stickId !== e.pointerId) return;
  let dx = e.clientX - touchState.ox;
  let dy = e.clientY - touchState.oy;
  const mag = Math.hypot(dx, dy);
  if (mag > PLAY_STICK) { dx = (dx / mag) * PLAY_STICK; dy = (dy / mag) * PLAY_STICK; }
  if (mag > 12 && !hasMovedTouch) { hasMovedTouch = true; $("sa-touch-pill")?.classList.add("hide"); }
  touchState.dx = dx;
  touchState.dy = dy;
  updateStickVisual();
}, { passive: true });

function endTouch(e) {
  let changed = false;
  if (touchState.stickId === e.pointerId) { touchState.stickId = null; touchState.dx = 0; touchState.dy = 0; changed = true; }
  if (touchState.fireId === e.pointerId) { touchState.fireId = null; changed = true; }
  if (touchState.jumpId === e.pointerId) { touchState.jumpId = null; changed = true; }
  if (changed) updateStickVisual();
}
addEventListener("pointerup", endTouch, { passive: true });
addEventListener("pointercancel", endTouch, { passive: true });

setTouchUI(touchUI);
updatePickerUI();
renderHud();
