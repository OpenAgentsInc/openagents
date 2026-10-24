/** Run through openagents browser run; licensed captures stay outside Git. */
import assert from "node:assert/strict";
import { mkdir, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const checkout = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set a private AF_CAPTURE_DIR.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== checkout && !output.startsWith(checkout + sep),
  "Keep licensed captures outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const url = new URL(process.env.AF_EDITOR_URL || "http://127.0.0.1:55316/");
const minimumFps = Number(process.env.AF_MIN_FPS || 54);
assert(Number.isFinite(minimumFps) && minimumFps > 0 && minimumFps <= 240);
assert(
  ["http:", "https:"].includes(url.protocol) &&
    ["127.0.0.1", "localhost"].includes(url.hostname) &&
    !url.username &&
    !url.password,
  "Verify a local Open Ground server.",
);
const report = {
  schema: "aethyrforge.open-ground-browser.v1",
  startedAt: new Date().toISOString(),
  url: url.href,
  viewport: [1280, 720],
  minimumSubmissionFps: minimumFps,
  ok: false,
  checks: [],
  receipts: [],
  captures: [],
  scenes: [],
  grassSettings: [],
  errors: [],
  console: [],
  failed: [],
};
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
  serviceWorkers: "block",
});
await context.addInitScript(() => {
  window.__groundReady = [];
  window.__groundInput = [];
  window.__groundGraphics = [];
  for (const type of ["blur", "focus", "keydown", "keyup", "pointerlockchange"])
    (type === "pointerlockchange" ? document : window).addEventListener(
      type,
      (event) => {
        if (window.__groundInput.length >= 128) return;
        window.__groundInput.push({
          type,
          code: event.code,
          at: performance.now(),
          locked: !!document.pointerLockElement,
          focused: document.hasFocus(),
        });
      },
    );
  window.addEventListener("message", (event) => {
    const frame = document.getElementById("preview-frame");
    if (
      !frame?.contentWindow ||
      event.source !== frame.contentWindow ||
      event.origin !== new URL(frame.src).origin
    )
      return;
    if (
      event.data?.protocol === "aethyrforge.preview.v1" &&
      event.data.type === "ready"
    )
      window.__groundReady.push({
        hasSourceSky: event.data.hasSourceSky,
        status: event.data.status?.phase,
      });
    const message = event.data?.status?.message || event.data?.message;
    if (
      event.data?.protocol === "aethyrforge.preview.v1" &&
      typeof message === "string" &&
      /^Graphics(?::| device stopped:)/.test(message) &&
      window.__groundGraphics.length < 16
    )
      window.__groundGraphics.push(message.slice(0, 4096));
  });
});
const page = await context.newPage();
page.setDefaultTimeout(30_000);
let closing = false;
const bodies = [];
page.on("pageerror", (error) => report.errors.push(String(error)));
page.on("console", (message) => {
  if (["error", "warning"].includes(message.type()))
    report.console.push({
      type: message.type(),
      text: message.text().slice(0, 2048),
    });
});
page.on("requestfailed", (request) => {
  if (!closing)
    report.failed.push({
      url: request.url(),
      error: request.failure()?.errorText,
    });
});
page.on("response", (response) => {
  if (response.status() >= 400)
    report.failed.push({ url: response.url(), status: response.status() });
  if (
    /\/private-assets\/scene-ground(?:-night)?\.json$/.test(
      new URL(response.url()).pathname,
    )
  ) {
    bodies.push(
      response.json().then((scene) => {
        report.scenes.push({
          url: response.url(),
          instances: scene.instances?.length,
          meshNames: scene.meshes?.map((mesh) => mesh.name),
          materials: scene.materials?.length,
          groundMaterial: scene.materials?.[0] && {
            name: scene.materials[0].name,
            baseColorFactor: scene.materials[0].baseColorFactor,
            baseColorUrl: scene.materials[0].baseColorUrl,
            normalUrl: scene.materials[0].normalUrl,
            normalConvention: scene.materials[0].normalConvention,
            metallic: scene.materials[0].metallic,
            roughness: scene.materials[0].roughness,
          },
          ground: scene.source?.groundZone,
          characterURL: scene.characterURL,
          environmentURL: scene.environmentURL,
          collisionURL: scene.collisionURL,
          grassURL: scene.grassURL,
          gameplayURL: scene.gameplayURL,
          creaturesURL: scene.creaturesURL,
          particlesURL: scene.particlesURL,
          waterURL: scene.waterURL,
          leavesURL: scene.leavesURL,
        });
      }),
    );
  }
  if (/\/grass-[a-f0-9]{64}\.json$/.test(new URL(response.url()).pathname))
    bodies.push(
      response.json().then((settings) => {
        report.grassSettings.push({ url: response.url(), ...settings });
      }),
    );
});
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const status = async () => (await snapshot()).status;
const feet = (value) => value.character.position;
const horizontal = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
const distance = (a, b) => Math.hypot(...a.map((n, i) => n - b[i]));
const record = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
async function until(test, message, timeout = 20_000) {
  const deadline = Date.now() + timeout;
  do {
    const result = await test();
    if (result) return result;
    await page.waitForTimeout(50);
  } while (Date.now() < deadline);
  throw Error(`${message}; status=${JSON.stringify(await status())}`);
}
async function waitFrames(frame, count = 2) {
  return until(async () => {
    const value = await status();
    assert.notEqual(value.phase, "error", value.message);
    return value.frame >= frame + count && value;
  }, "The world stopped rendering");
}
async function receiptAfter(action, ok) {
  const before = new Set(
    (await snapshot()).receipts.map((item) => item.requestId),
  );
  const id = await action();
  const receipt = await until(
    async () =>
      (await snapshot()).receipts.find((item) =>
        typeof id === "string"
          ? item.requestId === id
          : !before.has(item.requestId),
      ),
    "No command receipt arrived",
  );
  report.receipts.push(receipt);
  assert.equal(receipt.ok, ok, receipt.message);
  return receipt;
}
async function capture(name) {
  const path = resolve(output, `${name}.png`);
  const pixels = await page.screenshot({ path });
  report.captures.push(path);
  return pixels;
}
async function releaseInput() {
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
  await page.evaluate(() => {
    const frame = document.getElementById("preview-frame");
    if (!frame?.contentWindow) return;
    const origin = new URL(frame.src);
    const sessionId = new URLSearchParams(origin.hash.slice(1)).get("session");
    if (sessionId)
      frame.contentWindow.postMessage(
        {
          protocol: "aethyrforge.preview.v1",
          sessionId,
          requestId: crypto.randomUUID(),
          type: "input",
          input: {
            forward: 0,
            right: 0,
            up: 0,
            lookX: 0,
            lookY: 0,
            sprint: false,
            jump: false,
          },
        },
        origin.origin,
      );
  });
  await page.keyboard.press("Escape");
  const child = await (
    await page.locator("#preview-frame").elementHandle()
  )?.contentFrame();
  await child?.evaluate(() => document.exitPointerLock());
  await child?.waitForFunction(() => !document.pointerLockElement);
}
async function focusWorld() {
  const frame = await (
    await page.locator("#preview-frame").elementHandle()
  ).contentFrame();
  await frame.locator("#world").click({ position: { x: 640, y: 300 } });
  await frame.waitForFunction(
    () =>
      document.hasFocus() &&
      document.pointerLockElement === document.getElementById("world"),
  );
  // A previous Escape can finish releasing pointer lock after the next click.
  await frame.evaluate(
    () =>
      new Promise((done) =>
        requestAnimationFrame(() => requestAnimationFrame(done)),
      ),
  );
}
function noGameplay(value) {
  assert.notEqual(
    value.gameplay?.enabled,
    true,
    "Open Ground enabled spell gameplay",
  );
  assert.equal(value.game, undefined, "Open Ground installed a quest");
  assert.equal(
    value.instances,
    1,
    "The zone must contain only its source ground instance",
  );
}
async function move(sprint) {
  await releaseInput();
  const before = await waitFrames((await status()).frame);
  assert(before.character.grounded);
  await focusWorld();
  const started = Date.now();
  if (sprint) await page.keyboard.down("ShiftLeft");
  await page.keyboard.down("KeyW");
  try {
    await page.waitForTimeout(1000);
  } finally {
    await page.keyboard.up("KeyW");
    await page.keyboard.up("ShiftLeft");
  }
  const after = await waitFrames((await status()).frame);
  await releaseInput();
  assert(after.character.grounded, "The player lost the ground while moving");
  const travelled = horizontal(feet(before), feet(after));
  const details = {
    sprint,
    elapsedMs: Date.now() - started,
    before: feet(before),
    after: feet(after),
    distance: travelled,
    animation: after.character.animation,
  };
  (report.movement ||= []).push(details);
  assert(travelled > 1, `Real W input did not move the player: ${travelled}`);
  noGameplay(after);
  return details;
}

