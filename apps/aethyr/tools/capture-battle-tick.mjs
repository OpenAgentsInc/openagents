/**
 * Battle Lab fixed-tick capture (issue 12599). Holds the battle at an exact
 * simulation tick from a seed, with a fixed spectator camera and world clock,
 * and writes PNG + raw RGBA + a state digest per view. Two captures of the
 * same plan, from one build or two, must match byte for byte.
 *
 *   Capture: run through `openagents browser run --` (or set
 *            OPENAGENTS_CHROME_PORT / OPENAGENTS_CHROME_WS) with AF_CAPTURE_DIR.
 *   Compare: node tools/capture-battle-tick.mjs --compare a.json b.json
 *
 * Evidence stays outside Git, like verify-battle-crowd.mjs.
 */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import {
  battleStateDigest,
  battleTickPlan,
  compareBattleTickReports,
  pngSize,
} from "./battle-tick-capture.mjs";

const sha = (b) => createHash("sha256").update(b).digest("hex");

if (process.argv[2] === "--compare") {
  const [first, second] = process.argv.slice(3);
  assert(first && second, "Usage: --compare a.json b.json");
  const load = async (path) => ({
    path: resolve(path),
    ...JSON.parse(await readFile(path, "utf8")),
  });
  const a = await load(first),
    b = await load(second);
  const result = await compareBattleTickReports(a, b, (report, view) =>
    readFile(resolve(dirname(report.path), view.rgbaFile)),
  );
  console.log(JSON.stringify(result, null, 2));
  for (const view of result.views)
    console.log(
      `${view.ok ? "MATCH" : "DIFFER"} ${view.view}` +
        (view.pixels
          ? ` (${view.pixels.differing} of ${view.pixels.pixels} pixels, max ${view.pixels.maxDelta}/255)`
          : view.ok
            ? ""
            : ` (state ${view.state ? "same" : "differs"})`),
    );
  if (result.mismatchedPlan.length)
    console.log(`Plans differ in: ${result.mismatchedPlan.join(", ")}`);
  process.exit(result.ok ? 0 : 1);
}

const { chromium } = await import("playwright");
const { battleSourcePin, battleAssetPin } = await import(
  "./battle-evidence.mjs"
);
const { observeApplicationBrowserMode } = await import(
  "./application-pacing.mjs"
);
const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set a private AF_CAPTURE_DIR.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep battle captures outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run through openagents browser run.");
const url = new URL(process.env.AF_EDITOR_URL || "http://127.0.0.1:55316/");
assert(
  ["http:", "https:"].includes(url.protocol) &&
    ["localhost", "127.0.0.1"].includes(url.hostname) &&
    !url.username &&
    !url.password,
);
const plan = battleTickPlan(process.env);
const git = (...args) =>
  execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();
const revision = git("rev-parse", "HEAD");
const label = plan.label || revision.slice(0, 10);
const report = {
  schema: "aethyrforge.battle-tick-capture.v1",
  runId: randomUUID(),
  startedAt: new Date().toISOString(),
  ok: false,
  revision,
  dirty: !!git("status", "--porcelain"),
  kernelSha256: sha(await readFile(resolve(root, "public/kernel.wasm"))),
  url: url.href,
  plan: { ...plan, label },
  views: [],
  errors: [],
  console: [],
  limitations: [
    "Only spectator views hold: a participant's pose feeds the simulation every frame.",
    "MMO actors (the local avatar and other players) are hidden while held, so both builds show only the battle and the world.",
    "The GPU compaction order of visible fighters is not fixed; opaque depth-tested pixels do not depend on it, but exact depth ties could.",
  ],
};
report.sourcePin = await battleSourcePin(root);
report.assetPin = await battleAssetPin(
  url.href,
  process.env.AF_BATTLE_ASSET_ROOT || process.env.AETHYRFORGE_PRIVATE_ASSETS,
);
const browser = await chromium.connectOverCDP(endpoint);
report.browserMode = await observeApplicationBrowserMode(browser);
const context = await browser.newContext({
  viewport: plan.viewport,
  deviceScaleFactor: 1,
  serviceWorkers: "block",
});
const page = await context.newPage();
page.setDefaultTimeout(20000);
page.on("pageerror", (e) => report.errors.push(String(e)));
page.on("console", (m) => {
  if (["error", "warning"].includes(m.type()))
    report.console.push({ type: m.type(), text: m.text().slice(0, 4096) });
});
const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const status = async () => (await snapshot()).status;
async function until(predicate, what, timeout = 15000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    const s = await status();
    if (s.phase === "error") throw Error(s.message);
    const result = await predicate(s);
    if (result) return result === true ? s : result;
    await sleep(80);
  }
  throw Error(`${what}: ${JSON.stringify(await status())}`);
}
async function action(value) {
  const id = await page.evaluate((a) => window.aethyrforge.battle(a), value);
  const receipt = await until(
    async () => (await snapshot()).receipts.find((r) => r.requestId === id),
    `Battle ${value.op} did not finish`,
    30000,
  );
  assert.equal(receipt.ok, true, receipt.message);
  return receipt;
}
/** Raw RGBA and PNG of one offscreen frame at the held clock, as base64. */
const grab = (time) =>
  page.evaluate(async (time) => {
    const base64 = (buffer) => {
      const bytes = new Uint8Array(buffer);
      let text = "";
      for (let i = 0; i < bytes.length; i += 0x8000)
        text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      return btoa(text);
    };
    const rgba = await window.aethyrforge.capturePixels({ time });
    const png = await window.aethyrforge.captureBlob({ time, format: "png" });
    return { rgba: base64(rgba), png: base64(await png.arrayBuffer()) };
  }, time);
