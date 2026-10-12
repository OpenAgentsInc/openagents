// A real terminal session for the launch film (#11432): a command runs in a
// real pseudo-terminal (ptyrun.py), a headless xterm.js answers its
// terminal queries the way a terminal app would, scripted keystrokes type
// into it, and every output byte is kept with its time as an asciicast v2
// recording. replay-terminal.mjs renders the recording frame by frame.
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

const here = path.dirname(new URL(import.meta.url).pathname);
export const NODE_CACHE =
  process.env.FILM_NODE_CACHE || path.join(process.env.HOME, "work/film-out/.cache/node");
const require = createRequire(path.join(NODE_CACHE, "package.json"));
const { Terminal } = require("@xterm/headless");

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/// The screen without what changes on its own while a tool or model works.
export function meaningful(text) {
  return text
    .replace(/[\u2800-\u28ff\u25d0-\u25d3\u25cb\u25cf\u2022\u00b7]/g, "")
    .replace(/\d+(\.\d+)?\s*(ms|s|m|h)\b/g, "")
    .replace(/(Working|Thinking|Running)[.\u2026]*/g, "$1")
    .replace(/[ \t]+$/gm, "");
}

export class Session {
  constructor({ cols, rows, command, cwd, env }) {
    this.cols = cols;
    this.rows = rows;
    this.events = [];
    this.term = new Terminal({ cols, rows, allowProposedApi: true, scrollback: 5000 });
    this.t0 = performance.now();
    // The same moment on the wall clock, so other recordings line up.
    this.epoch0 = Date.now() / 1000;
    this.child = spawn("python3", [path.join(here, "ptyrun.py"), String(cols), String(rows), "--", ...command], {
      cwd,
      env: { ...process.env, ...env },
      stdio: ["pipe", "pipe", "inherit"],
    });
    this.child.stdout.on("data", (buf) => {
      const text = buf.toString("utf8");
      this.events.push([(performance.now() - this.t0) / 1000, "o", text]);
      this.term.write(text);
    });
    // Answers to terminal queries (cursor position, device attributes...).
    this.term.onData((d) => this.input(d, false));
    this.exited = new Promise((ok) => this.child.on("exit", ok));
    // Marks when the screen changes in a way a viewer would notice: spinner
    // frames and ticking timers don't count. The replay shortens the waits
    // between these marks.
    let seen = "";
    this.watch = setInterval(() => {
      const now = meaningful(this.screen());
      if (now !== seen) {
        seen = now;
        this.events.push([(performance.now() - this.t0) / 1000, "c", ""]);
      }
    }, 150);
    this.exited.then(() => clearInterval(this.watch));
  }
  input(data, mark = true) {
    if (mark) this.events.push([(performance.now() - this.t0) / 1000, "i", data]);
    if (this.child.stdin.writable) this.child.stdin.write(data);
  }
  /// The visible screen as text.
  screen() {
    const b = this.term.buffer.active;
    const lines = [];
    for (let y = 0; y < this.rows; y++) lines.push(b.getLine(b.viewportY + y)?.translateToString(true) ?? "");
    return lines.join("\n");
  }
  async waitFor(pattern, timeoutMs = 120000) {
    const re = typeof pattern === "string" ? new RegExp(pattern.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")) : pattern;
    const end = Date.now() + timeoutMs;
    while (Date.now() < end) {
      if (re.test(this.screen())) return;
      await sleep(100);
    }
    throw new Error(`timed out waiting for ${re}\n--- screen ---\n${this.screen()}`);
  }
  /// Waits until the screen stops changing for `quietMs`.
  async settle(quietMs = 1500, timeoutMs = 600000) {
    const end = Date.now() + timeoutMs;
    let last = this.screen();
    let since = Date.now();
    while (Date.now() < end) {
      await sleep(200);
      const now = this.screen();
      if (now !== last) {
        last = now;
        since = Date.now();
      } else if (Date.now() - since >= quietMs) return;
    }
  }
  async type(text, { base = 45, jitter = 40 } = {}) {
    for (const ch of text) {
      this.input(ch);
      await sleep(base + Math.random() * jitter);
    }
  }
  /// Wall-clock seconds of a moment on this session's clock.
  epoch(t) {
    return this.epoch0 + t;
  }
  /// Now, in this session's seconds.
  now() {
    return (performance.now() - this.t0) / 1000;
  }
  mark(label) {
    this.events.push([(performance.now() - this.t0) / 1000, "m", label]);
  }
  save(file) {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    const header = {
      version: 2,
      width: this.cols,
      height: this.rows,
      timestamp: this.epoch0,
      env: { TERM: "xterm-256color" },
    };
    fs.writeFileSync(file, [JSON.stringify(header), ...this.events.map((e) => JSON.stringify(e))].join("\n") + "\n");
  }
  kill() {
    clearInterval(this.watch);
    try {
      this.child.kill();
    } catch {}
  }
}
