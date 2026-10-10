// Captures Grow Little Bunny in headless Chrome at desktop and phone sizes.
//
//   node scripts/bunny-capture.mjs URL OUTDIR [--wait SECONDS] [--keys K1,K2] [--name NAME]
//
// Opens URL (for example http://127.0.0.1:4391/games/grow-little-bunny, or
// with a fragment such as #garden=3) once at 1440x900 and once as a phone
// (390x844, touch, device scale 3), presses each key in --keys after load
// (Enter presses the focused button; "click:TEXT" clicks the first button
// with that text; "hold:KEY:SECONDS" holds a key; "sleep:SECONDS" waits),
// waits --wait seconds (default 3), and writes
// OUTDIR/NAME-desktop.png and OUTDIR/NAME-phone.png. Console errors are
// printed. Uses Chrome's DevTools protocol over Node's own WebSocket, so it
// needs Chrome and Node 22 or newer, nothing else.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const args = process.argv.slice(2);
const url = args[0];
const out = args[1];
if (!url || !out) {
  console.error("usage: node scripts/bunny-capture.mjs URL OUTDIR [--wait S] [--keys K,..] [--name N]");
  process.exit(64);
}
const option = (name, fallback) => {
  const at = args.indexOf(name);
  return at >= 0 ? args[at + 1] : fallback;
};
const wait = Number(option("--wait", "3"));
const keys = option("--keys", "").split(",").filter(Boolean);
const name = option("--name", "bunny");
const only = option("--only", "");
const chrome =
  process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
mkdirSync(out, { recursive: true });

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

async function launch() {
  const profile = mkdtempSync(join(tmpdir(), "bunny-chrome-"));
  const port = 9300 + Math.floor(Math.random() * 500);
  const child = spawn(chrome, [
    "--headless=new",
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${profile}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--use-angle=swiftshader",
    "--enable-unsafe-swiftshader",
    "--hide-scrollbars",
    "about:blank",
  ], { stdio: "ignore" });
  for (let i = 0; i < 300; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      const page = list.find((t) => t.type === "page");
      if (page) return { child, socket: page.webSocketDebuggerUrl };
    } catch {}
    await sleep(100);
  }
  child.kill();
  throw new Error("Chrome didn't start");
}

function connect(address) {
  const ws = new WebSocket(address);
  let next = 1;
  const waiting = new Map();
  const listeners = [];
  ws.onmessage = (message) => {
    const data = JSON.parse(message.data);
    if (data.id && waiting.has(data.id)) {
      const { resolve, reject } = waiting.get(data.id);
      waiting.delete(data.id);
      data.error ? reject(new Error(data.error.message)) : resolve(data.result);
    } else if (data.method) {
      for (const listen of listeners) listen(data);
    }
  };
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = next++;
      waiting.set(id, { resolve, reject });
      ws.send(JSON.stringify({ id, method, params }));
    });
  return new Promise((resolve) => {
    ws.onopen = () => resolve({ send, on: (f) => listeners.push(f), close: () => ws.close() });
  });
}

const KEY_CODES = {
  Enter: 13, " ": 32, ArrowLeft: 37, ArrowUp: 38, ArrowRight: 39, ArrowDown: 40,
  Escape: 27, x: 88, p: 80,
};

async function press(page, key) {
  if (key.startsWith("click:")) {
    const text = key.slice(6);
    await page.send("Runtime.evaluate", {
      expression: `(() => { const b = [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === ${JSON.stringify(text)}); if (b) b.click(); return !!b; })()`,
    });
    return;
  }
  if (key.startsWith("hold:")) {
    // hold:KEY:SECONDS keeps a key down, for walking in the meadow.
    const [, name, seconds] = key.split(":");
    const code = KEY_CODES[name] ?? name.toUpperCase().charCodeAt(0);
    const params = { key: name, windowsVirtualKeyCode: code, nativeVirtualKeyCode: code, code: name };
    await page.send("Input.dispatchKeyEvent", { type: "keyDown", ...params });
    await sleep(Number(seconds) * 1000);
    await page.send("Input.dispatchKeyEvent", { type: "keyUp", ...params });
    return;
  }
  if (key.startsWith("sleep:")) {
    await sleep(Number(key.slice(6)) * 1000);
    return;
  }
  const code = KEY_CODES[key] ?? key.toUpperCase().charCodeAt(0);
  for (const type of ["keyDown", "keyUp"]) {
    await page.send("Input.dispatchKeyEvent", {
      type, key, windowsVirtualKeyCode: code, nativeVirtualKeyCode: code,
      code: key.length === 1 ? `Key${key.toUpperCase()}` : key,
    });
  }
}

async function capture(size) {
  const { child, socket } = await launch();
  try {
    const page = await connect(socket);
    page.on((event) => {
      if (event.method === "Runtime.consoleAPICalled" && event.params.type === "error") {
        console.error(`[${size.name}] console:`, event.params.args.map((a) => a.value ?? a.description).join(" "));
      }
      if (event.method === "Runtime.exceptionThrown") {
        console.error(`[${size.name}] exception:`, event.params.exceptionDetails.text,
          event.params.exceptionDetails.exception?.description ?? "");
      }
      if (event.method === "Log.entryAdded" && event.params.entry.level === "error") {
        console.error(`[${size.name}] log:`, event.params.entry.text);
      }
    });
    await page.send("Runtime.enable");
    await page.send("Log.enable");
    await page.send("Page.enable");
    await page.send("Emulation.setDeviceMetricsOverride", {
      width: size.width, height: size.height, deviceScaleFactor: size.scale, mobile: size.mobile,
    });
    if (size.mobile) {
      await page.send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
      await page.send("Emulation.setEmitTouchEventsForMouse", { enabled: true, configuration: "mobile" });
    }
    await page.send("Page.navigate", { url });
    await sleep(2500);
    for (const key of keys) {
      await press(page, key);
      await sleep(150);
    }
    await sleep(wait * 1000);
    const shot = await page.send("Page.captureScreenshot", { format: "png" });
    const file = join(out, `${name}-${size.name}.png`);
    writeFileSync(file, Buffer.from(shot.data, "base64"));
    console.log(file);
    page.close();
  } finally {
    child.kill();
  }
}

const sizes = [
  { name: "desktop", width: 1440, height: 900, scale: 1, mobile: false },
  { name: "phone", width: 390, height: 844, scale: 3, mobile: true },
];
for (const size of sizes) {
  if (!only || only === size.name) await capture(size);
}