function grassStats(value) {
  const stats = value.groundGrass;
  assert(stats, "The world did not publish its submitted grass pass");
  for (const key of [
    "nearPatches",
    "farPatches",
    "nearBlades",
    "farBlades",
    "blades",
    "triangles",
    "drawCalls",
    "droppedPatches",
    "accountedBytes",
  ])
    assert(
      Number.isSafeInteger(stats[key]) && stats[key] >= 0,
      `Invalid grass ${key}: ${stats[key]}`,
    );
  assert(Number.isFinite(stats.windTime) && stats.windTime >= 0);
  assert(
    stats.nearPatches > 0 && stats.farPatches > 0,
    "Both near and far grass detail must be submitted",
  );
  assert(stats.nearBlades > 0 && stats.farBlades > 0);
  assert.equal(stats.blades, stats.nearBlades + stats.farBlades);
  assert(
    stats.drawCalls > 0 &&
      stats.drawCalls <= stats.nearPatches + stats.farPatches,
  );
  assert(stats.triangles >= stats.blades * 2 && stats.accountedBytes > 0);
  return stats;
}

const percentile = (values, fraction) => {
  const sorted = [...values].sort((a, b) => a - b);
  return (
    sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * fraction))] ??
    null
  );
};
const submittedFrames = (value) =>
  value.runtimeMetrics?.submittedFrames ?? value.frame;
