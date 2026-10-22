// Real footage of the Grid for the Verse trailer (#12458): headless Chrome
// on a Grid page (a local `scripts/build-everglade-web.sh --with-pack`
// build, or openagents.com signed out), a visitor walked through it by key
// presses, recorded as a live screencast at 1920x1080 and 30 fps.
//
//   node scripts/film/footage/verse-grid.mjs TAKES.json [NAME ...]
//
// TAKES.json is {"out": DIR, "takes": [{"name", "url", "setup": [STEP],
// "steps": [STEP]}]}; `setup` runs before recording starts and `steps`
// while it runs. A STEP is one of:
//   {"wait": S}                 let the world run for S seconds
//   {"join": true}              tap W, so the homepage hands the camera over
//   {"face": [X, Z]}            turn the visitor to face a ground point
//   {"walk": [X, Z], "stop": M} face the point and hold W until within M m
//   {"hold": CODE, "for": S}    hold a key, such as KeyA, for S seconds
//   {"press": CODE}             tap a key, such as KeyE
//   {"portal": NAME}            go through a Grid arch (verse_portal)
//   {"type": TEXT}              type into the focused field
//   {"click": [SELECTOR, TEXT]} click the element with that text
//                               ("optional": true goes on when it's absent)
//   {"zoom": LINES}             wheel the camera closer (or farther, < 0)
//   {"drag": [DX, DY], "for": S} turn the camera with a mouse drag over S s
// A take's "override": {"/everglade/": DIR, ...} serves the page's files
// under those paths from local builds instead (this checkout's Verse and
// Pylons layer over openagents.com's page). The page's visit beacons are
// switched off, so a take records no visit.
// A take with "verse": false films a page that has no Verse world (a
// game); it waits "settle" seconds instead. Writes DIR/NAME.mp4 and DIR/NAME.json (where the visitor was each step).
import fs from "node:fs";
import path from "node:path";
import { launch, navigate, evaluate, sleep, Recorder, type, click, byText } from "./cdp.mjs";

const [file, ...only] = process.argv.slice(2);
if (!file) {
  console.error("usage: verse-grid.mjs TAKES.json [NAME ...]");
  process.exit(64);
}
const plan = JSON.parse(fs.readFileSync(file, "utf8"));
const out = plan.out.replace(/^~/, process.env.HOME);
fs.mkdirSync(out, { recursive: true });

const MODULE = `(globalThis.__verse ||= (async () => {
  for (const p of ["/everglade/everglade_web.js", "/everglade_web.js"]) {
    try { return await import(p); } catch {}
  }
  throw new Error("no Verse module");
})())`;

async function where(cdp) {
  return JSON.parse(await evaluate(cdp, `${MODULE}.then(m => m.verse_where())`));
}

async function key(cdp, code, down) {
  const k = code.startsWith("Key") ? code.slice(3).toLowerCase() : code;
  await cdp.send("Input.dispatchKeyEvent", { type: down ? "keyDown" : "keyUp", code, key: k });
}

async function face(cdp, [x, z]) {
  await evaluate(cdp, `${MODULE}.then(m => m.verse_face(${x}, ${z}))`);
}

