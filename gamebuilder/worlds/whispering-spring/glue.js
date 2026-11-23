// Firm reason: 1 (the page's half of The Whispering Spring: panels, journal, settings and sound;
// world-spring.coax decides everything that happens in the grove and tells this page in events).
import { SpringAudio } from "./audio.js";
import { GroveRoom, placeRibbons } from "./room.js";

const $ = (id) => document.getElementById(id);
const MSG = { mandala: 135, look: 134, pick: 120, stones: 121, ribbon: 122, pause: 123, settings: 124, save: 125, extra: 127, forget: 128, close: 129, shared: 131, speak: 132, decor: 133 };
const HIDDEN_KEY = "whispering-spring.hidden.v1", MUTED_KEY = "whispering-spring.muted.v1";
const readSet = (k) => { try { return new Set(JSON.parse(localStorage.getItem(k) || "[]")); } catch { return new Set(); } };
const keepSet = (k, s) => { try { localStorage.setItem(k, JSON.stringify([...s].slice(-500))); } catch {} };
const ACT_EVENT = 90;
const SETTINGS_KEY = "whispering-spring.settings.v1";

// ---- per-viewer settings (this browser only)
const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
const settings = { sound: true, music: 80, ambience: 60, captions: true, motion: reduce, bloom: false, text: false, voice: true, chat: false, night: false, toon: false };
try { Object.assign(settings, JSON.parse(localStorage.getItem(SETTINGS_KEY) || "{}")); } catch {}
const keepSettings = () => { try { localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings)); } catch {} };

// ---- words the page shows (the world sends ids)
const CLUE_TEXT = {
  pool: "The quail stops at the water's edge and lifts its head toward the pool.",
  stones: "The quail keeps scratching at the ground near the creek stones.",
  ribbon: "The quail goes still beneath the bare cedar branch, looking up.",
};
const HINT = {
  spring: "Listen to the water: touch the pool once, then wait.",
  cairn: "Something about the creek stones.",
  ribbon: "The bare branch looks as if it is waiting for something.",
};
const BIT = { spring: 1, cairn: 2, ribbon: 4 };
const EXTRA_OF = { spring: 0, cairn: 1, ribbon: 2 };
const STONES = [
  ["flat", "Flat stone", "wide and thin"], ["long", "Long stone", "narrow and long"], ["round", "Round stone", "rolls unless level"],
  ["wedge", "Wedge", "sits best tipped 15°"], ["pebble", "Pebble", "small; sits anywhere"],
];
const DECOR = [["Lantern", "\u{1F3EE}"], ["Candles", "\u{1F56F}\uFE0F"], ["Bluebonnets", "\u{1F33C}"], ["Crystal", "\u{1F48E}"], ["Sage bundle", "\u{1F33F}"], ["Offering bowl", "\u{1F963}"],
  ["Mother Mary", "\u{1F64F}"], ["Fountain", "\u26F2"], ["Buddha", "\u{1FAB7}"], ["Archangel Michael", "\u{1F47C}"], ["Serpent dragon", "\u{1F409}"], ["Bell (for the trees)", "\u{1F514}"], ["Lantern (for the trees)", "\u{1F3EE}"]];
const HANGS = new Set([11, 12]);
// the basket's shelves
const CATS = [["Items", [0, 1, 2, 3, 4, 5]], ["Statues", [6, 8, 9, 10, 7]], ["For the trees", [11, 12]], ["Terrain", []]];
const TOOLS = [["Paint tall grass", "\u{1F58C}\uFE0F"], ["Paint low grass", "\u{1F58C}\uFE0F"], ["Paint bare earth", "\u{1F58C}\uFE0F"]];
let decorCat = 0;
let decor = { n: 0, k: -1, ok: 0, m: -1, l: "", t: -1, g: 0, s: 0, i: [] };
const FABRICS = ["Cotton", "Silk", "Linen"];
const COLOURS = [["Prickly-pear rose", "#bc546b"], ["Bluebonnet", "#4d61b3"], ["Limestone cream", "#e0d6b8"], ["Sage", "#8c9e7a"], ["Persimmon", "#d97338"], ["Indigo", "#383873"]];
const CHARMS = ["None", "Bell", "Bead", "Feather"];
const BRANCHES = ["The low limb over the pool", "The long limb", "The high limb", "The right limb", "The far limb, by the bank", "The right high limb",
  "The woodland oak, low limb", "The woodland oak, high limb", "The meadow oak, low limb", "The meadow oak, high limb", "The golden field's oak, low limb", "The golden field's oak, high limb",
  "The oak past the mandala, low limb", "The oak past the mandala, high limb", "The far meadow's oak, low limb", "The far meadow's oak, high limb"];
const SLOT_AUTO = 7;
const QUAIL_TEXT = ["The quail forages, picking its way across the leaf litter.", "The quail scurries a few quick steps.", "The quail stops and scans the grove, head high.", "The quail pecks at the ground.", "The quail gives a soft call."];

// ---- the world, through the host
// The host is ready once its engine runs (coaxial.ready); until then messages wait here.
const waiting = [];
const ready = () => !!globalThis.coaxial?.ready && typeof globalThis.coaxial.send === "function";
function send(kind, text) {
  if (!ready()) { waiting.push([kind, String(text)]); return; }
  try { globalThis.coaxial.send(kind, String(text)); } catch {}
}
function flush() {
  if (!ready()) { requestAnimationFrame(flush); return; }
  while (waiting.length) { const [k, t] = waiting.shift(); send(k, t); }
}
requestAnimationFrame(flush);
// a phone (a touch screen that is small) gets light graphics unless the visitor turns them off
const phone = matchMedia("(pointer: coarse)").matches && Math.min(screen.width, screen.height) < 820;
if (settings.lite === undefined) settings.lite = phone;
const tellSettings = () => send(MSG.settings, `${settings.motion ? 1 : 0} ${settings.bloom ? 1 : 0} ${settings.night ? 1 : 0} ${settings.toon ? 1 : 0} ${settings.lite ? 1 : 0}`);

// ---- sound
let manifest = { tracks: [], extras: [], fragments: {}, stems: {}, ambience: {} };
const audio = new SpringAudio(manifest, { onCaption: (text) => caption(text) });
fetch("/whispering-spring/audio-manifest.json").then((r) => r.json()).then((m) => { manifest = m; audio.m = m; renderAlbum(); renderJournal(); renderDiscoveries(); }).catch(() => {});
const applySound = () => audio.setVolumes(settings.sound, settings.music / 100, (inGrove ? settings.ambience : 0) / 100);

// ---- state from the world
let state = { f: 0, x: 0, m: 0, s: 0, h: -1, o: 0, p: 31, a: 0, b: 0, w: 0, g: 0, y: 0, c: "", r: 0, rb: "", rw: "" };
let inGrove = false, paused = false, lastClue = "";
function parseState(text) {
  const next = { ...state, c: "", rb: "", rw: "" };
  for (const part of text.split(";")) {
    const i = part.indexOf("="); if (i < 1) continue;
    const k = part.slice(0, i), v = part.slice(i + 1);
    next[k] = ["c", "rb", "rw"].includes(k) ? v : Number(v);
  }
  return next;
}
// Each discovery reveals a page of lore (manifest.lore.pages); the album itself is the listening route.
const pageOf = (id) => (manifest.lore?.pages || []).find((p) => p.id === id);
const titleOf = (id) => pageOf(id)?.title || id;
const found = (id) => (state.f & BIT[id]) !== 0;

