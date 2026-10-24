/** Run through openagents browser run; licensed captures and fixtures stay private. */
import assert from "node:assert/strict";
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
  schema: "aethyrforge.druid-browser.v1",
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
};
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
  serviceWorkers: "block",
});
const page = await context.newPage();
page.setDefaultTimeout(30_000);
let intentionalShaderFailure = false;
let closing = false;
const headerAudits = [];
page.on("pageerror", (error) => report.errors.push(String(error)));
page.on("console", (message) => {
  if (["error", "warning"].includes(message.type()))
    report.console.push({
      type: message.type(),
      text: message.text().slice(0, 4096),
      intentionalShaderFailure,
    });
});
page.on("response", (response) => {
  if (response.status() >= 400)
    report.failed.push({ url: response.url(), status: response.status() });
});
page.on("requestfailed", (request) => {
  if (!closing)
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
const feet = (value) => value.character.position;
const distance = (a, b) => Math.hypot(...a.map((n, i) => n - b[i]));
const horizontal = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
function record(name, details = {}) {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
}
function checkStatus(value) {
  assert.notEqual(value.phase, "error", value.message);
  const vectors = [value.camera, value.view.eye, value.view.target];
  if (value.view.mode === "walk")
    assert(value.character, "Walking lost its character summary.");
  if (value.character) vectors.push(feet(value), value.character.velocity);
  for (const vector of vectors)
    assert(vector.every((n) => Number.isFinite(n) && Math.abs(n) < 20_000));
  const g = value.gameplay;
  assert(g?.enabled);
  for (const key of [
    "destroyed",
    "debris",
    "effects",
    "casts",
    "impacts",
    "tick",
  ])
    assert(Number.isSafeInteger(g[key]) && g[key] >= 0, key);
  assert(g.destroyed <= 65_536 && g.debris <= 256 && g.effects <= 256);
  assert(Number.isSafeInteger(g.rubble) && g.rubble >= 0 && g.rubble <= 1024);
  assert((g.rendered?.rubbleCount || 0) <= 1024);
  assert((g.rendered?.geometryBytes || 0) <= 512 * 1024 * 1024);
}
function assertPoseRetained(before, after) {
  assert.equal(after.view.mode, before.view.mode);
  if (before.view.mode === "walk") {
    assert(before.character && after.character);
    assert(
      distance(feet(after), feet(before)) < 0.12,
      "The walking player moved without input.",
    );
  } else {
    assert(
      distance(after.camera, before.camera) < 0.12,
      "The inspection camera moved without input.",
    );
    assert(
      distance(after.view.target, before.view.target) < 0.12,
      "The inspection camera changed its aim.",
    );
  }
}
async function until(test, message, timeout = 20_000) {
  const end = Date.now() + timeout;
  do {
    const result = await test();
    if (result) return result;
    await page.waitForTimeout(75);
  } while (Date.now() < end);
  throw Error(
    `${message}; status=${JSON.stringify(await status().catch(() => null))}`,
  );
}
async function receiptAfter(action, expected = true) {
  const previous = new Set((await snapshot()).receipts.map((r) => r.requestId));
  const requestId = await action();
  const receipt = await until(
    async () =>
      (await snapshot()).receipts.find((r) =>
        typeof requestId === "string"
          ? r.requestId === requestId
          : !previous.has(r.requestId),
      ),
    "No action receipt arrived",
  );
  report.receipts.push(receipt);
  assert.equal(receipt.ok, expected, receipt.message);
  return receipt;
}
async function frames(prior, count = 2) {
  return until(async () => {
    const next = await status();
    checkStatus(next);
    return next.frame >= prior + count && next;
  }, "The preview stopped submitting frames");
}
async function capture(name) {
  const path = resolve(output, `druid-${mode}-${name}.png`);
  const png = await page.screenshot({ path });
  report.captures.push({
    name,
    path,
    sha256: createHash("sha256").update(png).digest("hex"),
    frame: (await status()).frame,
  });
}
async function releaseKeys() {
  for (const key of [
    "KeyW",
    "KeyA",
    "KeyS",
    "KeyD",
    "Space",
    "ShiftLeft",
    "ShiftRight",
  ])
    await page.keyboard.up(key);
}
async function focusWorld() {
  await receiptAfter(() => invoke("selectAbility", null));
  await (
    await previewFrame()
  )
    .locator("#world")
    .click({ position: { x: 640, y: 300 } });
}
async function pose(probe, mode = "walk") {
  await releaseKeys();
  const prior = await status();
  await receiptAfter(() => invoke("selectAbility", null));
  await receiptAfter(() =>
    invoke("cameraPose", probe.eye, probe.target, 1, mode),
  );
  const expected = probe.target.map((n, i) => n - probe.eye[i]);
  const expectedLength = Math.hypot(...expected);
  let previousFrame = -1,
    stableFrames = 0,
    previousFeet;
  return until(async () => {
    const next = await status();
    checkStatus(next);
    const heading = next.view.target.map((n, i) => n - next.view.eye[i]);
    const headingLength = Math.hypot(...heading);
    const aligned =
      heading.reduce((sum, n, i) => sum + n * expected[i], 0) /
        (headingLength * expectedLength) >
      0.9995;
    const positioned =
      Math.hypot(
        next.view.eye[0] - probe.eye[0],
        next.view.eye[2] - probe.eye[2],
      ) < 0.05;
    if (next.frame > previousFrame) {
      const settled =
        mode === "fly" ||
        (next.character?.grounded &&
          Math.abs(next.character.velocity[1]) < 0.02 &&
          (!previousFeet || distance(feet(next), previousFeet) < 0.01));
      stableFrames =
        next.frame > prior.frame &&
        next.view.mode === mode &&
        aligned &&
        positioned &&
        settled
          ? stableFrames + 1
          : 0;
      previousFrame = next.frame;
      previousFeet = next.character?.position;
    }
    return stableFrames >= 3 && next;
  }, "The fixture camera did not settle");
}
async function selectWithKey(ability, key) {
  await focusWorld();
  await receiptAfter(() => page.keyboard.press(`Digit${key}`));
  return until(async () => {
    const next = await status();
    return next.gameplay.selected === ability && next.gameplay.target && next;
  }, `The real ${key} key did not select ${ability} or find a surface`);
}
async function walkBreach(broken, probe = fixtures.breach) {
  const before = await pose(probe);
  await focusWorld();
  const started = performance.now();
  const samples = [];
  let jumped = false,
    jumpReleased = false;
  if (probe.jumpAtTick === 0) {
    await page.keyboard.down("Space");
    jumped = true;
  }
  await page.keyboard.down("KeyW");
  try {
    await until(
      async () => {
        const value = await status();
        checkStatus(value);
        if (
          probe.jumpAtTick !== undefined &&
          !jumped &&
          value.gameplay.tick >= before.gameplay.tick + probe.jumpAtTick
        ) {
          await page.keyboard.down("Space");
          jumped = true;
        }
        if (
          jumped &&
          !jumpReleased &&
          value.gameplay.tick >= before.gameplay.tick + probe.jumpAtTick + 3
        ) {
          await page.keyboard.up("Space");
          jumpReleased = true;
        }
        if (!samples.length || value.frame !== samples.at(-1).frame)
          samples.push({
            frame: value.frame,
            tick: value.gameplay.tick,
            feet: feet(value),
            velocity: value.character.velocity,
            grounded: value.character.grounded,
          });
        return (
          value.gameplay.tick >= before.gameplay.tick + (probe.holdTicks || 60)
        );
      },
      "The held-W route did not advance its bounded simulation ticks",
      12_000,
    );
  } finally {
    await page.keyboard.up("KeyW");
    await page.keyboard.up("Space");
  }
  const next = await until(async () => {
    const value = await frames((await status()).frame, 1);
    return (
      value.character.grounded &&
      Math.hypot(value.character.velocity[0], value.character.velocity[2]) <
        0.1 &&
      value
    );
  }, "The capsule did not settle after the wall route");
  const axis = probe.axis ?? 2;
  const result = {
    broken,
    before: feet(before),
    after: feet(next),
    elapsedMs: performance.now() - started,
    axis,
    beforeTick: before.gameplay.tick,
    afterTick: next.gameplay.tick,
    samples,
    view: next.view,
    destroyed: next.gameplay.destroyed,
    debris: next.gameplay.debris,
    rubble: next.gameplay.rubble,
    renderedRubble: next.gameplay.rendered.rubbleCount,
    jumpAtTick: probe.jumpAtTick,
  };
  report.routes.push(result);
  const diagnostic = JSON.stringify(result);
  await (await previewFrame()).evaluate(() => document.exitPointerLock());
  await capture(
    `${broken ? "damaged-wall-route" : "intact-wall-blocks-walker"}-${report.routes.length}`,
  );
  assert.equal(next.revision, before.revision);
  if (
    broken &&
    feet(next)[axis] <= probe.minBroken &&
    probe.alternates?.length
  ) {
    console.log(
      `The first breach route is blocked by solid rubble; trying a separately verified opening: ${diagnostic}`,
    );
    for (const alternate of probe.alternates) {
      try {
        return await walkBreach(true, alternate);
      } catch (error) {
        if (
          !/damaged wall still blocks|W input did not move/.test(String(error))
        )
          throw error;
      }
    }
  }
  assert(
    feet(next)[axis] > feet(before)[axis] + 1,
    `The real W input did not move the capsule toward the wall: ${diagnostic}`,
  );
  if (broken)
    assert(
      feet(next)[axis] > probe.minBroken,
      `The damaged wall still blocks the walking capsule: ${diagnostic}`,
    );
  else
    assert(
      feet(next)[axis] < probe.maxIntact,
      `The intact wall failed to block the walking capsule: ${diagnostic}`,
    );
  record(
    broken
      ? probe.jumpAtTick !== undefined
        ? "Real W and jump cross the opening over solid rubble"
        : "Real W input walks the capsule through the damaged wall"
      : probe.jumpAtTick !== undefined
        ? "The same W and jump stop against the intact wall"
        : "The same W route stops against the intact wall",
    result,
  );
  return result;
}
async function restore() {
  await releaseKeys();
  if (fixtures.safeRestore) {
    const from = await status();
    const safe = await pose(fixtures.safeRestore);
    report.restoreSetups.push({
      from: from.character?.position ?? from.camera,
      to: feet(safe),
      reason:
        "Move outside original wall footprints before restoring their collision.",
    });
  }
  const before = await status();
  await receiptAfter(() => invoke("resetGameplay"));
  const next = await until(async () => {
    const value = await status();
    checkStatus(value);
    const game = value.gameplay,
      rendered = game.rendered;
    return (
      !game.destroyed &&
      !game.debris &&
      !game.rubble &&
      !game.effects &&
      !game.casts &&
      !game.impacts &&
      !rendered.maskedNodes &&
      !rendered.fracturedSources &&
      !rendered.debrisTriangles &&
      !rendered.effectTriangles &&
      !rendered.rubbleCount &&
      !rendered.rubbleTriangles &&
      value
    );
  }, "Restore left broken geometry or stale collision state");
  assert.equal(next.revision, before.revision);
  assertPoseRetained(before, next);
  return next;
}
async function observeCast(before, ability, move) {
  const samples = [],
    peaks = {
      effects: 0,
      debris: 0,
      destroyed: 0,
      rubble: 0,
      effectTriangles: 0,
      debrisTriangles: 0,
      standingTriangles: 0,
      rubbleCount: 0,
      rubbleTriangles: 0,
    };
  let effectCaptured = false,
    impactCaptured = false;
  const started = Date.now();
  if (move) await page.keyboard.down("KeyW");
  try {
    await until(
      async () => {
        if (move && Date.now() - started >= (move.holdMs || 450)) {
          await page.keyboard.up("KeyW");
          move = null;
        }
        const next = await status();
        checkStatus(next);
        assert.equal(
          next.revision,
          before.revision,
          "Playing changed authored scene revision.",
        );
        const g = next.gameplay;
        const sample = {
          frame: next.frame,
          fps: next.fps,
          feet: feet(next),
          gameplay: g,
        };
        if (!samples.length || sample.frame !== samples.at(-1).frame)
          samples.push(sample);
        for (const key of ["effects", "debris", "destroyed", "rubble"])
          peaks[key] = Math.max(peaks[key], g[key]);
        for (const key of [
          "effectTriangles",
          "debrisTriangles",
          "standingTriangles",
          "rubbleCount",
          "rubbleTriangles",
        ])
          peaks[key] = Math.max(peaks[key], g.rendered[key]);
        if (!effectCaptured && g.rendered.effectTriangles > 0) {
          effectCaptured = true;
          await capture(`${ability}-effect`);
        }
        if (
          ability === "meteor_shower" &&
          !impactCaptured &&
          g.impacts > before.gameplay.impacts &&
          g.rendered.effectTriangles > 0
        ) {
          impactCaptured = true;
          await capture("meteor_shower-impacts");
        }
        return (
          g.casts > before.gameplay.casts &&
          !g.casting &&
          g.impacts > before.gameplay.impacts &&
          Date.now() - started > 1600 &&
          (ability !== "meteor_shower" ||
            g.impacts >= before.gameplay.impacts + 6) &&
          next
        );
      },
      `${ability} did not finish its impacts`,
      25_000,
    );
  } finally {
    await releaseKeys();
  }
  assert(
    peaks.effects > 0 && peaks.effectTriangles > 0,
    "The spell did not submit visible effect geometry.",
  );
  const next = await frames((await status()).frame);
  report.samples.push({
    ability,
    elapsedMs: Date.now() - started,
    peaks,
    samples,
  });
  return { next, peaks };
}
async function observePersistentRubble(ability, prior) {
  const retained = await until(
    async () => {
      const value = await status();
      checkStatus(value);
      const game = value.gameplay;
      return (
        game.tick >= prior.gameplay.tick + 1380 &&
        game.debris === 0 &&
        game.rubble > 0 &&
        game.rendered.rubbleCount > 0 &&
        game.rendered.rubbleTriangles > 0 &&
        value
      );
    },
    `${ability} did not retain smaller textured rubble after the transient debris lifetime`,
    40_000,
  );
  assert.equal(retained.revision, prior.revision);
  assert.equal(
    retained.gameplay.rendered.rubbleCount,
    retained.gameplay.rubble,
    "A retained physical piece has no nonempty textured geometry.",
  );
  await capture(`${ability}-persistent-small-rubble`);
  record(`${ability} keeps textured rubble after 23 simulation seconds`, {
    frame: retained.frame,
    tick: retained.gameplay.tick,
    rubble: retained.gameplay.rubble,
    rendered: retained.gameplay.rendered,
  });
  await pose(
    fixtures.rubbleOverview || {
      eye: [10, 7, 27],
      target: [5.5, 0.5, 31],
    },
    "fly",
  );
  await capture(`${ability}-persistent-rubble-close-overhead`);
  const returned = await pose(
    { eye: retained.view.eye, target: retained.view.target },
    retained.view.mode,
  );
  assertPoseRetained(retained, returned);
  if (retained.gameplay.selected)
    await receiptAfter(() =>
      invoke("selectAbility", retained.gameplay.selected),
    );
  return retained;
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
      { timeout: 180_000 },
    );
    const pinned = JSON.parse(
      await page.locator("#engine-config").textContent(),
    );
    assert.equal(pinned.release, release.release);
  }
  await until(
    async () =>
      editorFrame() &&
      editorFrame().evaluate(() => Boolean(window.aethyrforge)),
    "The editor did not open",
    180_000,
  );
  const initial = await until(
    async () => {
      const value = await status();
      assert.notEqual(value.phase, "error", value.message);
      return (
        value.phase === "ready" &&
        value.frame > 2 &&
        value.character?.modelLoaded &&
        value.character.grounded &&
        value.gameplay?.enabled &&
        value
      );
    },
    "The Druid and source-bound destructible town did not become ready",
    180_000,
  );
  report.readyMs = performance.now() - begun;
  checkStatus(initial);
  const sourceBindings = await (
    await previewFrame()
  ).evaluate(async () => {
    const response = await fetch("/private-assets/scene.json");
    if (!response.ok) throw Error("The source scene did not load.");
    const scene = await response.json();
    return { gameplayURL: scene.gameplayURL, collisionURL: scene.collisionURL };
  });
  assert.deepEqual(
    sourceBindings,
    fixtures.scene,
    "The private probes belong to a different prepared town.",
  );
  const fragmentProof = JSON.parse(
    await readFile(fixtures.fragments.proofFile, "utf8"),
  );
  assert.equal(
    fragmentProof.schema,
    "aethyrforge.druid-rubble-kernel-proof.v1",
  );
  assert.equal(fragmentProof.gameplayURL, sourceBindings.gameplayURL);
  assert.equal(fragmentProof.collisionURL, sourceBindings.collisionURL);
  assert.equal(fragmentProof.fragmentGrid, 4);
  assert.equal(fragmentProof.fragmentSlots, 64);
  const liveKernelHash = await (
    await previewFrame()
  ).evaluate(async () => {
    const bytes = await fetch("/kernel.wasm").then((response) =>
      response.arrayBuffer(),
    );
    return [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
      .map((value) => value.toString(16).padStart(2, "0"))
      .join("");
  });
  assert.equal(fragmentProof.kernelSha256, liveKernelHash);
  assert.deepEqual(
    fragmentProof.scenarios.map((entry) => entry.ability).sort(),
    ["lightning_bolt", "meteor_shower", "thunderbolt"],
  );
  for (const entry of fragmentProof.scenarios) {
    assert(entry.rubbleAfterTTL > 0 && entry.rubbleAfterTTL <= 1024);
    assert.equal(entry.rubbleAfterTTL, entry.rubbleFragments.length);
    assert(entry.bodyGeometryValidated && entry.highWordBodies > 0);
    assert(Math.abs(entry.halfExtentRatioMin - 0.125) < 1e-5);
    assert(Math.abs(entry.halfExtentRatioMax - 0.125) < 1e-5);
    const maskBytes = await readFile(entry.fragmentMaskFile);
    assert.equal(
      createHash("sha256").update(maskBytes).digest("hex"),
      entry.fragmentMaskSha256,
    );
    const masks = new Map(
      JSON.parse(maskBytes).map(({ nodeId, mask }) => [nodeId, mask]),
    );
    for (const body of entry.rubbleFragments) {
      const mask = masks.get(body.nodeId);
      assert(
        mask && body.subdivisions === 4 && body.chunk >= 0 && body.chunk < 64,
      );
      assert(((mask[body.chunk >>> 5] >>> (body.chunk & 31)) & 1) !== 0);
      for (let axis = 0; axis < 3; axis++) {
        const half = (body.sourceMax[axis] - body.sourceMin[axis]) / 8;
        assert(
          half > 0 &&
            Math.abs(body.halfExtents[axis] - half) <=
              Math.max(2e-5, half * 2e-5),
        );
      }
    }
  }
  report.fragmentProof = {
    path: fixtures.fragments.proofFile,
    kernelSha256: liveKernelHash,
    fragmentGrid: 4,
    fragmentSlots: 64,
    abilities: fragmentProof.scenarios.map((entry) => ({
      ability: entry.ability,
      rubbleAfterTTL: entry.rubbleAfterTTL,
      halfExtentRatioMin: entry.halfExtentRatioMin,
      halfExtentRatioMax: entry.halfExtentRatioMax,
    })),
  };
  record(
    "The live kernel matches real 64-piece ordinary-spell geometry and size proofs",
    report.fragmentProof,
  );
  await capture("initial");
  record("The Druid, owned collision, and destructible town open", {
    readyMs: report.readyMs,
    status: initial,
  });
  const firstPerson = editorFrame().getByRole("button", {
    name: "Switch to first person",
    exact: true,
  });
  if (await firstPerson.count()) await receiptAfter(() => firstPerson.click());
  await until(
    async () => (await status()).character.perspective === "first",
    "First-person aiming did not activate",
  );
  if (fixtures.breach) {
    await restore();
    await walkBreach(false);
    for (const alternate of fixtures.breach.alternates || [])
      await walkBreach(false, alternate);
  }

  for (const [ability, key] of [
    ["lightning_bolt", "2"],
    ["meteor_shower", "1"],
    ["thunderbolt", "3"],
  ]) {
    await restore();
    const probe = fixtures.abilities[ability];
    assert(probe, `Missing ${ability} fixture.`);
    await pose(probe);
    const before = await selectWithKey(ability, key);
    if (probe.nodeId !== undefined)
      assert.equal(
        before.gameplay.target.nodeId,
        probe.nodeId,
        "The fixture no longer aims at its bound wall cell.",
      );
    await capture(`${ability}-intact`);
    let result;
    for (let attempt = 0; attempt < (probe.maxCasts || 4); attempt++) {
      const prior = await status();
      // Exercise the actual visible control as well as keyboard selection.
      await (await previewFrame()).evaluate(() => document.exitPointerLock());
      await receiptAfter(() =>
        editorFrame()
          .getByRole("button", { name: "Cast", exact: true })
          .click(),
      );
      if (ability === "meteor_shower" && probe.moveWhileCasting)
        await (await previewFrame()).locator("#world").focus();
      result = await observeCast(
        prior,
        ability,
        ability === "meteor_shower" && probe.moveWhileCasting
          ? probe.moveWhileCasting
          : null,
      );
      if (
        result.next.gameplay.destroyed > 0 &&
        (ability !== "lightning_bolt" ||
          result.next.gameplay.target?.nodeId !== probe.nodeId)
      )
        break;
    }
    assert(
      result.next.gameplay.destroyed > 0,
      `${ability} did not destroy real source cells.`,
    );
    assert(
      result.next.gameplay.rendered.maskedNodes > 0 &&
        result.next.gameplay.rendered.fracturedSources > 0,
    );
    assert(
      result.peaks.debris > 0 && result.peaks.debrisTriangles > 0,
      "Destroyed cells did not submit debris geometry.",
    );
    assert(
      result.next.gameplay.rendered.standingTriangles > 0,
      "The source replacement omitted all standing wall geometry.",
    );
    if (ability === "meteor_shower" && probe.moveWhileCasting)
      assert(
        horizontal(feet(before), feet(result.next)) > 0.2,
        "W did not move the player while the cast continued.",
      );
    else
      assert(
        distance(feet(before), feet(result.next)) < 0.12,
        "Casting moved the player without input.",
      );
    await capture(`${ability}-damaged`);
    record(
      `${ability} destroys source cells and renders effects, standing walls, and debris`,
      {
        before: before.gameplay,
        after: result.next.gameplay,
        peaks: result.peaks,
      },
    );

    if (ability === "lightning_bolt" && fixtures.partial) {
      const hole = await pose(probe, "fly");
      await receiptAfter(() => invoke("selectAbility", ability));
      const through = await frames(hole.frame);
      assert(
        !through.gameplay.target ||
          through.gameplay.target.nodeId !== probe.nodeId,
        "The broken wall still blocks the aim ray.",
      );
      await pose(fixtures.partial, "fly");
      await receiptAfter(() => invoke("selectAbility", ability));
      const surviving = await until(async () => {
        const next = await status();
        return next.gameplay.target?.nodeId === fixtures.partial.nodeId && next;
      }, "An undamaged cell of the same source disappeared with the broken cell");
      assert(surviving.gameplay.rendered.standingTriangles > 0);
      await capture("lightning-surviving-sibling");
      record(
        "A ray passes through the broken cell while another cell of the same source still blocks it",
        {
          destroyedNode: probe.nodeId,
          survivingNode: fixtures.partial.nodeId,
          sourceId: fixtures.sourceId,
        },
      );
    }

    await observePersistentRubble(ability, result.next);
    if (ability === "lightning_bolt" && fixtures.breach) {
      await walkBreach(true);
    }

    intentionalShaderFailure = true;
    const damaged = await status();
    const failure = await receiptAfter(
      () => invoke("shader", "this is deliberately invalid WGSL;"),
      false,
    );
    assert(
      /shader|WGSL|compil|parse|unexpected token|syntax/i.test(failure.message),
      failure.message,
    );
    const retained = await frames(damaged.frame, 3);
    intentionalShaderFailure = false;
    assert.equal(retained.gameplay.destroyed, damaged.gameplay.destroyed);
    assert.equal(retained.gameplay.rubble, damaged.gameplay.rubble);
    assert.equal(
      retained.gameplay.rendered.rubbleCount,
      damaged.gameplay.rendered.rubbleCount,
    );
    assert.equal(
      retained.gameplay.rendered.fracturedSources,
      damaged.gameplay.rendered.fracturedSources,
    );
    assert.equal(retained.revision, damaged.revision);
    assertPoseRetained(damaged, retained);
    await capture(`${ability}-failed-shader-retained`);
    record(
      "A failed shader edit preserves the damaged town, player, and running renderer",
      { ability, frameBefore: damaged.frame, frameAfter: retained.frame },
    );
    // Leave the opening before restoring its solid wall on the next iteration.
    if (ability === "lightning_bolt" && fixtures.breach) await pose(probe);
  }

  const reset = await restore();
  await pose(fixtures.abilities.lightning_bolt, "fly");
  await receiptAfter(() => invoke("selectAbility", "lightning_bolt"));
  const intact = await until(async () => {
    const next = await status();
    return (
      next.gameplay.target?.nodeId ===
        fixtures.abilities.lightning_bolt.nodeId && next
    );
  }, "Restoring did not restore the wall's collision");
  assert.equal(intact.gameplay.destroyed, 0);
  await capture("restored");
  record("Restore clears spells and debris and restores real wall collision", {
    revision: reset.revision,
  });
  await receiptAfter(() => invoke("selectAbility", null));
  const perfStart = await status(),
    perfTime = performance.now();
  await page.waitForTimeout(2200);
  const perfEnd = await frames(perfStart.frame);
  report.performance = {
    elapsedMs: performance.now() - perfTime,
    frameBefore: perfStart.frame,
    frameAfter: perfEnd.frame,
    submittedFps:
      ((perfEnd.frame - perfStart.frame) * 1000) /
      (performance.now() - perfTime),
    reportedFps: perfEnd.fps,
    qualification:
      "Short browser observation; no production performance qualification or GPU execution timing.",
  };
  await Promise.all(headerAudits);
  assert.deepEqual(report.errors, []);
  const unexpected = report.console.filter(
    (item) =>
      item.type === "error" &&
      !(
        item.intentionalShaderFailure &&
        /WGSL|shader|compil|parse|validation/i.test(item.text)
      ),
  );
  assert.deepEqual(unexpected, [], "Unexpected browser or GPU errors.");
  assert.deepEqual(report.failed, [], "A browser asset request failed.");
  assert.deepEqual(report.blockedWrites, []);
  assert.deepEqual(report.credentialViolations, []);
  report.ok = true;
} catch (error) {
  report.error = String(error?.stack || error);
  report.finalSnapshot = await snapshot().catch(() => null);
  try {
    await capture("failure");
  } catch {}
  throw error;
} finally {
  await releaseKeys().catch(() => {});
  report.endedAt = new Date().toISOString();
  await writeFile(
    resolve(output, `druid-${mode}-browser.json`),
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(
    JSON.stringify({
      ok: report.ok,
      report: resolve(output, `druid-${mode}-browser.json`),
      checks: report.checks.length,
      error: report.error,
    }),
  );
  closing = true;
  await context.close();
  await browser.close();
}