async function step(cdp, s, log) {
  if (s.wait) await sleep(s.wait * 1000);
  else if (s.join) {
    await key(cdp, "KeyW", true);
    await sleep(80);
    await key(cdp, "KeyW", false);
    await sleep(2500);
  } else if (s.walk) {
    const stop = s.stop ?? 1.5;
    await face(cdp, s.walk);
    await key(cdp, "KeyW", true);
    const end = Date.now() + (s.timeout ?? 25) * 1000;
    let k = 0;
    while (Date.now() < end) {
      const p = (await where(cdp)).player;
      if (Math.hypot(p[0] - s.walk[0], p[2] - s.walk[1]) < stop) break;
      if (++k % 10 === 0) await face(cdp, s.walk);
      await sleep(50);
    }
    await key(cdp, "KeyW", false);
  } else if (s.face) await face(cdp, s.face);
  else if (s.hold) {
    await key(cdp, s.hold, true);
    await sleep((s.for ?? 1) * 1000);
    await key(cdp, s.hold, false);
  } else if (s.press) {
    await key(cdp, s.press, true);
    await sleep(90);
    await key(cdp, s.press, false);
  } else if (s.portal) await evaluate(cdp, `${MODULE}.then(m => m.verse_portal(${JSON.stringify(s.portal)}))`);
  else if (s.type) await type(cdp, s.type);
  else if (s.click) {
    try {
      await click(cdp, await byText(cdp, s.click[0], s.click[1]));
    } catch (e) {
      if (!s.optional) throw e;
    }
  }
  else if (s.zoom) {
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseWheel", x: 960, y: 540, deltaX: 0, deltaY: -100 * s.zoom,
    });
    await sleep(600);
  } else if (s.drag) {
    const steps = Math.max(1, Math.round((s.for ?? 1) * 30));
    let x = 960, y = 540;
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", buttons: 1, clickCount: 1 });
    for (let i = 0; i < steps; i++) {
      x += s.drag[0] / steps;
      y += s.drag[1] / steps;
      await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, button: "left", buttons: 1 });
      await sleep(1000 / 30);
    }
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", buttons: 0, clickCount: 1 });
  }
  try {
    log.push({ step: s, where: await where(cdp) });
  } catch {
    log.push({ step: s });
  }
}

for (const take of plan.takes) {
  if (only.length && !only.includes(take.name)) continue;
  const cdp = await launch({ width: 1920, height: 1080, dpr: 1 });
  await cdp.send("Page.addScriptToEvaluateOnNewDocument", {
    source: "navigator.sendBeacon = () => true;",
  });
  if (take.override) {
    const types = { ".js": "text/javascript", ".wasm": "application/wasm" };
    await cdp.send("Fetch.enable", {
      patterns: Object.keys(take.override).map((p) => ({ urlPattern: `*${p}*`, requestStage: "Request" })),
    });
    cdp.on("Fetch.requestPaused", async (e) => {
      const url = new URL(e.request.url);
      const prefix = Object.keys(take.override).find((p) => url.pathname.startsWith(p));
      const local = prefix && path.join(take.override[prefix].replace(/^~/, process.env.HOME), url.pathname.slice(prefix.length));
      if (local && fs.existsSync(local)) {
        const headers = [{ name: "Content-Type", value: types[path.extname(local)] || "application/octet-stream" },
          { name: "Cache-Control", value: "no-store" }];
        await cdp.send("Fetch.fulfillRequest", { requestId: e.requestId, responseCode: 200, responseHeaders: headers,
          body: fs.readFileSync(local).toString("base64") }).catch(() => {});
      } else {
        await cdp.send("Fetch.continueRequest", { requestId: e.requestId }).catch(() => {});
      }
    });
  }
  await navigate(cdp, take.url);
  const end = Date.now() + 90000;
  for (; take.verse !== false; ) {
    try {
      if ((await where(cdp)).draw !== "opening") break;
    } catch {}
    if (Date.now() > end) throw new Error(`${take.name}: the world did not open`);
    await sleep(250);
  }
  await sleep((take.settle ?? 3) * 1000);
  const log = [];
  for (const s of take.setup ?? []) await step(cdp, s, log);
  const rec = new Recorder(cdp, path.join(out, `.${take.name}-frames`));
  await rec.start();
  for (const s of take.steps) await step(cdp, s, log);
  await rec.stop();
  const mp4 = path.join(out, `${take.name}.mp4`);
  rec.encode(mp4);
  fs.writeFileSync(path.join(out, `${take.name}.json`), JSON.stringify({ url: take.url, log }, null, 1));
  fs.rmSync(path.join(out, `.${take.name}-frames`), { recursive: true, force: true });
  console.log(mp4);
  await cdp.close();
}
