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
  process.env.AF_CASTLE_FIXTURES,
  "Set AF_CASTLE_FIXTURES to private, source-bound town probes.",
);
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== checkout && !output.startsWith(checkout + sep),
  "Keep licensed captures outside Git.",
);
const fixtures = JSON.parse(
  await readFile(process.env.AF_CASTLE_FIXTURES, "utf8"),
);
assert.equal(fixtures.schema, "aethyrforge.castle-browser-fixtures.v1");
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
const deadline = Date.now() + 240_000;
const report = {
  schema: "aethyrforge.castle-browser.v1",
  mode,
  release: release?.release,
  startedAt: new Date().toISOString(),
  url: url.href,
  viewport: [1280, 720],
  fixtures: fixtures.scenes,
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
  assert(g.castle.rubble <= 1024);
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
    assert(
      Date.now() < deadline,
      "The four-minute verification deadline elapsed.",
    );
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
  const path = resolve(output, `castle-${mode}-${name}.png`);
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
  // Let a preceding ability's queued look input finish before placing the probe.
  await frames(prior.frame, 2);
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

const focused = mode === "production" && process.env.AF_CASTLE_FOCUSED === "1";
const fixture = fixtures.scenes.find((item) => item.scene === "scene.json");
assert(fixture, "Supply the private day castle fixture.");
const game = (value) => value.gameplay;
const castle = (value) => game(value).castle;
const button = (name) =>
  editorFrame().getByRole("button", { name, exact: true });
const close = (a, b, tolerance = 0.12) => distance(a, b) < tolerance;
function summary(value) {
  return {
    frame: value.frame,
    fps: value.fps,
    camera: value.camera,
    feet: value.character?.position,
    revision: value.revision,
    gameplay: value.gameplay,
  };
}
async function cancel() {
  await receiptAfter(() => invoke("cancelAbility"));
  return until(async () => {
    const next = await status();
    return !castle(next).preparing && !castle(next).active && next;
  }, "The barrage did not stop");
}
async function selectButton() {
  await (await previewFrame()).evaluate(() => document.exitPointerLock());
  if (game(await status()).selected === "destroy_castle")
    await receiptAfter(() => invoke("selectAbility", null));
  await receiptAfter(() =>
    editorFrame()
      .getByRole("button", { name: /Destroy the Castle/ })
      .click(),
  );
  return until(
    async () => game(await status()).selected === "destroy_castle",
    "The castle button did not select its ability",
  );
}
async function begin() {
  const before = await status();
  await (await previewFrame()).evaluate(() => document.exitPointerLock());
  await button("Cast").click();
  return before;
}
async function cancelDuringPreparation() {
  const trace = await editorFrame().evaluate(async () => {
    const api = window.aethyrforge;
    const before = api.snapshot().status;
    const started = performance.now();
    const requestId = api.castAbility();
    const samples = [];
    while (performance.now() - started < 10_000) {
      const value = api.snapshot().status;
      const state = value.gameplay.castle;
      const sample = {
        wallMs: performance.now() - started,
        frame: value.frame,
        preparing: state.preparing,
        active: state.active,
        destroyed: state.destroyed,
        impacts: state.impacts,
      };
      samples.push(sample);
      if (state.preparing || state.active || state.complete)
        return {
          before,
          requestId,
          samples,
          observed: sample,
          cancelRequestId: api.cancelAbility(),
        };
      await new Promise((resolve) => setTimeout(resolve, 2));
    }
    return { before, requestId, samples, timedOut: true };
  });
  report.samples.push({ preparationCancellation: trace });
  console.log(`Preparation cancellation ${JSON.stringify(trace)}`);
  assert(
    !trace.timedOut,
    "The preparation cancellation never observed a cast.",
  );
  await receiptAfter(() => trace.cancelRequestId);
  const stopped = await until(async () => {
    const value = await status();
    return !castle(value).preparing && !castle(value).active && value;
  }, "Preparation cancellation did not stop the cast");
  report.samples.push({ preparationCancellationStopped: summary(stopped) });
  assert.equal(castle(stopped).destroyed, castle(trace.before).destroyed);
  assert(close(feet(trace.before), feet(stopped)));
  return { trace, stopped };
}
async function observePreparation() {
  await editorFrame().evaluate(() => {
    const api = window.aethyrforge;
    window.__castlePreparationObservation = new Promise((resolve) => {
      const clicked = (event) => {
        const button = event.target.closest?.("button");
        if (button?.textContent.trim() !== "Cast") return;
        document.removeEventListener("click", clicked, true);
        const before = api.snapshot().status;
        const started = performance.now();
        const samples = [];
        const poll = () => {
          const value = api.snapshot().status;
          const state = value.gameplay.castle;
          const wallMs = performance.now() - started;
          samples.push({
            wallMs,
            frame: value.frame,
            preparing: state.preparing,
            active: state.active,
            complete: state.complete,
          });
          if (state.active || state.complete || wallMs > 35_000) {
            resolve({ before: before.frame, samples, wallMs });
            return;
          }
          setTimeout(poll, 10);
        };
        poll();
      };
      document.addEventListener("click", clicked, true);
    });
  });
}
async function checkPreparationObservation() {
  const trace = await editorFrame().evaluate(
    () => window.__castlePreparationObservation,
  );
  const preparing = trace.samples.filter((sample) => sample.preparing);
  const preparationMs = preparing.length
    ? trace.wallMs - preparing[0].wallMs
    : trace.wallMs;
  const preparingFrames = new Set(preparing.map((sample) => sample.frame));
  const evidence = {
    ...trace,
    preparationMs,
    preparingFrames: [...preparingFrames],
    fastCompletion: preparationMs <= 250,
  };
  report.samples.push({ preparationContinuity: evidence });
  console.log(`Preparation continuity ${JSON.stringify(evidence)}`);
  assert(trace.wallMs <= 35_000, "Preparation did not complete.");
  assert(
    preparationMs <= 500 || preparingFrames.size > 1,
    "Preparation lasted over 500 ms without submitting another frame; see preparationContinuity samples.",
  );
  record(
    preparationMs <= 500
      ? "Preparation completes promptly without requiring extra HUD frames"
      : "Frames continue during the full asynchronous preparation",
    evidence,
  );
}
async function liveProof() {
  assert(
    process.env.AF_CASTLE_CPU_PROOF,
    "Set AF_CASTLE_CPU_PROOF to the private raw-ID Wasm completion proof.",
  );
  const proof = JSON.parse(
    await readFile(process.env.AF_CASTLE_CPU_PROOF, "utf8"),
  );
  assert.equal(proof.schema, "aethyrforge.castle-kernel-proof.v1");
  const entry = proof.scenes.find((item) => item.scene === "scene.json");
  assert(entry, "The raw-ID CPU proof needs its day scenario.");
  const bindings = await (
    await previewFrame()
  ).evaluate(async () => {
    const sceneResponse = await fetch("/private-assets/scene.json");
    if (!sceneResponse.ok) throw Error("The private town scene did not load.");
    const scene = await sceneResponse.json();
    const response = await fetch(scene.gameplayURL);
    if (!response.ok) throw Error("The private castle group did not load.");
    const bytes = await response.arrayBuffer();
    const manifest = JSON.parse(new TextDecoder().decode(bytes));
    const ids = manifest.targetGroups.castle;
    const hash = [
      ...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    ]
      .map((v) => v.toString(16).padStart(2, "0"))
      .join("");
    const wasm = await fetch("/kernel.wasm").then((r) => r.arrayBuffer());
    const kernelSha256 = [
      ...new Uint8Array(await crypto.subtle.digest("SHA-256", wasm)),
    ]
      .map((v) => v.toString(16).padStart(2, "0"))
      .join("");
    return {
      gameplayURL: scene.gameplayURL,
      collisionURL: scene.collisionURL,
      ids,
      sources: [
        ...new Set(ids.map((id) => manifest.objects[id].sourceId)),
      ].sort((a, b) => a - b),
      hash,
      kernelSha256,
    };
  });
  assert.equal(bindings.gameplayURL, fixture.gameplayURL);
  assert.equal(bindings.collisionURL, fixture.collisionURL);
  assert(
    bindings.gameplayURL.includes(bindings.hash),
    "The live group does not match its content hash.",
  );
  assert.equal(bindings.ids.length, fixture.castleNodes);
  assert.deepEqual(
    bindings.sources,
    [...fixture.castleSourceIds].sort((a, b) => a - b),
  );
  assert.equal(bindings.sources.length, fixture.castleSourceIds.length);
  assert.equal(entry.gameplayURL, bindings.gameplayURL);
  assert.equal(entry.collisionURL, bindings.collisionURL);
  assert.equal(
    proof.kernelSha256,
    bindings.kernelSha256,
    "The raw-ID completion proof used a different Wasm kernel.",
  );
  assert.deepEqual(entry.castleNodeIds, bindings.ids);
  assert.deepEqual(
    [...entry.destroyedIds].sort((a, b) => a - b),
    bindings.ids,
  );
  assert.deepEqual(entry.outsideGroupDestroyed, []);
  assert.equal(entry.remaining, 0);
  const fragments = fixture.castleFragments;
  assert.equal(fragments.subdivisions, 4);
  assert.equal(fragments.piecesPerCell, 64);
  assert.equal(fragments.maskFormat, "u32x2");
  assert.equal(entry.fragmentGrid, fragments.subdivisions);
  assert.equal(entry.fragmentSlots, fragments.piecesPerCell);
  assert.equal(entry.maskFormat, fragments.maskFormat);
  assert.equal(entry.maxPersistentRubble, fragments.persistentLimit);
  assert.equal(entry.maxTransientDebris, fragments.transientLimit);
  assert(
    Number.isInteger(entry.rubbleAfterTTL) &&
      entry.rubbleAfterTTL >= fragments.minimumPersistent &&
      entry.rubbleAfterTTL <= fragments.persistentLimit,
  );
  assert.equal(entry.rubbleFragments.length, entry.rubbleAfterTTL);
  const maskBytes = await readFile(fragments.maskFile);
  assert.equal(
    createHash("sha256").update(maskBytes).digest("hex"),
    entry.fragmentMaskSha256,
  );
  const maskFile = JSON.parse(maskBytes);
  const masks = Array.isArray(maskFile) ? maskFile : maskFile.castleFragments;
  assert.equal(masks.length, bindings.ids.length);
  assert(
    masks.every(
      (pair) =>
        Array.isArray(pair) &&
        pair.length === 2 &&
        pair.every(
          (word) => Number.isInteger(word) && word >= 0 && word <= 0xffffffff,
        ),
    ),
  );
  const masksById = new Map(bindings.ids.map((id, i) => [id, masks[i]]));
  for (const body of entry.rubbleFragments) {
    assert.equal(body.subdivisions, 4);
    assert(Number.isInteger(body.chunk) && body.chunk >= 0 && body.chunk < 64);
    const mask = masksById.get(body.nodeId);
    assert(mask, "Permanent rubble came from outside the castle group.");
    assert(
      ((mask[body.chunk >>> 5] >>> (body.chunk & 31)) & 1) !== 0,
      "A permanent body has no nonempty clipped render fragment.",
    );
    for (let axis = 0; axis < 3; axis++) {
      const expected = (body.sourceMax[axis] - body.sourceMin[axis]) / 8;
      assert(expected > 0);
      assert(
        Math.abs(body.halfExtents[axis] - expected) <=
          Math.max(2e-5, expected * 2e-5),
        "A castle piece did not halve the prior linear dimensions.",
      );
    }
  }
  assert(entry.observedHighWordBodies > 0);
  assert.equal(entry.rubbleGeometryValidated, true);
  assert.equal(entry.rubbleGroundMatches, entry.rubbleAfterTTL);
  assert(entry.rubbleGroundMaxError < 0.03);
  const terrain = JSON.parse(await readFile(fixture.terrainProof, "utf8"));
  assert.equal(terrain.gameplayURL, bindings.gameplayURL);
  assert.equal(terrain.collisionURL, bindings.collisionURL);
  assert.equal(terrain.compiledPermanentHits, terrain.sampleCount);
  assert.equal(terrain.permanentMatchesVisible, terrain.sampleCount);
  const geometryBytes = await readFile(entry.rendererGeometryProofFile);
  assert.equal(
    createHash("sha256").update(geometryBytes).digest("hex"),
    entry.rendererGeometryProofSha256,
  );
  const geometry = JSON.parse(geometryBytes);
  assert.equal(geometry.schema, "aethyrforge.private-castle-renderer-64.v1");
  assert.equal(geometry.gameplay, bindings.gameplayURL);
  assert.equal(geometry.castleNodes, fixture.castleNodes);
  assert.equal(geometry.admitted, true);
  assert(geometry.visibleChunks > 0 && geometry.highWordChunks > 0);
  assert.equal(
    geometry.completedWorstCase.rubbleCount,
    fragments.persistentLimit,
  );
  assert.equal(
    geometry.completedWorstCase.renderedDebris,
    fragments.transientLimit,
  );
  assert.equal(entry.priorHalfExtentRatio, 0.25);
  assert.equal(entry.halfLinearSizeComparedToPrior, true);
  assert.deepEqual(
    [...entry.maskedSourceIds].sort((a, b) => a - b),
    bindings.sources,
  );
  report.cpuProof = {
    path: process.env.AF_CASTLE_CPU_PROOF,
    kernelSha256: bindings.kernelSha256,
    gameplayURL: bindings.gameplayURL,
    collisionURL: bindings.collisionURL,
    castleNodes: bindings.ids.length,
    sourceIds: bindings.sources,
    completionTicks: entry.completionTicks,
    rubbleAfterTTL: entry.rubbleAfterTTL,
    fragmentGrid: entry.fragmentGrid,
    fragmentSlots: entry.fragmentSlots,
    maskFormat: entry.maskFormat,
    observedHighWordBodies: entry.observedHighWordBodies,
    halfExtentRatioMin: entry.halfExtentRatioMin,
    halfExtentRatioMax: entry.halfExtentRatioMax,
    priorHalfExtentRatio: entry.priorHalfExtentRatio,
    rendererGeometryProofSha256: entry.rendererGeometryProofSha256,
    rendererMaxPieceWidth: geometry.maxPieceWidth,
    rubbleGroundMatches: entry.rubbleGroundMatches,
    rubbleGroundYMin: entry.rubbleGroundYMin,
    rubbleGroundYMax: entry.rubbleGroundYMax,
    rubbleGroundMaxError: entry.rubbleGroundMaxError,
    outsideGroupDestroyed: [],
  };
  record(
    "The live source group and Wasm match exact-ID full-castle CPU completion proof",
    report.cpuProof,
  );
  assert(
    fixture.collisionProbe?.physicalProof,
    "Supply the separate source-bound physical sweep proof.",
  );
  const physical = JSON.parse(
    await readFile(fixture.collisionProbe.physicalProof, "utf8"),
  );
  assert.equal(
    physical.schema,
    "aethyrforge.castle-targeting-collision-proof.v1",
  );
  assert.equal(physical.kernelSha256, bindings.kernelSha256);
  assert.equal(physical.gameplayURL, bindings.gameplayURL);
  assert.equal(physical.collisionURL, bindings.collisionURL);
  assert.deepEqual(physical.probe.eye, fixture.collisionProbe.eye);
  assert.deepEqual(physical.probe.target, fixture.collisionProbe.target);
  assert.deepEqual(physical.before.eye, fixture.collisionProbe.eye);
  assert.equal(physical.before.aim.nodeId, fixture.collisionProbe.nodeId);
  assert(
    physical.before.physicalFraction > 0 &&
      physical.before.physicalFraction < 1,
  );
  assert.equal(physical.after.physicalFraction, 1);
  assert(
    !physical.after.aim || !bindings.ids.includes(physical.after.aim.nodeId),
  );
  assert.deepEqual(physical.reset, physical.before);
  assert.deepEqual(physical.destroyedIds, bindings.ids);
  report.physicalProof = {
    path: fixture.collisionProbe.physicalProof,
    kernelSha256: physical.kernelSha256,
    gameplayURL: physical.gameplayURL,
    collisionURL: physical.collisionURL,
    probe: physical.probe,
    before: physical.before,
    after: physical.after,
    reset: physical.reset,
  };
  record(
    "The matching CPU kernel blocks the physical sweep before destruction and clears it afterward",
    report.physicalProof,
  );
}
async function followCompletion() {
  const started = Date.now(),
    peaks = { effects: 0, debris: 0, effectTriangles: 0, debrisTriangles: 0 };
  let nextLog = 0,
    captureCount = 0;
  const complete = await until(
    async () => {
      const value = await status();
      checkStatus(value);
      const g = game(value),
        c = castle(value);
      assert.equal(value.revision, report.authoredRevision);
      for (const key of ["effects", "debris"])
        peaks[key] = Math.max(peaks[key], g[key]);
      for (const key of ["effectTriangles", "debrisTriangles"])
        peaks[key] = Math.max(peaks[key], g.rendered[key]);
      if (Date.now() >= nextLog) {
        console.log(
          `Castle ${c.destroyed}/${c.total}; waves=${c.waves}; frame=${value.frame}`,
        );
        report.samples.push(summary(value));
        nextLog = Date.now() + 5000;
      }
      if (
        captureCount < 2 &&
        g.rendered.effectTriangles > 0 &&
        (captureCount === 0 || c.destroyed > c.total / 2)
      ) {
        await capture(`barrage-${captureCount + 1}`);
        captureCount++;
      }
      return c.complete && !c.active && c.remaining === 0 && value;
    },
    "The sustained barrage did not finish the castle",
    120_000,
  );
  assert(
    peaks.effects > 0 &&
      peaks.debris > 0 &&
      peaks.effectTriangles > 0 &&
      peaks.debrisTriangles > 0,
    "The barrage did not submit its effect and textured-fragment geometry.",
  );
  assert.equal(
    game(complete).destroyed,
    fixture.castleNodes,
    "The barrage damaged objects outside its exact source group.",
  );
  assert.equal(game(complete).rendered.maskedNodes, fixture.castleNodes);
  assert.equal(
    game(complete).rendered.fracturedSources,
    fixture.castleSourceIds.length,
  );
  assert.equal(game(complete).rendered.standingTriangles, 0);
  await capture("completed");
  record(
    "The barrage removes every castle cell and attached source placement",
    {
      elapsedMs: Date.now() - started,
      complete: summary(complete),
      peaks,
    },
  );
  return complete;
}
async function targetingProbe(expected) {
  assert(
    fixture.collisionProbe,
    "Supply the source-bound castle targeting ray.",
  );
  await pose(fixture.collisionProbe, "fly");
  await receiptAfter(() => invoke("selectAbility", "lightning_bolt"));
  const value = await frames((await status()).frame, 2);
  const id = game(value).target?.nodeId;
  if (expected)
    assert.equal(
      id,
      fixture.collisionProbe.nodeId,
      "The intact castle no longer returns its expected selectable cell.",
    );
  else
    assert(
      id === undefined ||
        id < fixture.castleIDRange[0] ||
        id > fixture.castleIDRange[1],
      "The aim ray still selects a cell from the removed castle group.",
    );
  report.samples.push({
    targetingProbe: expected ? "intact" : "removed",
    target: game(value).target,
    frame: value.frame,
  });
  await receiptAfter(() => invoke("selectAbility", null));
}

try {
  report.browser = browser.version();
  report.focused = focused;
  assert.deepEqual(await context.cookies(), []);
  const started = performance.now();
  await page.goto(url.href, { waitUntil: "domcontentloaded" });
  if (release) {
    await page.waitForFunction(
      () =>
        window.medievalDemo &&
        JSON.parse(window.medievalDemo.snapshot()).phase === "open",
      null,
      { timeout: 180_000 },
    );
    const pin = JSON.parse(await page.locator("#engine-config").textContent());
    assert.equal(pin.release, release.release);
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
        value.character?.grounded &&
        castle(value)?.total &&
        value
      );
    },
    "The source-bound castle did not open",
    180_000,
  );
  report.readyMs = performance.now() - started;
  report.authoredRevision = initial.revision;
  await liveProof();
  assert.equal(castle(initial).total, fixture.castleNodes);
  assert.equal(castle(initial).remaining, fixture.castleNodes);
  await focusWorld();
  await receiptAfter(() => page.keyboard.press("Digit4"));
  const facing = await until(async () => {
    const value = await status();
    const toCastle = fixture.overview.target.map((n, i) => n - value.camera[i]);
    const heading = value.view.target.map((n, i) => n - value.view.eye[i]);
    const dot =
      toCastle.reduce((sum, n, i) => sum + n * heading[i], 0) /
      (Math.hypot(...toCastle) * Math.hypot(...heading));
    return (
      game(value).selected === "destroy_castle" &&
      value.frame > initial.frame &&
      dot > 0.98 &&
      value
    );
  }, "The real 4 key did not select the castle");
  assert(
    close(feet(initial), feet(facing)),
    "Selecting the castle teleported the player.",
  );
  const delta = fixture.overview.target.map((n, i) => n - facing.camera[i]);
  const heading = facing.view.target.map((n, i) => n - facing.view.eye[i]);
  assert(
    delta.reduce((sum, n, i) => sum + n * heading[i], 0) /
      (Math.hypot(...delta) * Math.hypot(...heading)) >
      0.98,
    "Selecting the castle did not face the source-bound district.",
  );
  await capture("normal-spawn-auto-facing");
  record(
    "Real key 4 faces the distant castle without moving the normal-spawn player",
    summary(facing),
  );
  await targetingProbe(true);
  await pose({ eye: initial.view.eye, target: initial.view.target });
  await selectButton();

  if (!focused) {
    const { trace, stopped: preparationStopped } =
      await cancelDuringPreparation();
    await capture("preparation-cancelled");
    record("An immediate cancellation applies no castle damage", {
      observedPreparing: trace.observed.preparing,
      before: summary(trace.before),
      stopped: summary(preparationStopped),
    });

    await pose({ eye: [100, 1.62, 100], target: [100, 1.62, 90] });
    await selectButton();
    await observePreparation();
    await begin();
    await checkPreparationObservation();
    const active = await until(
      async () => {
        const v = await status();
        return castle(v).active && game(v).rendered.effectTriangles > 0 && v;
      },
      "The retried barrage did not become active",
      35_000,
    );
    await (await previewFrame()).locator("#world").focus();
    await page.keyboard.down("KeyW");
    try {
      await until(
        async () => (await status()).gameplay.tick >= game(active).tick + 20,
        "Movement did not advance during the barrage",
        6000,
      );
    } finally {
      await page.keyboard.up("KeyW");
    }
    const moved = await status();
    assert(
      horizontal(feet(active), feet(moved)) > 0.2,
      "W did not move during the barrage.",
    );
    await (await previewFrame()).evaluate(() => document.exitPointerLock());
    await button("Stop barrage").click();
    const stopped = await until(async () => {
      const v = await status();
      return !castle(v).active && castle(v).cancelled && v;
    }, "The visible Stop barrage control did not stop new waves");
    await until(
      async () => game(await status()).tick >= game(stopped).tick + 120,
      "Pending strikes did not finish",
      8000,
    );
    const paused = await status();
    assert.equal(castle(paused).waves, castle(stopped).waves);
    assert(
      castle(paused).remaining > 0 &&
        castle(paused).remaining < fixture.castleNodes,
    );
    await capture("active-cancelled");
    record(
      "Real W movement continues during the barrage and Stop halts new waves",
      { active: summary(active), paused: summary(paused) },
    );
  }

  await pose({ eye: [100, 1.62, 100], target: [100, 1.62, 90] });
  await selectButton();
  await begin();
  await until(
    async () => castle(await status()).active,
    "The barrage did not resume",
    35_000,
  );
  await pose(fixture.overview, "fly");
  await capture("closer-source-bounds-overview");
  const completed = await followCompletion();
  console.log(
    "Castle complete; observing rubble beyond ordinary debris lifetime",
  );
  const persistent = await until(
    async () => {
      const value = await status();
      checkStatus(value);
      return (
        game(value).tick >= game(completed).tick + 1380 &&
        !game(value).debris &&
        castle(value).rubble >= fixture.castleFragments.minimumPersistent &&
        game(value).rendered.rubbleCount >=
          fixture.castleFragments.minimumPersistent &&
        game(value).rendered.rubbleTriangles > 0 &&
        value
      );
    },
    "Textured castle rubble did not survive the ordinary debris lifetime",
    40_000,
  );
  assert.equal(castle(persistent).remaining, 0);
  assert.equal(
    game(persistent).rendered.fracturedSources,
    fixture.castleSourceIds.length,
  );
  assert.equal(
    game(persistent).rendered.rubbleCount,
    castle(persistent).rubble,
    "Permanent fragments did not all submit nonempty textured geometry.",
  );
  await capture("persistent-textured-rubble");
  record(
    "Dense smaller textured rubble remains after more than 23 simulation seconds",
    summary(persistent),
  );
  await targetingProbe(false);
  await pose(fixture.overview, "fly");
  const beforeShader = await status();

  intentionalShaderFailure = true;
  const failed = await receiptAfter(
    () => invoke("shader", "this is deliberately invalid WGSL;"),
    false,
  );
  assert(
    /WGSL|shader|compil|parse|unexpected token|syntax/i.test(failed.message),
  );
  const retained = await frames(beforeShader.frame, 3);
  intentionalShaderFailure = false;
  assert.equal(castle(retained).remaining, 0);
  assert.equal(game(retained).rendered.maskedNodes, fixture.castleNodes);
  assert.equal(
    game(retained).rendered.rubbleCount,
    game(beforeShader).rendered.rubbleCount,
  );
  assertPoseRetained(beforeShader, retained);
  assert.equal(retained.revision, report.authoredRevision);
  await capture("failed-shader-keeps-rubble");
  record(
    "A failed shader keeps the completed destruction, camera, and textured rubble",
    summary(retained),
  );

  await pose({ eye: [100, 1.62, 100], target: [100, 1.62, 90] });
  const beforeReset = await status();
  await receiptAfter(() => button("Restore town").click());
  const reset = await until(async () => {
    const value = await status(),
      g = game(value),
      c = castle(value),
      r = g.rendered;
    return (
      c.remaining === fixture.castleNodes &&
      !c.active &&
      !c.preparing &&
      !c.rubble &&
      !g.destroyed &&
      !g.debris &&
      !g.effects &&
      !r.maskedNodes &&
      !r.fracturedSources &&
      !r.rubbleCount &&
      !r.rubbleTriangles &&
      value
    );
  }, "Restore did not reset the castle, collision ownership, and rubble");
  assertPoseRetained(beforeReset, reset);
  assert.equal(reset.revision, report.authoredRevision);
  await targetingProbe(true);
  await pose(fixture.overview, "fly");
  await capture("restored-castle");
  record(
    "Restore clears the castle destruction and rubble without authored edits",
    summary(reset),
  );
  const a = await status(),
    begun = performance.now();
  await page.waitForTimeout(2000);
  const b = await frames(a.frame);
  report.performance = {
    elapsedMs: performance.now() - begun,
    frameBefore: a.frame,
    frameAfter: b.frame,
    submittedFps: ((b.frame - a.frame) * 1000) / (performance.now() - begun),
    reportedFps: b.fps,
    qualification:
      "Short browser observation, not GPU execution timing or performance qualification.",
  };
  await Promise.all(headerAudits);
  assert.deepEqual(report.errors, []);
  assert.deepEqual(
    report.console.filter(
      (item) =>
        item.type === "error" &&
        !(
          item.intentionalShaderFailure &&
          /WGSL|shader|compil|parse|unexpected token|validation/i.test(
            item.text,
          )
        ),
    ),
    [],
  );
  assert.deepEqual(report.failed, []);
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
  const reportPath = resolve(output, `castle-${mode}-browser.json`);
  await writeFile(reportPath, JSON.stringify(report, null, 2) + "\n");
  console.log(
    JSON.stringify({
      ok: report.ok,
      report: reportPath,
      checks: report.checks.length,
      error: report.error,
    }),
  );
  closing = true;
  await context.close();
  await browser.close();
}
