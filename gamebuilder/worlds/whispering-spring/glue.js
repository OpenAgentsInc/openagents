// Firm reason: 1 (the page's half of The Whispering Spring: panels, journal, settings and sound;
// world-spring.coax decides everything that happens in the grove and tells this page in events).
import { SpringAudio } from "./audio.js";
import { GroveRoom, placeRibbons } from "./room.js";

const $ = (id) => document.getElementById(id);
const MSG = { pick: 120, stones: 121, ribbon: 122, pause: 123, settings: 124, save: 125, extra: 127, forget: 128, close: 129, shared: 131, speak: 132, decor: 133 };
const HIDDEN_KEY = "whispering-spring.hidden.v1", MUTED_KEY = "whispering-spring.muted.v1";
const readSet = (k) => { try { return new Set(JSON.parse(localStorage.getItem(k) || "[]")); } catch { return new Set(); } };
const keepSet = (k, s) => { try { localStorage.setItem(k, JSON.stringify([...s].slice(-500))); } catch {} };
const ACT_EVENT = 90;
const SETTINGS_KEY = "whispering-spring.settings.v1";

// ---- per-viewer settings (this browser only)
const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
const settings = { sound: true, music: 80, ambience: 60, captions: true, motion: reduce, bloom: false, text: false, voice: true, chat: false };
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
const BALANCE = ["Steady: it will hold.", "It will hold, with a wobble.", "It would slide off."];
const DECOR = [["Lantern", "\u{1F3EE}"], ["Candles", "\u{1F56F}\uFE0F"], ["Bluebonnets", "\u{1F33C}"], ["Crystal", "\u{1F48E}"], ["Sage bundle", "\u{1F33F}"], ["Offering bowl", "\u{1F963}"]];
let decor = { n: 0, k: -1, ok: 0, m: -1, l: "" };
const WHY = ["", " Turned a little far.", " Wider than the stone below.", " The round stone below rocks a little."];
const SLIP = ["", "It slid off at that angle. Turn it back toward level.", "", "", "Too far off the middle: it tumbled back to the pile. Let go nearer the centre."];
const FABRICS = ["Cotton", "Silk", "Linen"];
const COLOURS = [["Prickly-pear rose", "#bc546b"], ["Bluebonnet", "#4d61b3"], ["Limestone cream", "#e0d6b8"], ["Sage", "#8c9e7a"], ["Persimmon", "#d97338"], ["Indigo", "#383873"]];
const CHARMS = ["None", "Bell", "Bead", "Feather"];
const BRANCHES = ["The low limb over the pool", "The long limb", "The high limb", "The right limb", "The far limb, by the bank", "The right high limb"];
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
const tellSettings = () => send(MSG.settings, `${settings.motion ? 1 : 0} ${settings.bloom ? 1 : 0}`);

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
  else if (kind === "clue") { lastClue = rest; announce(CLUE_TEXT[rest] || ""); quailSays(-1, CLUE_TEXT[rest]); renderDiscoveries(); }
  else if (kind === "found") showFound(rest);
  else if (kind === "replayed") announce(`${titleOf(rest)} again.`);
  else if (kind === "mode") { if (rest === "stones") open("stones"); else if (rest === "ribbon") openRibbon(); }
  else if (kind === "hold") { const s = STONES.find((x) => x[0] === rest); announce(`${s ? s[1] : "A stone"} in your hand. Click over the cairn to let it go.`); }
  else if (kind === "decor") {
    if (rest === "blocked") { announce("It can't stand there. Try the grass."); $("decor-status").textContent = "It can't stand there. Try the grass."; }
    else if (rest === "full") announce("The grove holds sixteen things at most. Move or remove one.");
    else { const d = {}; for (const p of rest.split(";")) { const i = p.indexOf("="); if (i > 0) d[p.slice(0, i)] = p.slice(i + 1); } decor = { n: Number(d.n) || 0, k: Number(d.k), ok: Number(d.ok), m: Number(d.m), l: d.l || "" }; renderDecor(); renderHint(); }
  }
  else if (kind === "slip") { const w = Number(rest) || 1; announce(SLIP[w]); $("balance").textContent = SLIP[w]; $("balance").dataset.b = "2"; }
  else if (kind === "speak") quailSpeak("prayers");
  else if (kind === "cpos") { const [sp, x, y] = rest.split(",").map(Number); placeCallout(sp, x, y); }
  else if (kind === "qpos") { const [x, y] = rest.split(",").map(Number); placeBubble(x, y); }
});

