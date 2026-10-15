// A headless browser check of The Ice Cream Man's second round (#11667):
// touch sprint, item switching, no zoom, the jumpscare, the speedrun
// timer and best time, at a desktop size and on an emulated iPhone and
// Android phone with real touch events (two fingers at once).
//
//   PLAYWRIGHT_CORE=/path/to/node_modules/playwright-core/index.mjs \
//     node scripts/icecream-touch-check.mjs URL OUTDIR [PREFIX]
//
// URL is the game's page, for example https://openagents.com/games/the-ice-cream-man
// or a static server over `scripts/build-icecream-web.sh --with-page`.
// Screenshots go to OUTDIR/PREFIX-*.png (PREFIX defaults to icecream-r2).
// Exits 1 on any failed check or page error.
const pw = await import(process.env.PLAYWRIGHT_CORE ?? "playwright-core");
const chromium = pw.chromium ?? pw.default.chromium;
import { mkdirSync } from "node:fs";

const [url, out, prefix = "icecream-r2"] = process.argv.slice(2);
if (!url || !out) {
  console.error("usage: node scripts/icecream-touch-check.mjs URL OUTDIR [PREFIX]");
  process.exit(64);
}
mkdirSync(out, { recursive: true });
const browser = await chromium.launch({
  executablePath: process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  headless: true,
  args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--autoplay-policy=no-user-gesture-required"],
});
const failures = [];
const errors = [];
const check = (ok, what) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failures.push(what);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const shot = (page, name) => page.screenshot({ path: `${out}/${prefix}-${name}.png` });

const DEVICES = {
  desktop: { viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 },
  iphone: {
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 3,
    isMobile: true,
    hasTouch: true,
    userAgent:
      "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
  },
  android: {
    viewport: { width: 412, height: 915 },
    deviceScaleFactor: 2.625,
    isMobile: true,
    hasTouch: true,
    userAgent:
      "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0 Mobile Safari/537.36",
  },
};

async function open(device, hash) {
  const context = await browser.newContext(DEVICES[device]);
  const page = await context.newPage();
  page.on("pageerror", (e) => errors.push(`${device}: ${e}`));
  page.on("console", (m) => {
    if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) errors.push(`${device}: ${m.text()}`);
  });
  page.on("response", (r) => {
    if (r.status() >= 400 && !r.url().includes("favicon")) errors.push(`${device}: ${r.status()} ${r.url()}`);
  });
  await page.goto(url + hash, { waitUntil: "load" });
  await page.waitForFunction(() => document.querySelector("#icecream-canvas")?.dataset.phase, null, { timeout: 30000 });
  const cdp = await context.newCDPSession(page);
  return { context, page, cdp };
}
const data = (page) => page.evaluate(() => ({ ...document.querySelector("#icecream-canvas").dataset }));
async function until(page, test, ms = 3000) {
  const end = Date.now() + ms;
  let d = await data(page);
  while (!test(d) && Date.now() < end) {
    await sleep(50);
    d = await data(page);
  }
  return d;
}
const centre = (page, selector) =>
  page.evaluate((s) => {
    const r = document.querySelector(s)?.getBoundingClientRect();
    return r && r.width > 0 ? { x: r.x + r.width / 2, y: r.y + r.height / 2 } : null;
  }, selector);

// Real touches through the DevTools protocol: every finger down at once.
function fingers(cdp) {
  const down = new Map();
  const send = (type) =>
    cdp.send("Input.dispatchTouchEvent", {
      type,
      touchPoints: [...down.entries()].map(([id, p]) => ({ x: p.x, y: p.y, id, radiusX: 8, radiusY: 8, force: 1 })),
    });
  return {
    async start(id, p) {
      down.set(id, p);
      await send("touchStart");
    },
    async move(id, p) {
      down.set(id, p);
      await send("touchMove");
    },
    async end(id) {
      const p = down.get(id);
      down.delete(id);
      // touchEnd lists the finger that lifts; the others stay down.
      await cdp.send("Input.dispatchTouchEvent", {
        type: "touchEnd",
        touchPoints: [{ x: p.x, y: p.y, id }],
      });
      return p;
    },
    async tap(id, p) {
      await this.start(id, p);
      await sleep(40);
      await this.end(id);
    },
  };
}