addEventListener("coaxial", (e) => {
  const { code, text } = e.detail || {};
  // an engine (re)loading: tell it the visitor's settings again
  if (code === 0) { tellSettings(); send(MSG.pause, paused ? "1" : "0"); flush(); return; }
  if (code !== ACT_EVENT || typeof text !== "string") return;
  const bar = text.indexOf("|");
  const kind = bar < 0 ? text : text.slice(0, bar), rest = bar < 0 ? "" : text.slice(bar + 1);
  if (kind === "ready") { tellSettings(); send(MSG.pause, paused ? "1" : "0"); flush(); }
  else if (kind === "tune") {
    const t = {}; for (const p of rest.split(";")) { const [k, v] = p.split("="); if (k) t[k] = Number(v) / 100; }
    audio.setTune(t);
  }
  else if (kind === "state") { const before = state; state = parseState(rest); onState(before); }
  else if (kind === "cue") { const bar2 = rest.indexOf("|"); const id = bar2 < 0 ? rest : rest.slice(0, bar2); audio.cue(id, bar2 < 0 ? "" : rest.slice(bar2 + 1)); if (id === "quail.call") quailSays(4); }
  else if (kind === "clue") { lastClue = rest; quailSays(-1, CLUE_TEXT[rest]); renderDiscoveries(); }
  else if (kind === "found") showFound(rest);
  else if (kind === "replayed") announce(`${titleOf(rest)} again.`);
  else if (kind === "mode") { if (rest === "ribbon") openRibbon(); else if (rest === "mandala") openMandala(); }
  else if (kind === "mandala") { const [k, r, a] = rest.split(",").map(Number); room.placePiece(k, r, a).then((x) => { if (!x.ok) announce(x.status === 429 ? "A little slower: the mandala needs a breath between pieces." : "That piece stays on your screen; the shared mandala couldn't be reached."); refreshMandala(); }).catch(() => {}); announce(`You set ${M_KINDS[k][0].toLowerCase()} on ring ${r}; it blooms eight times around.`); }
  else if (kind === "pan") { zoneFor(Number(rest)); }
  else if (kind === "statue") { const [k, x, y] = rest.split(",").map(Number); statueSpeak(k, x, y); }
  else if (kind === "picked") { const nm = DECOR[Number(rest)]?.[0] || "It"; toast(`${DECOR[Number(rest)]?.[1] || ""} ${nm} → inventory`); announce(`${nm} is in your inventory.`); }
  else if (kind === "wrap") { const f = $("fade"); f.classList.remove("go"); void f.offsetWidth; f.classList.add("go"); }
  else if (kind === "ribbonat") { ribbonAt = Number(rest); }
  else if (kind === "hold") { const s = STONES.find((x) => x[0] === rest); announce(`${s ? s[1] : "A stone"} in your hand. ${touchy ? "Tap" : "Click"} anywhere to set it on the cairn.`); }
  else if (kind === "decor") {
    if (rest === "blocked") { announce("It can't stand there. Try the grass."); $("decor-status").textContent = "It can't stand there. Try the grass."; }
    else if (rest === "full") announce("The grove holds thirty-six things at most. Pick one up first.");
    else if (rest === "none") announce("None of those in your inventory. Find one in the grove and pick it up.");
    else { const d = {}; for (const p of rest.split(";")) { const i = p.indexOf("="); if (i > 0) d[p.slice(0, i)] = p.slice(i + 1); } decor = { n: Number(d.n) || 0, k: Number(d.k), ok: Number(d.ok), m: Number(d.m), l: d.l || "", t: Number(d.t ?? -1), g: Number(d.g) || 0, s: Number(d.s) || 0, i: (d.i || "").split(",").map(Number) };
      renderInventory(); renderDecor(); renderHint(); }
  }
  else if (kind === "ribbon") { const [l, s, mine] = rest.split(",").map(Number); showRibbon(l, s, mine === 1); }
  else if (kind === "speak") quailSpeak("prayers");
  else if (kind === "cpos") { const [sp, x, y] = rest.split(",").map(Number); placeCallout(sp, x, y); }
  else if (kind === "qpos") { const [x, y] = rest.split(",").map(Number); placeBubble(x, y); }
});

let stockAsked = false;
function onState(before) {
  if (!stockAsked) { stockAsked = true; send(MSG.decor, "S"); }
  if (freshVisit) { freshVisit = false; setTimeout(startOver, 1200); }
  renderDiscoveries(); renderJournal(); renderText(); renderMini(); renderHint();
  if (!$("decor").hidden && state.m !== 3 && before.m === 3 && decor.k < 0) close("decor", false);
  if (decor.k < 0 && decor.t < 0 && state.m === 3 && $("inventory").hidden && $("decor").hidden) send(MSG.decor, "q");
  const any = state.f || state.c || state.r;
  $("enter").textContent = any ? "Return to the Grove" : "Enter the Grove";
  // a ribbon just tied: hang it on the shared tree at the tie point the world chose, and the quail blesses it
  if (pendingTie && state.r && state.rb) {
    const [fabric, colour, charm, b] = state.rb.split(",").map(Number);
    const tie = { fabric, colour, charm, limb: Math.floor(b / 8), slot: b % 8, word: state.rw || "", share: pendingTie.share };
    pendingTie = null;
    room.tie(tie).then((r) => { if (!r.ok) announce(r.status === 429 ? "The tree is busy; your ribbon stays here and will join the shared tree next time." : "Your ribbon is tied here; the shared tree could not be reached."); refreshRibbons(); }).catch(() => announce("Your ribbon is tied here; the shared tree could not be reached."));
    setTimeout(() => quailSpeak("ribbon"), 2600);
  }
  if ((state.f & 7) === 7 && (before.f & 7) !== 7 && before.f !== undefined) setTimeout(() => quailSpeak("found_all"), 4000);
}

// ---- the in-grove frame: the small inventory card
const SHORT = { spring: "Spring", cairn: "Stones", ribbon: "Wind" };
// No quest card and no callout: the grove is found, not instructed. (The world still says where the
// next spot is; nothing shows it.)
function placeCallout() {}
function renderMini() {
  const ul = $("mini-list"); if (!ul) return; ul.textContent = "";
  for (const id of ["spring", "cairn", "ribbon"]) {
    const li = document.createElement("li");
    if (found(id)) { const m = document.createElement("span"); m.className = "found-mark"; m.textContent = "✦"; li.append(m, document.createTextNode(SHORT[id])); }
    else { const seal = document.createElement("span"); seal.className = "seal"; seal.setAttribute("aria-hidden", "true"); li.append(seal); const sr = document.createElement("span"); sr.className = "sr"; sr.textContent = "Not yet found"; li.append(sr); }
    ul.append(li);
  }
}
$("minijournal").addEventListener("click", () => open("inventory"));
function setMenu(openIt) { $("menu").hidden = !openIt; $("t-menu").setAttribute("aria-expanded", String(openIt)); if (openIt) $("menu").querySelector("button")?.focus(); }
$("t-menu").addEventListener("click", () => setMenu($("menu").hidden));
for (const b of $("menu").querySelectorAll("button")) b.addEventListener("click", () => setMenu(false));
$("t-quiet").addEventListener("click", () => { const q = !document.body.classList.contains("quiet"); document.body.classList.toggle("quiet", q); $("t-quiet").setAttribute("aria-pressed", String(q)); announce(q ? "Quiet view: only the grove." : "The quest and journal are back."); });
$("g-sound").addEventListener("click", () => { audio.start(); setSetting("sound", !settings.sound); });

// ---- the statues speak when tapped (placeholder words for the artist to rewrite)
const STATUE_LINES = {
  6: ["Peace to you, little one. Rest here a while.", "Every prayer on that tree, I hold close.", "The water remembers your name."],
  7: ["Listen. Each drop is a word the spring forgot.", "Make a wish. I keep them all, and tell no one.", "I only know one song, and I sing it all night."],
  8: ["Breathe in the grove. Breathe out the hurry.", "The stone does not strive, and still it holds the stack.", "Sit. Nothing here needs you to be anything."],
  9: ["I keep watch at dusk, so you can rest.", "Courage is a quiet thing. Like a quail.", "No shadow crosses this grove unseen."],
  10: ["Hssss. A visitor. I like your ribbons.", "I guard the pearl of the spring. You may look.", "Long ago, I was a river."],
};
const statueSaid = {};
let statueTimer = 0;
function statueSpeak(k, x, y) {
  const lines = STATUE_LINES[k]; if (!lines) return;
  const line = lines[(statueSaid[k] = (statueSaid[k] ?? -1) + 1) % lines.length];
  const name = DECOR[k][0];
  const b = $("bubble"), t = $("bubble-text");
  b.querySelector(".who").textContent = name;
  b.style.left = `${Math.max(180, Math.min(innerWidth - 180, x))}px`;
  b.style.top = `${Math.max(140, y)}px`;
  t.textContent = ""; b.classList.remove("gone"); bubbleShown = true; statueTalking = true;
  announce(`${name} says: ${line}`);
  const chars = [...line];
  const seconds = settings.voice && settings.sound
    ? audio.speak(line, (i) => { t.textContent = chars.slice(0, i + 1).join(""); })
    : (chars.forEach((_, i) => setTimeout(() => { t.textContent = chars.slice(0, i + 1).join(""); }, i * (settings.motion ? 0 : 30))), chars.length * 0.03 + 0.2);
  clearTimeout(bubbleTimer); clearTimeout(statueTimer);
  statueTimer = setTimeout(() => { b.classList.add("gone"); bubbleShown = false; statueTalking = false; b.querySelector(".who").textContent = "The quail"; }, (seconds + 2.5) * 1000);
}
let statueTalking = false;

// ---- the quail speaks: its words type into a bubble over it, in its babble voice
const said = { greeting: 0, prayers: 0, ribbon: 0, found_all: 0 };
let bubbleTimer = 0, bubbleShown = false;
function placeBubble(x, y) {
  if (!bubbleShown || statueTalking) return;
  const b = $("bubble");
  b.style.left = `${Math.max(180, Math.min(innerWidth - 180, x))}px`;
  b.style.top = `${Math.max(140, y)}px`;
}
function quailSpeak(kind) {
  const lines = manifest.voice?.[kind];
  if (!Array.isArray(lines) || !lines.length || !inGrove) return;
  const line = lines[said[kind]++ % lines.length];
  const b = $("bubble"), t = $("bubble-text");
  b.querySelector(".who").textContent = "The quail"; statueTalking = false;
  bubbleShown = true;
  t.textContent = "";
  if (!b.style.left) { b.style.left = "35%"; b.style.top = "55%"; }
  b.classList.remove("gone");
  announce(`The quail says: ${line}`);
  const chars = [...line];
  const voice = settings.voice && settings.sound;
  const seconds = voice
    ? audio.speak(line, (i) => { t.textContent = chars.slice(0, i + 1).join(""); })
    : (chars.forEach((_, i) => setTimeout(() => { t.textContent = chars.slice(0, i + 1).join(""); }, i * (settings.motion ? 0 : 30))), chars.length * 0.03 + 0.2);
  send(MSG.speak, String(Math.min(30, Math.ceil(seconds + 0.8))));
  clearTimeout(bubbleTimer);
  bubbleTimer = setTimeout(() => { b.classList.add("gone"); bubbleShown = false; }, (seconds + 2.5) * 1000);
}