function onState(before) {
  if (!$("stones").hidden && state.m !== 1 && before.m === 1) close("stones");
  renderStones(); renderDiscoveries(); renderJournal(); renderText(); renderMini(); renderHint();
  if (!$("decor").hidden && state.m !== 3 && before.m === 3 && decor.k < 0) close("decor", false);
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

// ---- the in-grove frame: the quest card, the small journal, the callout on the next spot
const GOAL = ["Listen to the spring", "Stack the creek stones", "Hang a prayer ribbon"];
const CALLOUT = ["Listen to the Spring", "Stack the Creek Stones", "Hang a Prayer Ribbon"];
const SHORT = { spring: "Spring", cairn: "Stones", ribbon: "Wind" };
let calloutSpot = -1;
function placeCallout(sp, x, y) {
  const c = $("callout");
  calloutSpot = sp;
  const n = ["spring", "cairn", "ribbon"].filter(found).length;
  $("quest-goal").textContent = sp < 0 ? "Every song found. Stay as long as you like." : GOAL[sp];
  $("quest-count").textContent = `${n} of 3 lore pages found`;
  const busy = state.m !== 0 || state.s === 1 || !$("found").hidden || settings.text || bubbleShown;
  if (sp < 0 || busy || !inGrove) { c.hidden = true; return; }
  if (c.textContent !== CALLOUT[sp]) c.textContent = CALLOUT[sp];
  c.hidden = false;
  c.style.left = `${Math.min(innerWidth - c.offsetWidth - 40, Math.max(12, x))}px`;
  c.style.top = `${Math.max(120, y - 20)}px`;
}
$("callout").addEventListener("click", () => { if (calloutSpot >= 0) { audio.start(); send(MSG.pick, String(calloutSpot)); } });
function renderMini() {
  const ul = $("mini-list"); if (!ul) return; ul.textContent = "";
  for (const id of ["spring", "cairn", "ribbon"]) {
    const li = document.createElement("li");
    if (found(id)) { const m = document.createElement("span"); m.className = "found-mark"; m.textContent = "✦"; li.append(m, document.createTextNode(SHORT[id])); }
    else { const seal = document.createElement("span"); seal.className = "seal"; seal.setAttribute("aria-hidden", "true"); li.append(seal); const sr = document.createElement("span"); sr.className = "sr"; sr.textContent = "Not yet found"; li.append(sr); }
    ul.append(li);
  }
}
$("minijournal").addEventListener("click", () => open("journal"));
function setMenu(openIt) { $("menu").hidden = !openIt; $("t-menu").setAttribute("aria-expanded", String(openIt)); if (openIt) $("menu").querySelector("button")?.focus(); }
$("t-menu").addEventListener("click", () => setMenu($("menu").hidden));
for (const b of $("menu").querySelectorAll("button")) b.addEventListener("click", () => setMenu(false));
$("t-quiet").addEventListener("click", () => { const q = !document.body.classList.contains("quiet"); document.body.classList.toggle("quiet", q); $("t-quiet").setAttribute("aria-pressed", String(q)); announce(q ? "Quiet view: only the grove." : "The quest and journal are back."); });
$("g-sound").addEventListener("click", () => { audio.start(); setSetting("sound", !settings.sound); });

// ---- the quail speaks: its words type into a bubble over it, in its babble voice
const said = { greeting: 0, prayers: 0, ribbon: 0, found_all: 0 };
let bubbleTimer = 0, bubbleShown = false;
function placeBubble(x, y) {
  if (!bubbleShown) return;
  const b = $("bubble");
  b.style.left = `${Math.max(180, Math.min(innerWidth - 180, x))}px`;
  b.style.top = `${Math.max(140, y)}px`;
}
function quailSpeak(kind) {
  const lines = manifest.voice?.[kind];
  if (!Array.isArray(lines) || !lines.length || !inGrove) return;
  const line = lines[said[kind]++ % lines.length];
  const b = $("bubble"), t = $("bubble-text");
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
    li.append(p);
    if (r.word && !mutedNames.has(r.name)) { const w = document.createElement("p"); w.className = "lyric"; w.textContent = r.word; li.append(w); }
    const hide = document.createElement("button"); hide.type = "button"; hide.className = "btn small"; hide.textContent = "Hide this ribbon";
    hide.addEventListener("click", () => { hiddenRibbons.add(r.n); keepSet(HIDDEN_KEY, hiddenRibbons); refreshRibbons(); announce("Hidden."); });
    li.append(hide); ul.append(li);
  }
}
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
const PANELS = ["discoveries", "stones", "decor", "ribbon", "journal", "prayers", "album", "settings"];
const toolFor = { decor: "t-decor", discoveries: "t-disc", journal: "t-journal", prayers: "t-prayers", settings: "t-settings" };
let returnFocus = null;
function open(id) {
  for (const p of PANELS) if (p !== id && !$(p).hidden) close(p, false);
  returnFocus = document.activeElement;
  $(id).hidden = false;
  if (toolFor[id]) $(toolFor[id]).setAttribute("aria-expanded", "true");
  if (id === "journal") renderJournal();
  if (id === "discoveries") renderDiscoveries();
  if (id === "album") renderAlbum();
  if (id === "stones") renderStones();
  if (id === "decor") { send(MSG.decor, "o"); renderDecor(); }
  $(id).focus();
}
function close(id, refocus = true) {
  if ($(id).hidden) return;
  $(id).hidden = true;
  if (toolFor[id]) $(toolFor[id]).setAttribute("aria-expanded", "false");
  if ((id === "stones" || id === "ribbon") && state.m !== 0) send(MSG.close, "0");
  if (id === "decor") send(MSG.decor, "q");
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
  $("s-motion").checked = settings.motion; $("s-bloom").checked = settings.bloom; $("s-text").checked = settings.text;
  $("s-voice").checked = settings.voice;
  $("g-sound").setAttribute("aria-pressed", String(settings.sound)); $("g-sound-label").textContent = settings.sound ? "Sound on" : "Sound off";
  $("g-sound").querySelector(".icon").textContent = settings.sound ? "♪" : "✕";
  document.body.classList.toggle("text-mode", settings.text && inGrove);
  $("textmode").hidden = !(settings.text && inGrove);
  applySound();
}
const setSetting = (k, v) => { settings[k] = v; keepSettings(); syncSoundInputs(); if (k === "motion" || k === "bloom") tellSettings(); };
$("l-sound").addEventListener("click", () => setSetting("sound", !settings.sound));
$("s-sound").addEventListener("change", (e) => setSetting("sound", e.target.checked));
for (const [a, b, k] of [["l-music", "s-music", "music"], ["l-amb", "s-amb", "ambience"]]) {
  for (const id of [a, b]) $(id).addEventListener("input", (e) => setSetting(k, Number(e.target.value)));
}
for (const id of ["l-captions", "s-captions"]) $(id).addEventListener("change", (e) => setSetting("captions", e.target.checked));
$("s-motion").addEventListener("change", (e) => setSetting("motion", e.target.checked));
$("s-bloom").addEventListener("change", (e) => setSetting("bloom", e.target.checked));
$("s-text").addEventListener("change", (e) => { setSetting("text", e.target.checked); renderText(); });
$("s-forget").addEventListener("click", () => {
  if (confirm("Start over? This forgets the tracks you found, your cairn and your ribbon on this device.")) { send(MSG.forget, "1"); announce("Started over."); }
});
$("l-settings").addEventListener("click", () => open("settings"));