async function touchChecks(device) {
  const { context, page, cdp } = await open(device, "#seed=7&skip&calm");
  const { width, height } = DEVICES[device].viewport;
  const f = fingers(cdp);
  check((await data(page)).phase === "playing", `${device}: playing`);
  check(await page.evaluate(() => matchMedia("(pointer: coarse)").matches), `${device}: a touch screen`);

  // Sprint: one finger walks the stick, a second holds Sprint.
  const stick = { x: 90, y: height - 190 };
  await f.start(1, stick);
  for (let i = 1; i <= 6; i++) await f.move(1, { x: stick.x, y: stick.y - i * 10 });
  const sprint = await centre(page, "#icecream-sprint");
  check(!!sprint, `${device}: the Sprint button is there`);
  await f.start(2, sprint);
  let d = await until(page, (d) => d.sprinting === "true", 2000);
  check(d.sprinting === "true", `${device}: holding Sprint with a second finger sprints`);
  await sleep(400);
  await shot(page, `${device}-sprint`);
  await f.end(2);
  d = await until(page, (d) => d.sprinting === "false", 2000);
  check(d.sprinting === "false" && d.sprint === "false", `${device}: letting go of Sprint stops`);

  // Item switching: 20 taps on the hot bar while the stick is held.
  const slots = await page.evaluate(() =>
    [...document.querySelectorAll("#icecream-hotbar [data-tool]")]
      .filter((b) => b.getBoundingClientRect().width > 0)
      .map((b) => {
        const r = b.getBoundingClientRect();
        return { tool: b.dataset.tool, x: r.x + r.width / 2, y: r.y + r.height / 2, w: r.width, h: r.height };
      }),
  );
  check(slots.length >= 3 && slots.every((s) => s.w >= 44 && s.h >= 44), `${device}: hot-bar slots at least 44 px (${slots.map((s) => `${Math.round(s.w)}x${Math.round(s.h)}`).join(" ")})`);
  let switched = 0;
  for (let i = 0; i < 20; i++) {
    const slot = slots[(i + 1) % 2];
    // A little slide, like a real thumb.
    await f.start(3, slot);
    await f.move(3, { x: slot.x + 3, y: slot.y - 2 });
    await f.end(3);
    d = await until(page, (d) => d.tool === slot.tool, 1000);
    if (d.tool === slot.tool) switched++;
  }
  check(switched === 20, `${device}: item switching ${switched}/20 with a finger on the stick`);
  // Next, and a swipe along the bar.
  const before = (await data(page)).tool;
  const next = await centre(page, "#icecream-next");
  check(!!next, `${device}: a Next item button`);
  if (next) await f.tap(4, next);
  d = await until(page, (d) => d.tool !== before, 1000);
  check(d.tool !== before, `${device}: Next picks another tool (${before} -> ${d.tool})`);
  const swipeFrom = d.tool;
  const bar = slots[1];
  await f.start(5, { x: bar.x + 40, y: bar.y });
  for (let i = 1; i <= 5; i++) await f.move(5, { x: bar.x + 40 - i * 20, y: bar.y });
  await f.end(5);
  d = await until(page, (d) => d.tool !== swipeFrom, 1000);
  check(d.tool !== swipeFrom, `${device}: swiping the bar switches (${swipeFrom} -> ${d.tool})`);
  await f.end(1);

  // Double taps never zoom: on the Jump button, a slot, and the canvas.
  for (const target of [await centre(page, "#icecream-jump"), slots[0], { x: width * 0.7, y: height * 0.4 }]) {
    for (let k = 0; k < 2; k++) {
      await f.tap(6, target);
      await sleep(60);
    }
    await sleep(300);
  }
  const scale = await page.evaluate(() => window.visualViewport.scale);
  check(Math.abs(scale - 1) < 0.01, `${device}: double taps don't zoom (scale ${scale})`);
  const actions = await page.evaluate(() =>
    [...document.querySelectorAll("button")].filter((b) => b.getBoundingClientRect().width > 0).map((b) => getComputedStyle(b).touchAction),
  );
  check(actions.length > 0 && actions.every((a) => a === "none" || a === "manipulation"), `${device}: every button says no zoom (${[...new Set(actions)]})`);

  // Tap-to-toggle sprint from the menu.
  await f.tap(7, await centre(page, "#icecream-menu"));
  await until(page, (d) => d.phase === "menu", 2000);
  await shot(page, `${device}-menu`);
  const sprintSetting = page.getByRole("button", { name: "Sprint button: hold" });
  check(await sprintSetting.isVisible(), `${device}: the menu has the sprint setting`);
  const version = await page.evaluate(() => document.querySelector("#icecream-version")?.textContent ?? "");
  console.log(`     ${device}: menu version line: "${version}"`);
  await sprintSetting.tap();
  await page.getByRole("button", { name: "Resume" }).tap();
  await until(page, (d) => d.phase === "playing", 2000);
  await f.tap(8, await centre(page, "#icecream-sprint"));
  d = await until(page, (d) => d.sprint === "true", 1000);
  check(d.sprint === "true" && d.settings.includes("sprint=tap"), `${device}: in tap mode a tap turns sprint on`);
  await f.tap(8, await centre(page, "#icecream-sprint"));
  d = await until(page, (d) => d.sprint === "false", 1000);
  check(d.sprint === "false", `${device}: and another tap turns it off`);
  await context.close();
}