try {
  await page.goto(url.href);
  report.observedViewport = await page.evaluate(() => ({
    width: window.innerWidth,
    height: window.innerHeight,
    devicePixelRatio: window.devicePixelRatio,
    userAgent: navigator.userAgent,
  }));
  await until(
    (s) => s.phase === "ready" && s.mmorpg,
    "The prepared battle world did not open",
    150000,
  );
  await page
    .locator("#mmo-name")
    .fill(`Tick-${Date.now().toString(36).slice(-6)}`);
  await page.getByRole("button", { name: "Enter world", exact: true }).click();
  await until(
    (s) => s.mmorpg?.connection === "online",
    "The Paladin did not join",
  );
  await page
    .locator(".mmo-login")
    .waitFor({ state: "detached", timeout: 3000 });
  await page.getByRole("button", { name: "Battle Lab", exact: true }).click();
  await action({ op: "start", count: plan.count, seed: plan.seed });
  for (const view of plan.views) {
    await action({ op: "camera", view });
    await action({
      op: "hold",
      tick: plan.tick,
      cameraSeconds: plan.cameraSeconds,
      time: plan.time,
    });
    const held = await until(
      (s) => s.battle?.hold?.reached && s.battle.camera === view,
      `The battle did not reach tick ${plan.tick}`,
      Math.max(30000, plan.tick * 100),
    );
    // Let the last held frames finish on the GPU before reading them back.
    await sleep(500);
    const metrics = await page.evaluate(() =>
      window.aethyrforge.cTownBenchmark({ op: "readBattle" }),
    );
    const probe = metrics.battleProbe;
    assert(probe?.finite, "The held battle probe is missing or invalid.");
    assert.equal(probe.tick, plan.tick, "The GPU did not stop at the tick.");
    const state = battleStateDigest(probe);
    const first = await grab(plan.time),
      second = await grab(plan.time);
    const rgba = Buffer.from(first.rgba, "base64"),
      png = Buffer.from(first.png, "base64");
    const { width, height } = pngSize(png);
    assert.equal(rgba.length, width * height * 4);
    const repeatable = second.rgba === first.rgba;
    const stem = `battle-tick-${label}-${view}-s${plan.seed}-t${plan.tick}`;
    await writeFile(resolve(output, `${stem}.png`), png);
    await writeFile(resolve(output, `${stem}.rgba`), rgba);
    report.views.push({
      view,
      width,
      height,
      tick: probe.tick,
      stateSha256: state.sha256,
      probeSampleSha256: probe.sampleSha256,
      rgbaSha256: sha(rgba),
      pngSha256: sha(png),
      repeatable,
      pngFile: `${stem}.png`,
      rgbaFile: `${stem}.rgba`,
      hold: held.battle.hold,
      gpu: probe.gpu,
    });
    console.log(
      `${repeatable ? "PASS" : "FAIL"} ${view} tick ${probe.tick}: rgba ${sha(rgba).slice(0, 16)} state ${state.sha256.slice(0, 16)}`,
    );
  }
  await action({ op: "release" });
  report.ok =
    report.views.length === plan.views.length &&
    report.views.every((v) => v.repeatable) &&
    report.errors.length === 0;
} catch (error) {
  report.errors.push(String(error?.stack ?? error));
} finally {
  report.finishedAt = new Date().toISOString();
  const path = resolve(
    output,
    `battle-tick-${label}-s${plan.seed}-t${plan.tick}.json`,
  );
  await writeFile(path, JSON.stringify(report, null, 2) + "\n");
  console.log(`Report: ${path}`);
  await context.close().catch(() => {});
  await browser.close().catch(() => {});
}
process.exit(report.ok ? 0 : 1);
