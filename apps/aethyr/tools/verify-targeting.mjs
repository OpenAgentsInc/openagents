/** Run through openagents browser run; licensed captures and fixtures stay private. */
import assert from "node:assert/strict";
import { projectWorldTarget } from "./gameplay-workloads.mjs";
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
  schema: "aethyrforge.targeting-browser.v1",
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
  return page.frames().find((frame) => {
    try {
      return new URL(frame.url()).origin === editorURL.origin;
    } catch {
      return false;
    }
  });
}
async function previewFrame() {
  const frame = await editorFrame().locator("#preview-frame").elementHandle();
  const child = await frame?.contentFrame();
  assert(child, "The editor did not open its preview.");
  return child;
}
const invoke = (name, ...args) =>
  editorFrame().evaluate(
    ({ name, args }) => window.aethyrforge[name](...args),
    { name, args },
  );
const snapshot = () => invoke("snapshot");
const status = async () => (await snapshot()).status;
const distance = (a, b) => Math.hypot(...a.map((v, i) => v - b[i]));
function record(name, details = {}) {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
}
async function until(test, message, timeout = 20000) {
  const end = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await page.waitForTimeout(60);
  } while (Date.now() < end);
  throw Error(
    `${message}; status=${JSON.stringify(await status().catch(() => null))}`,
  );
}
async function receiptAfter(action, expected = true) {
  const previous = new Set((await snapshot()).receipts.map((r) => r.requestId));
  const id = await action();
  const receipt = await until(
    async () =>
      (await snapshot()).receipts.find((r) =>
        typeof id === "string"
          ? r.requestId === id
          : !previous.has(r.requestId),
      ),
    "No action receipt",
  );
  report.receipts.push(receipt);
  assert.equal(receipt.ok, expected, receipt.message);
  return receipt;
}
async function frames(prior, count = 2) {
  return until(async () => {
    const s = await status();
    assert.notEqual(s.phase, "error", s.message);
    return s.frame >= prior + count && s;
  }, "The world stopped rendering");
}
async function capture(name) {
  const path = resolve(output, `targeting-${mode}-${name}.png`);
  const png = await page.screenshot({ path });
  const s = await status();
  report.captures.push({
    name,
    path,
    sha256: createHash("sha256").update(png).digest("hex"),
    frame: s.frame,
    target: s.gameplay?.target,
    aimTriangles: s.gameplay?.rendered?.aimTriangles,
  });
}
async function locked() {
  return (await previewFrame()).evaluate(
    () => document.pointerLockElement === document.querySelector("#world"),
  );
}
async function unlock() {
  await (await previewFrame()).evaluate(() => document.exitPointerLock());
  await until(async () => !(await locked()), "Pointer lock did not exit");
}
async function worldBounds() {
  const b = await (await previewFrame()).locator("#world").boundingBox();
  assert(b);
  return b;
}
async function pointer(x, y) {
  const b = await worldBounds();
  await page.mouse.move(b.x + b.width * x, b.y + b.height * y);
}
async function selectKey(ability, key) {
  await (await previewFrame()).locator("#world").focus();
  if ((await status()).gameplay.selected !== ability)
    await receiptAfter(() => page.keyboard.press(`Digit${key}`));
  return until(async () => {
    const s = await status();
    return s.gameplay.selected === ability && s;
  }, "The number key did not select the ability");
}
async function targetState() {
  return until(async () => {
    const s = await status();
    return s.gameplay.target && s.gameplay.rendered.aimTriangles === 128 && s;
  }, "The selected spell did not submit a world target ring");
}
async function pose(probe, mode = "walk") {
  await unlock();
  await receiptAfter(() => invoke("selectAbility", null));
  await frames((await status()).frame);
  await receiptAfter(() =>
    invoke("cameraPose", probe.eye, probe.target, 1, mode),
  );
  const before = await status();
  return until(async () => {
    const s = await status();
    return (
      s.frame > before.frame + 2 &&
      s.view.mode === mode &&
      (mode === "fly" ||
        (s.character?.grounded && Math.abs(s.character.velocity[1]) < 0.02)) &&
      s
    );
  }, "The test camera did not settle");
}
async function restore() {
  await pose(fixtures.safeRestore, "fly");
  await receiptAfter(() => invoke("resetGameplay"));
  await until(async () => {
    const s = await status();
    return (
      !s.gameplay.destroyed &&
      !s.gameplay.debris &&
      !s.gameplay.rubble &&
      !s.gameplay.effects &&
      s
    );
  }, "Restore left spell state");
  return pose(fixtures.abilities.lightning_bolt);
}
async function perspective(value) {
  if ((await status()).character.perspective !== value) {
    await (await previewFrame()).locator("#world").focus();
    await receiptAfter(() => page.keyboard.press("KeyV"));
  }
  const before = await status();
  await until(async () => {
    const s = await status();
    return (
      s.character?.perspective === value && s.frame > before.frame + 3 && s
    );
  }, "The V key did not switch perspective");
}
async function lockWorld() {
  await receiptAfter(() => invoke("selectAbility", null));
  const b = await worldBounds();
  await (
    await previewFrame()
  )
    .locator("#world")
    .click({ position: { x: b.width * 0.5, y: b.height * 0.5 } });
  await until(locked, "The world did not acquire pointer lock");
}
async function projectedTarget(s, x, y) {
  const b = await worldBounds();
  const actual = projectWorldTarget({
    eye: s.camera,
    viewEye: s.view.eye,
    target: s.view.target,
    point: s.gameplay.target.point,
    width: b.width,
    height: b.height,
  });
  report.samples.push({
    frame: s.frame,
    expectedNDC: [x, y],
    actualNDC: actual,
    target: s.gameplay.target,
    camera: s.camera,
    perspective: s.character?.perspective,
  });
  assert(
    Math.abs(actual[0] - x) < 0.025 && Math.abs(actual[1] - y) < 0.025,
    `The world marker misses its cursor: ${actual} expected ${[x, y]}`,
  );
}
async function castAtMarker(action, expectedLocked, scatter = 0) {
  const before = await targetState();
  const point = [...before.gameplay.target.point];
  await receiptAfter(action);
  const after = await until(
    async () => {
      const s = await status();
      return (
        s.gameplay.casts === before.gameplay.casts + 1 &&
        s.gameplay.impacts > before.gameplay.impacts &&
        s.gameplay.lastImpact &&
        s
      );
    },
    "The selected spell did not impact",
    25000,
  );
  assert.equal(
    await locked(),
    expectedLocked,
    "Casting changed pointer lock and retargeted the click",
  );
  assert(
    distance(point, after.gameplay.lastImpact.point) < scatter + 0.08,
    `Impact misses displayed marker: ${JSON.stringify({ point, impact: after.gameplay.lastImpact })}`,
  );
  assert.equal(
    after.revision,
    before.revision,
    "Play changed the authored town",
  );
  assert(
    distance(before.character.position, after.character.position) < 0.15,
    "Casting moved the player without input",
  );
  return { before, after, point };
}
async function proveVisibleRing() {
  await pose(fixtures.abilities.lightning_bolt, "fly");
  await (await previewFrame()).locator("#world").focus();
  await pointer(0.5, 0.5);
  await frames((await status()).frame);
  const baseline = await editorFrame().evaluate(async () => {
    const pixels = new Uint8Array(
      await window.aethyrforge.capturePixels({ time: 0 }),
    );
    window.__targetingBaseline = pixels;
    return { bytes: pixels.byteLength };
  });
  await selectKey("meteor_shower", "1");
  const selected = await targetState();
  await capture("visible-ring-on");
  const pixels = await editorFrame().evaluate(async () => {
    const before = window.__targetingBaseline;
    const after = new Uint8Array(
      await window.aethyrforge.capturePixels({ time: 0 }),
    );
    if (!before || before.length !== after.length)
      throw Error("Ring captures have different sizes");
    let changed = 0,
      greenRing = 0,
      maxGreenIncrease = 0;
    for (let i = 0; i < after.length; i += 4) {
      const delta =
        Math.abs(after[i] - before[i]) +
        Math.abs(after[i + 1] - before[i + 1]) +
        Math.abs(after[i + 2] - before[i + 2]);
      if (delta > 24) changed++;
      const green = after[i + 1] - before[i + 1];
      maxGreenIncrease = Math.max(maxGreenIncrease, green);
      if (
        delta > 24 &&
        green > 8 &&
        after[i + 1] - after[i] > before[i + 1] - before[i] + 4
      )
        greenRing++;
    }
    delete window.__targetingBaseline;
    return { bytes: after.length, changed, greenRing, maxGreenIncrease };
  });
  assert.equal(pixels.bytes, baseline.bytes);
  assert(
    pixels.greenRing > 40,
    `No visibly colored surface ring: ${JSON.stringify(pixels)}`,
  );
  await receiptAfter(() => invoke("selectAbility", null));
  await until(
    async () => (await status()).gameplay.rendered.aimTriangles === 0,
    "Deselecting left the target ring",
  );
  await capture("visible-ring-off");
  record(
    "A submitted world ring adds visible pixels on the actual wall and disappears on deselect",
    { pixels, target: selected.gameplay.target },
  );
}
async function checkFullscreenLayout() {
  const surfaces = [];
  if (release) {
    const host = await page.evaluate(() => {
      const frame = document.querySelector("#engine");
      if (!(frame instanceof HTMLIFrameElement))
        throw Error("The host has no world frame");
      const visible = (element) => {
        const style = getComputedStyle(element),
          rect = element.getBoundingClientRect();
        return (
          style.display !== "none" &&
          style.visibility !== "hidden" &&
          rect.width > 0 &&
          rect.height > 0
        );
      };
      const headers = Array.from(
        document.querySelectorAll(
          "header,[role=banner],.world-header,.page-header,h1",
        ),
      )
        .filter(visible)
        .map((element) => element.textContent.trim().slice(0, 160));
      const rect = frame.getBoundingClientRect();
      return {
        role: "host",
        width: innerWidth,
        height: innerHeight,
        rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
        headers,
      };
    });
    assert.deepEqual(
      host.headers,
      [],
      "The published host still has a visible title header",
    );
    surfaces.push(host);
  }
  for (const [role, frame, selector] of [
    ["editor", editorFrame(), "#preview-frame"],
    ["preview", await previewFrame(), "#world"],
  ]) {
    surfaces.push(
      await frame.evaluate(
        ({ role, selector }) => {
          const element = document.querySelector(selector);
          if (!element) throw Error(`Missing ${selector}`);
          const rect = element.getBoundingClientRect();
          return {
            role,
            width: innerWidth,
            height: innerHeight,
            rect: {
              x: rect.x,
              y: rect.y,
              width: rect.width,
              height: rect.height,
            },
          };
        },
        { role, selector },
      ),
    );
  }
  for (const surface of surfaces) {
    const r = surface.rect;
    assert(
      Math.abs(r.x) <= 1 &&
        Math.abs(r.y) <= 1 &&
        Math.abs(r.width - surface.width) <= 1 &&
        Math.abs(r.height - surface.height) <= 1,
      `The ${surface.role} world does not fill its viewport: ${JSON.stringify(surface)}`,
    );
  }
  record(
    release
      ? "The host has no title header and every world surface fills the viewport"
      : "Editor and preview world surfaces fill the viewport",
    { surfaces },
  );
}
async function checkAssetRecovery() {
  if (!injectAssetFailure) return;
  await Promise.all(responseCompletions);
  for (const frame of page.frames()) {
    const notices = await frame
      .evaluate(() => window.__targetingRetryNotices || [])
      .catch(() => []);
    report.assetRecovery.retryNotices.push(
      ...notices.map((notice) => ({ frame: frame.url(), ...notice })),
    );
  }
  assert(
    interruptedAssetURL,
    "No source-normal geometry request was interrupted",
  );
  assert.equal(
    report.assetRecovery.intentionalFailures.length,
    1,
    "The injected failure was not observed exactly once",
  );
  assert(
    report.assetRecovery.attempts.filter(
      (attempt) => attempt.url === interruptedAssetURL,
    ).length >= 2,
    "The interrupted geometry was not requested again",
  );
  assert(
    report.assetRecovery.completedResponses.some(
      (response) =>
        response.url === interruptedAssetURL &&
        response.status === 200 &&
        response.error === null,
    ),
    "The retry did not finish a successful geometry response",
  );
  assert(
    report.assetRecovery.retryNotices.some((notice) => notice.visible),
    "The retry message was never visible in the world",
  );
  record(
    "One interrupted source-normal download retries, shows a message, and opens the world",
    { assetRecovery: report.assetRecovery },
  );
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
        s.frame > 2 &&
        s.character?.modelLoaded &&
        s.character.grounded &&
        s.gameplay?.enabled &&
        s
      );
    },
    "The druid town did not open",
    180000,
  );
  report.readyMs = performance.now() - begun;
  await checkAssetRecovery();
  await checkFullscreenLayout();
  const bindings = await (
    await previewFrame()
  ).evaluate(async () => {
    const r = await fetch("/private-assets/scene.json");
    if (!r.ok) throw Error("Scene failed");
    const s = await r.json();
    return { gameplayURL: s.gameplayURL, collisionURL: s.collisionURL };
  });
  assert.deepEqual(bindings, fixtures.scene);
  assert.equal(
    await editorFrame().locator(".druid-reticle").count(),
    0,
    "The body-centered DOM crosshair remains",
  );
  assert.equal(
    await (await previewFrame()).locator("#reticle").count(),
    0,
    "The legacy center dot remains",
  );
  record("The current source-bound town opens without a DOM crosshair", {
    readyMs: report.readyMs,
    frame: initial.frame,
  });
  await proveVisibleRing();
  await restore();
  await perspective("third");
  await lockWorld();
  await selectKey("meteor_shower", "1");
  let aimed = await targetState();
  await projectedTarget(aimed, 0, 0.22);
  await capture("third-person-locked");
  record(
    "Locked third-person aim projects above the avatar using the rendered camera",
    { target: aimed.gameplay.target },
  );
  await unlock();
  await pointer(0.6, 0.48);
  await selectKey("lightning_bolt", "2");
  aimed = await targetState();
  await until(async () => {
    const s = await targetState();
    try {
      await projectedTarget(s, 0.2, 0.04);
      return s;
    } catch {
      return false;
    }
  }, "The unlocked third-person cursor did not move its world ring");
  await capture("third-person-cursor");
  const b = await worldBounds();
  const lightning = await castAtMarker(
    () =>
      previewFrame().then((f) =>
        f
          .locator("#world")
          .click({ position: { x: b.width * 0.6, y: b.height * 0.48 } }),
      ),
    false,
  );
  record(
    "Unlocked cursor click keeps pointer lock off and Lightning hits the displayed world point",
    { point: lightning.point, impact: lightning.after.gameplay.lastImpact },
  );
  await capture("third-person-click-impact");
  await restore();
  await perspective("first");
  await lockWorld();
  await selectKey("thunderbolt", "3");
  aimed = await targetState();
  await projectedTarget(aimed, 0, 0);
  await capture("first-person-locked");
  const thunder = await castAtMarker(() => page.keyboard.press("Enter"), true);
  record(
    "First-person locked aim uses the viewport center and Enter strikes that same point",
    { point: thunder.point, impact: thunder.after.gameplay.lastImpact },
  );
  await capture("first-person-enter-impact");
  await restore();
  await perspective("first");
  await unlock();
  await pointer(0.5, 0.5);
  await selectKey("meteor_shower", "1");
  aimed = await targetState();
  await projectedTarget(aimed, 0, 0);
  await capture("first-person-cursor");
  const meteor = await castAtMarker(
    () => page.keyboard.press("Enter"),
    false,
    6.1,
  );
  record(
    "First-person unlocked aiming selects Meteor and keeps its impacts within the displayed target footprint",
    { point: meteor.point, impact: meteor.after.gameplay.lastImpact },
  );
  await capture("meteor-cursor-impact");
  await restore();
  await pose({ eye: [100, 50, 100], target: [100, 150, 90] }, "fly");
  await pointer(0.5, 0.5);
  await selectKey("meteor_shower", "1");
  await until(async () => {
    const s = await status();
    return !s.gameplay.target && s.gameplay.rendered.aimTriangles === 0 && s;
  }, "An invalid sky aim kept the target ring");
  const before = await status();
  await receiptAfter(() => page.keyboard.press("Enter"), false);
  const after = await frames(before.frame);
  assert.equal(after.gameplay.casts, before.gameplay.casts);
  assert.equal(after.revision, before.revision);
  record("Invalid sky aim hides the ring and cannot cast");
  await Promise.all(headerAudits);
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.failed, []);
  assert.deepEqual(report.blockedWrites, []);
  assert.deepEqual(report.credentialViolations, []);
  const intentionalConsole = (message) =>
    injectAssetFailure &&
    /Failed to load resource: net::ERR_(CONNECTION_RESET|FAILED)/.test(
      message.text,
    ) &&
    (message.location?.url === interruptedAssetURL ||
      message.text.includes(interruptedAssetURL));
  report.assetRecovery.intentionalConsole =
    report.console.filter(intentionalConsole);
  assert.deepEqual(
    report.console.filter((m) => m.type === "error" && !intentionalConsole(m)),
    [],
    "Unexpected browser or GPU errors",
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
    resolve(output, `targeting-${mode}-browser.json`),
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(
    JSON.stringify({
      ok: report.ok,
      checks: report.checks.length,
      report: resolve(output, `targeting-${mode}-browser.json`),
      error: report.error,
    }),
  );
  closing = true;
  await context.close();
  await browser.close();
}
