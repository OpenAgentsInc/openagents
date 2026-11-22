// Firm reason: 1 (the page's half of The Whispering Spring: panels, journal, settings and sound;
// world-spring.coax decides everything that happens in the grove and tells this page in events).
import { SpringAudio } from "./audio.js";

const $ = (id) => document.getElementById(id);
const MSG = { pick: 120, stones: 121, ribbon: 122, pause: 123, settings: 124, save: 125, extra: 127, forget: 128, close: 129 };
const ACT_EVENT = 90;
const SETTINGS_KEY = "whispering-spring.settings.v1";

// ---- per-viewer settings (this browser only)
const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
const settings = { sound: true, music: 80, ambience: 60, captions: true, motion: reduce, bloom: false, text: false };
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
const WHY = ["", " Try turning it.", " It is wider than the stone below: put wider stones lower.", " The round stone below would roll: only a pebble sits on it."];
const SLIP = ["", "It slid off at that angle. Try turning it.", "Too wide for the stone below; it slid off. Wider stones go lower.", "The round stone rolled it off. Only a pebble sits on a round stone."];
const FABRICS = ["Cotton", "Silk", "Linen"];
const COLOURS = [["Prickly-pear rose", "#bc546b"], ["Bluebonnet", "#4d61b3"], ["Limestone cream", "#e0d6b8"], ["Sage", "#8c9e7a"], ["Persimmon", "#d97338"], ["Indigo", "#383873"]];
const CHARMS = ["None", "Bell", "Bead", "Feather"];
const BRANCHES = ["Near the trunk", "Middle of the branch", "Out at the tip"];
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
let state = { f: 0, x: 0, m: 0, s: 0, h: -1, o: 0, p: 31, a: 0, b: 0, w: 0, c: "", r: 0, rb: "", rw: "" };
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
const trackOf = (id) => manifest.tracks.find((t) => t.id === id);
const titleOf = (id) => trackOf(id)?.title || id;
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
  else if (kind === "slip") { const w = Number(rest) || 1; announce(SLIP[w]); $("balance").textContent = SLIP[w]; $("balance").dataset.b = "2"; }
});

function onState(before) {
  if (!$("stones").hidden && state.m !== 1 && before.m === 1) close("stones");
  renderStones(); renderDiscoveries(); renderJournal(); renderText();
  const any = state.f || state.c || state.r;
  $("enter").textContent = any ? "Return to the Grove" : "Enter the Grove";
}

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
const PANELS = ["discoveries", "stones", "ribbon", "journal", "album", "settings"];
const toolFor = { discoveries: "t-disc", journal: "t-journal", settings: "t-settings" };
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
  $(id).focus();
}
function close(id, refocus = true) {
  if ($(id).hidden) return;
  $(id).hidden = true;
  if (toolFor[id]) $(toolFor[id]).setAttribute("aria-expanded", "false");
  if ((id === "stones" || id === "ribbon") && state.m !== 0) send(MSG.close, "0");
  if (refocus && returnFocus?.isConnected) returnFocus.focus();
}
const toggle = (id) => ($(id).hidden ? open(id) : close(id));
for (const b of document.querySelectorAll("[data-close]")) b.addEventListener("click", () => close(b.closest(".panel").id));
const topPanel = () => [...PANELS].reverse().find((p) => !$(p).hidden);

// ---- landing
function syncSoundInputs() {
  $("l-sound").setAttribute("aria-pressed", String(settings.sound)); $("l-sound").textContent = settings.sound ? "Sound on" : "Sound off";
  $("s-sound").checked = settings.sound;
  $("l-music").value = $("s-music").value = settings.music;
  $("l-amb").value = $("s-amb").value = settings.ambience;
  $("l-captions").checked = $("s-captions").checked = settings.captions;
  $("s-motion").checked = settings.motion; $("s-bloom").checked = settings.bloom; $("s-text").checked = settings.text;
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
  announce("You are in the grove. A black quail is foraging in front of you. Use the buttons at the bottom to touch the water, the creek stones or the ribbon branch.");
  (settings.text ? document.querySelector("#textmode [data-spot]") : $("spots").querySelector("button"))?.focus();
}
function leaveGrove() {
  send(MSG.save, "1"); send(MSG.close, "0");
  audio.stopTrack();
  inGrove = false; paused = false;
  for (const p of PANELS) close(p, false);
  $("paused").hidden = true; $("found").hidden = true;
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
  $("disc-clue").textContent = lastClue && n < 3 ? `Lately: ${CLUE_TEXT[lastClue]}` : n === 3 ? "You have found every track in the grove." : "Watch the quail: where it lingers matters.";
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
    for (const [name, v] of [["fabric", f], ["colour", c], ["charm", ch], ["branch", b]]) { const r = document.querySelector(`input[name=${name}][value="${v}"]`); if (r) r.checked = true; }
    $("r-word").value = state.rw || "";
  }
  open("ribbon");
}
$("ribbon-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const v = (n) => Number(document.querySelector(`input[name=${n}]:checked`)?.value || 0);
  const word = $("r-word").value.replace(/[;,|\n\r\t]/g, " ").slice(0, 40);
  audio.start();
  send(MSG.ribbon, `${v("fabric")},${v("colour")},${v("charm")},${v("branch")},${word}`);
  $("ribbon").hidden = true;
  announce(`You tie a ${COLOURS[v("colour")][0].toLowerCase()} ${FABRICS[v("fabric")].toLowerCase()} ribbon ${BRANCHES[v("branch")].toLowerCase()}. It catches the wind.`);
  if (returnFocus?.isConnected) returnFocus.focus();
});