function headingAngle(a, b) {
  const direction = (value) => {
    const vector = value.view.target.map((v, i) => v - value.view.eye[i]);
    const length = Math.hypot(...vector);
    return vector.map((v) => v / length);
  };
  const x = direction(a),
    y = direction(b);
  return Math.acos(
    Math.max(
      -1,
      Math.min(
        1,
        x.reduce((sum, v, i) => sum + v * y[i], 0),
      ),
    ),
  );
}

async function frameSample(durationMs, moving) {
  await releaseInput();
  if (moving) await focusWorld();
  await page.evaluate(() => {
    const sample = { active: true, last: null, intervals: [] };
    window.__groundCadence = sample;
    const frame = (now) => {
      if (!sample.active) return;
      if (sample.last !== null && sample.intervals.length < 2048)
        sample.intervals.push(now - sample.last);
      sample.last = now;
      requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);
  });
  const before = await status(),
    samples = [],
    startedAt = performance.now();
  if (moving) await page.keyboard.down("KeyW");
  let after, uiIntervals, endedAt;
  try {
    for (let i = 0; i < durationMs / 250; i++) {
      if (moving)
        await page.mouse.move(
          640 + Math.round(120 * Math.sin((i * Math.PI) / 12)),
          300 + Math.round(30 * Math.sin((i * Math.PI) / 17)),
        );
      await page.waitForTimeout(250);
      const value = await status();
      assert.equal(value.phase, "ready", value.message);
      noGameplay(value);
      const grass = grassStats(value);
      samples.push({
        atMs: performance.now() - startedAt,
        frame: value.frame,
        submittedFrames: submittedFrames(value),
        fps: value.fps,
        position: feet(value),
        camera: value.camera,
        grounded: value.character.grounded,
        headingChangeRadians: headingAngle(before, value),
        grass,
      });
    }
    after = await status();
    endedAt = performance.now();
  } finally {
    await page.keyboard.up("KeyW");
    uiIntervals = await page.evaluate(() => {
      const sample = window.__groundCadence;
      sample.active = false;
      return sample.intervals;
    });
    await releaseInput();
  }
  const seconds = (endedAt - startedAt) / 1000;
  const frames = submittedFrames(after) - submittedFrames(before);
  const result = {
    moving,
    seconds,
    submittedFrames: frames,
    submissionFps: frames / seconds,
    hudFpsMedian: percentile(
      samples.map((sample) => sample.fps),
      0.5,
    ),
    hudFpsMinimum: Math.min(...samples.map((sample) => sample.fps)),
    uiRafP95Ms: percentile(uiIntervals, 0.95),
    uiRafOver50Ms: uiIntervals.filter((interval) => interval > 50).length,
    positionDistance: horizontal(feet(before), feet(after)),
    maxHeadingChangeRadians: Math.max(
      ...samples.map((sample) => sample.headingChangeRadians),
    ),
    samples,
  };
  (report.frameSamples ||= []).push(result);
  assert(
    result.submissionFps >= minimumFps,
    `Grass frame rate fell below ${minimumFps}: ${result.submissionFps.toFixed(2)}`,
  );
  if (moving) {
    assert(
      result.positionDistance > 20,
      `Sustained W movement stopped: ${result.positionDistance}`,
    );
    assert(
      result.maxHeadingChangeRadians > 0.1,
      "Pointer-lock mouse look did not rotate the view",
    );
    assert(
      samples.every((sample) => sample.grounded),
      "Walking through grass lost the plane",
    );
  }
  return result;
}

