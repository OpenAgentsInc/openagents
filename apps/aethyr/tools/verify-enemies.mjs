/** Run through openagents browser run; licensed captures and fixtures stay private. */
import assert from "node:assert/strict";
import { distance, assertHeldCorpses } from "./gameplay-workloads.mjs";
import { createHash } from "node:crypto";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const checkout = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set a private AF_CAPTURE_DIR.");
assert(
  process.env.AF_DRUID_FIXTURES,
  "Set AF_DRUID_FIXTURES to private, source-bound town probes.",
);
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== checkout && !output.startsWith(checkout + sep),
  "Keep licensed captures outside Git.",
);
const fixtures = JSON.parse(
  await readFile(process.env.AF_DRUID_FIXTURES, "utf8"),
);
assert.equal(fixtures.schema, "aethyrforge.druid-browser-fixtures.v1");
const release = process.env.AETHYRFORGE_RELEASE_CONFIG
  ? JSON.parse(await readFile(process.env.AETHYRFORGE_RELEASE_CONFIG, "utf8"))
  : undefined;
const mode = release
  ? process.env.AETHYRFORGE_CHECK_MODE || "production"
  : "local";
assert(["local", "candidate", "production"].includes(mode));
const injectAssetFailure = process.env.AF_INJECT_ASSET_FAILURE === "1";
const url = new URL(
  release
    ? process.env.AF_HOST_URL || "https://openagents.com/demo/medieval-town"
    : process.env.AF_EDITOR_URL || "http://127.0.0.1:55314/",
);
function checkURL(value) {
  assert(!value.username && !value.password);
  assert(
    value.protocol === "https:" ||
      (value.protocol === "http:" &&
        ["localhost", "127.0.0.1"].includes(value.hostname)),
  );
}
checkURL(url);
if (!release) assert(["localhost", "127.0.0.1"].includes(url.hostname));
const editorURL = release && new URL(release.editor);
const previewURL = release && new URL(release.preview);
if (release) {
  checkURL(editorURL);
  checkURL(previewURL);
  assert.equal(
    new Set([url.origin, editorURL.origin, previewURL.origin]).size,
    3,
  );
}
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run through openagents browser run.");
const report = {
  schema: "aethyrforge.enemies-browser.v1",
  mode,
  release: release?.release,
  startedAt: new Date().toISOString(),
  url: url.href,
  viewport: [1280, 720],
  fixtures: { scene: fixtures.scene, sourceId: fixtures.sourceId },
  ok: false,
  checks: [],
  receipts: [],
  routes: [],
  restoreSetups: [],
  captures: [],
  samples: [],
  errors: [],
  console: [],
  failed: [],
  blockedWrites: [],
  credentialViolations: [],
  assetRecovery: {
    enabled: injectAssetFailure,
    attempts: [],
    intentionalFailures: [],
    completedResponses: [],
    retryNotices: [],
  },
};
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
  serviceWorkers: "block",
});
await context.addInitScript(() => {
  window.__targetingRetryNotices = [];
  const save = (message, visible) => {
    if (/A world file stopped downloading\. Trying again/.test(message || ""))
      window.__targetingRetryNotices.push({
        message,
        visible,
        at: performance.now(),
      });
  };
  window.addEventListener("message", (event) => {
    if (event.data?.type === "status") save(event.data.status?.message, false);
  });
  document.addEventListener("DOMContentLoaded", () => {
    const sample = () => {
      const notice = document.querySelector("#notice");
      if (!notice) return;
      const style = getComputedStyle(notice);
      save(
        notice.textContent,
        style.display !== "none" &&
          style.visibility !== "hidden" &&
          notice.getBoundingClientRect().height > 0,
      );
    };
    new MutationObserver(sample).observe(document.documentElement, {
      subtree: true,
      childList: true,
      characterData: true,
    });
    sample();
  });
});
const page = await context.newPage();
page.setDefaultTimeout(30_000);
let intentionalShaderFailure = false;
let closing = false;
const headerAudits = [];
const sourceNormals = (value) =>
  /\/private-assets\/geometry\/source-normals-[a-f0-9]{64}\.bin$/.test(
    new URL(value).pathname,
  );