// ---- the shared grove: everyone's ribbons and the chat
const room = new GroveRoom();
let pendingTie = null, chatOpen = false, unread = 0, othersHere = 0, sharedRibbons = [];
const hiddenRibbons = readSet(HIDDEN_KEY), mutedNames = readSet(MUTED_KEY);
async function refreshRibbons() {
  try {
    const all = await room.ribbons();
    sharedRibbons = all.filter((r) => !hiddenRibbons.has(r.n));
    const rows = sharedRibbons.filter((r) => !r.mine).map((r) => `${r.fabric},${r.colour},${r.charm},${r.limb},${r.slot}`).join(";");
    send(MSG.shared, rows);
    renderPrayers();
  } catch { renderPrayers(true); }
}
function renderPrayers(offline = false) {
  const ul = $("prayers-list"); ul.textContent = "";
  const others = sharedRibbons.filter((r) => !r.mine);
  $("prayers-note").textContent = offline ? "The shared tree can't be reached right now; your own ribbon still hangs here."
    : others.length ? `${others.length} ribbon${others.length === 1 ? "" : "s"} from other visitors, newest first. Hide any you'd rather not see.` : "No one else has tied a ribbon yet.";
  for (const r of others) {
    const li = document.createElement("li");
    const p = document.createElement("p");
    const cw = COLOURS[r.colour][0].toLowerCase();
    p.textContent = `${r.name || "A visitor"} tied ${/^[aeiou]/.test(cw) ? "an" : "a"} ${cw} ${FABRICS[r.fabric].toLowerCase()} ribbon on ${BRANCHES[r.limb].toLowerCase()}${r.charm ? `, with a ${CHARMS[r.charm].toLowerCase()}` : ""}.`;
    const mini = document.createElement("div"); mini.className = "pr-mini"; mini.setAttribute("aria-hidden", "true");
    mini.innerHTML = `<svg viewBox="0 0 240 420" width="70" height="122">${ribbonSvg(r, `p${r.n}`)}</svg>`;
    li.classList.add("pr-item"); li.append(mini);
    const body = document.createElement("div"); li.append(body);
    body.append(p);
    if (r.word && !mutedNames.has(r.name)) { const w = document.createElement("p"); w.className = "lyric"; w.textContent = r.word; body.append(w); }
    const hide = document.createElement("button"); hide.type = "button"; hide.className = "btn small"; hide.textContent = "Hide this ribbon";
    hide.addEventListener("click", () => { hiddenRibbons.add(r.n); keepSet(HIDDEN_KEY, hiddenRibbons); refreshRibbons(); announce("Hidden."); });
    const look = document.createElement("button"); look.type = "button"; look.className = "btn small"; look.textContent = "See it up close";
    look.addEventListener("click", () => showRibbon(r.limb, r.slot, false));
    const row = document.createElement("div"); row.className = "row"; row.append(look, hide);
    body.append(row); ul.append(li);
  }
}
// ---- a ribbon up close: drawn big as it was tied (its fabric, colour and charm), its words and who tied it
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
function ribbonSvg(r, uid = "c") {
  const hex = COLOURS[r.colour]?.[1] || "#bc546b";
  const weave = r.fabric === 1
    ? `<linearGradient id="rcf${uid}" x1="0" x2="1"><stop offset="0" stop-color="#000" stop-opacity=".25"/><stop offset=".35" stop-color="#fff" stop-opacity=".35"/><stop offset=".55" stop-color="#fff" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity=".3"/></linearGradient>`
    : r.fabric === 2
      ? `<pattern id="rcf${uid}" width="9" height="7" patternUnits="userSpaceOnUse"><path d="M0 2h9M0 5.5h9" stroke="#fff" stroke-opacity=".13" stroke-width="1.2"/><path d="M3 0v7M7 0v7" stroke="#000" stroke-opacity=".12" stroke-width="1"/></pattern>`
      : `<pattern id="rcf${uid}" width="5" height="5" patternUnits="userSpaceOnUse"><path d="M0 0h5v1H0zM0 0h1v5H0z" fill="#000" fill-opacity=".14"/></pattern>`;
  const charm = r.charm === 1 ? `<g transform="translate(120 368)"><path d="M-16 0q0-24 16-26 16 2 16 26z" fill="#e7b75a" stroke="#7a5420" stroke-width="2"/><rect x="-19" y="-1" width="38" height="6" rx="3" fill="#c9952f"/><circle cy="10" r="5" fill="#7a5420"/></g>`
    : r.charm === 2 ? `<g transform="translate(120 366)"><circle r="15" fill="#3f6fa8"/><circle r="15" fill="url(#rcb${uid})"/><circle cx="-5" cy="-5" r="4" fill="#fff" fill-opacity=".6"/></g>`
    : r.charm === 3 ? `<g transform="translate(120 356) rotate(8)"><path d="M0-6C14 10 12 40 0 58C-12 40-14 10 0-6z" fill="#efe5d0" stroke="#a89878" stroke-width="1.5"/><path d="M0-4V60" stroke="#8d7d5d" stroke-width="1.5"/></g>` : "";
  const word = esc(r.word || "");
  // the ribbon's script: little glyphs drawn from its words (each letter picks one), down its middle.
  // Up close they lift off as sparks, fly out into a ring, and the ring becomes the words, turning
  // slowly round the ribbon like a halo.
  const raw = String(r.word || "");
  const n = Math.max(5, Math.min(10, Math.ceil(raw.length / 2) || 5));
  const centre = (y) => 126 + 12 * Math.sin(((y - 90) / 210) * Math.PI);
  const HX = 120, HY = 190, HR = 104;
  let glyphs = "", sparks = "";
  for (let i = 0; i < n; i++) {
    const code = raw ? raw.charCodeAt((i * 7) % raw.length) + i : 37 + i * 13;
    const y = 84 + i * (230 / (n - 1 || 1)), x = centre(y);
    const rot = ((code * 37) % 30) - 15;
    glyphs += `<g class="rc-g" style="--i:${i}" transform="translate(${(x - 8).toFixed(1)} ${(y - 8).toFixed(1)}) rotate(${rot} 8 8)"><path d="${GLYPHS[code % GLYPHS.length]}"/></g>`;
    if (raw) for (let p = 0; p < 6; p++) {
      const a = ((i * 6 + p) / (n * 6)) * Math.PI * 2 - Math.PI / 2;
      const tx = HX + Math.cos(a) * HR - x, ty = HY + Math.sin(a) * HR - y;
      sparks += `<circle class="rc-p" cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="${(1.6 + (p % 3) * 0.6).toFixed(1)}" style="--i:${i};--tx:${tx.toFixed(1)}px;--ty:${ty.toFixed(1)}px"/>`;
    }
  }
  let ring = raw ? `${raw}  ✦  ` : "✦  ·  ";
  while (ring.length < 84) ring += raw ? `${raw}  ✦  ` : "✦  ·  ";
  const halo = `<g class="rc-halo${raw ? "" : " quiet"}"><text font-family="Georgia,serif" font-style="italic" font-size="15" fill="#ffe6b0"><textPath href="#rchalo${uid}">${esc(ring.slice(0, 96))}</textPath></text></g>`;
  return `<defs>${weave}<radialGradient id="rcb${uid}"><stop offset="0" stop-color="#fff" stop-opacity=".2"/><stop offset="1" stop-color="#000" stop-opacity=".35"/></radialGradient>
    <path id="rchalo${uid}" d="M${HX} ${HY} m-${HR} 0 a${HR} ${HR} 0 1 1 ${HR * 2} 0 a${HR} ${HR} 0 1 1 -${HR * 2} 0"/></defs>
    ${halo}
    <rect x="20" y="22" width="200" height="16" rx="8" fill="#5a3a24"/><path d="M30 30h180" stroke="#3a2416" stroke-width="2"/>
    <path d="M104 30 C 92 60, 130 70, 116 92 L 110 330 C 150 300, 150 250, 160 200 C 168 150, 132 110, 136 30 Z" fill="${hex}"/>
    <path d="M104 30 C 92 60, 130 70, 116 92 L 110 330 C 150 300, 150 250, 160 200 C 168 150, 132 110, 136 30 Z" fill="url(#rcf${uid})"/>
    <path d="M110 330 l-10 22 18-10 4 20 8-30" fill="${hex}"/>
    <circle cx="120" cy="32" r="10" fill="${hex}" stroke="#00000033" stroke-width="2"/>
    <g class="rc-script" fill="none" stroke="#fff8ec" stroke-opacity=".9" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">${glyphs}</g>
    <g class="rc-sparks" fill="#ffe6b0">${sparks}</g>
    ${charm}`;
}
// the ribbon script's glyphs, each in a 16-unit box: spiral, eye, quail, wave, sun, drop, star, feather
const GLYPHS = [
  "M8 8m-1 0a1 1 0 1 1 2 0a3 3 0 1 1-6 0a5 5 0 1 1 10 0",
  "M1 8q7-7 14 0q-7 7-14 0zM6 8a2 2 0 1 0 4 0a2 2 0 1 0-4 0",
  "M2 11q4-6 9-3l3-2-1 3q-2 4-8 3zM9 6q0-3 2-4",
  "M1 10q2-4 4 0t4 0t4 0t3-1",
  "M5 8a3 3 0 1 0 6 0a3 3 0 1 0-6 0M8 1v2M8 13v2M1 8h2M13 8h2",
  "M8 2q5 6 0 12q-5-6 0-12z",
  "M8 1l2 5h5l-4 3 2 5-5-3-5 3 2-5-4-3h5z",
  "M3 13q0-10 10-10q0 10-10 10zM3 13l7-7",
];
let shownRibbon = null;
function showRibbon(limb, slot, mine) {
  const r = mine
    ? (() => { const [fabric, colour, charm] = (state.rb || "0,0,0").split(",").map(Number); return { fabric, colour, charm, limb, slot, word: state.rw || "", name: $("s-name").value || "You", mine: true }; })()
    : sharedRibbons.find((x) => !x.mine && x.limb === limb && x.slot === slot);
  if (!r) return;
  shownRibbon = r;
  const colour = COLOURS[r.colour][0], fabric = FABRICS[r.fabric];
  $("rc-svg").innerHTML = ribbonSvg(r);
  $("rc-limb").textContent = BRANCHES[r.limb] || "On the prayer tree";
  $("rc-title").textContent = `${colour} ${fabric.toLowerCase()}${r.charm ? `, with a ${CHARMS[r.charm].toLowerCase()}` : ""}`;
  const muted = r.name && mutedNames.has(r.name);
  $("rc-word").textContent = muted ? "" : r.word ? `“${r.word}”` : "";
  $("rc-word").hidden = muted || !r.word;
  const when = r.at ? new Date(r.at < 1e12 ? r.at * 1000 : r.at).toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" }) : "";
  $("rc-meta").textContent = `${mine ? "Tied by you" : `Tied by ${r.name || "a visitor"}`}${when ? ` · ${when}` : ""}${!r.word ? " · no words, or kept private" : ""}`;
  $("rc-hide").hidden = !!mine;
  $("ribbon-card").hidden = false;
  announce(`${mine ? "Your" : `${r.name || "A visitor"}'s`} ribbon: ${colour} ${fabric}${r.word && !muted ? `. It says: ${r.word}` : ""}.`);
  $("rc-close").focus();
}
const closeRibbonCard = () => { $("ribbon-card").hidden = true; shownRibbon = null; };
$("rc-close").addEventListener("click", closeRibbonCard);
$("ribbon-card").addEventListener("click", (e) => { if (e.target === $("ribbon-card")) closeRibbonCard(); });
$("rc-hide").addEventListener("click", () => { if (!shownRibbon) return; hiddenRibbons.add(shownRibbon.n); keepSet(HIDDEN_KEY, hiddenRibbons); closeRibbonCard(); refreshRibbons(); announce("Hidden."); });

// ---- the mandala clearing: an overhead view where everyone builds one design together
const M_KINDS = [["Rose petals", "#e05a73"], ["Marigold petals", "#f6ae2d"], ["Bluebonnets", "#4d61e0"], ["Crystals", "#b48cf0"], ["Candles", "#ffe0a0"], ["River stones", "#9a968f"], ["Leaves", "#5a8a33"], ["Pearls", "#fff6ea"]];
let mandalaKind = 0, mandalaPieces = [];
async function refreshMandala() {
  try {
    mandalaPieces = await room.mandala();
    send(MSG.mandala, "s" + mandalaPieces.map((p) => `${p.kind},${p.ring},${p.a}`).join(";"));
    const people = new Set(mandalaPieces.map((p) => p.name)).size;
    $("m-count").textContent = mandalaPieces.length ? `${mandalaPieces.length} pieces from ${people} ${people === 1 ? "person" : "people"}, each blooming eightfold.` : "The clearing is bare. Set the first piece.";
  } catch { $("m-count").textContent = "The shared mandala can't be reached right now; what you set stays on your screen."; }
}
function renderMandalaTray() {
  const box = $("m-kinds");
  if (!box.children.length) M_KINDS.forEach(([name, hex], k) => {
    const b = document.createElement("button"); b.type = "button"; b.className = "btn m-kind"; b.dataset.k = k;
    b.innerHTML = `<span class="dot" style="background:${hex}" aria-hidden="true"></span>${name}`;
    b.addEventListener("click", () => { mandalaKind = k; send(MSG.mandala, `k ${k}`); renderMandalaTray(); });
    box.append(b);
  });
  for (const b of box.children) b.setAttribute("aria-pressed", String(Number(b.dataset.k) === mandalaKind));
}
function openMandala() { renderMandalaTray(); send(MSG.mandala, `k ${mandalaKind}`); $("mandala").hidden = false; $("m-join").hidden = true; refreshMandala(); $("mandala").focus(); }
function closeMandala() { if ($("mandala").hidden) return; $("mandala").hidden = true; send(MSG.mandala, "x"); }
$("m-leave").addEventListener("click", closeMandala);
$("m-clear").addEventListener("click", async () => {
  if (!confirm("Clear the mandala for everyone and start a fresh design?")) return;
  try { const r = await room.clearMandala(); if (!r.ok) announce("The mandala couldn't be cleared just now."); } catch { announce("The shared mandala can't be reached right now."); }
  refreshMandala(); announce("The clearing is bare again.");
});
$("m-place").addEventListener("click", () => send(MSG.mandala, `p ${$("m-ring").value} ${Math.round(Number($("m-angle").value) / 360 * 64) % 64}`));
$("m-ring").addEventListener("input", () => { $("m-ring-out").textContent = $("m-ring").value; });
$("m-angle").addEventListener("input", () => { $("m-angle-out").textContent = `${$("m-angle").value}°`; });

// ---- where along the grove the view is: a quiet name for the place, and a way into the mandala
const ZONES = [[-54, "The Open Field"], [-38, "The Old Woods"], [-23, "The Mandala Clearing"], [-8, "The Woodland"], [8, "The Spring"], [22, "The Meadow"], [44, "The Golden Field"], [999, "The Open Field"]];
let zoneShown = "";
function zoneFor(x) {
  const name = ZONES.find(([edge]) => x < edge)[1];
  $("m-join").hidden = !(x < -23 && x > -38) || !$("mandala").hidden;
  if (name === zoneShown) return;
  zoneShown = name;
  const z = $("zone"); z.textContent = name; z.classList.remove("gone"); void z.offsetWidth; z.classList.add("show");
  clearTimeout(zoneFor.t); zoneFor.t = setTimeout(() => z.classList.remove("show"), 2600);
}
$("m-join").addEventListener("click", () => { audio.start(); send(MSG.mandala, "e"); });
$("look-l").addEventListener("click", () => send(MSG.look, "p -60"));
$("look-r").addEventListener("click", () => send(MSG.look, "p 60"));

function chatLine(m) {
  if (mutedNames.has(m.name)) return;
  const li = document.createElement("li"); if (m.mine) li.className = "mine";
  const who = document.createElement("button"); who.type = "button"; who.className = "who"; who.textContent = m.name || "Guest";
  who.title = m.mine ? "You" : `Hide messages from ${m.name}`;
  if (!m.mine) who.addEventListener("click", () => { if (confirm(`Hide messages and words from ${m.name} on this device?`)) { mutedNames.add(m.name); keepSet(MUTED_KEY, mutedNames); li.remove(); renderPrayers(); } });
  li.append(who, document.createTextNode(`: ${m.text}`));
  $("chat-log").append(li);
  while ($("chat-log").children.length > 120) $("chat-log").firstChild.remove();
}
function chatNote(text) { const li = document.createElement("li"); li.className = "note"; li.textContent = text; $("chat-log").append(li); $("chat-log").scrollTop = 1e9; }
async function pollChat() {
  try {
    const fresh = await room.chatSince();
    for (const m of fresh) chatLine(m);
    if (fresh.length) {
      $("chat-log").scrollTop = 1e9;
      if (!chatOpen) { unread += fresh.filter((m) => !m.mine).length; $("chat-unread").textContent = unread ? `(${unread})` : ""; }
    }
  } catch { if (!room.offlineSaid) { room.offlineSaid = true; chatNote("The grove chat can't be reached right now."); } }
}
async function pollPresence() {
  try { const p = await room.presence(); if (p) { othersHere = p.others; $("chat-count").textContent = othersHere ? `${othersHere + 1} here` : "just you"; if (p.you?.name && !$("s-name").value) $("s-name").placeholder = p.you.name; } } catch {}
}
function setChat(open) {
  chatOpen = open; $("chat").hidden = !open; $("t-chat").setAttribute("aria-expanded", String(open));
  if (open) { unread = 0; $("chat-unread").textContent = ""; $("chat-log").scrollTop = 1e9; }
  settings.chat = open; keepSettings();
}
$("t-chat").addEventListener("click", () => { setChat(!chatOpen); if (chatOpen) $("chat-input").focus(); });
$("chat-close").addEventListener("click", () => { setChat(false); $("t-chat").focus(); });
$("chat-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const text = $("chat-input").value.trim();
  if (!text) return;
  $("chat-input").value = "";
  try {
    const r = await room.say(text);
    if (!r.ok) chatNote(r.status === 429 ? "A little slower, please." : r.status === 403 ? "You can't post in the grove right now." : "That didn't send; try again.");
    pollChat();
  } catch { chatNote("That didn't send; the grove chat can't be reached."); }
});
$("t-prayers").addEventListener("click", () => toggle("prayers"));
$("s-voice").addEventListener("change", (e) => setSetting("voice", e.target.checked));
$("s-name-save").addEventListener("click", async () => {
  const name = $("s-name").value.trim();
  try { const r = await room.rename(name); announce(r.ok ? (name ? `You are ${name} in the grove.` : "Your name is back to the guest one.") : "That name wasn't accepted."); pollPresence(); } catch { announce("Couldn't reach the grove to save your name."); }
});
let tick = 0;
setInterval(() => {
  if (!inGrove || paused || document.hidden) return;
  tick++;
  pollChat();
  if (tick % 3 === 1) refreshRibbons();
  if (tick % 3 === 2) pollPresence();
  if (!$("mandala").hidden) refreshMandala();
}, 3500);