function enterGrove() {
  audio.start(); audio.startAmbience();
  inGrove = true; paused = false;
  $("landing").hidden = true; $("hud").hidden = false;
  close("album", false); close("settings", false);
  syncSoundInputs(); renderText();
  send(MSG.pause, "0");
  announce("You are in the grove. A black quail is foraging in front of you. Use the buttons at the bottom to touch the water, the creek stones, the prayer tree, or greet the quail.");
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
  $("paused").hidden = true; $("found").hidden = true;
  $("chat").hidden = true; $("bubble").classList.add("gone"); bubbleShown = false;
  $("hud").hidden = true; $("landing").hidden = false;
  syncSoundInputs();
  $("enter").focus();
}
$("enter").addEventListener("click", enterGrove);
$("listen").addEventListener("click", () => { audio.start(); open("album"); });

// ---- in the grove
for (const b of document.querySelectorAll("[data-spot]")) b.addEventListener("click", () => { audio.start(); send(MSG.pick, b.dataset.spot); });
$("t-disc").addEventListener("click", () => toggle("discoveries"));
$("t-journal").addEventListener("click", () => toggle("journal"));
$("t-settings").addEventListener("click", () => toggle("settings"));
function setPaused(p) {
  paused = p; send(MSG.pause, p ? "1" : "0"); audio.pause(p);
  $("paused").hidden = !p; $("t-pause").textContent = p ? "Resume" : "Pause";
  if (p) $("resume").focus();
}
$("t-pause").addEventListener("click", () => setPaused(!paused));
$("resume").addEventListener("click", () => setPaused(false));
$("t-leave").addEventListener("click", leaveGrove);
$("paused-leave").addEventListener("click", leaveGrove);