const intentionalAborts = new WeakSet();
const responseCompletions = [];
let interruptedAssetURL;
page.on("pageerror", (error) => report.errors.push(String(error)));
page.on("console", (message) => {
  if (["error", "warning"].includes(message.type()))
    report.console.push({
      type: message.type(),
      text: message.text().slice(0, 4096),
      location: message.location(),
      intentionalShaderFailure,
    });
});
page.on("response", (response) => {
  if (response.status() >= 400)
    report.failed.push({ url: response.url(), status: response.status() });
  if (injectAssetFailure && sourceNormals(response.url()))
    responseCompletions.push(
      response.finished().then((error) => {
        report.assetRecovery.completedResponses.push({
          url: response.url(),
          status: response.status(),
          error: error ? String(error) : null,
        });
      }),
    );
});
page.on("requestfailed", (request) => {
  if (intentionalAborts.has(request))
    report.assetRecovery.intentionalFailures.push({
      url: request.url(),
      error: request.failure()?.errorText,
    });
  else if (!closing)
    report.failed.push({
      url: request.url(),
      error: request.failure()?.errorText,
    });
});
page.on("request", (request) => {
  if (
    !release ||
    ![editorURL.origin, previewURL.origin].includes(
      new URL(request.url()).origin,
    )
  )
    return;
  headerAudits.push(
    request.allHeaders().then((headers) => {
      for (const name of ["authorization", "cookie"])
        if (headers[name])
          report.credentialViolations.push({ url: request.url(), name });
    }),
  );
});
// Spells and movement affect this disposable preview only. No account write is needed.
await page.route("**/*", async (route) => {
  if (
    injectAssetFailure &&
    route.request().method() === "GET" &&
    sourceNormals(route.request().url())
  ) {
    report.assetRecovery.attempts.push({
      url: route.request().url(),
      at: new Date().toISOString(),
    });
    if (!interruptedAssetURL) {
      interruptedAssetURL = route.request().url();
      intentionalAborts.add(route.request());
      await route.abort("connectionreset");
      return;
    }
  }
  if (!["GET", "HEAD", "OPTIONS"].includes(route.request().method())) {
    report.blockedWrites.push({
      method: route.request().method(),
      url: route.request().url(),
    });
    await route.abort("blockedbyclient");
  } else await route.continue();
});
if (mode === "candidate") {
  assert(process.env.AETHYRFORGE_HOST_RESPONSE_URL);
  const candidate = new URL(process.env.AETHYRFORGE_HOST_RESPONSE_URL);
  checkURL(candidate);
  await page.route(url.origin + url.pathname + "**", async (route) => {
    const requested = new URL(route.request().url());
    const response = await context.request.get(
      new URL(requested.pathname + requested.search, candidate).href,
    );
    const headers = { ...response.headers() };
    for (const name of [
      "content-encoding",
      "content-length",
      "transfer-encoding",
    ])
      delete headers[name];
    await route.fulfill({
      status: response.status(),
      headers,
      body: await response.body(),
    });
  });
}

function editorFrame() {
  if (!release) return page.mainFrame();
  return page.frames().find((f) => {
    try {
      return new URL(f.url()).origin === editorURL.origin;
    } catch {
      return false;
    }
  });
}
async function previewFrame() {
  const handle = await editorFrame().locator("#preview-frame").elementHandle();
  const frame = await handle?.contentFrame();
  assert(frame);
  return frame;
}
const invoke = (name, ...args) =>
  editorFrame().evaluate(
    ({ name, args }) => window.aethyrforge[name](...args),
    { name, args },
  );