// ---- telling the visitor
let captionTimer = 0;
function caption(text) {
  if (!text) return;
  announce(text);
  if (!settings.captions || !inGrove) return;
  const c = $("caption"); c.textContent = text; c.classList.remove("gone");
  clearTimeout(captionTimer); captionTimer = setTimeout(() => c.classList.add("gone"), 5200);
}
let liveTimer = 0;
function announce(text) {
  if (!text) return;
  const l = $("live"); l.textContent = ""; clearTimeout(liveTimer);
  liveTimer = setTimeout(() => { l.textContent = text; }, 60);
}
let quailTimer = 0;
function quailSays(i, text) {
  if (!settings.text) return;
  const now = Date.now(); if (i >= 0 && now - quailTimer < 6000) return; quailTimer = now;
  $("text-quail").textContent = text || QUAIL_TEXT[i] || QUAIL_TEXT[0];
}

// ---- panels
const PANELS = ["decor", "ribbon", "journal", "prayers", "album", "settings", "inventory", "account", "mandala"];
const toolFor = { decor: "t-decor", inventory: "t-inv", journal: "t-journal", prayers: "t-prayers", settings: "t-settings" };
let returnFocus = null;
function open(id) {
  for (const p of PANELS) if (p !== id && !$(p).hidden) close(p, false);
  returnFocus = document.activeElement;
  $(id).hidden = false;
  if (toolFor[id]) $(toolFor[id]).setAttribute("aria-expanded", "true");
  if (id === "journal") renderJournal();
  if (id === "inventory") { send(MSG.decor, "o"); renderInventory(); }
  if (id === "album") renderAlbum();
  if (id === "decor") { send(MSG.decor, "o"); renderDecor(); }
  $(id).focus();
}
function close(id, refocus = true) {
  if ($(id).hidden) return;
  $(id).hidden = true;
  if (toolFor[id]) $(toolFor[id]).setAttribute("aria-expanded", "false");
  if (id === "ribbon" && state.m !== 0) send(MSG.close, "0");
  if (id === "decor" || id === "inventory") send(MSG.decor, "q");
  if (refocus && returnFocus?.isConnected) returnFocus.focus();
}
const toggle = (id) => ($(id).hidden ? open(id) : close(id));
for (const b of document.querySelectorAll("[data-close]")) b.addEventListener("click", () => close(b.closest(".panel").id));
const topPanel = () => [...PANELS].reverse().find((p) => !$(p).hidden);