async function windMotion() {
  await releaseInput();
  await waitFrames((await status()).frame, 30);
  const before = await status();
  const first = await capture("ground-grass-wind-a");
  await page.waitForTimeout(850);
  const second = await capture("ground-grass-wind-b");
  const after = await status();
  const beforeGrass = grassStats(before),
    afterGrass = grassStats(after);
  assert(
    distance(feet(before), feet(after)) < 0.001,
    "Wind proof moved the player",
  );
  assert(
    distance(before.camera, after.camera) < 0.001,
    "Wind proof moved the camera",
  );
  assert(
    afterGrass.windTime > beforeGrass.windTime + 0.3,
    "The submitted wind clock stopped",
  );
  const regions = await page.evaluate(
    async ({ first, second }) => {
      const decode = async (bytes) =>
        createImageBitmap(
          new Blob(
            [
              Uint8Array.from(atob(bytes), (character) =>
                character.charCodeAt(0),
              ),
            ],
            { type: "image/png" },
          ),
        );
      const a = await decode(first),
        b = await decode(second);
      // Foreground to either side of the Druid, above the hotbar, excludes sky and idle animation.
      const regions = [
        { x: 80, y: 430, width: 360, height: 180 },
        { x: 840, y: 430, width: 360, height: 180 },
      ];
      const results = regions.map((region) => {
        const canvas = new OffscreenCanvas(region.width, region.height),
          context = canvas.getContext("2d");
        context.drawImage(
          a,
          region.x,
          region.y,
          region.width,
          region.height,
          0,
          0,
          region.width,
          region.height,
        );
        const pixelsA = context.getImageData(
          0,
          0,
          region.width,
          region.height,
        ).data;
        context.clearRect(0, 0, region.width, region.height);
        context.drawImage(
          b,
          region.x,
          region.y,
          region.width,
          region.height,
          0,
          0,
          region.width,
          region.height,
        );
        const pixelsB = context.getImageData(
          0,
          0,
          region.width,
          region.height,
        ).data;
        let changed = 0,
          absolute = 0;
        for (let i = 0; i < pixelsA.length; i += 4) {
          const changes = [0, 1, 2].map((channel) =>
            Math.abs(pixelsA[i + channel] - pixelsB[i + channel]),
          );
          if (Math.max(...changes) >= 12) changed++;
          absolute += changes.reduce((sum, value) => sum + value, 0);
        }
        const pixels = region.width * region.height;
        return {
          ...region,
          pixels,
          changedPixels: changed,
          changedRatio: changed / pixels,
          meanAbsoluteChannelChange: absolute / (pixels * 3),
        };
      });
      a.close();
      b.close();
      return results;
    },
    { first: first.toString("base64"), second: second.toString("base64") },
  );
  const result = {
    beforeGrass,
    afterGrass,
    camera: before.camera,
    player: feet(before),
    regions,
  };
  report.windMotion = result;
  assert(
    regions.every(
      (region) =>
        region.changedRatio > 0.01 && region.meanAbsoluteChannelChange > 0.3,
    ),
    `Submitted grass does not visibly sway: ${JSON.stringify(regions)}`,
  );
  return result;
}