// ---- discoveries
function renderDiscoveries() {
  const ul = $("disc-list"); ul.textContent = "";
  let n = 0;
  for (const id of ["spring", "cairn", "ribbon"]) {
    const li = document.createElement("li"); const got = found(id); if (got) n++;
    const s = document.createElement("p"); s.className = "status" + (got ? " found" : ""); s.textContent = got ? "Found" : "Not yet found";
    const t = document.createElement("p"); t.textContent = got ? titleOf(id) : HINT[id];
    li.append(s, t); ul.append(li);
  }
  $("disc-count").textContent = `${n} of 3 found.`;
  $("disc-clue").textContent = lastClue && n < 3 ? `Lately: ${CLUE_TEXT[lastClue]}` : n === 3 ? "You have found every page of the quail's story." : "Watch the quail: where it lingers matters.";
  for (const b of $("spots").querySelectorAll("button")) {
    const id = ["spring", "cairn", "ribbon"][Number(b.dataset.spot)];
    const clue = { pool: 0, stones: 1, ribbon: 2 }[lastClue];
    b.classList.toggle("clue", !found(id) && clue === Number(b.dataset.spot));
  }
}

// ---- stones
function renderStones() {
  const box = $("stone-choices"); box.textContent = "";
  for (let k = 0; k < 5; k++) {
    if ((state.p & (1 << k)) === 0) continue;
    const [, name, note] = STONES[k];
    const l = document.createElement("label"); l.className = "choice";
    const r = document.createElement("input"); r.type = "radio"; r.name = "stone"; r.value = k; r.checked = state.h === k;
    r.addEventListener("change", () => send(MSG.stones, `c ${k}`));
    l.append(r, document.createTextNode(`${name} (${note})`)); box.append(l);
  }
  if (!box.children.length) box.textContent = "Every stone is on the cairn.";
  $("turn-at").textContent = state.h >= 0 ? `${state.o}°` : "—";
  $("turn-l").disabled = $("turn-r").disabled = state.h < 0 || !!state.a;
  const b = $("balance");
  if (state.h < 0) { b.dataset.b = "0"; b.textContent = "Pick a stone to hold."; }
  else { b.dataset.b = String(state.b); b.textContent = BALANCE[state.b] + (state.b ? WHY[state.w] : ""); }
  $("assist").checked = !!state.a;
  const placed = state.c ? state.c.split(",").filter(Boolean) : [];
  $("cairn-desc").textContent = placed.length
    ? `On the cairn, bottom to top: ${placed.map((s) => { const [n, r] = s.split(":"); return `${STONES.find((x) => x[0] === n)?.[1] || n}${Number(r) ? ` at ${r}°` : ""}`; }).join(", ")}. ${placed.length >= 4 ? "Complete." : `${4 - placed.length} more to finish.`}`
    : "The cairn is empty. Four stones finish it.";
  $("place").disabled = placed.length >= 4;
}
$("turn-l").addEventListener("click", () => send(MSG.stones, "t -1"));
$("turn-r").addEventListener("click", () => send(MSG.stones, "t 1"));
$("place").addEventListener("click", () => send(MSG.stones, "p"));
$("assist").addEventListener("change", (e) => send(MSG.stones, `a ${e.target.checked ? 1 : 0}`));
$("rebuild").addEventListener("click", () => send(MSG.stones, "r"));

// ---- the hint while something is in hand
function renderHint() {
  const el = $("hint");
  let html = "";
  if (state.h >= 0 && state.y === 1) {
    html = (state.g ? "<b>It will hold here.</b> " : "<b>It would fall here.</b> Move it over the middle. ") +
      "Click to let go · <kbd>Q</kbd>/<kbd>E</kbd> or the wheel turns it · <kbd>Esc</kbd> puts it back";
    el.dataset.ok = state.g ? "1" : "0";
  } else if (decor.k >= 0) {
    html = (decor.ok ? "<b>It can stand here.</b> " : "<b>Not here.</b> ") + `Click the ground to set the ${DECOR[decor.k][0].toLowerCase()} · <kbd>Esc</kbd> puts it back`;
    el.dataset.ok = decor.ok ? "1" : "0";
  } else if (state.m === 1 && $("stones").hidden && state.c.split(",").filter(Boolean).length < 4) {
    html = "Click a creek stone in the pile to pick it up"; el.dataset.ok = "1";
  }
  el.hidden = !html || !inGrove;
  if (html && el.innerHTML !== html) el.innerHTML = html;
}