// ---- landing
function syncSoundInputs() {
  $("l-sound").setAttribute("aria-pressed", String(settings.sound)); $("l-sound-label").textContent = settings.sound ? "Sound on" : "Sound off";
  $("l-sound").querySelector(".icon").textContent = settings.sound ? "♪" : "✕";
  $("s-sound").checked = settings.sound;
  $("l-music").value = $("s-music").value = settings.music;
  $("l-amb").value = $("s-amb").value = settings.ambience;
  $("l-captions").checked = $("s-captions").checked = settings.captions;
  $("s-motion").checked = settings.motion; $("s-bloom").checked = settings.bloom; $("s-lite").checked = !!settings.lite; $("s-text").checked = settings.text;
  $("s-voice").checked = settings.voice;
  $("g-sound").setAttribute("aria-pressed", String(settings.sound)); $("g-sound-label").textContent = settings.sound ? "Sound on" : "Sound off";
  $("g-sound").querySelector(".icon").textContent = settings.sound ? "♪" : "✕";
  document.body.classList.toggle("text-mode", settings.text && inGrove);
  $("textmode").hidden = !(settings.text && inGrove);
  applySound();
}
const setSetting = (k, v) => { settings[k] = v; keepSettings(); syncSoundInputs(); if (k === "motion" || k === "bloom" || k === "night" || k === "toon" || k === "lite") tellSettings(); document.body.classList.toggle("night", !!settings.night); document.body.classList.toggle("calm", !!settings.motion); };
document.body.classList.toggle("calm", !!settings.motion);
$("l-sound").addEventListener("click", () => setSetting("sound", !settings.sound));
$("s-sound").addEventListener("change", (e) => setSetting("sound", e.target.checked));
for (const [a, b, k] of [["l-music", "s-music", "music"], ["l-amb", "s-amb", "ambience"]]) {
  for (const id of [a, b]) $(id).addEventListener("input", (e) => setSetting(k, Number(e.target.value)));
}
for (const id of ["l-captions", "s-captions"]) $(id).addEventListener("change", (e) => setSetting("captions", e.target.checked));
$("s-motion").addEventListener("change", (e) => setSetting("motion", e.target.checked));
$("s-bloom").addEventListener("change", (e) => setSetting("bloom", e.target.checked));
$("s-lite").addEventListener("change", (e) => setSetting("lite", e.target.checked));
for (const r of document.querySelectorAll("input[name=s-time]")) r.addEventListener("change", () => setSetting("night", r.value === "night" && r.checked));
for (const r of document.querySelectorAll("input[name=s-shade]")) r.addEventListener("change", () => setSetting("toon", r.value === "toon" && r.checked));
$("t-night").addEventListener("click", () => { setSetting("night", !settings.night); syncLook(); announce(settings.night ? "Night falls over the grove." : "Dusk again."); });
function syncLook() {
  for (const r of document.querySelectorAll("input[name=s-time]")) r.checked = (r.value === "night") === !!settings.night;
  for (const r of document.querySelectorAll("input[name=s-shade]")) r.checked = (r.value === "toon") === !!settings.toon;
  $("t-night").setAttribute("aria-pressed", String(!!settings.night));
  document.body.classList.toggle("night", !!settings.night);
}
syncLook();
$("s-text").addEventListener("change", (e) => { setSetting("text", e.target.checked); renderText(); });
// Start over as a new visitor: the world forgets its save (found tracks, cairn, decorations, ribbon),
// the page forgets its own (settings, hints already shown, hidden ribbons, muted names), and the
// visit begins again at the title page with the quail's greeting. Your guest name in the shared
// grove (verse.player, shared with other GameBuilder worlds) is kept. ?new on the URL does the same.
function startOver() {
  send(MSG.forget, "1"); send(MSG.save, "1");
  setTimeout(() => {
    try { for (const k of Object.keys(localStorage)) if (k.startsWith("whispering-spring")) localStorage.removeItem(k); } catch {}
    const u = new URL(location.href); u.searchParams.delete("new");
    location.replace(u.href);
  }, 700);
}
$("s-forget").addEventListener("click", () => {
  if (confirm("Start over as a new visitor? This forgets the tracks you found, your cairn, decorations and ribbon, and your settings, on this device.")) { announce("Starting over."); startOver(); }
});
// ?new: once last visit's save has had time to come back, forget it the same way
let freshVisit = new URLSearchParams(location.search).has("new");
$("l-settings").addEventListener("click", () => open("settings"));

function enterGrove() {
  audio.start(); audio.startAmbience();
  inGrove = true; paused = false;
  $("landing").hidden = true; $("hud").hidden = false;
  close("album", false); close("settings", false);
  syncSoundInputs(); renderText();
  send(MSG.pause, "0");
  announce("You are in the grove. A black quail is foraging in front of you, by a spring under a tall earthen bank.");
  (settings.text ? document.querySelector("#textmode [data-spot]") : $("spots").querySelector("button"))?.focus();
  setChat(!!settings.chat);
  refreshRibbons(); pollChat(); pollPresence();
  if (!said.greeting) setTimeout(() => quailSpeak("greeting"), 1400);
}
function leaveGrove() {
  send(MSG.save, "1"); send(MSG.close, "0");
  audio.stopTrack();
  inGrove = false; paused = false;
  for (const p of PANELS) close(p, false);
  $("found").hidden = true;
  $("chat").hidden = true; $("bubble").classList.add("gone"); bubbleShown = false;
  $("hud").hidden = true; $("landing").hidden = false;
  syncSoundInputs();
  $("enter").focus();
}
$("enter").addEventListener("click", enterGrove);
$("listen").addEventListener("click", () => { audio.start(); open("album"); });

