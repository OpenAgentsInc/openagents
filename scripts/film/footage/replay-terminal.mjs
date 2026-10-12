// Renders an asciicast recording (terminal.mjs) into an exact-size H.264
// clip, frame by frame: xterm.js in headless Chrome with Paper Mono and a
// Coder Noir palette, fed the recorded output up to each frame's time on its
// own clock, one screenshot per frame. Long idle waits are shortened; the
// bytes and their order are the recording's.
//
// node replay-terminal.mjs CAST OUT.mp4 [--max-gap S] [--from S] [--to S]
//   [--speed FROM:TO:X ...] [--hold S]
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import * as c from "./cdp.mjs";
import { NODE_CACHE } from "./terminal.mjs";

const here = path.dirname(new URL(import.meta.url).pathname);
const repoRoot = path.resolve(here, "../../..");
export const WIDTH = 1512;
export const HEIGHT = 982;
export const DPR = 2;
export const FONT_SIZE = Number(process.env.FILM_TERM_FONT || 19);
export const PAD = 36;

const THEME = {
  background: "#0a0a0b",
  foreground: "#e6e6e3",
  cursor: "#e6e6e3",
  cursorAccent: "#0a0a0b",
  selectionBackground: "#3a3a3f",
  black: "#1c1c1f",
  red: "#ef6b73",
  green: "#7fd88f",
  yellow: "#e8c76a",
  blue: "#78a9ff",
  magenta: "#c49bff",
  cyan: "#6fd6d6",
  white: "#d7d7d2",
  brightBlack: "#6b6b70",
  brightRed: "#ff8a91",
  brightGreen: "#9be8a8",
  brightYellow: "#f3d98c",
  brightBlue: "#9cc0ff",
  brightMagenta: "#d6b8ff",
  brightCyan: "#93e6e6",
  brightWhite: "#ffffff",
};

export function pageHtml(cols, rows) {
  const xterm = path.join(NODE_CACHE, "node_modules/@xterm/xterm");
  const font = path.join(repoRoot, "crates/paper-mono/fonts/PaperMono-Variable.woff2");
  return `<!doctype html><html><head><meta charset="utf-8">
<link rel="stylesheet" href="file://${xterm}/css/xterm.css">
<style>
@font-face { font-family: "Paper Mono"; src: url("file://${font}") format("woff2"); font-weight: 100 900; }
html, body { margin: 0; background: ${THEME.background}; width: ${WIDTH}px; height: ${HEIGHT}px; overflow: hidden; }
#t { position: absolute; left: ${PAD}px; top: ${PAD}px; }
.xterm-viewport { overflow: hidden !important; background: ${THEME.background} !important; }
/* The block cursor always drawn, never blinking or switching to its
   unfocused look between shots: a frame without it flickers on film. */
.xterm-rows .xterm-cursor { background-color: ${THEME.cursor} !important; color: ${THEME.cursorAccent} !important;
  animation: none !important; outline: none !important; box-shadow: none !important; }
</style></head><body><div id="t"></div>
<script src="file://${xterm}/lib/xterm.js"></script>
<script>
window.ready = document.fonts.load("${FONT_SIZE}px 'Paper Mono'").then(() => {
  const term = new Terminal({ cols: ${cols}, rows: ${rows}, fontFamily: "'Paper Mono', monospace",
    fontSize: ${FONT_SIZE}, lineHeight: 1.22, fontWeight: 400, fontWeightBold: 650,
    cursorBlink: false, cursorStyle: "block", cursorInactiveStyle: "block", allowProposedApi: true, scrollback: 0,
    theme: ${JSON.stringify(THEME)} });
  term.open(document.getElementById("t"));
  term.focus();
  window.term = term;
  const d = term._core._renderService.dimensions.css.cell;
  return { cellW: d.width, cellH: d.height };
});
// After each write, wait until the cursor is drawn again before the shot
// (two frames at least, at most ten): the DOM renderer redraws the
// cursor's row a frame or so after the text, and a shot in between shows
// the cursor missing, a flicker on film.
window.feed = (s) => new Promise((ok) => term.write(s, () => {
  let n = 0;
  const drawn = () => term._core.coreService.isCursorHidden || !!document.querySelector(".xterm-rows .xterm-cursor");
  const tick = () => (++n >= 2 && (drawn() || n >= 10) ? ok() : requestAnimationFrame(tick));
  requestAnimationFrame(tick);
}));
</script></body></html>`;
}