// ---- found
let foundId = "";
function showFound(id) {
  foundId = id;
  const extra = { spring: "A handwritten lyric is in your journal.", cairn: "An alternate take is in your journal.", ribbon: "A voice note is in your journal." }[id];
  $("found-title").textContent = titleOf(id);
  $("found-extra").textContent = extra || "";
  $("found").hidden = false;
  announce(`You found a track: ${titleOf(id)}. ${extra || ""}`);
  $("found-play").focus();
}
const closeFound = () => { $("found").hidden = true; (settings.text ? document.querySelector("#textmode [data-spot]") : $("spots").querySelector("button"))?.focus(); };
$("found-play").addEventListener("click", () => { closeFound(); playTrack(foundId); open("journal"); });
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
  for (const t of manifest.tracks) {
    const li = document.createElement("li");
    const h = document.createElement("h3"); h.textContent = `${t.n}. ${found(t.id) ? t.title : "Not yet found"}`;
    li.append(h);
    if (!found(t.id)) { const p = document.createElement("p"); p.className = "muted small"; p.textContent = HINT[t.id]; li.append(p); ul.append(li); continue; }
    const row = document.createElement("div"); row.className = "row"; row.style.marginTop = "8px";
    row.append(playButton(t.id, "Play full track"), downloadButton("Download (.wav)", () => audio.download(t.id), `${slug(t.title) || t.id}.wav`));
    li.append(row);
    const x = manifest.extras.find((e) => e.track === t.id);
    if (x) {
      const box = document.createElement("div"); box.style.marginTop = "10px";
      const xh = document.createElement("p"); xh.className = "status found"; xh.textContent = x.title; box.append(xh);
      const xr = document.createElement("div"); xr.className = "row"; xr.style.marginTop = "6px";
      if (x.kind === "lyric") {
        const p = document.createElement("p"); p.className = "lyric"; p.textContent = x.text; box.append(p);
        xr.append(downloadButton("Download lyric (.txt)", async () => new Blob([x.text], { type: "text/plain" }), "lyric.txt"));
      } else if (x.kind === "alternate") {
        xr.append(playButton(x.id, "Play alternate take"), downloadButton("Download (.wav)", () => audio.download(x.id), "alternate-take.wav"));
      } else if (x.kind === "voice") {
        xr.append(playButton(x.id, "Play voice note"));
        const p = document.createElement("p"); p.className = "transcript"; p.textContent = `Transcript: ${x.transcript}`; box.append(p);
        xr.append(downloadButton("Download transcript (.txt)", async () => new Blob([x.transcript], { type: "text/plain" }), "voice-note-transcript.txt"));
      }
      const kept = (state.x & (1 << x.id)) !== 0;
      const keep = document.createElement("button"); keep.type = "button"; keep.className = "btn"; keep.textContent = kept ? "Saved to journal ✓" : "Save to journal";
      keep.disabled = kept; keep.addEventListener("click", () => send(MSG.extra, String(x.id)));
      xr.append(keep); box.append(xr); li.append(box);
    }
    ul.append(li);
  }
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
    if (!$("found").hidden) { closeFound(); e.preventDefault(); return; }
    const p = topPanel(); if (p) { close(p); e.preventDefault(); return; }
    if (paused) { setPaused(false); e.preventDefault(); }
    return;
  }
  if (typing || !inGrove || paused || !$("found").hidden) return;
  const k = e.key.toLowerCase();
  if (!$("stones").hidden) {
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
renderDiscoveries(); renderStones();
$("enter").focus();
