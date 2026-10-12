// A small Chrome DevTools Protocol driver for the launch film's screen
// footage (#11432): headless Chrome without browser chrome, an exact CSS
// viewport and device scale, and two ways to record.
//
// - Recorder (live): Page.startScreencast frames with their timestamps,
//   turned into constant 30 fps H.264 by ffmpeg. For pages whose motion comes
//   from the network (a streaming answer, a live sidebar).
// - shoot (frame-exact): one Page.captureScreenshot per frame while the
//   caller advances its own clock. For replays (the terminal).
//
// No npm dependencies: Node's built-in WebSocket and fetch.
import { spawn, execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const CHROME =
  process.env.CHROME ||
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

export async function launch({ width, height, dpr, mobile = false, userAgent } = {}) {
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), "oa-film-chrome-"));
  const args = [
    ...(process.env.FILM_HEADED ? ["--window-position=6000,6000", "--disable-backgrounding-occluded-windows"] : ["--headless=new"]),
    "--remote-debugging-port=0",
    `--user-data-dir=${profile}`,
    `--force-device-scale-factor=${dpr}`,
    "--hide-scrollbars",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-extensions",
    "--disable-background-timer-throttling",
    "--disable-renderer-backgrounding",
    "--force-color-profile=srgb",
    "--font-render-hinting=none",
    `--window-size=${width},${height}`,
    "about:blank",
  ];
  const proc = spawn(CHROME, args, { stdio: "ignore" });
  const portFile = path.join(profile, "DevToolsActivePort");
  let port;
  for (let i = 0; i < 100 && !port; i++) {
    await sleep(100);
    if (fs.existsSync(portFile)) port = fs.readFileSync(portFile, "utf8").split("\n")[0];
  }
  if (!port) throw new Error("Chrome did not start");
  const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const page = targets.find((t) => t.type === "page");
  const cdp = await connect(page.webSocketDebuggerUrl);
  cdp.close = async () => {
    try {
      cdp.ws.close();
    } catch {}
    proc.kill();
    await sleep(300);
    fs.rmSync(profile, { recursive: true, force: true });
  };
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");
  await cdp.send("Network.enable");
  await cdp.send("Emulation.setDeviceMetricsOverride", {
    width,
    height,
    deviceScaleFactor: dpr,
    mobile,
    screenWidth: width,
    screenHeight: height,
  });
  if (mobile) {
    await cdp.send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  }
  // Pages draw as the focused window (solid cursors, focus rings).
  await cdp.send("Emulation.setFocusEmulationEnabled", { enabled: true });
  // No text caret: its blink lands unevenly on frames, worst where waits
  // are shortened, and reads as flicker in the film.
  await cdp.send("Page.addScriptToEvaluateOnNewDocument", {
    source: `addEventListener("DOMContentLoaded", () => {
      const s = document.createElement("style");
      s.textContent = "*, *::before, *::after { caret-color: transparent !important; }";
      document.head.append(s);
    });`,
  });
  if (userAgent) await cdp.send("Emulation.setUserAgentOverride", { userAgent });
  await cdp.send("Emulation.setEmulatedMedia", {
    features: [{ name: "prefers-color-scheme", value: "dark" }],
  });
  cdp.size = { width, height, dpr };
  return cdp;
}

async function connect(url) {
  const ws = new WebSocket(url);
  await new Promise((ok, fail) => {
    ws.onopen = ok;
    ws.onerror = fail;
  });
  let id = 0;
  const pending = new Map();
  const listeners = new Map();
  ws.onmessage = (e) => {
    const msg = JSON.parse(e.data);
    if (msg.id && pending.has(msg.id)) {
      const { ok, fail } = pending.get(msg.id);
      pending.delete(msg.id);
      msg.error ? fail(new Error(`${msg.error.message} ${msg.error.data || ""}`)) : ok(msg.result);
    } else if (msg.method) {
      for (const fn of listeners.get(msg.method) || []) fn(msg.params);
    }
  };
  return {
    ws,
    send(method, params = {}) {
      const n = ++id;
      ws.send(JSON.stringify({ id: n, method, params }));
      return new Promise((ok, fail) => pending.set(n, { ok, fail }));
    },
    on(method, fn) {
      if (!listeners.has(method)) listeners.set(method, []);
      listeners.get(method).push(fn);
      return () => listeners.set(method, listeners.get(method).filter((f) => f !== fn));
    },
  };
}

/// Evaluates an expression in the page and returns its value.
export async function evaluate(cdp, expression) {
  const r = await cdp.send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (r.exceptionDetails) throw new Error(`${expression}: ${JSON.stringify(r.exceptionDetails)}`);
  return r.result.value;
}

export async function navigate(cdp, url) {
  const loaded = new Promise((ok) => {
    const off = cdp.on("Page.loadEventFired", () => {
      off();
      ok();
    });
  });
  await cdp.send("Page.navigate", { url });
  await loaded;
}