try {
  report.browser = await browser.version();
  await page.goto(url.href, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => !!window.aethyrforge, null, {
    timeout: 30_000,
  });
  const initial = await until(
    async () => {
      const value = await status();
      assert.notEqual(value.phase, "error", value.message);
      return (
        value.phase === "ready" &&
        value.frame > 2 &&
        value.character?.modelLoaded === true &&
        value.character.grounded &&
        value
      );
    },
    "The source ground and Druid did not become ready",
    180_000,
  );
  assert.equal(initial.view.mode, "walk");
  assert.equal(initial.character.perspective, "third");
  assert(
    horizontal(initial.camera, feet(initial)) > 0.2,
    "The third-person camera must sit outside the Druid",
  );
  noGameplay(initial);
  await Promise.all(bodies);
  const scene = report.scenes.find((item) =>
    new URL(item.url).pathname.endsWith("/scene-ground.json"),
  );
  assert(scene, "The browser did not load the standalone ground manifest");
  assert.equal(scene.instances, 1);
  assert.equal(scene.meshNames.length, 1);
  assert.equal(scene.materials, 1);
  assert.equal(scene.ground?.profile, "ground-sky-player-v1");
  assert.equal(scene.ground.layout, "plane");
  assert.equal(scene.ground.meshName, "OpenGroundPlane");
  assert.equal(scene.ground.meshName, scene.meshNames[0]);
  assert.equal(scene.ground.sourceMeshName, "SM_ground_01");
  assert.equal(scene.ground.materialOrigin, "reference");
  assert.equal(scene.ground.materialName, "Open ground green");
  assert.equal(scene.ground.sourceGeometryRetained, false);
  assert.equal(scene.ground.originalMaterialMapsRetained, false);
  assert.equal(scene.ground.sizeMetres, 2048);
  assert.equal(
    scene.ground.reference.repository,
    "https://github.com/simondevyoutube/Quick_Grass",
  );
  assert.equal(
    scene.ground.reference.revision,
    "6b56164272f213e353a4045d1439d071d5a968cd",
  );
  assert.equal(
    scene.ground.reference.terrainShader,
    "public/shaders/terrain-lighting-model-vsh.glsl",
  );
  assert.deepEqual(
    scene.ground.reference.terrainColorLinear,
    [0.0125, 0.05, 0.0025],
  );
  assert.equal(scene.ground.reference.license, "MIT");
  assert(scene.ground.reference.licenseText.includes("simondevyoutube"));
  assert.equal(scene.groundMaterial.name, scene.ground.materialName);
  assert.deepEqual(scene.groundMaterial.baseColorFactor, [1, 1, 1, 1]);
  assert.match(scene.groundMaterial.baseColorUrl, /[a-f0-9]{64}\.png$/);
  assert.equal(scene.groundMaterial.normalUrl, undefined);
  assert.equal(scene.groundMaterial.metallic, 0);
  assert.equal(scene.groundMaterial.roughness, 1);
  assert(/^[a-f0-9]{64}$/.test(scene.ground.sourceSceneSha256));
  assert.equal(initial.triangles, 2);
  assert(Math.abs(feet(initial)[1]) < 0.05, "The plane floor must be at zero");
  assert(scene.characterURL && scene.collisionURL && scene.environmentURL);
  assert(scene.grassURL, "The scene did not load its grass settings");
  const settings = report.grassSettings.find(
    (item) => item.url === new URL(scene.grassURL, scene.url).href,
  );
  assert(settings, "The browser did not fetch the source-bound grass settings");
  assert.equal(settings.schema, "aethyrforge.grass.v1");
  assert.equal(settings.profile, "quick-grass-v1");
  assert.equal(settings.enabled, true);
  assert.equal(settings.groundHeight, 0);
  assert.equal(settings.bladeWidth, 0.05);
  assert.equal(settings.bladeHeight, 0.75);
  assert.equal(settings.reference.originalBladeHeight, 1.5);
  assert.equal(settings.lodDistance, 15);
  assert.equal(settings.maxDistance, 100);
  assert.equal(settings.interactionRadius, 2.5);
  assert.equal(settings.windStrength, 1);
  const submittedGrass = grassStats(initial);
  for (const field of [
    "gameplayURL",
    "creaturesURL",
    "particlesURL",
    "waterURL",
    "leavesURL",
  ])
    assert(!scene[field], `The ground-only scene contains ${field}`);
  const sourceSky = await page.evaluate(() => window.__groundReady);
  assert(
    sourceSky.some(
      (item) => item.status === "ready" && item.hasSourceSky === true,
    ),
    "The renderer did not report a loaded source sky",
  );
  report.sourceSky = sourceSky;
  record(
    "Continuous terrain, source sky, Druid and both grass detail levels are submitted",
    { scene, character: initial.character, grass: submittedGrass, settings },
  );
  const ui = await page.evaluate(() => ({
    title: document.title,
    buttons: Array.from(document.querySelectorAll("button"))
      .filter((element) => element.getClientRects().length)
      .map(
        (element) =>
          element.getAttribute("aria-label") || element.textContent?.trim(),
      ),
    editors: Array.from(
      document.querySelectorAll(
        "textarea,input[type=text],[contenteditable=true]",
      ),
    ).filter((element) => element.getClientRects().length).length,
    frame: (() => {
      const r = document
        .getElementById("preview-frame")
        .getBoundingClientRect();
      return { x: r.x, y: r.y, width: r.width, height: r.height };
    })(),
    viewport: [innerWidth, innerHeight],
    fps: (() => {
      const element = document.querySelector(
        '[aria-label="Frames per second"]',
      );
      if (!element) return null;
      const r = element.getBoundingClientRect();
      return { text: element.textContent, x: r.x, y: r.y, width: r.width };
    })(),
  }));
  assert(/Open Ground/.test(ui.title));
  assert(
    ui.fps && /\d+ FPS/.test(ui.fps.text),
    "The live FPS counter is missing",
  );
  assert(
    ui.fps.x > ui.viewport[0] / 2 && ui.fps.y < 100,
    "FPS must sit at the top right",
  );
  assert.equal(ui.editors, 0, "The walking zone exposed an editor");
  assert(
    ui.buttons.length > 0 &&
      ui.buttons.every((name) =>
        /^(Switch to (first|third) person|Run|Jump|Effects)$/.test(name),
      ),
    `Unexpected controls: ${JSON.stringify(ui.buttons)}`,
  );
  assert(
    Math.abs(ui.frame.x) < 1 &&
      Math.abs(ui.frame.y) < 1 &&
      Math.abs(ui.frame.width - ui.viewport[0]) < 1 &&
      Math.abs(ui.frame.height - ui.viewport[1]) < 1,
    "The world is not fullscreen",
  );
  record(
    "Fullscreen world exposes movement and live effects without quests, spells, or scene editing",
    ui,
  );
  await capture("ground-third-person");

  record("Five-second grass frame sample", await frameSample(5000, false));
  record(
    "Stationary foreground grass visibly moves with the submitted wind",
    await windMotion(),
  );
  const walk = await move(false),
    sprint = await move(true),
    ratio = sprint.distance / walk.distance;
  assert(
    ratio > 1.15 && ratio < 3,
    `Sprint did not increase speed: ${JSON.stringify({ walk, sprint, ratio })}`,
  );
  record("Real W and Shift input move across the source ground", {
    walk,
    sprint,
    ratio,
  });

  const beforeJump = await status(),
    jumpSamples = [];
  await focusWorld();
  await page.keyboard.down("Space");
  try {
    await until(
      async () => {
        const value = await status();
        jumpSamples.push({
          frame: value.frame,
          feet: feet(value),
          grounded: value.character.grounded,
        });
        return (
          !value.character.grounded &&
          feet(value)[1] > feet(beforeJump)[1] + 0.1
        );
      },
      "Space did not start a jump",
      6000,
    );
    await until(
      async () => {
        const value = await status();
        jumpSamples.push({
          frame: value.frame,
          feet: feet(value),
          grounded: value.character.grounded,
        });
        return (
          value.character.grounded &&
          Math.abs(feet(value)[1] - feet(beforeJump)[1]) < 0.15
        );
      },
      "The Druid did not land on the source ground",
      8000,
    );
  } finally {
    await page.keyboard.up("Space");
    await releaseInput();
  }
  record("Space jumps and lands", { samples: jumpSamples });
  const beforeView = await status();
  await focusWorld();
  await page.keyboard.press("KeyV");
  const first = await until(async () => {
    const value = await status();
    return (
      value.character.perspective === "first" &&
      horizontal(value.camera, feet(value)) < 0.1 &&
      value
    );
  }, "V did not switch to first person");
  assert(horizontal(first.camera, feet(first)) < 0.1);
  assert(
    distance(feet(first), feet(beforeView)) < 0.1,
    "Changing view moved the player",
  );
  await releaseInput();
  await capture("ground-first-person");
  await focusWorld();
  await page.keyboard.press("KeyV");
  const third = await until(async () => {
    const value = await status();
    return (
      value.character.perspective === "third" &&
      horizontal(value.camera, feet(value)) >
        value.character.orbitDistance - 0.1 &&
      value
    );
  }, "V did not restore third person");
  assert(horizontal(third.camera, feet(third)) > 0.2);
  assert(distance(feet(first), feet(third)) < 0.1);
  await releaseInput();
  record(
    "Real V key switches first and third person without moving the player",
  );
  record(
    "Ten seconds of walking and mouse look keep grass and controls responsive",
    await frameSample(10000, true),
  );
  await waitFrames((await status()).frame, 30);
  await capture("ground-after-movement");

  const beforeDenial = await status();
  const denied = await receiptAfter(
    () =>
      page.evaluate(() => window.aethyrforge.selectAbility("lightning_bolt")),
    false,
  );
  const afterDenial = await waitFrames(beforeDenial.frame, 3);
  noGameplay(afterDenial);
  assert.equal(afterDenial.revision, initial.revision);
  assert(distance(feet(beforeDenial), feet(afterDenial)) < 0.1);
  record(
    "Spell selection is denied without enabling gameplay or changing the world",
    { receipt: denied },
  );
  report.finalStatus = afterDenial;
  grassStats(afterDenial);
  report.graphicsMessages = await page.evaluate(() => window.__groundGraphics);
  assert.deepEqual(
    report.graphicsMessages,
    [],
    "WebGPU rejected a submitted pass",
  );
  await Promise.all(bodies);
  assert.deepEqual(report.errors, [], "The page raised an uncaught error");
  assert.deepEqual(
    report.failed.filter(
      (item) => new URL(item.url).pathname !== "/favicon.ico",
    ),
    [],
    "World requests failed",
  );
  assert(
    !report.console.some(
      (item) =>
        item.type === "error" &&
        /GPUValidation|device lost|WGSL.*error/i.test(item.text),
    ),
    "A graphics error was logged",
  );
  report.ok = true;
} catch (error) {
  report.failure = String(error?.stack || error);
  console.error(report.failure);
  process.exitCode = 1;
  try {
    report.finalStatus = await status();
    report.graphicsMessages = await page.evaluate(
      () => window.__groundGraphics,
    );
  } catch {}
  try {
    const frame = await (
      await page.locator("#preview-frame").elementHandle()
    ).contentFrame();
    report.inputEvents = await frame.evaluate(() => window.__groundInput);
  } catch {}
  try {
    await capture("ground-failure");
  } catch {}
} finally {
  closing = true;
  try {
    await releaseInput();
  } catch {}
  report.finishedAt = new Date().toISOString();
  try {
    await writeFile(
      resolve(output, "ground-browser.json"),
      JSON.stringify(report, null, 2) + "\n",
    );
  } finally {
    await page.close().catch(() => {});
    await context.close().catch(() => {});
    await browser.close();
  }
}
