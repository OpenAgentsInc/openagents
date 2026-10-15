// Plays and captures Snap Station in headless Chrome at desktop and phone
// sizes, with the real pointer: pieces are dragged from the tray onto the
// station the way a player drags them.
//
//   node scripts/snap-capture.mjs URL OUTDIR [--name NAME] [--only desktop|phone]
//        [--size WxH] [--steps STEP,STEP,...]
//
// Opens URL (for example https://openagents.com/games/snap-station) once at
// 1440x900 (or --size) and once as a phone (390x844, touch, device scale 3), runs each
// step, and writes OUTDIR/NAME-SIZE.png at the end. Steps:
//
//   click:TEXT          click the first button whose words start with TEXT
//   drag:KIND:X:Y       drag a piece from the tray to the point (X, Y) of the
//                       station, in metres; with :N after it, turn it N times
//   hold:KIND:X:Y       the same drag, still held (for a picture of it)
//   go                  press Go
//   key:KEY             press a key
//   wait:SECONDS        wait
//   shot:LABEL          write OUTDIR/NAME-SIZE-LABEL.png
//   expect:TEXT         fail unless the page shows TEXT
//
// The page's canvas carries `data-view="CX CY SCALE LEFT TOP"`, the middle
// of its view in metres, pixels a metre, and its corner, so a point of the
// station maps to the screen. Under `openagents browser run` it uses that
// Chrome (OPENAGENTS_CHROME_PORT); otherwise it starts its own, so it needs
// Chrome and Node 22 or newer. Console errors are printed; a failed expect
// exits 1.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const args = process.argv.slice(2);
const url = args[0];
const out = args[1];
if (!url || !out) {
  console.error("usage: node scripts/snap-capture.mjs URL OUTDIR [--name N] [--only SIZE] [--steps S,S]");
  process.exit(64);
}
const option = (name, fallback) => {
  const at = args.indexOf(name);
  return at >= 0 ? args[at + 1] : fallback;
};
const name = option("--name", "snap");
const only = option("--only", "");
const steps = option("--steps", "").split(",").filter(Boolean);
const chrome =
  process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
mkdirSync(out, { recursive: true });
const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
let failed = false;

async function launch() {
  if (process.env.OPENAGENTS_CHROME_PORT) {
    const port = process.env.OPENAGENTS_CHROME_PORT;
    const target = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: "PUT" })).json();
    return { child: null, socket: target.webSocketDebuggerUrl };
  }
  const profile = mkdtempSync(join(tmpdir(), "snap-chrome-"));
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

async function evaluate(page, expression) {
  const result = await page.send("Runtime.evaluate", { expression, returnByValue: true });
  return result.result.value;
}

// A press, move or release: the mouse on a desktop, a finger on a phone.
let touchScreen = false;
async function mouse(page, type, x, y) {
  if (touchScreen) {
    const touch = { mousePressed: "touchStart", mouseMoved: "touchMove", mouseReleased: "touchEnd" }[type];
    await page.send("Input.dispatchTouchEvent", {
      type: touch,
      touchPoints: touch === "touchEnd" ? [] : [{ x, y, id: 1 }],
    });
    return;
  }
  await page.send("Input.dispatchMouseEvent", {
    type, x, y, button: "left", buttons: type === "mouseReleased" ? 0 : 1, clickCount: 1,
    pointerType: "mouse",
  });
}

// Where a station point is on the screen, from the canvas's data-view.
async function screen(page, x, y) {
  return evaluate(page, `(() => {
    const c = document.querySelector('canvas');
    const [cx, cy, scale, left, top] = c.dataset.view.split(' ').map(Number);
    const r = c.getBoundingClientRect();
    return [left + r.width / 2 + (${x} - cx) * scale, top + r.height / 2 - (${y} - cy) * scale];
  })()`);
}

async function centreOf(page, selector) {
  return evaluate(page, `(() => {
    const e = document.querySelector(${JSON.stringify(selector)});
    if (!e) return null;
    const r = e.getBoundingClientRect();
    return [r.left + r.width / 2, r.top + r.height / 2];
  })()`);
}

