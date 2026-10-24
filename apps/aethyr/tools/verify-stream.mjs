/** Run through openagents browser run. Licensed captures and raw reports stay outside Git. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set AF_CAPTURE_DIR outside the checkout.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep captures and raw reports outside the checkout.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const world = process.env.AF_VERIFY_WORLD || "starter";
assert(["starter", "medieval-town"].includes(world));
const manifest =
  process.env.VITE_STREAM_PACKAGE_URL ||
  (world === "starter"
    ? "/starter/stream-package.json"
    : "/private-assets/stream-package.json");
const report = {
  schema: "aethyrforge.stream-browser.v1",
  startedAt: new Date().toISOString(),
  world,
  manifest,
  viewport: [800, 500],
  capture: { width: 512, height: 320, time: 0 },
  scope:
    "Sequential real-WebGPU baseline and streamed sessions. Warm means a same-context reload, not a guaranteed HTTP cache hit. Payload bytes use response content lengths; transferred body bytes use Playwright request sizes, including worker fetches. Resource accounting is not process memory. Captures use one fixed camera, exposure, environment, animation time, and final quality. No frame-rate improvement is inferred.",
  ok: false,
  checks: [],
  startups: [],
  network: [],
  receipts: [],
  errors: [],
  console: [],
};
let browser,
  context,
  page,
  servers = [],
  networkPhase = "idle";
let referenceCamera, referenceEnvironment, referencePixels;
const networkRequests = new WeakMap();
const networkPending = new Set();
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const horizontalDistance = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const status = async () => (await snapshot()).status;
const record = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
async function until(test, message, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await page.waitForTimeout(80);
  } while (Date.now() < deadline);
  throw new Error(`${message}; status=${JSON.stringify(await status())}`);
}
async function ready() {
  await page.waitForFunction(() => !!window.aethyrforge, null, {
    timeout: 30_000,
  });
  return until(
    async () => {
      const value = await status();
      assert.notEqual(value.phase, "error", value.message);
      return value.phase === "ready" && value.character && value;
    },
    "The world did not become controllable",
    180_000,
  );
}
async function receiptAfter(action, expected = true) {
  const before = new Set((await snapshot()).receipts.map((r) => r.requestId));
  const requestId = await action();
  const receipt = await until(
    async () =>
      (await snapshot()).receipts.find((item) =>
        typeof requestId === "string"
          ? item.requestId === requestId
          : !before.has(item.requestId),
      ),
    "No command receipt arrived",
    180_000,
  );
  report.receipts.push(receipt);
  if (expected !== null) assert.equal(receipt.ok, expected, receipt.message);
  return receipt;
}
async function input(forward = 0, right = 0) {
  await page.evaluate(
    ({ forward, right }) =>
      window.aethyrforgeEditor.service.send({
        type: "input",
        input: {
          forward,
          right,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          jump: false,
        },
      }),
    { forward, right },
  );
}
async function shaderSource() {
  return page.evaluate(async () => {
    const { editorAtoms } = await import("/packages/editor-services/state.ts");
    return window.aethyrforgeEditor.service.registry.get(
      editorAtoms.activeShader,
    );
  });
}
async function waitGrounded() {
  await until(async () => {
    const character = (await status()).character;
    return character?.grounded && Math.abs(character.velocity[1]) < 0.05;
  }, "The walking controller did not settle on its collision surface");
  const before = (await status()).character.position;
  await page.waitForTimeout(200);
  const after = (await status()).character;
  assert(
    after?.grounded && Math.abs(before[1] - after.position[1]) < 0.02,
    "The grounded player did not remain stable before the streaming check.",
  );
}
async function setCamera(pose, mode) {
  await receiptAfter(() =>
    page.evaluate(
      ({ pose, mode }) =>
        window.aethyrforge.cameraPose(
          pose.eye,
          pose.target,
          pose.fov ?? 1,
          mode,
        ),
      { pose, mode },
    ),
  );
  const before = (await status()).frame;
  await until(
    async () => (await status()).frame > before,
    "The camera did not render",
  );
  if (mode === "walk") await waitGrounded();
}
async function camera(mode) {
  await setCamera(referenceCamera, mode);
}
async function checkMovement(label) {
  await camera("walk");
  await page.waitForTimeout(300);
  const movement = [];
  for (const [forward, right] of [
    [1, 0],
    [0, 1],
    [-1, 0],
  ]) {
    const before = (await status()).character.position;
    await input(forward, right);
    try {
      await page.waitForTimeout(400);
    } finally {
      await input();
    }
    await page.waitForTimeout(250);
    const after = (await status()).character.position;
    movement.push({
      input: { forward, right },
      before,
      after,
      meters: horizontalDistance(before, after),
    });
    if (movement.at(-1).meters > 0.1) break;
  }
  assert(
    movement.some((trial) => trial.meters > 0.1),
    `${label}: character input did not move the player.`,
  );
  const stopped = (await status()).character.position;
  await page.waitForTimeout(300);
  assert(
    horizontalDistance(stopped, (await status()).character.position) < 0.1,
    "The character kept moving after input release.",
  );
  record(label, { movement });
  await camera("walk");
  await page.waitForTimeout(300);
}
async function collectQuestKey() {
  const marker = (await status()).game?.keyPosition;
  assert(marker, "The quest did not create a key marker.");
  const setups = [];
  for (const [dx, dz] of [
    [0, 1.2],
    [1.2, 0],
    [-1.2, 0],
    [0, -1.2],
  ]) {
    const eye = [marker[0] + dx, marker[1] + 1.62, marker[2] + dz];
    const moved = await receiptAfter(
      () =>
        page.evaluate(
          ({ eye, marker }) =>
            window.aethyrforge.cameraPose(
              eye,
              [marker[0], eye[1], marker[2]],
              1,
              "walk",
            ),
          { eye, marker },
        ),
      null,
    );
    setups.push({ eye, ok: moved.ok });
    if (!moved.ok) continue;
    const interacted = await receiptAfter(
      () => page.evaluate(() => window.aethyrforge.game("interact")),
      null,
    );
    if (!interacted.ok) continue;
    await until(
      async () => (await status()).game?.hasKey,
      "The key interaction did not update inventory",
    );
    record(
      "Camera-positioned key interaction creates progress to preserve across streaming",
      { setups },
    );
    return;
  }
  throw new Error(
    `No open camera-positioned interaction setup beside the key: ${JSON.stringify(setups)}`,
  );
}
async function capture(label) {
  const previous = (await status()).view;
  assert(previous, "No view checkpoint exists for capture restoration.");
  await camera("fly");
  await page.evaluate(
    (settings) => window.aethyrforge.environment(settings),
    referenceEnvironment,
  );
  await page.waitForTimeout(300);
  await page.evaluate((size) => window.aethyrforge.beginFilmCapture(size), {
    width: report.capture.width,
    height: report.capture.height,
  });
  try {
    const encoded = await page.evaluate(async (pose) => {
      const pixels = new Uint8Array(
        await window.aethyrforge.capturePixels({
          time: 0,
          camera: pose,
          exposure: 1,
        }),
      );
      let value = "";
      for (let i = 0; i < pixels.length; i += 8192)
        value += String.fromCharCode(...pixels.subarray(i, i + 8192));
      return btoa(value);
    }, referenceCamera);
    const pixels = Buffer.from(encoded, "base64");
    assert.equal(
      pixels.length,
      report.capture.width * report.capture.height * 4,
    );
    const image = `${world}-${label}-fixed.rgba`;
    await writeFile(resolve(output, image), pixels);
    report.captures ??= [];
    report.captures.push({ image, sha256: hash(pixels), bytes: pixels.length });
    return pixels;
  } finally {
    await page.evaluate(() => window.aethyrforge.endFilmCapture());
    await setCamera(previous, previous.mode);
  }
}
function assetRequest(url) {
  const path = new URL(url).pathname;
  return path.startsWith("/private-assets/") || path.startsWith("/starter/");
}
function networkSummary(phase) {
  const rows = report.network.filter(
    (row) => row.phase === phase && assetRequest(row.url),
  );
  return {
    requests: rows.length,
    declaredPayloadBytes: rows.reduce(
      (sum, row) => sum + (row.contentLength ?? 0),
      0,
    ),
    transferredBodyBytes: rows.reduce(
      (sum, row) => sum + (row.responseBodyBytes ?? 0),
      0,
    ),
    unavailableBodySizes: rows.filter((row) => row.responseBodyBytes === null)
      .length,
  };
}
async function settleNetwork() {
  while (networkPending.size) await Promise.allSettled([...networkPending]);
}
async function openServers(streamed) {
  const define = {
    "import.meta.env.VITE_WORLD_PROFILE": JSON.stringify(world),
    "import.meta.env.VITE_STREAM_PACKAGE_URL": JSON.stringify(
      streamed ? manifest : "",
    ),
  };
  const editor = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-stream-${process.pid}-${streamed ? "stream" : "baseline"}-editor`,
    server: { host: "127.0.0.1", port: 0, strictPort: false },
    define: { ...define },
  });
  servers.push(editor);
  await editor.listen();
  const origin = new URL(editor.resolvedUrls.local[0]).origin;
  const preview = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-stream-${process.pid}-${streamed ? "stream" : "baseline"}-preview`,
    server: { host: "127.0.0.1", port: 0, strictPort: false },
    define: {
      ...define,
      "import.meta.env.VITE_EDITOR_ORIGIN": JSON.stringify(origin),
    },
  });
  servers.push(preview);
  await preview.listen();
  editor.config.define["import.meta.env.VITE_PREVIEW_URL"] = JSON.stringify(
    new URL(preview.resolvedUrls.local[0]).origin + "/preview.html",
  );
  return origin;
}
async function closeSession() {
  if (page && !page.isClosed()) {
    await input().catch(() => {});
    await page
      .evaluate(() => window.aethyrforgeEditor?.service.close())
      .catch(() => {});
  }
  await context?.close();
  context = page = undefined;
  for (const server of servers.splice(0).reverse()) await server.close();
}
async function openSession(streamed) {
  const origin = await openServers(streamed);
  context = await browser.newContext({ viewport: { width: 800, height: 500 } });
  context.on("request", (request) =>
    networkRequests.set(request, networkPhase),
  );
  context.on("requestfinished", (request) => {
    const task = (async () => {
      const response = await request.response();
      if (!response) return;
      const length = Number(response.headers()["content-length"]);
      let sizes;
      try {
        sizes = await request.sizes();
      } catch {}
      report.network.push({
        phase: networkRequests.get(request) ?? "unknown",
        url: request.url(),
        status: response.status(),
        contentLength: Number.isFinite(length) ? length : null,
        responseBodyBytes: Number.isFinite(sizes?.responseBodySize)
          ? sizes.responseBodySize
          : null,
      });
    })();
    networkPending.add(task);
    task.then(
      () => networkPending.delete(task),
      (error) => {
        networkPending.delete(task);
        report.errors.push(`Asset measurement failed: ${String(error)}`);
      },
    );
  });
  page = await context.newPage();
  page.on("pageerror", (error) => report.errors.push(String(error)));
  page.on("console", (message) => {
    if (
      ["warning", "error"].includes(message.type()) &&
      report.console.length < 80
    )
      report.console.push({
        type: message.type(),
        text: message.text().slice(0, 2048),
      });
  });
  for (const temperature of ["cold", "warm"]) {
    networkPhase = `${streamed ? "stream" : "baseline"}-${temperature}`;
    const started = performance.now();
    if (temperature === "cold")
      await page.goto(origin, { waitUntil: "domcontentloaded" });
    else await page.reload({ waitUntil: "domcontentloaded" });
    const value = await ready();
    const wallToReadyMs = performance.now() - started;
    await settleNetwork();
    assert(
      value.runtimeMetrics,
      "The runtime did not expose startup measurement boundaries.",
    );
    const startup = {
      profile: streamed ? "stream" : "baseline",
      temperature,
      wallToReadyMs,
      runtimeMetrics: value.runtimeMetrics,
      streaming: value.streaming,
      network: networkSummary(networkPhase),
    };
    report.startups.push(startup);
    assert(
      startup.network.requests > 0,
      "No worker asset requests were measured.",
    );
    if (streamed) assert.equal(value.streaming?.activeCell, "entry");
    else assert.equal(value.streaming, undefined);
    if (!referenceCamera) {
      await waitGrounded();
      const settled = await status();
      assert(settled.view, "No initial view was reported.");
      referenceCamera = {
        eye: settled.view.eye,
        target: settled.view.target,
        up: [0, 1, 0],
        fov: 1,
        vignette: null,
      };
      referenceEnvironment = (await snapshot()).environment;
    }
    record(
      `${startup.profile} ${temperature} startup reaches real control`,
      startup,
    );
  }
}
async function loadCell(cell) {
  networkPhase = `stream-load-${cell}`;
  await receiptAfter(() =>
    page.evaluate(
      (cell) =>
        window.aethyrforgeEditor.service.send(
          {
            type: "stream",
            action: "load",
            cell,
          },
          true,
        ),
      cell,
    ),
  );
  await until(
    async () => {
      const value = (await status()).streaming;
      return value?.activeCell === cell && !value.loading && value;
    },
    `The ${cell} cell did not activate`,
    180_000,
  );
  await settleNetwork();
}
async function continuity() {
  const value = await status();
  return {
    revision: value.revision,
    game: value.game,
    player: value.character.position,
    canUndo: value.canUndo,
    canRedo: value.canRedo,
    shaderSha256: hash(await shaderSource()),
    skyIntensity: (await snapshot()).environment.skyIntensity,
    gameplay: gameplayContinuity(value.gameplay),
  };
}
function gameplayContinuity(gameplay) {
  if (!gameplay) return null;
  const rendered = gameplay.rendered;
  return {
    enabled: gameplay.enabled,
    selected: gameplay.selected,
    destroyed: gameplay.destroyed,
    casts: gameplay.casts,
    impacts: gameplay.impacts,
    rendered: rendered
      ? {
          maskedNodes: rendered.maskedNodes,
          fracturedSources: rendered.fracturedSources,
          standingTriangles: rendered.standingTriangles,
          geometryBytes: rendered.geometryBytes,
        }
      : null,
  };
}
async function settleDestruction() {
  let previous,
    previousFrame = -1,
    stableFrames = 0;
  return until(
    async () => {
      const value = await status();
      const gameplay = value.gameplay;
      assert(
        gameplay?.enabled,
        "The destructible town lost its gameplay summary.",
      );
      if (value.frame === previousFrame) return false;
      previousFrame = value.frame;
      const current = JSON.stringify(gameplayContinuity(gameplay));
      stableFrames =
        !gameplay.casting && gameplay.effects === 0 && gameplay.debris === 0
          ? current === previous
            ? stableFrames + 1
            : 1
          : 0;
      previous = current;
      return stableFrames >= 3 && value;
    },
    "Destruction did not settle before cell activation",
    40_000,
  );
}
async function damageTown() {
  assert(
    process.env.AF_DRUID_FIXTURES,
    "Set AF_DRUID_FIXTURES to source-bound private probes when Druid gameplay is enabled.",
  );
  const fixturePath = await realpath(resolve(process.env.AF_DRUID_FIXTURES));
  assert(
    fixturePath !== root && !fixturePath.startsWith(root + sep),
    "Keep licensed destruction probes outside the checkout.",
  );
  const fixtureBytes = await readFile(fixturePath);
  const fixtures = JSON.parse(fixtureBytes);
  assert.equal(fixtures.schema, "aethyrforge.druid-browser-fixtures.v1");
  const probe = fixtures.abilities?.lightning_bolt;
  assert(probe, "The source-bound fixtures need a lightning_bolt probe.");
  for (const vector of [probe.eye, probe.target])
    assert(
      Array.isArray(vector) &&
        vector.length === 3 &&
        vector.every(Number.isFinite),
      "Choose finite, source-bound camera vectors for destruction verification.",
    );
  assert(
    Number.isSafeInteger(probe.nodeId),
    "The lightning probe needs its source node ID.",
  );
  const previewFrame = page
    .frames()
    .find((frame) => new URL(frame.url()).pathname === "/preview.html");
  assert(previewFrame, "The live isolated preview frame is unavailable.");
  const sourceBindings = await previewFrame.evaluate(async (url) => {
    const response = await fetch(url);
    if (!response.ok)
      throw Error("The active streaming package could not be inspected.");
    const manifest = await response.json();
    const sceneURL = manifest.cells.find((cell) => cell.id === "town")?.scenes
      .day;
    if (!sceneURL) throw Error("The package has no full-quality town scene.");
    const sceneResponse = await fetch(sceneURL);
    if (!sceneResponse.ok)
      throw Error("The full-quality scene could not be inspected.");
    const scene = await sceneResponse.json();
    return { gameplayURL: scene.gameplayURL, collisionURL: scene.collisionURL };
  }, manifest);
  assert.deepEqual(
    sourceBindings,
    fixtures.scene,
    "The destruction probes belong to a different collision or gameplay source.",
  );
  await setCamera(probe, "fly");
  await receiptAfter(() =>
    page.evaluate(() =>
      window.aethyrforgeEditor.service.send(
        {
          type: "selectAbility",
          ability: "lightning_bolt",
        },
        true,
      ),
    ),
  );
  const aimed = await until(async () => {
    const value = await status();
    return (
      value.gameplay?.selected === "lightning_bolt" &&
      value.gameplay.target &&
      value
    );
  }, "The bound lightning probe did not aim at a destructible surface");
  assert.equal(
    aimed.gameplay.target.nodeId,
    probe.nodeId,
    "The lightning probe no longer aims at its source-bound wall cell.",
  );
  const before = gameplayContinuity(aimed.gameplay);
  const limit = probe.maxCasts ?? 4;
  assert(
    Number.isSafeInteger(limit) && limit > 0 && limit <= 4,
    "Limit streaming destruction verification to at most four casts.",
  );
  let damaged;
  for (let cast = 0; cast < limit; cast++) {
    await receiptAfter(() =>
      page.evaluate(() =>
        window.aethyrforgeEditor.service.send(
          {
            type: "castAbility",
          },
          true,
        ),
      ),
    );
    damaged = await settleDestruction();
    if (damaged.gameplay.destroyed > before.destroyed) break;
  }
  assert(
    damaged.gameplay.destroyed > before.destroyed,
    "The supported lightning action did not destroy real source cells.",
  );
  assert(
    damaged.gameplay.rendered.maskedNodes > 0 &&
      damaged.gameplay.rendered.fracturedSources > 0,
    "Destroyed cells did not reach the renderer's fracture representation.",
  );
  await camera("walk");
  await settleDestruction();
  const continuityBefore = await continuity();
  report.destructionFixture = {
    sha256: hash(fixtureBytes),
    scene: sourceBindings,
    sourceId: fixtures.sourceId,
  };
  record("A source-bound lightning action damages the town before streaming", {
    before,
    after: continuityBefore.gameplay,
    settling:
      "No active cast, effects, or debris; stable bounded damage/render summaries across three distinct frames. Simulation ticks continue.",
  });
  return continuityBefore;
}
function sameContinuity(before, after) {
  const { player: first, ...a } = before,
    { player: second, ...b } = after;
  assert.deepEqual(a, b);
  assert(
    horizontalDistance(first, second) < 0.02,
    "Streaming moved the player horizontally.",
  );
  assert(
    Math.abs(first[1] - second[1]) < 0.05,
    "Streaming moved the grounded player vertically.",
  );
}

try {
  browser = await chromium.connectOverCDP(endpoint);
  report.browser = await browser.version();
  await openSession(false);
  referencePixels = await capture("baseline");
  await closeSession();
  await openSession(true);
  const first = (await status()).streaming;
  assert(first && !first.loading);
  assert(
    await page
      .getByRole("button", { name: "Complete town", exact: true })
      .isVisible(),
  );
  await checkMovement(
    "Entry collision and controller accept movement before full-town loading",
  );
  const beforeFull = await continuity();
  await loadCell("town");
  sameContinuity(beforeFull, await continuity());
  record(
    "Full-cell activation keeps the same player and authored/gameplay state",
  );
  const streamedPixels = await capture("stream-full");
  assert.equal(
    hash(streamedPixels),
    hash(referencePixels),
    "The final-quality streamed image differs from the full baseline.",
  );
  record("Full-quality pixels exactly match the nonstreamed scene", {
    sha256: hash(streamedPixels),
    camera: referenceCamera,
    environment: referenceEnvironment,
  });
  referenceEnvironment = { ...referenceEnvironment, skyIntensity: 0.83 };
  await page.evaluate(
    (settings) => window.aethyrforge.environment(settings),
    referenceEnvironment,
  );
  await until(
    async () => (await snapshot()).environment.skyIntensity === 0.83,
    "The explicit sky intensity did not reach editor state",
  );
  await page.waitForTimeout(300);
  record(
    "An explicit sky intensity override is applied before cell transitions",
    {
      skyIntensity: referenceEnvironment.skyIntensity,
    },
  );
  const coldBaseline = report.startups.find(
    (row) => row.profile === "baseline" && row.temperature === "cold",
  );
  const coldStream = report.startups.find(
    (row) => row.profile === "stream" && row.temperature === "cold",
  );
  assert(
    coldStream.network.declaredPayloadBytes <
      coldBaseline.network.declaredPayloadBytes,
    "The entry package did not reduce the asset payload needed for first control.",
  );
  record(
    "Entry first control requests fewer asset bytes at the same eventual final quality",
    {
      baselineBytes: coldBaseline.network.declaredPayloadBytes,
      entryBytes: coldStream.network.declaredPayloadBytes,
    },
  );
  await checkMovement(
    "The real character controller accepts movement after activation",
  );
  await receiptAfter(() =>
    page.evaluate(() => window.aethyrforge.game("start")),
  );
  await collectQuestKey();
  const originalShaderPixels = await capture("quest-original-shader");
  const working = await shaderSource();
  const shaderReturn = "return vec4(max(color,vec3(0.0)),alpha);";
  assert(
    working.includes(shaderReturn),
    "Update the verification shader patch for this renderer.",
  );
  const changedShader = working.replace(
    shaderReturn,
    "return vec4(max(color,vec3(0.0))*vec3(0.35,0.7,1.0),alpha);",
  );
  await receiptAfter(() =>
    page.evaluate((source) => window.aethyrforge.shader(source), changedShader),
  );
  const workingShaderPixels = await capture("quest-working-shader");
  assert.notEqual(
    hash(workingShaderPixels),
    hash(originalShaderPixels),
    "The valid shader edit did not visibly change the image.",
  );
  const beforeBadShader = await continuity();
  await receiptAfter(
    () =>
      page.evaluate(() => window.aethyrforge.shader("this is not valid WGSL")),
    false,
  );
  sameContinuity(beforeBadShader, await continuity());
  const rejectedShaderPixels = await capture("quest-rejected-shader");
  assert.equal(
    hash(rejectedShaderPixels),
    hash(workingShaderPixels),
    "The rejected edit changed the last working image.",
  );
  record(
    "A rejected shader retains the last working source and player/game state",
  );
  const beforeEntry = await continuity();
  await loadCell("entry");
  sameContinuity(beforeEntry, await continuity());
  record(
    "Returning to the entry cell unloads the full scene and retains live state",
    {
      streaming: (await status()).streaming,
    },
  );
  await checkMovement(
    "The entry controller remains usable after unloading the full scene",
  );
  const beforeRevisit = await continuity();
  await loadCell("town");
  sameContinuity(beforeRevisit, await continuity());
  record(
    "Revisiting the full town retains the edited shader, quest, and player",
    {
      streaming: (await status()).streaming,
    },
  );
  const revisitedShaderPixels = await capture("quest-revisited-shader");
  assert.equal(
    hash(revisitedShaderPixels),
    hash(workingShaderPixels),
    "Returning to the full town lost the visibly edited working shader.",
  );
  record(
    "A visibly edited shader survives a rejected edit and entry/full revisit",
    {
      originalSha256: hash(originalShaderPixels),
      workingSha256: hash(workingShaderPixels),
      rejectedSha256: hash(rejectedShaderPixels),
      revisitedSha256: hash(revisitedShaderPixels),
    },
  );
  assert.equal((await snapshot()).environment.skyIntensity, 0.83);
  record("Entry/full transitions preserve the explicit sky intensity override", {
    skyIntensity: (await snapshot()).environment.skyIntensity,
  });
  if ((await status()).gameplay?.enabled) {
    const beforeDamageUnload = await damageTown();
    await loadCell("entry");
    sameContinuity(beforeDamageUnload, await continuity());
    record(
      "Returning to entry preserves real destruction, spell selection, quest, player, and edited shader",
      {
        gameplay: (await continuity()).gameplay,
      },
    );
    const beforeDamageRevisit = await continuity();
    await loadCell("town");
    sameContinuity(beforeDamageRevisit, await continuity());
    record(
      "The full-quality revisit preserves damaged source geometry and live gameplay",
      {
        gameplay: (await continuity()).gameplay,
      },
    );
  } else {
    record("This fixture has no Druid destruction capability", {
      skipped: true,
    });
  }
  const frame = (await status()).frame;
  await until(
    async () => (await status()).frame > frame,
    "The scene stopped rendering after revisiting",
  );
  await page.screenshot({ path: resolve(output, `${world}-stream-final.png`) });
  assert.deepEqual(
    report.errors,
    [],
    "The browser reported an uncaught page error.",
  );
  report.ok = true;
} catch (error) {
  report.failure = String(error);
  throw error;
} finally {
  try {
    await closeSession();
  } catch (error) {
    (report.cleanupErrors ??= []).push(String(error));
    report.ok = false;
  }
  try {
    await browser?.close();
  } catch (error) {
    (report.cleanupErrors ??= []).push(String(error));
    report.ok = false;
  }
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, `${world}-stream-browser.json`),
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(
    JSON.stringify({
      ok: report.ok,
      world,
      checks: report.checks.length,
      failure: report.failure,
      report: resolve(output, `${world}-stream-browser.json`),
    }),
  );
}