const snapshot = () => invoke("snapshot");
const status = async () => (await snapshot()).status;
async function inspect() {
  return editorFrame().evaluate(() =>
    window.aethyrforge.agent.inspect({ limit: 1 }),
  );
}
async function until(test, message, timeout = 20000) {
  const end = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await page.waitForTimeout(60);
  } while (Date.now() < end);
  throw Error(
    message + "; status=" + JSON.stringify(await status().catch(() => null)),
  );
}
async function receiptAfter(action, ok = true) {
  const previous = new Set((await snapshot()).receipts.map((r) => r.requestId));
  const id = await action();
  const r = await until(
    async () =>
      (await snapshot()).receipts.find((r) =>
        typeof id === "string"
          ? r.requestId === id
          : !previous.has(r.requestId),
      ),
    "No action receipt",
    40000,
  );
  report.receipts.push(r);
  assert.equal(r.ok, ok, r.message);
  return r;
}
function record(name, details = {}) {
  report.checks.push({ name, ...details });
  console.log("PASS " + name);
}
async function capture(name) {
  const path = resolve(output, `enemies-${mode}-${name}.png`);
  const png = await page.screenshot({ path });
  report.captures.push({
    name,
    path,
    sha256: createHash("sha256").update(png).digest("hex"),
    enemies: (await status()).gameplay?.enemies,
  });
}
async function point(target, eye = [6, 3.37, 20]) {
  await (await previewFrame()).evaluate(() => document.exitPointerLock());
  await receiptAfter(() => invoke("cameraPose", eye, target, 1, "fly"));
  const canvas = (await previewFrame()).locator("#world");
  await canvas.focus();
  const b = await canvas.boundingBox();
  assert(b);
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
}
async function select(ability, key) {
  if ((await status()).gameplay.selected !== ability) {
    await (await previewFrame()).locator("#world").focus();
    await receiptAfter(() => page.keyboard.press(`Digit${key}`));
  }
}
async function restore() {
  await receiptAfter(() => invoke("selectAbility", null));
  await point([6, 2, 26.5]);
  await receiptAfter(() => invoke("resetGameplay"));
  await until(async () => {
    const s = await status();
    return (
      s.gameplay?.enemies?.alive === 8 &&
      s.gameplay.enemies.killed === 0 &&
      !s.gameplay.destroyed &&
      s
    );
  }, "Restore did not revive all knights");
}
async function actualEnemyState() {
  const value = await inspect();
  assert(value.enemies && value.enemies.actors.length === 8);
  assert.equal(value.enemies.alive + value.enemies.killed, 8);
  return value;
}
async function cast() {
  return receiptAfter(() => page.keyboard.press("Enter"));
}
try {
  report.browser = browser.version();
  assert.deepEqual(await context.cookies(), []);
  const begun = performance.now();
  await page.goto(url.href, { waitUntil: "domcontentloaded" });
  if (release) {
    await page.waitForFunction(
      () =>
        window.medievalDemo &&
        JSON.parse(window.medievalDemo.snapshot()).phase === "open",
      null,
      { timeout: 180000 },
    );
    assert.equal(
      JSON.parse(await page.locator("#engine-config").textContent()).release,
      release.release,
    );
    assert.equal(await page.locator("header").count(), 0);
  }
  await until(
    async () =>
      editorFrame() &&
      editorFrame().evaluate(() => Boolean(window.aethyrforge)),
    "The editor did not open",
    180000,
  );
  const initial = await until(
    async () => {
      const s = await status();
      assert.notEqual(s.phase, "error", s.message);
      return (
        s.phase === "ready" &&
        s.frame > 3 &&
        s.character?.modelLoaded &&
        s.gameplay?.enemies?.modelLoaded &&
        s.gameplay.enemies.rendered === 8 &&
        s
      );
    },
    "The animated knights did not open",
    180000,
  );
  report.readyMs = performance.now() - begun;
  const before = await actualEnemyState();
  assert.equal(before.enemies.alive, 8);
  assert.equal(before.enemies.killed, 0);
  await page.waitForTimeout(1200);
  const moved = await actualEnemyState();
  const moving = moved.enemies.actors.filter(
    (a) =>
      distance(
        a.position,
        before.enemies.actors.find((b) => b.id === a.id).position,
      ) > 0.5,
  );
  assert(
    moving.length >= 6,
    JSON.stringify({ before: before.enemies, after: moved.enemies }),
  );
  assert.equal(moved.context.revision, before.context.revision);
  await capture("running-town");
  record(
    "Eight animated knights patrol the actual town without authored edits",
    {
      readyMs: report.readyMs,
      moving: moving.length,
      enemies: initial.gameplay.enemies,
    },
  );

  await restore();
  await point([100, 150, 100], [100, 50, 100]);
  await select("lightning_bolt", "2");
  await until(
    async () => !(await status()).gameplay.target,
    "Sky aim should be empty",
  );
  await receiptAfter(() => page.keyboard.press("Enter"), false);
  assert.equal((await actualEnemyState()).enemies.killed, 0);
  record("A missed spell cannot kill a knight");

  await restore();
  await point([6.5, 1.9, 26.25], [6, 1.87, 35]);
  await select("lightning_bolt", "2");
  await until(
    async () => (await status()).gameplay.target,
    "The arch did not block the shot",
  );
  await cast();
  await page.waitForTimeout(1000);
  assert.equal((await actualEnemyState()).enemies.killed, 0);
  await capture("arch-blocks-lightning");
  record("The intact arch blocks Lightning from killing knights behind it");

  await restore();
  let lightningBefore = await actualEnemyState(),
    lightningAfter;
  await select("lightning_bolt", "2");
  for (let attempt = 0; attempt < 5; attempt++) {
    const live = await actualEnemyState();
    const actor =
      live.enemies.actors.find((a) => a.id === 6 && a.alive) ??
      live.enemies.actors.find((a) => a.id === 7 && a.alive);
    assert(actor);
    await point([
      actor.position[0],
      actor.position[1] + 0.9,
      actor.position[2],
    ]);
    try {
      await cast();
    } catch (error) {
      if (attempt === 4) throw error;
      continue;
    }
    await page.waitForTimeout(300);
    lightningAfter = await actualEnemyState();
    if (lightningAfter.enemies.killed > 0) break;
  }
  assert(
    lightningAfter?.enemies.killed > 0,
    "Lightning did not hit a moving knight",
  );
  assert.equal(
    lightningAfter.context.revision,
    lightningBefore.context.revision,
  );
  await capture("lightning-knight-falls");
  record("Real Lightning hits a moving knight and starts its death animation", {
    before: lightningBefore.enemies,
    after: lightningAfter.enemies,
  });
  await page.waitForTimeout(2200);
  const held = await actualEnemyState();
  assertHeldCorpses(lightningAfter.enemies, held.enemies);
  await capture("dead-knight-stays-down");
  record("Defeated knights stop moving and remain down after the death clip");

  await restore();
  await point([6.8, 1.0, 26.5]);
  await select("meteor_shower", "1");
  await until(
    async () => (await status()).gameplay.target,
    "Meteor has no ground target",
  );
  const meteorBefore = await actualEnemyState();
  await capture("meteor-target");
  await cast();
  const meteorAfter = await until(
    async () => {
      const value = await actualEnemyState();
      return value.enemies.killed > 0 && value;
    },
    "Meteor impacts did not kill any running knights",
    12000,
  );
  assert.equal(meteorAfter.context.revision, meteorBefore.context.revision);
  await page.waitForTimeout(2500);
  await capture("meteor-knights-down");
  record(
    "Meteor impacts kill knights at their live positions while the town keeps running",
    { before: meteorBefore.enemies, after: meteorAfter.enemies },
  );
  const damage = await actualEnemyState();
  intentionalShaderFailure = true;
  await receiptAfter(
    () => invoke("shader", "this is deliberately invalid WGSL;"),
    false,
  );
  await page.waitForTimeout(500);
  intentionalShaderFailure = false;
  const retained = await actualEnemyState();
  assert.equal(retained.enemies.killed, damage.enemies.killed);
  assert.equal(retained.context.revision, damage.context.revision);
  assert(distance(retained.player.position, damage.player.position) < 0.1);
  record("A failed shader preserves the player and defeated knights");
  await restore();
  await capture("restored-knights");
  record("Restore town revives every knight and clears building damage");
  const lighting = (await snapshot()).environment;
  await receiptAfter(() =>
    invoke("environment", { ...lighting, mode: "night" }),
  );
  await until(async () => {
    const s = await status();
    return (
      s.gameplay?.enemies?.alive === 8 && s.gameplay.enemies.rendered === 8 && s
    );
  }, "The night patrol did not appear");
  const night = await actualEnemyState();
  await page.waitForTimeout(1000);
  const nightMoved = await actualEnemyState();
  assert(
    nightMoved.enemies.actors.filter(
      (a) =>
        distance(
          a.position,
          night.enemies.actors.find((b) => b.id === a.id).position,
        ) > 0.5,
    ).length >= 6,
  );
  await capture("night-running-knights");
  record("Night uses the same animated, collision-checked patrols");
  await receiptAfter(() => invoke("environment", { ...lighting, mode: "day" }));
  const perfStart = await status(),
    at = performance.now();
  await page.waitForTimeout(2200);
  const perfEnd = await status();
  report.performance = {
    elapsedMs: performance.now() - at,
    frameBefore: perfStart.frame,
    frameAfter: perfEnd.frame,
    reportedFps: perfEnd.fps,
    scope:
      "Short running-knight view, no GPU execution timing or whole-town qualification",
  };
  await Promise.all(headerAudits);
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.failed, []);
  assert.deepEqual(report.blockedWrites, []);
  assert.deepEqual(report.credentialViolations, []);
  assert.deepEqual(
    report.console.filter(
      (x) =>
        x.type === "error" &&
        !(
          x.intentionalShaderFailure &&
          /WGSL|shader|compil|parse|validation/i.test(x.text)
        ),
    ),
    [],
  );
  report.ok = true;
} catch (error) {
  report.error = String(error?.stack || error);
  report.finalSnapshot = await snapshot().catch(() => null);
  try {
    await capture("failure");
  } catch {}
  throw error;
} finally {
  report.endedAt = new Date().toISOString();
  await writeFile(
    resolve(output, `enemies-${mode}-browser.json`),
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(
    JSON.stringify({
      ok: report.ok,
      checks: report.checks.length,
      report: resolve(output, `enemies-${mode}-browser.json`),
      error: report.error,
    }),
  );
  closing = true;
  await context.close();
  await browser.close();
}