// ---- decorating
function renderDecor() {
  const box = $("decor-items");
  if (!box.children.length) {
    DECOR.forEach(([name, ico], k) => {
      const b = document.createElement("button"); b.className = "btn"; b.type = "button"; b.dataset.k = k;
      b.innerHTML = `<span class="ico" aria-hidden="true">${ico}</span>${name}`;
      b.addEventListener("click", () => { audio.start(); send(MSG.decor, `k ${k}`); });
      box.append(b);
    });
  }
  for (const b of box.children) b.setAttribute("aria-pressed", String(Number(b.dataset.k) === decor.k));
  const carrying = decor.k >= 0;
  $("decor-keys").hidden = !carrying;
  $("decor-remove").hidden = !(carrying && decor.m >= 0);
  $("decor-set").disabled = !decor.ok;
  const counts = {}; for (const k of decor.l.split(",").filter((s) => s !== "")) counts[k] = (counts[k] || 0) + 1;
  const list = Object.entries(counts).map(([k, n]) => `${n} ${DECOR[k][0].toLowerCase()}`).join(", ");
  $("decor-status").textContent = carrying
    ? `Carrying the ${DECOR[decor.k][0].toLowerCase()}. ${decor.ok ? "It can stand here." : "Not here: find some grass."}`
    : decor.n ? `In the grove: ${list}. (${16 - decor.n} more can go.)` : "Nothing set out yet.";
  $("decor-clear").disabled = !decor.n;
}
$("t-decor").addEventListener("click", () => toggle("decor"));
for (const b of document.querySelectorAll("[data-move]")) b.addEventListener("click", () => send(MSG.decor, `m ${b.dataset.move}`));
$("decor-set").addEventListener("click", () => send(MSG.decor, "p"));
$("decor-back").addEventListener("click", () => send(MSG.decor, "x"));
$("decor-remove").addEventListener("click", () => send(MSG.decor, "d"));
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
function openRibbon() {
  if (state.r && state.rb) {
    const [f, c, ch, b] = state.rb.split(",").map(Number);
    for (const [name, v] of [["fabric", f], ["colour", c], ["charm", ch], ["branch", Math.floor(b / 8)]]) { const r = document.querySelector(`input[name=${name}][value="${v}"]`); if (r) r.checked = true; }
    $("r-word").value = state.rw || "";
  }
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
addEventListener("keydown", (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  const typing = e.target instanceof HTMLInputElement && (e.target.type === "text" || e.target.type === "range");
  if (e.key === "Escape") {
    if (inGrove && state.h >= 0 && state.y === 1) { send(MSG.stones, "b"); e.preventDefault(); return; }
    if (inGrove && decor.k >= 0) { send(MSG.decor, "x"); e.preventDefault(); return; }
    if (!$("menu").hidden) { setMenu(false); $("t-menu").focus(); e.preventDefault(); return; }
    if (!$("found").hidden) { closeFound(); e.preventDefault(); return; }
    const p = topPanel(); if (p) { close(p); e.preventDefault(); return; }
    if (paused) { setPaused(false); e.preventDefault(); }
    return;
  }
  if (typing || !inGrove || paused || !$("found").hidden) return;
  const k = e.key.toLowerCase();
  if (decor.k >= 0) {
    const mv = { arrowleft: "-2 0", arrowright: "2 0", arrowup: "0 -2", arrowdown: "0 2" }[k];
    if (mv) { send(MSG.decor, `m ${mv}`); e.preventDefault(); return; }
    if (e.key === "Enter" && !(e.target instanceof HTMLButtonElement)) { send(MSG.decor, "p"); e.preventDefault(); return; }
  }
  if (!$("stones").hidden || (state.h >= 0 && state.y === 1)) {
    if (k === "q") { send(MSG.stones, "t -1"); e.preventDefault(); return; }
    if (k === "e") { send(MSG.stones, "t 1"); e.preventDefault(); return; }
    if (e.key === "Enter" && e.target === $("stones")) { send(MSG.stones, "p"); e.preventDefault(); return; }
  }
  if (e.target instanceof HTMLButtonElement || e.target instanceof HTMLInputElement) { if (e.key === "Enter" || e.key === " ") return; }
  if (k === "1" || k === "2" || k === "3") { audio.start(); send(MSG.pick, String(Number(k) - 1)); e.preventDefault(); }
  else if (k === "d") { toggle("discoveries"); e.preventDefault(); }
  else if (k === "j") { toggle("journal"); e.preventDefault(); }
  else if (k === "p") { setPaused(true); e.preventDefault(); }
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
renderDiscoveries(); renderStones(); renderMini();
$("enter").focus();