// ---- in the grove
for (const b of document.querySelectorAll("[data-spot]")) b.addEventListener("click", () => { audio.start(); send(MSG.pick, b.dataset.spot); });
$("t-inv").addEventListener("click", () => toggle("inventory"));
$("t-journal").addEventListener("click", () => toggle("journal"));
$("t-settings").addEventListener("click", () => toggle("settings"));
const setPaused = () => {};
$("t-leave").addEventListener("click", leaveGrove);

// ---- the spots' named buttons (for the keyboard and screen readers): a glint on the one the quail favours
function renderDiscoveries() {
  for (const b of $("spots").querySelectorAll("button")) {
    const id = ["spring", "cairn", "ribbon"][Number(b.dataset.spot)];
    const clue = { pool: 0, stones: 1, ribbon: 2 }[lastClue];
    b.classList.toggle("clue", !!id && !found(id) && clue === Number(b.dataset.spot));
  }
  renderInventory();
}

// ---- the hint while something is in hand
const touchy = matchMedia("(pointer: coarse)").matches;
let swipeHintUntil = 0;
// an upright phone sees part of the grove: say once that a swipe looks around
function swipeHintOnce() {
  if (innerWidth / innerHeight >= 0.9) return;
  try { if (localStorage.getItem("whispering-spring.swiped")) return; localStorage.setItem("whispering-spring.swiped", "1"); } catch {}
  swipeHintUntil = Date.now() + 6000; renderHint(); setTimeout(renderHint, 6100);
}
function renderHint() {
  const el = $("hint");
  let html = "";
  if (decor.t >= 0) { html = `<b>${TOOLS[decor.t][0]}</b> · <kbd>Esc</kbd> puts it away`; el.dataset.ok = "1"; }
  else if (decor.k >= 0) {
    html = HANGS.has(decor.k)
      ? (decor.ok ? "<b>It can hang here.</b>" : "<b>Bring it to a branch: the gold points.</b>")
      : (decor.ok ? "<b>It can stand here.</b>" : "<b>Not here.</b>");
    el.dataset.ok = decor.ok ? "1" : "0";
  } else if (Date.now() < swipeHintUntil) {
    html = "Swipe sideways to look around the grove"; el.dataset.ok = "1";
  }
  if (touchy) html = html.replace("Click the ground", "Tap the ground").replace(" · <kbd>Esc</kbd> puts it back", "");
  el.hidden = !html || !inGrove;
  if (html && el.innerHTML !== html) el.innerHTML = html;
}

// ---- decorating
function renderDecor() {
  const tabs = $("decor-tabs");
  if (!tabs.children.length) CATS.forEach(([name], c) => {
    const b = document.createElement("button"); b.type = "button"; b.className = "tab"; b.setAttribute("role", "tab"); b.textContent = name; b.dataset.c = c;
    b.addEventListener("click", () => { decorCat = c; if (c !== 3 && decor.t >= 0) send(MSG.decor, "t -1"); renderDecor(); });
    tabs.append(b);
  });
  for (const b of tabs.children) b.setAttribute("aria-selected", String(Number(b.dataset.c) === decorCat));
  const box = $("decor-items");
  box.textContent = "";
  if (decorCat === 3) {
    TOOLS.forEach(([name, ico], t) => {
      const b = document.createElement("button"); b.className = "btn"; b.type = "button"; b.dataset.t = t;
      b.innerHTML = `<span class="ico" aria-hidden="true">${ico}</span>${name}`;
      b.setAttribute("aria-pressed", String(decor.t === t));
      b.addEventListener("click", () => { audio.start(); send(MSG.decor, `t ${decor.t === t ? -1 : t}`); });
      box.append(b);
    });
  } else {
    for (const k of CATS[decorCat][1]) {
      const [name, ico] = DECOR[k];
      const b = document.createElement("button"); b.className = "btn"; b.type = "button"; b.dataset.k = k;
      b.innerHTML = `<span class="ico" aria-hidden="true">${ico}</span>${name}`;
      b.addEventListener("click", () => { audio.start(); send(MSG.decor, `k ${k}`); });
      box.append(b);
    }
  }
  for (const b of box.children) if (b.dataset.k !== undefined) b.setAttribute("aria-pressed", String(Number(b.dataset.k) === decor.k));
  $("decor-wipe").hidden = decorCat !== 3 || !decor.g;
  const carrying = decor.k >= 0;
  $("decor-keys").hidden = !carrying;
  $("decor-remove").hidden = !(carrying && decor.m >= 0);
  $("decor-set").disabled = !decor.ok;
  const counts = {}; for (const k of decor.l.split(",").filter((s) => s !== "")) counts[k] = (counts[k] || 0) + 1;
  const list = Object.entries(counts).map(([k, n]) => `${n} ${DECOR[k][0].toLowerCase()}`).join(", ");
  $("decor-status").textContent = decor.t >= 0 ? `Brush: ${TOOLS[decor.t][0].toLowerCase()}.` : carrying
    ? `Carrying the ${DECOR[decor.k][0].toLowerCase()}. ${decor.ok ? "It can stand here." : "Not here: find some grass."}`
    : decor.n ? `In the grove: ${list}. (${24 - decor.n} more can go.)` : "Nothing set out yet.";
  $("decor-clear").disabled = !decor.n;
}
$("t-bag").addEventListener("click", () => toggle("inventory"));
for (const b of document.querySelectorAll("[data-move]")) b.addEventListener("click", () => send(MSG.decor, `m ${b.dataset.move}`));
$("decor-set").addEventListener("click", () => send(MSG.decor, "p"));
$("decor-back").addEventListener("click", () => send(MSG.decor, "x"));
$("decor-remove").addEventListener("click", () => send(MSG.decor, "d"));
$("decor-wipe").addEventListener("click", () => { if (confirm("Put all the terrain you painted back as it was?")) send(MSG.decor, "w"); });
$("decor-clear").addEventListener("click", () => { if (confirm("Take every decoration out of the grove?")) send(MSG.decor, "c"); });

// ---- ribbon
function radios(box, name, items, label) {
  box.textContent = "";
  items.forEach((it, i) => {
    const l = document.createElement("label"); l.className = "choice";
    const r = document.createElement("input"); r.type = "radio"; r.name = name; r.value = i; if (i === 0) r.checked = true;
    l.append(r);
    if (label) l.append(label(it)); else l.append(document.createTextNode(it));
    box.append(l);
  });
}
radios($("r-fabric"), "fabric", FABRICS);
radios($("r-colour"), "colour", COLOURS, ([name, hex]) => { const f = document.createDocumentFragment(); const s = document.createElement("span"); s.className = "swatch"; s.style.background = hex; s.setAttribute("aria-hidden", "true"); f.append(s, document.createTextNode(name)); return f; });
radios($("r-charm"), "charm", CHARMS);
radios($("r-branch"), "branch", BRANCHES);
let ribbonAt = -1;
function openRibbon() {
  if (state.r && state.rb) {
    const [f, c, ch, b] = state.rb.split(",").map(Number);
    for (const [name, v] of [["fabric", f], ["colour", c], ["charm", ch], ["branch", Math.floor(b / 8)]]) { const r = document.querySelector(`input[name=${name}][value="${v}"]`); if (r) r.checked = true; }
    $("r-word").value = state.rw || "";
  }
  // a ribbon begun at one of the zones' prayer trees goes on that tree's limb
  if (ribbonAt >= 0) { const r = document.querySelector(`input[name=branch][value="${ribbonAt}"]`); if (r) r.checked = true; ribbonAt = -1; }
  open("ribbon");
}
$("ribbon-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const v = (n) => Number(document.querySelector(`input[name=${n}]:checked`)?.value || 0);
  const word = $("r-word").value.replace(/[;,|\n\r\t]/g, " ").slice(0, 40);
  audio.start();
  pendingTie = { share: $("r-share").checked };
  send(MSG.ribbon, `${v("fabric")},${v("colour")},${v("charm")},${v("branch") * 8 + SLOT_AUTO},${word}`);
  $("ribbon").hidden = true;
  announce(`You tie a ${COLOURS[v("colour")][0].toLowerCase()} ${FABRICS[v("fabric")].toLowerCase()} ribbon on ${BRANCHES[v("branch")].toLowerCase()}. It catches the wind, where everyone in the grove can see it.`);
  if (returnFocus?.isConnected) returnFocus.focus();
});

// ---- found
let foundId = "";
function showFound(id) {
  foundId = id;
  const n = ["spring", "cairn", "ribbon"].filter(found).length;
  $("found-title").textContent = titleOf(id);
  $("found-extra").textContent = `A new page of the quail's story is in your journal. ${n} of 3 pages found.`;
  $("found").hidden = false;
  announce(`You uncovered lore: ${titleOf(id)}. It is in your journal.`);
  $("found-play").focus();
}
const closeFound = () => { $("found").hidden = true; (settings.text ? document.querySelector("#textmode [data-spot]") : $("spots").querySelector("button"))?.focus(); };
$("found-play").addEventListener("click", () => { closeFound(); open("journal"); document.getElementById(`lore-${foundId}`)?.scrollIntoView({ block: "start" }); });
$("found-journal").addEventListener("click", () => { closeFound(); open("journal"); });
$("found-close").addEventListener("click", closeFound);

