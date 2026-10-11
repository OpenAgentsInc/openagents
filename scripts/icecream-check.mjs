// A headless browser check of The Ice Cream Man, and the guide's
// screenshots.
//
//   PLAYWRIGHT_CORE=/path/to/node_modules/playwright-core/index.mjs \
//     node scripts/icecream-check.mjs URL OUTDIR
//
// URL is the game's page, for example
// http://127.0.0.1:8080/index.html (a static server over
// `scripts/build-icecream-web.sh --with-page OUTDIR`) or
// https://openagents.com/games/the-ice-cream-man. With Playwright's own
// package: `npm i --prefix /tmp/pw playwright-core` then point
// PLAYWRIGHT_CORE at it (it drives the installed Chrome, CHROME overrides
// the path, so nothing is downloaded).
//
// It checks, in three fresh browsers at 960x600:
//   1. the first run opens the four-card tutorial, Skip closes it, and
//      Play starts a run;
//   2. on seed 7, the avatar walks with W, the hot bar switches with 1
//      and 2, Ctrl toggles Shift Lock, and E hides in the bedroom closet;
//   3. the built-in bot plays seed 7: the Ice Cream Man chases, fudge
//      hits him, and the run is escaped.
// and writes OUTDIR/icecream-{tour,hall,hide,man,fudge,door}.png. Exits 1
// on any failed check or page error.
const pw = await import(process.env.PLAYWRIGHT_CORE ?? "playwright-core");
const chromium = pw.chromium ?? pw.default.chromium;
import { mkdirSync } from "node:fs";

const [url, out] = process.argv.slice(2);
if (!url || !out) {
  console.error("usage: node scripts/icecream-check.mjs URL OUTDIR");
  process.exit(64);
}
mkdirSync(out, { recursive: true });
const executablePath =
  process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const browser = await chromium.launch({
  executablePath,
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

async function open(hash) {
  const context = await browser.newContext({ viewport: { width: 960, height: 600 }, deviceScaleFactor: 1 });
  const page = await context.newPage();
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => {
    if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) errors.push(m.text());
  });
  page.on("response", (r) => {
    if (r.status() >= 400 && !r.url().includes("favicon")) errors.push(`${r.status()} ${r.url()}`);
  });
  await page.goto(url + hash, { waitUntil: "load" });
  await page.waitForFunction(() => document.querySelector("#icecream-canvas")?.dataset.phase, null, { timeout: 30000 });
  return { context, page };
}
const data = (page) => page.evaluate(() => ({ ...document.querySelector("#icecream-canvas").dataset }));
const point = (text) => text.split(",").map(Number);
/// Waits up to `ms` for the canvas's data to pass `test`; returns it.
async function until(page, test, ms = 3000) {
  const end = Date.now() + ms;
  let d = await data(page);
  while (!test(d) && Date.now() < end) {
    await sleep(100);
    d = await data(page);
  }
  return d;
}
const visible = async (locator, ms = 3000) => {
  try {
    await locator.waitFor({ state: "visible", timeout: ms });
    return true;
  } catch {
    return false;
  }
};

// 1. The tutorial on a first run.
{
  const { context, page } = await open("");
  let d = await data(page);
  check(d.tour === "true", "the first run opens the tutorial");
  check(await visible(page.getByText("Move", { exact: true })), "the tutorial's first card is Move");
  await page.getByRole("button", { name: "Next" }).click();
  check(await visible(page.getByText("Look", { exact: true })), "Next shows Look");
  await page.screenshot({ path: `${out}/icecream-tour.png` });
  await page.getByRole("button", { name: "Skip" }).click();
  d = await until(page, (d) => d.tour === "false");
  check(d.tour === "false", "Skip closes the tutorial");
  await page.getByRole("button", { name: "Play", exact: true }).click();
  d = await until(page, (d) => d.phase === "playing");
  check(d.phase === "playing" && d.tour === "false", "Play starts a run");
  await context.close();
}

