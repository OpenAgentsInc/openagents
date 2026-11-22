// Firm reason: 1 (browser network glue for The Whispering Spring's shared grove: the OpenAgents API's
// rooms, coax/web/api/room.c and migrations/0014_verse_rooms.sql, do the rules).
// Two rooms, both moderated by the platform (a muted or banned player says nothing; an admin can
// take a line out: POST /api/playtest/chat/<room>/remove):
//   whispering-spring          the grove's chat: the last 50 lines, 500 characters, 5 lines in 10 s
//   whispering-spring-ribbons  one line per ribbon tied: "ribbon1|fabric,colour,charm,limb,slot|word"
// A visitor is a playtest guest (POST /api/playtest/player), its key kept where the host keeps it
// (localStorage verse.player), sent as X-Verse-Player. Nothing here decides what the grove shows:
// the ribbons go to world-spring.coax as rows, which draws them.

const API = "/api/playtest";
export const CHAT_ROOM = "whispering-spring";
export const RIBBON_ROOM = "whispering-spring-ribbons";
const LIMBS = 6, SLOTS = 6, SHOWN = 36;
const ORDER = [2, 3, 1, 4, 0, 5];

function stored() { try { const p = JSON.parse(localStorage.getItem("verse.player") || "null"); return p && typeof p.key === "string" ? p : null; } catch { return null; } }

export class GroveRoom {
  constructor() { this.player = stored(); this.chatLast = 0; this.lines = []; this.online = false; }

  async ensurePlayer() {
    if (this.player) return this.player;
    const r = await fetch(`${API}/player`, { method: "POST", headers: { "content-type": "application/json" }, body: "{}" });
    if (!r.ok) throw Error(`player ${r.status}`);
    const p = await r.json();
    if (!p || typeof p.key !== "string") throw Error("no player key");
    this.player = { key: p.key, id: p.id, account: p.account || "" };
    try { localStorage.setItem("verse.player", JSON.stringify(this.player)); } catch {}
    return this.player;
  }
  async post(path, body) {
    const p = await this.ensurePlayer();
    const r = await fetch(`${API}/${path}`, { method: "POST", headers: { "content-type": "application/json", "x-verse-player": p.key }, body: JSON.stringify(body) });
    let json = {}; try { json = await r.json(); } catch {}
    return { ok: r.ok, status: r.status, json };
  }
  async get(path) {
    const headers = this.player ? { "x-verse-player": this.player.key } : {};
    const r = await fetch(`${API}/${path}`, { headers });
    if (!r.ok) throw Error(`${path} ${r.status}`);
    return r.json();
  }

  // ---- chat
  async chatSince() {
    const j = await this.get(`chat/${CHAT_ROOM}?after=${this.chatLast}`);
    this.online = true;
    const fresh = (j.messages || []).filter((m) => m.n > this.chatLast);
    if (fresh.length) this.chatLast = Math.max(...fresh.map((m) => m.n));
    else if (typeof j.last === "number" && j.last < this.chatLast) this.chatLast = j.last;   // the room was reset
    return fresh;
  }
  say(text) { return this.post(`chat/${CHAT_ROOM}`, { text: String(text).slice(0, 300) }); }
  rename(name) { return this.post("player/name", { name: String(name).slice(0, 40) }); }
  // who else is here (and the room's newest line): the platform counts visitors seen in the last 6 s
  async presence() {
    const r = await this.post(`presence/${CHAT_ROOM}`, { x: 0, y: 0, z: 0, yaw: 0 });
    if (!r.ok) return null;
    return { you: r.json.you, others: (r.json.others || []).length };
  }

  // ---- ribbons
  async ribbons() {
    const j = await this.get(`chat/${RIBBON_ROOM}?after=0`);
    this.online = true;
    return placeRibbons(j.messages || []);
  }
  tie(r) {
    const word = r.share ? String(r.word || "").replace(/[|\n\r\t]/g, " ").slice(0, 40) : "";
    return this.post(`chat/${RIBBON_ROOM}`, { text: `ribbon1|${r.fabric},${r.colour},${r.charm},${r.limb},${r.slot}|${word}` });
  }
}

// Every visitor's screen places the same ribbons on the same tie points: newest first, each on its
// asked-for slot or the free one nearest its limb's middle; at most SHOWN, older ones blown away.
export function placeRibbons(messages, hidden = new Set()) {
  const taken = new Set(), out = [];
  const lines = [...messages].sort((a, b) => b.n - a.n);
  for (const m of lines) {
    if (out.length >= SHOWN) break;
    const parts = String(m.text || "").split("|");
    if (parts[0] !== "ribbon1" || parts.length < 2) continue;
    const v = parts[1].split(",").map(Number);
    if (v.length !== 5 || v.some((x) => !Number.isInteger(x))) continue;
    const [fabric, colour, charm, limb, slot] = v;
    if (fabric < 0 || fabric > 2 || colour < 0 || colour > 5 || charm < 0 || charm > 3 || limb < 0 || limb >= LIMBS) continue;
    if (hidden.has(m.n)) continue;
    let s = slot >= 0 && slot < SLOTS && !taken.has(limb * 8 + slot) ? slot : ORDER.find((c) => !taken.has(limb * 8 + c));
    if (s === undefined) continue;
    taken.add(limb * 8 + s);
    out.push({ n: m.n, fabric, colour, charm, limb, slot: s, word: parts.slice(2).join("|").trim(), name: m.name, mine: !!m.mine, at: m.at });
  }
  return out;
}