/// Waits until `expression` is truthy in the page.
export async function waitFor(cdp, expression, timeoutMs = 30000, label = expression) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    try {
      if (await evaluate(cdp, expression)) return;
    } catch {}
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${label}`);
}

/// The CSS-pixel centre of the first element matching `selector`.
export async function center(cdp, selector) {
  return evaluate(
    cdp,
    `(() => { const e = document.querySelector(${JSON.stringify(selector)}); if (!e) return null;
      const r = e.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`,
  );
}

/// Marks the first visible element matching `selector` whose text is
/// `text` with data-film-target and returns that selector.
export async function byText(cdp, selector, text) {
  const ok = await evaluate(
    cdp,
    `(() => { document.querySelectorAll('[data-film-target]').forEach(e => e.removeAttribute('data-film-target'));
      const e = [...document.querySelectorAll(${JSON.stringify(selector)})].find(e => e.offsetParent && e.innerText.trim() === ${JSON.stringify(text)});
      if (!e) return false; e.setAttribute('data-film-target', '1'); return true; })()`,
  );
  if (!ok) throw new Error(`no ${selector} with text ${text}`);
  return "[data-film-target]";
}

export async function click(cdp, selector, { touch = false } = {}) {
  const p = await center(cdp, selector);
  if (!p) throw new Error(`no element ${selector}`);
  if (touch) {
    await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [p] });
    await sleep(60);
    await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    return;
  }
  await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", ...p });
  await sleep(40);
  await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", button: "left", clickCount: 1, ...p });
  await sleep(50);
  await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", button: "left", clickCount: 1, ...p });
}

/// Types `text` one character at a time with a human rhythm.
export async function type(cdp, text, { base = 55, jitter = 45 } = {}) {
  for (const ch of text) {
    await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", text: ch, unmodifiedText: ch, key: ch });
    await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", key: ch });
    const pause = ch === " " ? base * 1.4 : base;
    await sleep(pause + Math.random() * jitter);
  }
}

export async function pressEnter(cdp) {
  const k = { key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 };
  await cdp.send("Input.dispatchKeyEvent", { type: "rawKeyDown", ...k });
  await cdp.send("Input.dispatchKeyEvent", { type: "char", text: "\r", ...k });
  await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", ...k });
}

/// A live recording: screencast frames with their capture times.
export class Recorder {
  constructor(cdp, dir) {
    this.cdp = cdp;
    this.dir = dir;
    this.frames = [];
    fs.rmSync(dir, { recursive: true, force: true });
    fs.mkdirSync(dir, { recursive: true });
  }
  async start() {
    const { width, height, dpr } = this.cdp.size;
    this.off = this.cdp.on("Page.screencastFrame", async (f) => {
      const file = path.join(this.dir, `${String(this.frames.length).padStart(6, "0")}.jpg`);
      fs.writeFileSync(file, Buffer.from(f.data, "base64"));
      this.frames.push({ file, t: f.metadata.timestamp });
      this.cdp.send("Page.screencastFrameAck", { sessionId: f.sessionId }).catch(() => {});
    });
    await this.cdp.send("Page.startScreencast", {
      format: "jpeg",
      quality: 97,
      maxWidth: width * dpr,
      maxHeight: height * dpr,
      everyNthFrame: 1,
    });
    this.t0 = Date.now() / 1000;
  }
  /// Seconds since start, on the frames' clock.
  now() {
    return Date.now() / 1000 - this.t0;
  }
  async stop() {
    await this.cdp.send("Page.stopScreencast");
    await sleep(200);
    this.off();
    this.t1 = Date.now() / 1000;
  }
  /// Encodes [from, to] seconds (wall clock since start) into `out` at
  /// 30 fps constant frame rate and the exact device pixel size.
  /// `speed` lists [{from, to, x}] wall-clock ranges played x times faster
  /// (a dull wait shortened).
  encode(out, { from = 0, to, speed = [] } = {}) {
    const { width, height, dpr } = this.cdp.size;
    const end = to ?? this.t1 - this.t0;
    const list = [];
    const fps = 30;
    const rate = (t) => speed.find((s) => t >= s.from && t < s.to)?.x ?? 1;
    let k = 0;
    for (let t = from; t < end; t += rate(t) / fps) {
      const abs = this.t0 + t;
      while (k + 1 < this.frames.length && this.frames[k + 1].t <= abs) k++;
      list.push(`file '${this.frames[k].file}'\nduration ${1 / fps}`);
    }
    const listFile = path.join(this.dir, "frames.txt");
    fs.writeFileSync(listFile, list.join("\n") + "\n");
    encodeList(listFile, out, width * dpr, height * dpr);
  }
}

/// Frame-exact capture: one screenshot per call, written as frame N.
export async function shoot(cdp, dir, n) {
  const r = await cdp.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  fs.writeFileSync(path.join(dir, `${String(n).padStart(6, "0")}.png`), Buffer.from(r.data, "base64"));
}

export function encodeList(listFile, out, w, h) {
  fs.mkdirSync(path.dirname(out), { recursive: true });
  execFileSync(
    "ffmpeg",
    ["-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i", listFile,
      "-vf", `scale=${w}:${h}:flags=lanczos,pad=ceil(iw/2)*2:ceil(ih/2)*2:0:0:color=0x0a0a0b,fps=30,format=yuv420p`, "-r", "30",
      "-c:v", "libx264", "-preset", "slow", "-crf", "12", "-an", "-movflags", "+faststart", out],
    { stdio: "inherit" },
  );
}

export function encodeFrames(dir, out, w, h) {
  fs.mkdirSync(path.dirname(out), { recursive: true });
  execFileSync(
    "ffmpeg",
    ["-hide_banner", "-loglevel", "error", "-y", "-framerate", "30", "-i", path.join(dir, "%06d.png"),
      "-vf", `scale=${w}:${h}:flags=lanczos,pad=ceil(iw/2)*2:ceil(ih/2)*2:0:0:color=0x0a0a0b,format=yuv420p`,
      "-c:v", "libx264", "-preset", "slow", "-crf", "12", "-an", "-movflags", "+faststart", out],
    { stdio: "inherit" },
  );
}