/// The cols and rows that fill the frame at FONT_SIZE.
export async function fit() {
  const cdp = await c.launch({ width: WIDTH, height: HEIGHT, dpr: 1 });
  const file = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "oa-term-")), "probe.html");
  fs.writeFileSync(file, pageHtml(80, 24));
  await c.navigate(cdp, `file://${file}`);
  const cell = await c.evaluate(cdp, "window.ready");
  await cdp.close();
  return {
    cols: Math.floor((WIDTH - 2 * PAD) / cell.cellW),
    rows: Math.floor((HEIGHT - 2 * PAD) / cell.cellH),
  };
}

/// Maps recording time to film time: gaps longer than maxGap shrink to
/// maxGap, and [from, to] ranges in `speed` play X times faster.
function timeline(events, { maxGap, speed, idle }) {
  let film = 0;
  let prev = 0;
  // Waits with no meaningful change (only spinners and timers) longer than
  // `idle` play in `idle` seconds.
  const marks = events.filter((e) => e[1] === "c" || e[1] === "i").map((e) => e[0]);
  const auto = [];
  for (let i = 1; i < marks.length; i++) {
    const gap = marks[i] - marks[i - 1];
    if (gap > idle) auto.push({ from: marks[i - 1] + 0.15, to: marks[i], x: (gap - 0.15) / (idle - 0.15) });
  }
  const rate = (t) => {
    for (const s of speed) if (t >= s.from && t < s.to) return s.x;
    for (const s of auto) if (t >= s.from && t < s.to) return s.x;
    return 1;
  };
  return events.map(([t, kind, data]) => {
    let dt = t - prev;
    prev = t;
    dt = dt / rate(t);
    if (dt > maxGap) dt = maxGap;
    film += dt;
    return { t: film, kind, data, at: t };
  });
}

/// How many frames a half-finished redraw may wait. Coder's synchronized
/// updates can stay open for a few hundred ms around a reply arriving; a
/// shorter wait shows the half-drawn screen, cursor missing.
const HOLD_FRAMES = 12;

/// Where `s` starts a redraw it doesn't finish: the last cursor hide or
/// synchronized-update start with no show or end after it; -1 when settled.
function unsettled(s) {
  const open = Math.max(s.lastIndexOf("\x1b[?25l"), s.lastIndexOf("\x1b[?2026h"));
  const close = Math.max(s.lastIndexOf("\x1b[?25h"), s.lastIndexOf("\x1b[?2026l"));
  return open > close ? open : -1;
}