async function scare(device) {
  // A short run: low health and a quick Ice Cream Man, standing still.
  const { context, page } = await open(device, "#seed=7&skip&t.max_health=10&t.sniff_every=3&t.notice=20&t.man_wander=8&t.man_investigate=10");
  let d = await until(page, (d) => d.scare !== "none", 60000);
  check(d.scare !== "none", `${device}: scooped, the jumpscare plays`);
  d = await until(page, (d) => Number(d.scare) >= 0.15, 3000);
  await shot(page, `${device}-lunge`);
  d = await until(page, (d) => Number(d.scare) >= 0.6, 3000);
  const splat = await page.evaluate(() => Number(getComputedStyle(document.querySelector("#icecream-scare")).opacity));
  await shot(page, `${device}-jumpscare`);
  check(splat > 0.4, `${device}: the red syrup flash shows (${splat})`);
  d = await until(page, (d) => d.phase === "over", 3000);
  check(d.phase === "over", `${device}: then the scooped card`);
  await shot(page, `${device}-scooped`);
  await context.close();
}

async function speedrun(device) {
  const { context, page } = await open(device, "#seed=7&bot&countbest");
  await sleep(1500);
  const timer = await page.evaluate(() => {
    const t = document.querySelector("#icecream-timer");
    return t && getComputedStyle(t).display !== "none" ? t.textContent : null;
  });
  check(!!timer && /^\d+:\d\d\.\d$/.test(timer), `${device}: the run timer shows (${timer})`);
  await shot(page, `${device}-timer`);
  const d = await until(page, (d) => d.phase === "over", 240000);
  check(d.status === "escaped" && d.newBest === "true", `${device}: escaped, a new best (${d.best})`);
  const stored = await page.evaluate(() => localStorage.getItem("icecream.best.v1.normal"));
  check(!!stored && Number(stored) > 0, `${device}: the best time is saved (${stored})`);
  const card = await page.evaluate(() => document.body.innerText);
  check(card.includes("New best!") && card.includes("Your time:"), `${device}: the escape card shows the time and New best!`);
  await shot(page, `${device}-escaped`);
  await context.close();
}

const only = process.env.ONLY;
for (const device of ["iphone", "android"]) {
  if (!only || only.includes("touch")) await touchChecks(device);
  if (!only || only.includes("scare")) await scare(device);
}
if (!only || only.includes("desktop")) {
  await scare("desktop");
  await speedrun("desktop");
}
await browser.close();
for (const e of errors) console.log(`error: ${e}`);
if (failures.length || errors.length) {
  console.log(`${failures.length} failed, ${errors.length} page errors`);
  process.exit(1);
}
console.log("all good");