async function clickText(page, text) {
  const found = await evaluate(page, `(() => {
    const b = [...document.querySelectorAll('button')].find((b) =>
      b.offsetParent !== null && b.textContent.trim().startsWith(${JSON.stringify(text)}));
    if (!b) return null;
    const r = b.getBoundingClientRect();
    return [r.left + r.width / 2, r.top + r.height / 2];
  })()`);
  if (!found) {
    console.error(`no button "${text}"`);
    failed = true;
    return;
  }
  await tap(page, found);
}

async function tap(page, [x, y]) {
  await mouse(page, "mousePressed", x, y);
  await sleep(40);
  await mouse(page, "mouseReleased", x, y);
}

async function drag(page, kind, x, y, turns, release = true) {
  const from = await centreOf(page, `[data-piece="${kind}"]`);
  if (!from) {
    console.error(`no ${kind} in the tray`);
    failed = true;
    return;
  }
  const to = await screen(page, x, y);
  if (turns > 0) {
    // Tap the piece, turn it, then tap where it goes: the touch way.
    await tap(page, from);
    await sleep(100);
    for (let i = 0; i < turns; i++) {
      await clickText(page, "↻");
      await sleep(80);
    }
    await tap(page, to);
    return;
  }
  await mouse(page, "mousePressed", from[0], from[1]);
  const frames = 12;
  for (let i = 1; i <= frames; i++) {
    const t = i / frames;
    await mouse(page, "mouseMoved", from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t);
    await sleep(25);
  }
  await sleep(120);
  if (release) await mouse(page, "mouseReleased", to[0], to[1]);
}

async function shoot(page, file) {
  const shot = await page.send("Page.captureScreenshot", { format: "png" });
  writeFileSync(file, Buffer.from(shot.data, "base64"));
  console.log(file);
}

async function run(page, size, step) {
  const [verb, ...rest] = step.split(":");
  switch (verb) {
    case "click": await clickText(page, rest.join(":")); break;
    case "go": await tap(page, await centreOf(page, '[data-action="go"]')); break;
    case "drag": await drag(page, rest[0], Number(rest[1]), Number(rest[2]), Number(rest[3] ?? 0)); break;
    case "hold": await drag(page, rest[0], Number(rest[1]), Number(rest[2]), 0, false); break;
    case "key":
      for (const type of ["keyDown", "keyUp"]) {
        await page.send("Input.dispatchKeyEvent", { type, key: rest[0] });
      }
      break;
    case "wait": await sleep(Number(rest[0]) * 1000); break;
    case "shot": await shoot(page, join(out, `${name}-${size.name}-${rest[0]}.png`)); break;
    case "expect": {
      const text = rest.join(":");
      const shown = await evaluate(page, `document.body.innerText.includes(${JSON.stringify(text)})`);
      console.log(`[${size.name}] expect "${text}": ${shown ? "PASS" : "FAIL"}`);
      if (!shown) failed = true;
      break;
    }
    default: console.error(`unknown step ${step}`); failed = true;
  }
  await sleep(150);
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
    });
    await page.send("Runtime.enable");
    await page.send("Page.enable");
    await page.send("Emulation.setDeviceMetricsOverride", {
      width: size.width, height: size.height, deviceScaleFactor: size.scale, mobile: size.mobile,
    });
    touchScreen = size.mobile;
    if (size.mobile) {
      await page.send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
    }
    await page.send("Page.navigate", { url });
    await sleep(3000);
    for (const step of steps) {
      await run(page, size, step);
    }
    await shoot(page, join(out, `${name}-${size.name}.png`));
    page.close();
  } finally {
    child?.kill();
  }
}

const [width, height] = option("--size", "1440x900").split("x").map(Number);
const sizes = [
  { name: "desktop", width, height, scale: 1, mobile: false },
  { name: "phone", width: 390, height: 844, scale: 3, mobile: true },
];
for (const size of sizes) {
  if (!only || only === size.name) await capture(size);
}
process.exit(failed ? 1 : 0);