export async function replay(castFile, out, opts = {}) {
  const { maxGap = 0.6, speed = [], hold = 1.5, from, to, idle = 0.8 } = opts;
  const lines = fs.readFileSync(castFile, "utf8").trim().split("\n");
  const header = JSON.parse(lines[0]);
  const events = lines.slice(1).map((l) => JSON.parse(l));
  const tl = timeline(events, { maxGap, speed, idle });
  const start = from === undefined ? 0 : (tl.find((e) => e.at >= from)?.t ?? 0);
  const last = to === undefined ? tl.length : tl.findLastIndex((e) => e.at <= to) + 1;
  const endAt = (last > 0 ? tl[last - 1].t : 0) + hold;

  const work = fs.mkdtempSync(path.join(os.tmpdir(), "oa-term-replay-"));
  const page = path.join(work, "replay.html");
  fs.writeFileSync(page, pageHtml(header.width, header.height));
  const cdp = await c.launch({ width: WIDTH, height: HEIGHT, dpr: DPR });
  await c.navigate(cdp, `file://${page}`);
  await c.evaluate(cdp, "window.ready");
  const frames = path.join(work, "frames");
  fs.mkdirSync(frames);
  let k = 0;
  // Everything before the first frame, at once.
  let pending = "";
  while (k < tl.length && tl[k].t <= start) {
    if (tl[k].kind === "o") pending += tl[k].data;
    k++;
  }
  if (pending) await c.evaluate(cdp, `window.feed(${JSON.stringify(pending)})`);
  let n = 0;
  // A frame never shows half a redraw: output after the last point where
  // Coder hides the cursor (or opens a synchronized update) without showing
  // it again waits for the next frame, so the cursor doesn't flicker. It
  // waits at most HOLD_FRAMES.
  let carry = "";
  let held = 0;
  for (let t = start; t < endAt; t += 1 / 30) {
    let chunk = carry;
    while (k < last && tl[k].t <= t) {
      if (tl[k].kind === "o") chunk += tl[k].data;
      k++;
    }
    const cut = unsettled(chunk);
    if (cut >= 0 && held < HOLD_FRAMES && k < last) {
      carry = chunk.slice(cut);
      chunk = chunk.slice(0, cut);
      held++;
    } else {
      carry = "";
      held = 0;
    }
    if (chunk) await c.evaluate(cdp, `window.feed(${JSON.stringify(chunk)})`);
    await c.shoot(cdp, frames, n++);
  }
  await cdp.close();
  c.encodeFrames(frames, out, WIDTH * DPR, HEIGHT * DPR);
  fs.rmSync(work, { recursive: true, force: true });
  return { frames: n, seconds: n / 30 };
}

/// Renders the recording at exactly the given times (seconds on the
/// recording's own clock, never decreasing), one frame each: for footage
/// cut on a clock shared with other recordings. A jump forward feeds
/// everything in between at once, so the screen is right after a cut.
export async function replayAt(castFile, out, times) {
  const lines = fs.readFileSync(castFile, "utf8").trim().split("\n");
  const header = JSON.parse(lines[0]);
  const events = lines.slice(1).map((l) => JSON.parse(l)).filter((e) => e[1] === "o");
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "oa-term-replay-"));
  const page = path.join(work, "replay.html");
  fs.writeFileSync(page, pageHtml(header.width, header.height));
  const cdp = await c.launch({ width: WIDTH, height: HEIGHT, dpr: DPR });
  await c.navigate(cdp, `file://${page}`);
  await c.evaluate(cdp, "window.ready");
  const frames = path.join(work, "frames");
  fs.mkdirSync(frames);
  let k = 0;
  let n = 0;
  let carry = "";
  let held = 0;
  for (const t of times) {
    let chunk = carry;
    while (k < events.length && events[k][0] <= t) chunk += events[k++][2];
    const cut = unsettled(chunk);
    if (cut >= 0 && held < HOLD_FRAMES && k < events.length) {
      carry = chunk.slice(cut);
      chunk = chunk.slice(0, cut);
      held++;
    } else {
      carry = "";
      held = 0;
    }
    if (chunk) await c.evaluate(cdp, `window.feed(${JSON.stringify(chunk)})`);
    await c.shoot(cdp, frames, n++);
  }
  await cdp.close();
  c.encodeFrames(frames, out, WIDTH * DPR, HEIGHT * DPR);
  fs.rmSync(work, { recursive: true, force: true });
  return { frames: n, seconds: n / 30 };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [cast, out, ...rest] = process.argv.slice(2);
  const opts = { speed: [] };
  for (let i = 0; i < rest.length; i++) {
    const v = rest[i + 1];
    if (rest[i] === "--max-gap") (opts.maxGap = Number(v)), i++;
    else if (rest[i] === "--from") (opts.from = Number(v)), i++;
    else if (rest[i] === "--to") (opts.to = Number(v)), i++;
    else if (rest[i] === "--idle") (opts.idle = Number(v)), i++;
    else if (rest[i] === "--hold") (opts.hold = Number(v)), i++;
    else if (rest[i] === "--speed") {
      const [a, b, x] = v.split(":").map(Number);
      opts.speed.push({ from: a, to: b, x });
      i++;
    } else if (rest[i] === "--fit") {
      console.log(await fit());
      process.exit(0);
    }
  }
  console.log(await replay(cast, out, opts));
}