// 2. Moving, the hot bar, Shift Lock, and hiding.
{
  const { context, page } = await open("#seed=7&skip");
  await page.locator("#icecream-canvas").focus();
  await sleep(800);
  let d = await data(page);
  check(d.phase === "playing", "#skip starts playing");
  const [x0, z0] = point(d.player);
  // The camera starts behind the avatar looking toward -z: W walks -z, D +x.
  await page.keyboard.down("w");
  await sleep(700);
  await page.keyboard.up("w");
  d = await until(page, (d) => point(d.player)[1] < z0 - 1.0);
  const [x1, z1] = point(d.player);
  check(z1 < z0 - 1.0, `W walks the avatar (${z0.toFixed(1)} to ${z1.toFixed(1)})`);
  await page.keyboard.press("2");
  check((await until(page, (d) => d.tool === "fart")).tool === "fart", "2 picks the Fart Launcher");
  await page.keyboard.press("1");
  check((await until(page, (d) => d.tool === "fudge")).tool === "fudge", "1 picks Hot Fudge");
  await page.keyboard.press("Control");
  check((await until(page, (d) => d.shiftLock === "true")).shiftLock === "true", "Ctrl turns Shift Lock on");
  await page.keyboard.press("Control");
  check((await until(page, (d) => d.shiftLock === "false")).shiftLock === "false", "Ctrl turns it off");
  // Walk to the bedroom closet: the room's centre is the spawn less 1 m in z.
  const centre = [x0, z0 - 1];
  const goal = [centre[0] - 2.6, centre[1] - 3.6];
  for (let i = 0; i < 80; i++) {
    const [x, z] = point((await data(page)).player);
    const dx = goal[0] - x;
    const dz = goal[1] - z;
    if (Math.hypot(dx, dz) < 0.3) break;
    const keys = [];
    if (dz < -0.15) keys.push("w");
    if (dz > 0.15) keys.push("s");
    if (dx > 0.15) keys.push("d");
    if (dx < -0.15) keys.push("a");
    for (const k of keys) await page.keyboard.down(k);
    await sleep(Math.min(150, 40 + 60 * Math.hypot(dx, dz)));
    for (const k of keys) await page.keyboard.up(k);
  }
  await page.mouse.move(480, 300);
  await page.mouse.down({ button: "right" });
  await page.mouse.move(380, 330, { steps: 6 });
  await page.mouse.up({ button: "right" });
  await sleep(300);
  await page.screenshot({ path: `${out}/icecream-hall.png` });
  d = await until(page, (d) => d.prompt === "hide");
  check(d.prompt === "hide", `by the closet, E would hide (${d.prompt})`);
  await page.keyboard.press("e");
  check((await until(page, (d) => d.hidden === "true")).hidden === "true", "E hides in the closet");
  await sleep(300);
  await page.screenshot({ path: `${out}/icecream-hide.png` });
  await page.keyboard.press("e");
  check((await until(page, (d) => d.hidden === "false")).hidden === "false", "E comes out again");
  await page.keyboard.press("Escape");
  check((await until(page, (d) => d.phase === "menu")).phase === "menu", "Esc opens the menu");
  await context.close();
}

// 3. The bot plays seed 7: a chase, fudge hits, an escape.
{
  const { context, page } = await open("#seed=7&bot&speed=3");
  let chased = false;
  let hits = 0;
  let manShot = false;
  let fudgeShot = false;
  const started = Date.now();
  let d = await data(page);
  while (Date.now() - started < 300000) {
    d = await data(page);
    const [px, pz] = point(d.player);
    const [mx, mz] = point(d.man);
    const near = Math.hypot(px - mx, pz - mz);
    if (d.manState === "chase") chased = true;
    const fudgeHits = Number(d.fudgeHits);
    if (!manShot && d.manState === "chase" && near < 8) {
      manShot = true;
      await page.screenshot({ path: `${out}/icecream-man.png` });
    }
    if (!fudgeShot && fudgeHits > hits && fudgeHits >= 2 && d.clouds === "0" && near < 9) {
      fudgeShot = true;
      await page.screenshot({ path: `${out}/icecream-fudge.png` });
    }
    hits = Math.max(hits, fudgeHits);
    if (d.phase === "over") break;
    await sleep(150);
  }
  check(chased, "the Ice Cream Man chases");
  check(hits > 0, `hot fudge hits him (${hits})`);
  check(d.status === "escaped", `the seeded run is escaped (${d.status}, keys ${d.keys})`);
  check(await visible(page.getByText("You escaped!")), "the run card says You escaped!");
  await page.screenshot({ path: `${out}/icecream-door.png` });
  await context.close();
}

await browser.close();
const real = errors.filter((e) => !/AudioContext|favicon/.test(e));
check(real.length === 0, `no page errors${real.length ? ": " + real.join(" | ") : ""}`);
console.log(failures.length ? `${failures.length} failed` : "all checks passed");
process.exit(failures.length ? 1 : 0);