// ---- playing, saving and downloading
let playing = "";
async function playTrack(id) {
  if (playing === id) { audio.stopTrack(); return; }
  playing = id; renderJournal(); renderAlbum();
  await audio.playTrack(id, () => { if (playing === id) { playing = ""; renderJournal(); renderAlbum(); } });
}
function playButton(id, label) {
  const b = document.createElement("button"); b.type = "button"; b.className = "btn";
  b.textContent = playing === String(id) ? `Stop` : label;
  b.setAttribute("aria-pressed", String(playing === String(id)));
  b.addEventListener("click", () => playTrack(String(id)));
  return b;
}
function saveFile(blob, name) {
  if (!blob) { announce("Nothing to download yet."); return; }
  const a = document.createElement("a"); a.href = URL.createObjectURL(blob); a.download = name;
  document.body.append(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(a.href), 5000);
}
const slug = (s) => String(s).toLowerCase().replace(/\(.*?\)/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
function downloadButton(label, make, name) {
  const b = document.createElement("button"); b.type = "button"; b.className = "btn"; b.textContent = label;
  b.addEventListener("click", async () => { b.disabled = true; b.textContent = "Preparing…"; try { saveFile(await make(), name); } finally { b.disabled = false; b.textContent = label; } });
  return b;
}
function renderJournal() {
  const ul = $("journal-list"); if (!ul) return; ul.textContent = "";
  const pages = manifest.lore?.pages || [];
  pages.forEach((pg, i) => {
    const li = document.createElement("li"); li.id = `lore-${pg.id}`;
    const h = document.createElement("h3"); h.textContent = `${pg.n}. ${found(pg.id) ? pg.title : "A sealed page"}`;
    li.append(h);
    if (!found(pg.id)) { const p = document.createElement("p"); p.className = "muted small"; p.textContent = HINT[pg.id]; li.append(p); ul.append(li); return; }
    const text = document.createElement("p"); text.className = "lyric"; text.textContent = pg.text; li.append(text);
    const row = document.createElement("div"); row.className = "row"; row.style.marginTop = "8px";
    if (pg.file) row.append(playButton(`lore:${pg.id}`, "Hear it read"));
    row.append(downloadButton("Download page (.txt)", async () => new Blob([`${pg.title}\n\n${pg.text}\n`], { type: "text/plain" }), `${slug(pg.title)}.txt`));
    const kept = (state.x & (1 << i)) !== 0;
    const keep = document.createElement("button"); keep.type = "button"; keep.className = "btn"; keep.textContent = kept ? "Kept in your journal ✓" : "Keep this page";
    keep.disabled = kept; keep.addEventListener("click", () => send(MSG.extra, String(i)));
    row.append(keep); li.append(row);
    ul.append(li);
  });
}
function renderAlbum() {
  const ul = $("album-list"); if (!ul) return; ul.textContent = "";
  for (const t of manifest.tracks) {
    const li = document.createElement("li"); li.className = "row"; li.style.justifyContent = "space-between";
    const p = document.createElement("span"); p.textContent = `${t.n}. ${t.title}`;
    li.append(p, playButton(t.id, "Play")); ul.append(li);
  }
}

// ---- the inventory: the album as a bag, like an adventurer's. Relics (the lore pages) are sealed
// until found; songs play; the album itself sits in the bag once owned (a preview for now).
const RARITY_NAME = { legend: "Legendary", epic: "Epic", rare: "Rare", common: "Keepsake", poor: "Sealed" };
const acct = { in: false, owns: false };
try { Object.assign(acct, JSON.parse(localStorage.getItem("whispering-spring.preview.v1") || "{}")); } catch {}
const keepAcct = () => { try { localStorage.setItem("whispering-spring.preview.v1", JSON.stringify(acct)); } catch {} };
let selected = 0;
function bagItems() {
  const items = [];
  const pages = manifest.lore?.pages || [];
  const relicIcon = { spring: "\u{1F4A7}", cairn: "\u{1FAA8}", ribbon: "\u{1F38F}" };
  for (const pg of pages) {
    const got = found(pg.id);
    items.push(got
      ? { icon: relicIcon[pg.id] || "\u{1F4DC}", r: "epic", name: pg.title, kind: "Lore relic · Soulbound", flavor: (pg.text || "").split(/(?<=[.!?])\s/)[0], use: "Use: read this page of the quail's story.", act: () => { close("inventory", false); open("journal"); document.getElementById(`lore-${pg.id}`)?.scrollIntoView({ block: "start" }); } }
      : { icon: "?", r: "poor", locked: true, name: "A sealed relic", kind: "Undiscovered", flavor: "Something in the grove is keeping it.", use: "" });
  }
  for (const t of manifest.tracks || []) {
    items.push({ icon: "\u{1F3B5}", r: "rare", name: t.title.replace(/\s*\(placeholder title\)/, ""), kind: `Song · track ${t.n}`, flavor: "Placeholder audio until the artist's recording arrives.", use: playing === t.id ? "Use: stop." : "Use: play it.", act: () => { playTrack(t.id); renderInventory(); } });
  }
  items.push(acct.owns
    ? { icon: "\u{1F4BF}", r: "legend", name: "The Whispering Spring", kind: "Digital album · Obsidian Quail", flavor: "Every song, to keep.", use: "Use: download (MP3, FLAC or WAV).", act: () => soon("Downloads aren't ready yet. When they are, this is where you'll pick MP3, FLAC or lossless WAV and save the whole album, artwork and liner notes included.") }
    : { icon: "\u{1F4BF}", r: "poor", locked: true, name: "The Whispering Spring (album)", kind: "Not owned", flavor: "The whole album, to keep and download.", use: "Use: see how buying it will work.", act: () => { close("inventory", false); open("account"); } });
  // keepsakes: what can be set out in the grove (picked up from it), and the terrain brushes
  DECOR.forEach(([name, ico], k) => {
    const have = decor.s ? (decor.i[k] || 0) : 1;
    if (!have) return;
    items.push({ icon: ico, r: k >= 6 && k <= 10 ? "epic" : HANGS.has(k) ? "rare" : "common", qty: decor.s ? have : 0, name, kind: HANGS.has(k) ? "Keepsake · hangs in a tree" : k >= 6 && k <= 10 ? "Monument · one of a kind" : "Keepsake", flavor: "Found in the grove. It can go anywhere.", use: HANGS.has(k) ? "Use: hang it on a branch." : "Use: set it down in the grove.", act: () => { close("inventory", false); send(MSG.decor, "o"); send(MSG.decor, `k ${k}`); } });
  });
  TOOLS.forEach(([name, ico], t) => items.push({ icon: ["\u{1F33E}", "\u{1F331}", "\u{1F7EB}"][t], r: "common", name, kind: "Terrain brush · paints, doesn't place", flavor: "Click or drag on the ground to paint it.", use: decor.g ? "Use: paint with it. (Undo my terrain is in the brush's hint.)" : "Use: paint with it.", act: () => { close("inventory", false); send(MSG.decor, "o"); send(MSG.decor, `t ${t}`); } }));
  const placed = 0;
  if (placed) items.push({ icon: "\u{1F3EE}", r: "common", qty: placed, name: "Things you've set out", kind: "Keepsakes in the grove", flavor: "The grove remembers where you put them.", use: "Use: open the basket.", act: () => { close("inventory", false); open("decor"); } });
  return items;
}
let toastTimer = 0;
function toast(text) { const t = $("toast"); t.textContent = text; t.classList.remove("show"); void t.offsetWidth; t.classList.add("show"); clearTimeout(toastTimer); toastTimer = setTimeout(() => t.classList.remove("show"), 2200); }
function renderInventory() {
  const bag = $("bag"); if (!bag) return;
  const items = bagItems();
  bag.textContent = "";
  const n = ["spring", "cairn", "ribbon"].filter(found).length;
  $("inv-gold").textContent = `${n} of 3 relics found`;
  // a phone shows just the rows it needs; a wider screen a fuller bag
  const SLOTS = Math.max(innerWidth <= 640 ? 10 : 25, Math.ceil(items.length / 5) * 5);
  for (let i = 0; i < SLOTS; i++) {
    const it = items[i];
    const b = document.createElement("button"); b.type = "button"; b.className = "slot" + (it ? (it.locked ? " locked" : "") : " empty");
    b.setAttribute("role", "option");
    if (it) {
      b.dataset.r = it.r; b.textContent = it.icon; b.setAttribute("aria-label", `${it.name}, ${RARITY_NAME[it.r]}`);
      if (it.qty) { const q = document.createElement("span"); q.className = "qty"; q.textContent = it.qty; b.append(q); }
      b.setAttribute("aria-selected", String(i === selected));
      b.addEventListener("click", () => { if (selected === i && it.act) { it.act(); return; } selected = i; renderInventory(); });
      b.addEventListener("dblclick", () => it.act && it.act());
      b.addEventListener("focus", () => { if (selected !== i) { selected = i; showTip(it); for (const s of bag.children) s.setAttribute("aria-selected", String(s === b)); } });
    } else { b.disabled = true; b.setAttribute("aria-label", "Empty slot"); }
    bag.append(b);
  }
  showTip(items[selected] || items[0]);
}
function showTip(it) {
  const tip = $("tip"); tip.textContent = "";
  if (!it) return;
  const name = document.createElement("p"); name.className = "name"; name.dataset.r = it.r; name.textContent = it.name;
  const kind = document.createElement("p"); kind.className = "kind"; kind.textContent = `${RARITY_NAME[it.r]} · ${it.kind}`;
  const fl = document.createElement("p"); fl.className = "flavor"; fl.textContent = it.flavor ? `“${it.flavor}”` : "";
  tip.append(name, kind, fl);
  if (it.use) { const u = document.createElement("p"); u.className = "use"; u.textContent = it.use; tip.append(u); }
  if (it.act) { const b = document.createElement("button"); b.type = "button"; b.className = "btn primary"; b.style.marginTop = "10px"; b.textContent = it.locked ? "Find out more" : "Use"; b.addEventListener("click", it.act); tip.append(b); }
}
// ---- the account: a preview of signing in, owning the album and downloading it. Nothing is sent
// anywhere and nothing is charged: each live action says it isn't ready yet.
function soon(text) { $("soon-text").textContent = text; $("soon").hidden = false; $("soon-ok").focus(); announce(`Not ready yet. ${text}`); }
const closeSoon = () => { $("soon").hidden = true; };
$("soon-ok").addEventListener("click", closeSoon);
function renderAccount() {
  $("acct-out").hidden = acct.in; $("acct-in").hidden = !acct.in;
  $("t-account-label").textContent = acct.in ? "Quail Friend" : "Sign in";
  $("own-state").textContent = acct.owns ? "In your inventory. (A preview.)" : "Not in your inventory yet.";
  $("own-preview").setAttribute("aria-pressed", String(acct.owns));
  $("buy").hidden = acct.owns;
  renderInventory();
}
$("t-account").addEventListener("click", () => toggle("account"));
$("signin").addEventListener("submit", (e) => { e.preventDefault(); $("si-email").value = ""; soon("Signing in isn't ready yet. You'll get a six-digit code by email, type it here, and your relics, ribbon and keepsakes will follow you to any device. Nothing was sent."); });
$("si-oa").addEventListener("click", () => soon("Signing in with your OpenAgents account isn't ready yet. It will be one click, and the grove will know you."));
$("si-preview").addEventListener("click", () => { acct.in = true; keepAcct(); renderAccount(); announce("Previewing a signed-in account."); });
$("si-out").addEventListener("click", () => { acct.in = false; acct.owns = false; keepAcct(); renderAccount(); });
$("buy").addEventListener("click", () => soon("Buying the album isn't ready yet: the artist hasn't chosen a price or a way to pay. When it is, it will be one checkout, and the album will appear in your inventory to stream or download."));
$("own-preview").addEventListener("click", () => { acct.owns = !acct.owns; keepAcct(); renderAccount(); announce(acct.owns ? "Previewing the album in your inventory." : "Album preview removed."); });
renderAccount();

// ---- the chat pane: drag it by its header, or move it with the arrow keys on its grip
const CHAT_POS = "whispering-spring.chatpos.v1";
function placeChat(x, y) {
  const c = $("chat"); const w = c.offsetWidth || 340, hgt = c.offsetHeight || 200;
  x = Math.max(8, Math.min(innerWidth - w - 8, x)); y = Math.max(8, Math.min(innerHeight - hgt - 8, y));
  c.style.left = `${x}px`; c.style.top = `${y}px`; c.style.bottom = "auto";
  try { localStorage.setItem(CHAT_POS, JSON.stringify([x, y])); } catch {}
}
try { const p = JSON.parse(localStorage.getItem(CHAT_POS) || "null"); if (p) requestAnimationFrame(() => placeChat(p[0], p[1])); } catch {}
{
  let drag = null;
  $("chat-head").addEventListener("pointerdown", (e) => { if (e.target.closest("#chat-close")) return; const r = $("chat").getBoundingClientRect(); drag = [e.clientX - r.left, e.clientY - r.top]; $("chat-head").setPointerCapture(e.pointerId); e.preventDefault(); });
  $("chat-head").addEventListener("pointermove", (e) => { if (drag) placeChat(e.clientX - drag[0], e.clientY - drag[1]); });
  $("chat-head").addEventListener("pointerup", () => { drag = null; });
  $("chat-grip").addEventListener("keydown", (e) => {
    const d = { ArrowLeft: [-20, 0], ArrowRight: [20, 0], ArrowUp: [0, -20], ArrowDown: [0, 20] }[e.key];
    if (!d) return; e.preventDefault(); e.stopPropagation();
    const r = $("chat").getBoundingClientRect(); placeChat(r.left + d[0], r.top + d[1]);
  });
}

// ---- the text-first grove
function renderText() {
  document.body.classList.toggle("text-mode", settings.text && inGrove);
  $("textmode").hidden = !(settings.text && inGrove);
  if (!settings.text) return;
  const bits = [];
  bits.push(found("spring") ? "The pool is quiet now; you have heard what it had to say." : "The pool is still and blue-green, with a warm patch of light on it.");
  const placed = state.c ? state.c.split(",").filter(Boolean).length : 0;
  bits.push(placed >= 4 ? "Your cairn of four creek stones stands on the slab." : placed ? `Your cairn has ${placed} of 4 stones.` : "The creek stones lie in a loose heap.");
  bits.push(state.r ? `Your ribbon hangs from the branch${state.rw ? `, carrying the words "${state.rw.trim()}"` : ""}.` : "The cedar's bare branch reaches out over the pool.");
  $("text-state").textContent = bits.join(" ");
}

// ---- keys
// held movement keys glide the view each frame: A/D or ←/→ along the grove, W/S or ↑/↓ in and out
const MOVE_KEYS = { a: [-1, 0], arrowleft: [-1, 0], d: [1, 0], arrowright: [1, 0], w: [0, 1], arrowup: [0, 1], s: [0, -1], arrowdown: [0, -1] };
const held = new Set(); let moving = false;
function moveLoop() {
  if (!held.size || !inGrove) { moving = false; return; }
  let dx = 0, dz = 0; for (const k of held) { dx += MOVE_KEYS[k][0]; dz += MOVE_KEYS[k][1]; }
  if (dx) send(MSG.look, `p ${dx > 0 ? 2 : -2}`);
  if (dz) send(MSG.look, `z ${dz > 0 ? 1 : -1}`);
  requestAnimationFrame(moveLoop);
}
addEventListener("keyup", (e) => held.delete(e.key.toLowerCase()));
addEventListener("blur", () => held.clear());
addEventListener("keydown", (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  const typing = e.target instanceof HTMLInputElement && (e.target.type === "text" || e.target.type === "range");
  if (e.key === "Escape") {
    if (inGrove && state.h >= 0 && state.y === 1) { send(MSG.stones, "b"); e.preventDefault(); return; }
    if (inGrove && decor.k >= 0) { send(MSG.decor, "x"); send(MSG.decor, "q"); e.preventDefault(); return; }
    if (inGrove && decor.t >= 0) { send(MSG.decor, "q"); e.preventDefault(); return; }
    if (!$("soon").hidden) { closeSoon(); e.preventDefault(); return; }
    if (!$("ribbon-card").hidden) { closeRibbonCard(); e.preventDefault(); return; }
    if (!$("mandala").hidden) { closeMandala(); e.preventDefault(); return; }
    if (!$("menu").hidden) { setMenu(false); $("t-menu").focus(); e.preventDefault(); return; }
    if (!$("found").hidden) { closeFound(); e.preventDefault(); return; }
    const p = topPanel(); if (p) { close(p); e.preventDefault(); return; }
    return;
  }
  if (typing || !inGrove || paused || !$("found").hidden) return;
  const k = e.key.toLowerCase();
  if (decor.k >= 0) {
    const mv = { arrowleft: "-2 0", arrowright: "2 0", arrowup: "0 -2", arrowdown: "0 2" }[k];
    if (mv) { send(MSG.decor, `m ${mv}`); e.preventDefault(); return; }
    if (e.key === "Enter" && !(e.target instanceof HTMLButtonElement)) { send(MSG.decor, "p"); e.preventDefault(); return; }
  }
  if (MOVE_KEYS[k] && !(e.target instanceof HTMLInputElement)) { held.add(k); if (!moving) { moving = true; requestAnimationFrame(moveLoop); } e.preventDefault(); return; }
  if (e.target instanceof HTMLButtonElement || e.target instanceof HTMLInputElement) { if (e.key === "Enter" || e.key === " ") return; }
  if (k === "1" || k === "2" || k === "3") { audio.start(); send(MSG.pick, String(Number(k) - 1)); e.preventDefault(); }
  else if (k === "i") { toggle("inventory"); e.preventDefault(); }
  else if (k === "j") { toggle("journal"); e.preventDefault(); }
});

// ---- leaving the page keeps what was found; a hidden page goes quiet
document.addEventListener("visibilitychange", () => { if (document.hidden) { send(MSG.save, "1"); audio.pause(true); } else if (!paused) audio.pause(false); });
addEventListener("pagehide", () => send(MSG.save, "1"));

// The host hears pointers on the whole window; a press on this page's panels and buttons is the
// page's, not a touch in the grove behind them.
for (const type of ["pointerdown", "pointermove", "pointerup", "pointercancel"]) {
  $("ws-ui").addEventListener(type, (e) => { if (e.target !== $("coaxial")) e.stopPropagation(); });
}

syncSoundInputs();
renderDiscoveries(); renderMini(); renderInventory();
$("enter").focus();
