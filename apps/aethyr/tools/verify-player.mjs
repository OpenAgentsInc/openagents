/** Run through openagents browser run. Licensed captures and raw reports stay outside Git. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { createServer as createHTTPServer } from "node:http";
import { mkdir, readFile, realpath, stat, writeFile } from "node:fs/promises";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";
import { tsImport } from "tsx/esm/api";
import { summarizeResponseBodies } from "../experiments/c-town/measurement.mjs";

const { verifyQuestTransition, projectOracleBox, OWNED_STARTER_LANDMARKS } =
  await tsImport("../packages/runtime/agent-oracle.ts", import.meta.url);
const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(
  process.env.AF_PLAYER_ROOT,
  "Set AF_PLAYER_ROOT to the built profile parent directory.",
);
assert(process.env.AF_CAPTURE_DIR, "Set AF_CAPTURE_DIR outside the checkout.");
const builds = await realpath(resolve(process.env.AF_PLAYER_ROOT));
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep raw reports and captures outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const profiles = (
  process.env.AF_PLAYER_PROFILES || "empty,room,character"
).split(",");
assert(
  profiles.length &&
    new Set(profiles).size === profiles.length &&
    profiles.every((name) =>
      ["empty", "room", "character", "town"].includes(name),
    ),
);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const json = async (path) => JSON.parse(await readFile(path, "utf8"));
const saveJSON = (path, value) =>
  writeFile(path, JSON.stringify(value, null, 2) + "\n");
const size = { width: 512, height: 320 };
const report = {
  schema: "aethyrforge.standalone-player-browser.v1",
  startedAt: new Date().toISOString(),
  ok: false,
  profiles: [],
  scope:
    "Built static players and the editor load identical cooked artifacts in sequential browser contexts. Cold means a fresh context; warm means a same-context reload, with measured cache information. Fixed images use 512 by 320 RGBA, time zero, exposure one, and an explicit fly camera with no vignette. Quest interactions use staged admitted camera positions, not a continuous walking route. Resource and renderer counters are partial accounting, not process memory. No gallery or frame-rate improvement claim is made.",
};
let browser, context, page, httpServer, editor, preview;
let current, adapter, networkWindow;
const pendingNetwork = new Set();
function record(name, detail = {}) {
  current.checks.push({ name, ...detail });
  console.log(`PASS ${current.profile}: ${name}`);
}
async function until(test, label, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await page.waitForTimeout(80);
  } while (Date.now() < deadline);
  throw Error(
    `${label}; state=${JSON.stringify(await adapter.inspect().catch(() => ({})))}`,
  );
}
function watch() {
  const observed = current;
  page.on("pageerror", (error) => observed.errors.push(String(error)));
  page.on("console", (message) => {
    if (
      ["error", "warning"].includes(message.type()) &&
      observed.console.length < 100
    )
      observed.console.push({
        phase: networkWindow,
        type: message.type(),
        text: message.text().slice(0, 2048),
      });
  });
  page.on("response", (response) => {
    const phase = networkWindow;
    const task = (async () => {
      try {
        await response.finished();
        const sizes = await response.request().sizes();
        observed.network.push({
          phase,
          url: response.url(),
          status: response.status(),
          declaredBytes: Number(response.headers()["content-length"]) || null,
          ...sizes,
        });
      } catch (error) {
        observed.networkErrors.push({
          phase,
          url: response.url(),
          error: String(error),
        });
      }
    })();
    pendingNetwork.add(task);
    task.finally(() => pendingNetwork.delete(task));
  });
}
async function collectStartup(kind, started) {
  await Promise.allSettled([...pendingNetwork]);
  const value = await adapter.inspect();
  const timing = await page.evaluate(() => ({
    navigation: performance
      .getEntriesByType("navigation")
      .map((entry) => entry.toJSON()),
    resources: performance.getEntriesByType("resource").map((entry) => ({
      name: entry.name,
      duration: entry.duration,
      transferSize: entry.transferSize,
      encodedBodySize: entry.encodedBodySize,
      decodedBodySize: entry.decodedBodySize,
      deliveryType: entry.deliveryType,
    })),
    heap: performance.memory
      ? {
          usedJSHeapSize: performance.memory.usedJSHeapSize,
          totalJSHeapSize: performance.memory.totalJSHeapSize,
        }
      : null,
  }));
  const responses = current.network.filter((entry) => entry.phase === kind);
  const responseBodyObservation = summarizeResponseBodies(responses);
  current.startups.push({
    kind,
    navigationToReadyMs: performance.now() - started,
    startup: value.raw.startup ?? value.raw.status?.runtimeMetrics,
    renderer: value.raw.renderer,
    timing,
    observedResponses: responses.length,
    observedResponseBodyBytes: responseBodyObservation.responseBodyBytes,
    responseBodyObservation,
    declaredResponseBytes: responses.reduce(
      (n, value) => n + (value.declaredBytes ?? 0),
      0,
    ),
    byteScope:
      "Playwright sizes for observed responses, including cache behavior exposed by Chromium. Invalid or missing sizes make the total unavailable, with known partial bytes retained separately. Resource timing covers the owning document; worker and iframe resources are additionally observed through response events. These are not whole-process memory or physical wire measurements.",
  });
}
async function staticServer(directory) {
  const trusted = await realpath(directory);
  const server = createHTTPServer(async (request, response) => {
    try {
      assert(["GET", "HEAD"].includes(request.method));
      const url = new URL(request.url, "http://localhost");
      const path = await realpath(
        resolve(
          trusted,
          "." +
            decodeURIComponent(
              url.pathname === "/" ? "/player.html" : url.pathname,
            ),
        ),
      );
      assert(path.startsWith(trusted + sep));
      const info = await stat(path);
      assert(info.isFile());
      const types = {
        ".html": "text/html",
        ".js": "text/javascript",
        ".css": "text/css",
        ".json": "application/json",
        ".wasm": "application/wasm",
        ".png": "image/png",
        ".jpg": "image/jpeg",
        ".webp": "image/webp",
      };
      response.setHeader(
        "Content-Type",
        types[extname(path)] ?? "application/octet-stream",
      );
      response.setHeader("Content-Length", info.size);
      response.setHeader(
        "Cache-Control",
        path.endsWith("player-artifact.json") || path.endsWith(".html")
          ? "no-store"
          : "public,max-age=31536000,immutable",
      );
      if (request.method === "HEAD") response.end();
      else {
        const stream = createReadStream(path);
        stream.on("error", () => response.destroy());
        response.on("close", () => stream.destroy());
        stream.pipe(response);
      }
    } catch {
      response.statusCode = 404;
      response.end("Unavailable");
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return { server, origin: `http://127.0.0.1:${server.address().port}` };
}
function phase(game) {
  return !game
    ? null
    : game.completed
      ? "complete"
      : game.keyCollected
        ? "find-exit"
        : "find-key";
}
function playerAdapter() {
  let actorReference;
  return {
    async captureActorReference() {
      actorReference ??= (
        await page.evaluate(() => {
          const enabled =
            window.aethyrforgePlayer.status().state?.gameplay?.enemies?.total >
            0;
          return window.aethyrforgePlayer.benchmark({
            op: "pinActors",
            ...(enabled ? { canonical: true } : {}),
          });
        })
      ).resources.actorReference;
      assert(actorReference, "The player did not pin its current actors.");
      return actorReference;
    },
    async inspect() {
      const raw = await page.evaluate(() => window.aethyrforgePlayer.status());
      return {
        raw,
        phase: raw.phase,
        player: raw.state?.player,
        revision: raw.state?.revision,
        game: raw.game,
        gamePhase: phase(raw.game),
        input: raw.input,
        perspective: raw.perspective,
        camera: raw.camera,
      };
    },
    ready() {
      return until(
        async () => {
          const value = await this.inspect();
          assert.notEqual(value.phase, "error", JSON.stringify(value.raw));
          return value.phase === "ready" && value;
        },
        "The static player did not become ready",
        180_000,
      );
    },
    pose(eye, target, mode = "fly") {
      return page.evaluate(
        ({ eye, target, mode }) =>
          window.aethyrforgePlayer.setCamera(eye, target, mode),
        { eye, target, mode },
      );
    },
    async interact(action = "interact") {
      return page.evaluate((action) => {
        try {
          const result =
            action === "reset"
              ? window.aethyrforgePlayer.reset()
              : window.aethyrforgePlayer.interact();
          return { ok: true, result };
        } catch (error) {
          return { ok: false, message: String(error) };
        }
      }, action);
    },
    capture(request) {
      return page.evaluate(
        (request) => window.aethyrforgePlayer.capture(request),
        request,
      );
    },
    pause(value) {
      return page.evaluate(
        (value) => window.aethyrforgePlayer.pause?.(value),
        value,
      );
    },
    focus() {
      return page.locator("#world").focus();
    },
    blur() {
      return page.evaluate(() => dispatchEvent(new Event("blur")));
    },
    close() {
      actorReference = undefined;
      return page.evaluate(() => window.aethyrforgePlayer.close());
    },
  };
}
function editorAdapter(empty) {
  let actorReference;
  return {
    async captureActorReference() {
      actorReference ??= (
        await page.evaluate(async () => {
          const enabled =
            (await window.aethyrforge.agent.inspect()).enemies?.total > 0;
          return window.aethyrforge.cTownBenchmark({
            op: "pinActors",
            ...(enabled ? { canonical: true } : {}),
          });
        })
      ).resources.actorReference;
      assert(actorReference, "The editor did not pin its current actors.");
      return actorReference;
    },
    async inspect() {
      const raw = await page.evaluate(async (empty) => {
        const state = window.aethyrforge.snapshot();
        if (empty) return { status: state.status };
        try {
          return {
            status: state.status,
            inspection: await window.aethyrforge.agent.inspect(),
          };
        } catch {
          return { status: state.status };
        }
      }, empty);
      const value = raw.inspection;
      return {
        raw,
        phase: raw.status.phase,
        player: value?.player,
        revision: value?.context.revision,
        gamePhase: value?.game?.phase,
        input: value?.input,
        perspective: value?.player.perspective,
        camera: value?.view,
      };
    },
    ready() {
      return until(
        async () => {
          const value = await this.inspect();
          assert.notEqual(value.phase, "error", JSON.stringify(value.raw));
          return value.phase === "ready" && value;
        },
        "The editor did not open the published artifact",
        180_000,
      );
    },
    async pose(eye, target, mode = "fly") {
      const step = await page.evaluate(
        ({ eye, target, mode }) =>
          window.aethyrforge.agent.execute({
            type: "camera",
            eye,
            target,
            fov: 1,
            mode,
          }),
        { eye, target, mode },
      );
      assert(step.receipt.ok, step.receipt.message);
      return step;
    },
    async interact(action = "interact") {
      const step = await page.evaluate(
        (action) => window.aethyrforge.agent.execute({ type: "game", action }),
        action,
      );
      return step.receipt;
    },
    async capture(request) {
      return page.evaluate(
        async ({ request, empty }) => {
          if (empty) {
            await window.aethyrforge.beginFilmCapture({
              width: request.width,
              height: request.height,
            });
            try {
              const pixels = await window.aethyrforge.capturePixels({
                time: request.time,
              });
              let binary = "";
              const bytes = new Uint8Array(pixels);
              for (let i = 0; i < bytes.length; i += 8192)
                binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
              return {
                metadata: {
                  width: request.width,
                  height: request.height,
                  time: request.time,
                },
                base64: btoa(binary),
              };
            } finally {
              await window.aethyrforge.endFilmCapture();
            }
          }
          const before = await window.aethyrforge.agent.inspect();
          const { worldId, tick, ...expect } = before.context;
          const { vignette, ...camera } = request.camera;
          const value = await window.aethyrforge.agent.capture({
            ...request,
            camera,
            expect,
          });
          const { pixels, ...metadata } = value;
          let binary = "";
          const bytes = new Uint8Array(pixels);
          for (let i = 0; i < bytes.length; i += 8192)
            binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
          return { metadata, base64: btoa(binary) };
        },
        { request, empty },
      );
    },
    pause() {},
    async focus() {
      const frame = page.frame({ url: /\/preview\.html/ });
      assert(frame);
      await frame.locator("#world").focus();
    },
    async blur() {
      const frame = page.frame({ url: /\/preview\.html/ });
      await frame.evaluate(() => dispatchEvent(new Event("blur")));
    },
    close() {
      actorReference = undefined;
      return page.evaluate(() => window.aethyrforgeEditor.service.close());
    },
  };
}
async function capture(name, camera, validateCamera = true) {
  const request = {
    ...size,
    time: 0,
    exposure: 1,
    ...(camera ? { camera: { ...camera, vignette: null } } : {}),
    ...(current.profile !== "empty"
      ? { actorReference: await adapter.captureActorReference() }
      : {}),
  };
  const value = await adapter.capture(request);
  if (request.actorReference)
    assert.deepEqual(value.metadata.actors, request.actorReference);
  const pixels = Buffer.from(value.base64, "base64");
  assert.equal(pixels.length, size.width * size.height * 4);
  if (camera && validateCamera) {
    assert(
      value.metadata.camera,
      "The capture omitted its actual rendered camera.",
    );
    assert.deepEqual(value.metadata.camera.eye, camera.eye);
    const direction = (pose) => {
      const delta = pose.target.map((n, i) => n - pose.eye[i]);
      return delta.map((n) => n / Math.hypot(...delta));
    };
    const actual = direction(value.metadata.camera),
      expected = direction(camera);
    assert(Math.hypot(...actual.map((n, i) => n - expected[i])) < 1e-6);
    assert(Math.abs(value.metadata.camera.fov - camera.fov) < 1e-6);
  }
  const path = `${current.profile}-${networkWindow}-${name}.rgba`,
    pixelSha256 = hash(pixels);
  await writeFile(resolve(output, path), pixels);
  current.captures.push({
    name,
    phase: networkWindow,
    path,
    pixelSha256,
    metadata: value.metadata,
  });
  return { pixels, pixelSha256, metadata: value.metadata };
}
function verifyVisibleWorld(image, town) {
  let min = 255,
    max = 0;
  for (let offset = 0; offset < image.pixels.length; offset += 4) {
    const luma =
      (image.pixels[offset] +
        image.pixels[offset + 1] +
        image.pixels[offset + 2]) /
      3;
    min = Math.min(min, luma);
    max = Math.max(max, luma);
  }
  assert(
    max - min > 8,
    "The fixed world capture is unexpectedly flat or black.",
  );
  if (town) return { lumaRange: max - min };
  const marker = OWNED_STARTER_LANDMARKS.find(
    (item) => item.id === "exit-left-post",
  );
  const roi = projectOracleBox(
    image.metadata.camera,
    size.width,
    size.height,
    marker.box,
  );
  assert(roi, "The known owned exit post is outside the fixed camera.");
  let green = 0;
  for (let y = roi.minY; y < roi.maxY; y++)
    for (let x = roi.minX; x < roi.maxX; x++) {
      const offset = (y * size.width + x) * 4;
      const [r, g, b] = image.pixels.subarray(offset, offset + 3);
      if (g > 40 && g > r * 1.2 && g > b * 1.05) green++;
    }
  assert(
    green >= 4,
    "The independently specified green exit post is missing from the fixed view.",
  );
  return { lumaRange: max - min, landmark: marker.id, greenPixels: green, roi };
}
function changed(a, b, box) {
  const roi = box
    ? projectOracleBox(a.metadata.camera, size.width, size.height, box)
    : { minX: 0, minY: 0, maxX: size.width, maxY: size.height };
  assert(roi, "The independently positioned object is outside the capture.");
  let count = 0;
  for (let y = roi.minY; y < roi.maxY; y++)
    for (let x = roi.minX; x < roi.maxX; x++) {
      const p = (y * size.width + x) * 4;
      if (
        Math.max(
          ...[0, 1, 2].map((channel) =>
            Math.abs(a.pixels[p + channel] - b.pixels[p + channel]),
          ),
        ) > 8
      )
        count++;
    }
  return count;
}
function segmentHitsBox(eye, target, box) {
  let near = 0,
    far = 1;
  for (let axis = 0; axis < 3; axis++) {
    const delta = target[axis] - eye[axis];
    if (Math.abs(delta) < 1e-9) {
      if (eye[axis] < box.min[axis] || eye[axis] > box.max[axis]) return false;
    } else {
      const a = (box.min[axis] - eye[axis]) / delta,
        b = (box.max[axis] - eye[axis]) / delta;
      near = Math.max(near, Math.min(a, b));
      far = Math.min(far, Math.max(a, b));
      if (near > far) return false;
    }
  }
  return far >= 0 && near <= 1;
}
function semantic(value) {
  return {
    position: value.player?.position,
    revision: value.revision,
    game: value.game,
    worldId: value.raw.worldId,
    sourceIdentity: value.raw.sourceIdentity,
    entities: value.raw.state?.entities,
  };
}
async function movementAndBlur() {
  const before = await adapter.inspect();
  await adapter.focus();
  await page.keyboard.down("KeyD");
  await until(
    async () => {
      const value = await adapter.inspect();
      return (
        Math.hypot(
          ...value.player.position.map((n, i) => n - before.player.position[i]),
        ) > 0.03
      );
    },
    "Keyboard input did not move the live controller",
    10_000,
  );
  await adapter.blur();
  await page.keyboard.up("KeyD");
  await until(async () => {
    const value = await adapter.inspect();
    return (
      value.input &&
      ["forward", "right", "up"].every((key) => !value.input[key])
    );
  }, "Blur did not release movement");
  record(
    "Real keyboard input moves the player and the blur handler releases held input",
  );
}
async function grounded() {
  return until(async () => {
    const value = await adapter.inspect();
    return (
      value.player?.grounded &&
      Math.abs(value.player.velocity?.[1] ?? 0) < 0.03 &&
      value
    );
  }, "The staged walking position did not settle");
}
async function quest(world) {
  assert(world.game);
  const config = world.game;
  async function transition(action, run) {
    const before = await adapter.inspect(),
      result = await run(),
      after = await adapter.inspect();
    const oracle = verifyQuestTransition({
      action,
      beforePhase: before.gamePhase,
      afterPhase: after.gamePhase,
      beforeRevision: before.revision,
      afterRevision: after.revision,
      accepted: result.ok,
    });
    assert(oracle.ok, JSON.stringify({ action, result, oracle }));
    current.quests.push({ phase: networkWindow, action, oracle, result });
  }
  async function near(target) {
    const attempts = [];
    for (const [x, z] of [
      [0, 1.2],
      [1.2, 0],
      [-1.2, 0],
      [0, -1.2],
      [0.85, 0.85],
      [-0.85, 0.85],
    ]) {
      const eye = [target[0] + x, target[1] + 1.62, target[2] + z];
      try {
        await adapter.pose(eye, [target[0], eye[1], target[2]], "walk");
        const value = await grounded();
        if (
          Math.hypot(...value.player.position.map((n, i) => n - target[i])) <=
          1.8
        )
          return;
      } catch (error) {
        attempts.push({ eye, error: String(error) });
      }
    }
    throw Error(
      `No staged position reached the objective: ${JSON.stringify(attempts)}`,
    );
  }
  await adapter.pause(false);
  await adapter.pose(
    world.spawn.position.map((n, i) => n + (i === 1 ? 1.62 : 0)),
    [
      world.spawn.position[0],
      world.spawn.position[1] + 1.62,
      world.spawn.position[2] - 1,
    ],
    "walk",
  );
  await grounded();
  await transition("out-of-range", () => adapter.interact());
  await near(config.exit.position);
  await transition("locked-exit", () => adapter.interact());
  await near(config.key.position);
  await transition("collect-key", () => adapter.interact());
  await near(config.exit.position);
  await transition("unlock-exit", () => adapter.interact());
  await transition("reset", () => adapter.interact("reset"));
  record(
    "Independent rules verify range, locked exit, key, completion, and reset",
    { setup: "Admitted staged camera positions; no continuous route claim." },
  );
}
async function invalidCandidates(artifact, fixedCamera, baseline) {
  await adapter.pause(true);
  const before = semantic(await adapter.inspect());
  for (const [name, mutate] of [
    ["abi", (value) => (value.abi = 999)],
    [
      "profile",
      (value) => (value.profile = value.profile === "empty" ? "room" : "empty"),
    ],
    ["runtime", (value) => (value.runtime.sha256 = "f".repeat(64))],
    ...(artifact.kernel
      ? [["kernel", (value) => (value.kernel.sha256 = "f".repeat(64))]]
      : []),
  ]) {
    const candidate = structuredClone(artifact);
    mutate(candidate);
    const result = await page.evaluate(async (candidate) => {
      try {
        await window.aethyrforgePlayer.validateCandidate(candidate);
        return { accepted: true };
      } catch (error) {
        return { accepted: false, message: String(error) };
      }
    }, candidate);
    assert.equal(result.accepted, false, `The ${name} candidate was accepted.`);
    assert.deepEqual(semantic(await adapter.inspect()), before);
    assert.equal(
      (await capture(`invalid-${name}`, fixedCamera)).pixelSha256,
      baseline.pixelSha256,
    );
    current.invalidCandidates.push({ name, ...result });
  }
  record(
    "Incompatible artifact candidates retain live player, rules, geometry, and pixels",
  );
}
async function closeContext() {
  if (adapter && page && !page.isClosed())
    await adapter.close().catch(() => {});
  if (context) {
    if (page && !page.isClosed()) {
      const deadline = Date.now() + 10_000;
      while (page.workers().length && Date.now() < deadline)
        await page.waitForTimeout(80);
      assert.equal(
        page.workers().length,
        0,
        "The closed project retained a worker.",
      );
    }
    await context.close();
    context = undefined;
    page = undefined;
  }
  adapter = undefined;
}
async function editorServers(directory, town) {
  process.env.AETHYRFORGE_STREAM_ROOT = directory;
  const define = {
    "import.meta.env.VITE_C_TOWN_BENCHMARK": JSON.stringify("1"),
    "import.meta.env.VITE_WORLD_PROFILE": JSON.stringify(
      town ? "medieval-town" : "starter",
    ),
    "import.meta.env.VITE_STREAM_PACKAGE_URL": JSON.stringify(""),
    "import.meta.env.VITE_PLAYER_ARTIFACT_URL": JSON.stringify(
      "/player-artifact.json",
    ),
  };
  editor = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-player-${process.pid}-editor`,
    server: { host: "127.0.0.1", port: 0 },
    define,
  });
  await editor.listen();
  const origin = new URL(editor.resolvedUrls.local[0]).origin;
  preview = await createServer({
    root,
    configFile: resolve(root, "vite.config.ts"),
    cacheDir: `node_modules/.vite-player-${process.pid}-preview`,
    server: { host: "127.0.0.1", port: 0 },
    define: {
      ...define,
      "import.meta.env.VITE_EDITOR_ORIGIN": JSON.stringify(origin),
    },
  });
  await preview.listen();
  editor.config.define["import.meta.env.VITE_PREVIEW_URL"] = JSON.stringify(
    new URL(preview.resolvedUrls.local[0]).origin + "/preview.html",
  );
  return origin;
}
try {
  browser = await chromium.connectOverCDP(endpoint);
  report.browser = await browser.version();
  for (const profile of profiles) {
    const directory = await realpath(resolve(builds, profile));
    const artifact = await json(resolve(directory, "player-artifact.json"));
    current = {
      profile,
      ok: false,
      checks: [],
      startups: [],
      network: [],
      networkErrors: [],
      captures: [],
      quests: [],
      invalidCandidates: [],
      errors: [],
      console: [],
    };
    report.profiles.push(current);
    current.audit = await json(resolve(builds, `${profile}.audit.json`));
    assert(current.audit.ok, JSON.stringify(current.audit.errors));
    record(
      "The exact delivered build passes source and dependency graph auditing",
      { delivered: current.audit.delivered, totals: current.audit.totals },
    );
    const empty = profile === "empty",
      town = profile === "town";
    const entryCell = artifact.content?.package.cells.find(
      (cell) => cell.id === artifact.content.entryCell,
    );
    const sourceScene = empty
      ? undefined
      : await json(resolve(directory, "." + entryCell.scenes.day));
    const fixedCamera = empty
      ? undefined
      : town
        ? {
            eye: sourceScene.spawn.position,
            target: sourceScene.spawn.target,
            fov: 1,
          }
        : { eye: [0, 5, 7], target: [0, 1, -2], fov: 1 };
    const world = empty
      ? undefined
      : await json(resolve(directory, "." + artifact.content.worldURL));
    const served = await staticServer(directory);
    httpServer = served.server;
    context = await browser.newContext({
      viewport: { width: 800, height: 500 },
    });
    page = await context.newPage();
    watch();
    adapter = playerAdapter();
    for (const warm of [false, true]) {
      networkWindow = warm ? "player-warm" : "player-cold";
      const started = performance.now();
      if (warm) {
        await adapter.close();
        await page.reload({ waitUntil: "domcontentloaded" });
      } else
        await page.goto(served.origin + "/player.html", {
          waitUntil: "domcontentloaded",
        });
      await page.waitForFunction(() => window.aethyrforgePlayer);
      await adapter.ready();
      await collectStartup(networkWindow, started);
      if (!empty) {
        await adapter.pose(fixedCamera.eye, fixedCamera.target, "fly");
        await movementAndBlur();
        await adapter.pose(fixedCamera.eye, fixedCamera.target, "fly");
        await adapter.pause(true);
      }
      const image = await capture("fixed", fixedCamera);
      if (!empty) {
        const visible = (await adapter.inspect()).raw.render;
        assert(
          visible?.submittedInstances > 0 && visible?.submittedTriangles > 0,
          "The static player fixed capture submitted no indexed world geometry.",
        );
        current.checks.push({
          name: "Fixed image contains independently checked world geometry",
          phase: networkWindow,
          render: visible,
          ...verifyVisibleWorld(image, town),
        });
      }
      const actorProof = image.metadata.actors
        ? {
            basis: image.metadata.actors.basis ?? "live-reference",
            modelIdentity: image.metadata.actors.modelIdentity,
            poseSha256: image.metadata.actors.poseSha256,
            actorIds: image.metadata.actors.actorIds,
            count: image.metadata.actors.count,
          }
        : null;
      if (!warm) {
        current.playerFixed = image.pixelSha256;
        current.playerActorProof = actorProof;
      } else {
        assert.deepEqual(
          actorProof,
          current.playerActorProof,
          "Cold and warm captures used different native actor poses or models.",
        );
        assert.equal(
          image.pixelSha256,
          current.playerFixed,
          "Cold and warm loads produced different fixed images.",
        );
      }
    }
    record("Cold and warm static loads reproduce the same fixed GPU image");
    if (empty) {
      assert.equal(current.audit.totals.wasm.rawBytes, 0);
      assert(
        !current.network.some(
          (entry) =>
            entry.phase.startsWith("player-") &&
            new URL(entry.url).pathname.endsWith(".wasm"),
        ),
        "The empty player fetched a simulation kernel.",
      );
      record("The empty player delivers and requests no simulation kernel");
    }
    const baseline = await capture("before-invalid", fixedCamera);
    await invalidCandidates(artifact, fixedCamera, baseline);
    if (!empty) {
      await quest(world);
      if (artifact.profile === "character") {
        await adapter.pose(
          [
            world.spawn.position[0],
            world.spawn.position[1] + 1.62,
            world.spawn.position[2],
          ],
          [
            world.spawn.position[0],
            world.spawn.position[1] + 1.62,
            world.spawn.position[2] - 1,
          ],
          "walk",
        );
        await grounded();
        await adapter.pause(true);
        const player = (await adapter.inspect()).player.position;
        const view = {
          eye: [player[0] + 2, player[1] + 3, player[2] + 2],
          target: [player[0], player[1] + 1, player[2]],
          fov: 1,
        };
        assert(
          [0, 2].every(
            (axis) =>
              view.eye[axis] > sourceScene.bounds.min[axis] &&
              view.eye[axis] < sourceScene.bounds.max[axis],
          ),
          "The avatar camera is outside the source world's horizontal bounds.",
        );
        const blockers = world.colliders
          .filter((box) => segmentHitsBox(view.eye, view.target, box))
          .map((box) => box.id);
        assert.deepEqual(
          blockers,
          [],
          "The avatar camera-to-character segment crosses a known authored solid.",
        );
        const previousEye = [player[0] + 4, player[1] + 3, player[2] + 4];
        const previousBlockers = world.colliders
          .filter((box) => segmentHitsBox(previousEye, view.target, box))
          .map((box) => box.id);
        current.avatarCamera = {
          ...view,
          sourceBounds: sourceScene.bounds,
          knownSolidBlockers: blockers,
          previousProbe: {
            eye: previousEye,
            knownSolidBlockers: previousBlockers,
          },
          scope:
            "The earlier owned-fixture probe crossed its south wall and hid the avatar in both views. This probe stays inside source horizontal bounds and its camera-to-avatar segment crosses no authored solid. Source triangle occlusion is still tested by the mandatory changed-pixel check.",
        };
        async function settlePerspective(perspective, change) {
          const before = await adapter.inspect();
          await adapter.pause(false);
          await change();
          await page.waitForTimeout(400);
          await until(async () => {
            const value = await adapter.inspect();
            return (
              value.perspective === perspective &&
              value.raw.spans.submissions - before.raw.spans.submissions >= 4 &&
              value.player.grounded &&
              Math.abs(value.player.velocity?.[1] ?? 0) < 0.03
            );
          }, "The real perspective transition did not complete its live frames");
          await adapter.pause(true);
          const after = await adapter.inspect();
          assert(
            Math.hypot(...after.player.position.map((n, i) => n - player[i])) <
              0.03,
            "The perspective transition moved the settled player.",
          );
          current.perspectiveTransitions ??= [];
          current.perspectiveTransitions.push({
            perspective,
            before: before.raw.spans,
            after: after.raw.spans,
            position: after.player.position,
          });
        }
        await settlePerspective("first", () =>
          page.evaluate(() => window.aethyrforgePlayer.perspective("first")),
        );
        const first = await capture("avatar-first", view);
        await settlePerspective("third", () =>
          page.getByRole("button", { name: "View", exact: true }).click(),
        );
        assert.equal((await adapter.inspect()).perspective, "third");
        const third = await capture("avatar-third", view);
        const count = changed(first, third, {
          min: [player[0] - 0.9, player[1], player[2] - 0.9],
          max: [player[0] + 0.9, player[1] + 2.3, player[2] + 0.9],
        });
        assert(
          count > 8,
          "Third person did not draw the avatar in its independently projected region.",
        );
        record(
          "The view control reveals the avatar at a fixed independent camera",
          { changedAvatarRegionPixels: count },
        );
      }
    }
    if (process.env.AF_PLAYER_PERFORMANCE === "1") {
      current.performance = await (
        await import("./player-performance.mjs")
      ).qualifyPlayer({
        page,
        profile,
        artifact,
        sourceScene,
        directory,
        world,
        output,
      });
    }
    await closeContext();
    await new Promise((resolve, reject) =>
      httpServer.close((error) => (error ? reject(error) : resolve())),
    );
    httpServer = undefined;
    const origin = await editorServers(directory, town);
    networkWindow = "editor-cold";
    context = await browser.newContext({
      viewport: { width: 800, height: 500 },
    });
    page = await context.newPage();
    watch();
    adapter = editorAdapter(empty);
    const started = performance.now();
    await page.goto(origin, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.aethyrforge);
    await adapter.ready();
    await collectStartup(networkWindow, started);
    if (!empty) {
      await adapter.pose(fixedCamera.eye, fixedCamera.target, "fly");
      await movementAndBlur();
      await adapter.pose(fixedCamera.eye, fixedCamera.target, "fly");
    }
    const editorImage = await capture("fixed", fixedCamera);
    const actors = editorImage.metadata.actors;
    assert.deepEqual(
      actors
        ? {
            basis: actors.basis ?? "live-reference",
            modelIdentity: actors.modelIdentity,
            poseSha256: actors.poseSha256,
            actorIds: actors.actorIds,
            count: actors.count,
          }
        : null,
      current.playerActorProof,
      "Editor and player captures used different native actor poses or models.",
    );
    if (!empty) {
      const visibility = verifyVisibleWorld(editorImage, town);
      const afterCapture = await adapter.inspect();
      assert(
        afterCapture.raw.inspection?.render.instances > 0 &&
          afterCapture.raw.inspection?.render.triangles > 0,
        "The editor fixed capture submitted no indexed world geometry.",
      );
      current.checks.push({
        name: "Editor fixed capture contains visible indexed geometry",
        ...visibility,
        render: afterCapture.raw.inspection.render,
      });
    }
    assert.equal(
      editorImage.pixelSha256,
      current.playerFixed,
      "The published player and editor did not render the same artifact identically.",
    );
    record(
      "The UI-free player and editor produce exactly equal fixed RGBA pixels",
      { pixelSha256: editorImage.pixelSha256 },
    );
    if (!empty) {
      const mesh = async (enabled) => {
        const requestId = await page.evaluate(
          (enabled) => window.aethyrforgeEditor.service.send({ type: "mesh", enabled }),
          enabled,
        );
        const receipt = await until(
          () => page.evaluate((id) => window.aethyrforge.snapshot().receipts.find(
            (receipt) => receipt.requestId === id,
          ), requestId),
          "The wireframe command did not finish",
        );
        assert(receipt.ok, receipt.message);
      };
      await mesh(true);
      let overlay;
      try {
        overlay = await capture("wire-overlay", fixedCamera);
        assert.notEqual(overlay.pixelSha256, editorImage.pixelSha256,
          "The editor wire overlay did not change visible geometry.");
      } finally {
        await mesh(false);
      }
      const restored = await capture("wire-restored", fixedCamera);
      assert.equal(restored.pixelSha256, editorImage.pixelSha256,
        "Closing the wire overlay changed the original fixed image.");
      record("Wireframe opens over real geometry and restores the exact stock image", {
        wireSha256: overlay.pixelSha256,
        restoredSha256: restored.pixelSha256,
      });
      await quest(world);
    }
    await closeContext();
    await preview.close();
    preview = undefined;
    await editor.close();
    editor = undefined;
    assert.deepEqual(
      current.errors,
      [],
      "The browser reported an uncaught error.",
    );
    current.ok = current.performance?.ok ?? true;
    await saveJSON(resolve(output, `${profile}-player-browser.json`), current);
  }
  report.ok = report.profiles.every((value) => value.ok);
} catch (error) {
  report.failure = String(error);
  if (current) current.failure = String(error);
  throw error;
} finally {
  try {
    await closeContext();
  } catch (error) {
    (report.cleanupErrors ??= []).push(String(error));
    report.ok = false;
  }
  for (const resource of [preview, editor, browser])
    try {
      await resource?.close();
    } catch (error) {
      (report.cleanupErrors ??= []).push(String(error));
      report.ok = false;
    }
  if (httpServer) await new Promise((resolve) => httpServer.close(resolve));
  if (current)
    await saveJSON(
      resolve(output, `${current.profile}-player-browser.json`),
      current,
    );
  report.finishedAt = new Date().toISOString();
  await saveJSON(resolve(output, "player-browser.json"), report);
  if (!report.ok) process.exitCode = 1;
  console.log(
    JSON.stringify({
      ok: report.ok,
      profiles: report.profiles.map((value) => ({
        profile: value.profile,
        ok: value.ok,
        checks: value.checks.length,
      })),
      failure: report.failure,
    }),
  );
}
