/** Run through openagents browser run. Logs and licensed captures stay outside Git. */
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const script = fileURLToPath(import.meta.url);
const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set AF_CAPTURE_DIR outside the checkout.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep logs and captures outside the checkout.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const world = process.env.AF_VERIFY_WORLD || "starter";
assert(["starter", "medieval-town"].includes(world));
const phase = process.argv
  .find((value) => value.startsWith("--phase="))
  ?.split("=")[1];
assert(phase === undefined || ["record", "replay"].includes(phase));
const readJSON = async (path) => JSON.parse(await readFile(path, "utf8"));
const writeJSON = async (path, value) =>
  writeFile(path, JSON.stringify(value, null, 2) + "\n");
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");

if (!phase) {
  const summary = {
    schema: "aethyrforge.agent-process-browser.v1",
    world,
    startedAt: new Date().toISOString(),
    ok: false,
  };
  const started = performance.now();
  try {
    for (const childPhase of ["record", "replay"])
      await new Promise((resolve, reject) => {
        const child = spawn(
          process.execPath,
          [script, `--phase=${childPhase}`],
          {
            cwd: root,
            env: process.env,
            stdio: ["ignore", "inherit", "inherit"],
          },
        );
        child.once("error", reject);
        child.once("exit", (code, signal) =>
          code === 0
            ? resolve()
            : reject(
                new Error(
                  `The ${childPhase} process failed (${signal || code}).`,
                ),
              ),
        );
      });
    const recorded = await readJSON(
      resolve(output, `${world}-agent-record.json`),
    );
    const replayed = await readJSON(
      resolve(output, `${world}-agent-replay.json`),
    );
    assert(recorded.ok && replayed.ok);
    assert.notEqual(
      recorded.processId,
      replayed.processId,
      "Replay reused the recording Node process.",
    );
    assert.deepEqual(
      replayed.final,
      recorded.final,
      "Replay did not reproduce the final semantic world, quest, code, and player.",
    );
    assert.equal(
      replayed.finalCapture.pixelSha256,
      recorded.finalCapture.pixelSha256,
      "Replay did not reproduce the fixed final GPU image.",
    );
    Object.assign(summary, {
      ok: true,
      processIds: [recorded.processId, replayed.processId],
      restartBoundary:
        "The recording Node/Vite process exits and closes its browser context and workers before a fresh replay Node/Vite process opens a fresh context. The dedicated Chrome process stays alive.",
      origins: [recorded.editorOrigin, replayed.editorOrigin],
      authoredIdentity: recorded.final.authoredIdentity,
      finalPixelSha256: recorded.finalCapture.pixelSha256,
      recordChecks: recorded.checks.length,
      replayChecks: replayed.checks.length,
      inferenceCalls: 0,
      elapsedMs: performance.now() - started,
    });
    console.log(
      "PASS Fresh processes reproduce accepted edits, quest state, player, and final pixels",
    );
  } catch (error) {
    summary.failure = String(error);
    throw error;
  } finally {
    summary.finishedAt = new Date().toISOString();
    await writeJSON(resolve(output, `${world}-agent-browser.json`), summary);
    console.log(JSON.stringify(summary));
  }
} else {
  const town = world === "medieval-town";
  const camera = town
    ? { eye: [150.5, 6, 161], target: [150.5, 1, 152], fov: 1 }
    : { eye: [0, 5, 7], target: [0, 1, -2], fov: 1 };
  const request = { camera, width: 512, height: 320, time: 0, exposure: 1 };
  const probeId = "agent-prop";
  const report = {
    schema: "aethyrforge.agent-phase-browser.v1",
    world,
    phase,
    processId: process.pid,
    startedAt: new Date().toISOString(),
    ok: false,
    scope:
      "One real GPU world. Typed agent actions and bounded inspections share the editor's command authority. Captures use explicit context, camera, size, time, and exposure. Independent analytic projection, known owned landmarks, semantic state, and causal geometry controls supplement pixel hashes. Staged interaction cameras do not establish a continuous walking route. Visible confirmation means completed GPU capture/readback, not physical display latency or gallery parity.",
    checks: [],
    attempts: 0,
    retries: 0,
    inferenceCalls: 0,
    steps: [],
    captures: [],
    errors: [],
    console: [],
  };
  const started = performance.now();
  let editor, preview, browser, context, page;
  let asset, oldPosition, newPosition, oldBox, newBox, sourceFacts;
  const record = (name, details = {}) => {
    report.checks.push({ name, ...details });
    console.log(`PASS ${phase}: ${name}`);
  };
  const agent = (method, ...args) =>
    page.evaluate(
      async ({ method, args }) => {
        const driver = await import("/tools/fixtures/agent-driver.ts");
        return driver[method](...args);
      },
      { method, args },
    );
  const inspect = (options) => agent("inspect", options);
  async function inspectAuthored() {
    const limit = 2;
    const first = await inspect({ limit });
    const objects = [];
    const seen = new Set();
    let page = first,
      cursor = 0,
      pages = 0;
    do {
      pages++;
      assert(pages <= 1024, "Authored pagination exceeded the fixture budget.");
      assert.deepEqual(
        {
          sessionId: page.sessionId,
          authoredIdentity: page.authoredIdentity,
          totalObjects: page.totalObjects,
          ...Object.fromEntries(
            [
              "worldId",
              "sourceIdentity",
              "revision",
              "generation",
              "shaderIdentity",
            ].map((key) => [key, page.context[key]]),
          ),
        },
        {
          sessionId: first.sessionId,
          authoredIdentity: first.authoredIdentity,
          totalObjects: first.totalObjects,
          ...Object.fromEntries(
            [
              "worldId",
              "sourceIdentity",
              "revision",
              "generation",
              "shaderIdentity",
            ].map((key) => [key, first.context[key]]),
          ),
        },
        "The authored document changed between bounded inspection pages.",
      );
      const end = Math.min(first.totalObjects, cursor + limit);
      assert.equal(page.objects.length, end - cursor);
      for (const object of page.objects) {
        assert(!seen.has(object.id), "Authored pagination repeated an object.");
        seen.add(object.id);
        objects.push(object);
      }
      const sample = page.readOnlyObjects ?? [];
      assert.equal(page.readOnlyTotal, sourceFacts.total);
      assert.equal(
        sample.length,
        Math.min(sourceFacts.total, limit - page.objects.length),
      );
      for (const [index, object] of sample.entries())
        assert.deepEqual(
          object,
          sourceFacts.sample[index],
          "A read-only sample does not describe its compiled source instance.",
        );
      const authoredRemainder = end < first.totalObjects;
      assert.equal(page.nextCursor, authoredRemainder ? end : undefined);
      assert.equal(
        page.truncated,
        authoredRemainder || sample.length < sourceFacts.total,
        "Truncation must report omitted authored records or read-only source instances.",
      );
      if (!authoredRemainder) break;
      cursor = end;
      page = await inspect({
        cursor,
        limit,
        expectedRevision: first.context.revision,
      });
    } while (true);
    assert.equal(objects.length, first.totalObjects);
    return {
      inspection: { ...first, objects, nextCursor: undefined },
      enumeration: {
        pages,
        pageLimit: limit,
        authoredObjects: objects.length,
        readOnlyTotal: sourceFacts.total,
        readOnlySampleChecked: first.readOnlyObjects?.length ?? 0,
        readOnlyScope:
          "Bounded source sample checked against compiled instance facts; authored records enumerated in full.",
      },
    };
  }
  async function until(test, message, timeout = 30_000) {
    const deadline = Date.now() + timeout;
    do {
      const value = await test();
      if (value) return value;
      await page.waitForTimeout(80);
    } while (Date.now() < deadline);
    throw Error(`${message}; inspection=${JSON.stringify(await inspect())}`);
  }
  async function execute(action, expected = true) {
    report.attempts++;
    let step;
    try {
      step = await agent("execute", action);
    } catch (error) {
      if (expected !== false && expected !== null) throw error;
      const failure = {
        receipt: { ok: false, message: String(error) },
        action,
        rejectedByClient: true,
      };
      report.steps.push(failure);
      return failure;
    }
    report.steps.push(step);
    if (expected !== null)
      assert.equal(step.receipt.ok, expected, step.receipt.message);
    return step;
  }
  async function fixedCapture(
    name,
    landmarks = [],
    allowFlatEmptyControl = false,
  ) {
    const captured = await agent(
      "capture",
      name,
      request,
      landmarks,
      allowFlatEmptyControl,
    );
    assert(captured.oracle.ok, JSON.stringify(captured.oracle));
    const bytes = Buffer.from(captured.base64, "base64");
    assert.equal(hash(bytes), captured.metadata.pixelSha256);
    const path = `${world}-${phase}-${name}.rgba`;
    await writeFile(resolve(output, path), bytes);
    const { base64, ...saved } = captured;
    report.captures.push({ ...saved, path });
    return saved;
  }
  function worldBox(position) {
    return {
      min: asset.bounds.min.map((n, i) => position[i] + n * 2),
      max: asset.bounds.max.map((n, i) => position[i] + n * 2),
    };
  }
  function pose(position) {
    return { position, yaw: 0, scale: 2 };
  }
  function continuity(value) {
    return {
      authoredIdentity: value.authoredIdentity,
      revision: value.context.revision,
      generation: value.context.generation,
      shaderIdentity: value.context.shaderIdentity,
      game: value.game,
      player: value.player.position,
      view: value.view,
    };
  }
  function sameContinuity(before, after) {
    const { player: p, ...a } = continuity(before),
      { player: q, ...b } = continuity(after);
    assert.deepEqual(b, a);
    assert(
      Math.hypot(...p.map((n, i) => n - q[i])) < 0.03,
      "A rejected action moved the player.",
    );
  }
  async function groundPose(eye, target) {
    await execute({ type: "camera", eye, target, fov: 1, mode: "walk" });
    await until(async () => {
      const value = await inspect();
      return (
        value.player.grounded &&
        Math.abs(value.player.velocity?.[1] ?? 0) < 0.03 &&
        value
      );
    }, "The staged walking player did not settle");
    await page.waitForTimeout(200);
  }
  async function interactAt(position) {
    const setups = [];
    for (const [dx, dz] of [
      [0, 1.2],
      [1.2, 0],
      [-1.2, 0],
      [0, -1.2],
      [0.85, 0.85],
      [-0.85, 0.85],
    ]) {
      if (setups.length) report.retries++;
      const eye = [position[0] + dx, position[1] + 1.62, position[2] + dz];
      const result = await execute(
        {
          type: "camera",
          eye,
          target: [position[0], eye[1], position[2]],
          fov: 1,
          mode: "walk",
        },
        null,
      );
      setups.push({ eye, accepted: result.receipt.ok });
      if (!result.receipt.ok) continue;
      await page.waitForTimeout(200);
      const before = await inspect();
      const interaction = await execute(
        { type: "game", action: "interact" },
        null,
      );
      const after = await inspect();
      if (interaction.receipt.ok) return { before, after, interaction, setups };
    }
    throw Error(
      `No open staged interaction position worked: ${JSON.stringify(setups)}`,
    );
  }
  function finalState(value) {
    return {
      worldId: value.context.worldId,
      sourceIdentity: value.context.sourceIdentity,
      authoredIdentity: value.authoredIdentity,
      shaderIdentity: value.context.shaderIdentity,
      objects: [...value.objects]
        .sort((a, b) => a.id.localeCompare(b.id))
        .map(({ id, kind, position, yaw, scale, collidable }) => ({
          id,
          kind,
          position,
          yaw,
          scale,
          collidable,
        })),
      game: value.game
        ? {
            phase: value.game.phase,
            hasKey: value.game.hasKey,
            keyPosition: value.game.keyPosition,
            exitPosition: value.game.exitPosition,
          }
        : null,
      player: {
        position: value.player.position,
        yaw: value.player.yaw,
        pitch: value.player.pitch,
      },
      view: value.view,
    };
  }
  try {
    const define = {
      "import.meta.env.VITE_WORLD_PROFILE": JSON.stringify(world),
      "import.meta.env.VITE_STREAM_PACKAGE_URL": JSON.stringify(""),
    };
    editor = await createServer({
      root,
      configFile: resolve(root, "vite.config.ts"),
      cacheDir: `node_modules/.vite-agent-${process.pid}-editor`,
      server: { host: "127.0.0.1", port: 0, strictPort: false },
      define: { ...define },
    });
    await editor.listen();
    const editorOrigin = new URL(editor.resolvedUrls.local[0]).origin;
    report.editorOrigin = editorOrigin;
    preview = await createServer({
      root,
      configFile: resolve(root, "vite.config.ts"),
      cacheDir: `node_modules/.vite-agent-${process.pid}-preview`,
      server: { host: "127.0.0.1", port: 0, strictPort: false },
      define: {
        ...define,
        "import.meta.env.VITE_EDITOR_ORIGIN": JSON.stringify(editorOrigin),
      },
    });
    await preview.listen();
    editor.config.define["import.meta.env.VITE_PREVIEW_URL"] = JSON.stringify(
      new URL(preview.resolvedUrls.local[0]).origin + "/preview.html",
    );
    browser = await chromium.connectOverCDP(endpoint);
    report.browser = await browser.version();
    context = await browser.newContext({
      viewport: { width: 800, height: 500 },
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
    await page.goto(editorOrigin, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(
      () =>
        window.aethyrforge?.agent &&
        window.aethyrforge.snapshot().status.phase === "ready",
      null,
      { timeout: 180_000 },
    );
    sourceFacts = await page.evaluate(async (world) => {
      const response = await fetch(
        world === "starter"
          ? "/starter/scene.json"
          : "/private-assets/scene.json",
      );
      if (!response.ok) throw Error("The known compiled scene could not open.");
      const scene = await response.json();
      return {
        total: scene.instances.length,
        sample: scene.instances.slice(0, 2).map((instance, index) => ({
          id: `source.${index}`,
          kind: `mesh.${instance.mesh}`,
          editable: false,
          position: [
            instance.model[12],
            instance.model[13],
            instance.model[14],
          ],
          sourceIndex: index,
        })),
      };
    }, world);
    const discovery = await agent("discover"),
      initialPages = await inspectAuthored(),
      initial = initialPages.inspection;
    assert.equal(discovery.schema, "aethyrforge.agent.v1");
    assert.equal(initial.context.sourceIdentity, discovery.sourceIdentity);
    report.initial = initial;
    record(
      "Versioned discovery and bounded semantic inspection describe the real world",
      {
        discovery,
        context: initial.context,
        enumeration: initialPages.enumeration,
      },
    );
    if (phase === "replay") {
      const log = await readJSON(resolve(output, `${world}-agent-run.json`));
      report.replay = await agent("replay", log);
      const finalPages = await inspectAuthored(),
        final = finalPages.inspection;
      report.final = finalState(final);
      report.finalCapture = (await fixedCapture("final")).metadata;
      record(
        "A fresh process replays the accepted run through the same typed actions",
        {
          context: final.context,
          authoredIdentity: final.authoredIdentity,
          enumeration: finalPages.enumeration,
        },
      );
    } else {
      asset = await page.evaluate(async (world) => {
        const response = await fetch(
          world === "starter"
            ? "/starter/scene.json"
            : "/private-assets/scene.json",
        );
        if (!response.ok)
          throw Error("The known compiled scene could not open.");
        const scene = await response.json();
        const catalogResponse = await fetch(scene.catalog);
        if (!catalogResponse.ok)
          throw Error("The known compiled catalog could not open.");
        const catalog = await catalogResponse.json();
        const asset = catalog.assets.find((asset) => asset.id === "barrel");
        if (!asset) throw Error("The fixture needs a barrel mesh.");
        return { bounds: asset.bounds, dimensions: asset.dimensions };
      }, world);
      const rootY = -asset.bounds.min[1] * 2;
      oldPosition = town ? [148, rootY, 152] : [-2.5, rootY, -2];
      newPosition = town ? [153, rootY, 152] : [2.5, rootY, -2];
      oldBox = worldBox(oldPosition);
      newBox = worldBox(newPosition);
      report.fixture = {
        asset,
        oldPosition,
        newPosition,
        oldBox,
        newBox,
        request,
      };
      await execute({ type: "camera", ...camera, mode: "fly" });
      const beforeInput = await inspect();
      await execute({
        type: "input",
        durationMs: 160,
        input: {
          forward: 0,
          right: 1,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          jump: false,
        },
      });
      const afterInput = await inspect();
      assert(
        Math.hypot(
          ...afterInput.player.position.map(
            (n, i) => n - beforeInput.player.position[i],
          ),
        ) > 0.02,
        "The bounded agent input did not move the real player.",
      );
      assert(
        ["forward", "right", "up", "lookX", "lookY"].every(
          (key) => afterInput.input[key] === 0,
        ),
        "The bounded input action left held input behind.",
      );
      record(
        "Bounded typed input moves the live player and releases its held state",
      );
      await execute({ type: "camera", ...camera, mode: "fly" });
      await execute({
        type: "command",
        command: {
          kind: "prop.add",
          assetId: "barrel",
          id: "agent-anchor",
          position: [300, rootY, 300],
          scale: 2,
        },
      });
      const empty = await fixedCapture("empty", [], town);
      const landmarks = town
        ? []
        : await page.evaluate(async () =>
            (
              await import("/tools/fixtures/agent-driver.ts")
            ).OWNED_STARTER_LANDMARKS.filter(
              (item) => item.id === "exit-left-post",
            ),
          );
      await execute({
        type: "command",
        command: {
          kind: "prop.add",
          assetId: "barrel",
          id: probeId,
          position: oldPosition,
          scale: 2,
        },
      });
      const old = await fixedCapture("old", landmarks);
      await execute({
        type: "command",
        command: {
          kind: "entity.transform",
          id: probeId,
          position: newPosition,
        },
      });
      const current = await fixedCapture("current", landmarks);
      const addExpected = {
        entityId: probeId,
        beforeBox: null,
        afterBox: oldBox,
        beforePose: null,
        afterPose: pose(oldPosition),
      };
      const transformExpected = {
        entityId: probeId,
        beforeBox: oldBox,
        afterBox: newBox,
        beforePose: pose(oldPosition),
        afterPose: pose(newPosition),
      };
      for (const [name, before, after, expected] of [
        ["add", "empty", "old", addExpected],
        ["transform", "old", "current", transformExpected],
      ]) {
        const oracle = await agent("causal", before, after, expected, "empty");
        assert(oracle.ok, JSON.stringify(oracle));
        record(
          `Independent semantic, geometry, and pixel checks verify the ${name}`,
          { oracle },
        );
      }
      const wrongRequest = {
        ...request,
        camera: {
          eye: camera.eye,
          target: [camera.eye[0], camera.eye[1] + 10, camera.eye[2] - 0.1],
          fov: 1,
        },
      };
      const wrongView = await agent("capture", "wrong-view", wrongRequest);
      const wrongViewPath = `${world}-${phase}-wrong-view.rgba`;
      await writeFile(
        resolve(output, wrongViewPath),
        Buffer.from(wrongView.base64, "base64"),
      );
      const { base64: wrongViewBase64, ...wrongViewReport } = wrongView;
      report.captures.push({
        ...wrongViewReport,
        path: wrongViewPath,
        intentionalNegative: true,
      });
      report.negativeControls = await agent("negativeControls", {
        empty: "empty",
        old: "old",
        current: "current",
        wrongView: "wrong-view",
        add: { ...addExpected, afterBox: newBox, afterPose: pose(newPosition) },
        transform: transformExpected,
      });
      for (const [name, oracle] of Object.entries(report.negativeControls))
        if (oracle && typeof oracle === "object" && "ok" in oracle)
          assert.equal(
            oracle.ok,
            false,
            `The ${name} counterfeit passed independent verification.`,
          );
      record(
        "Rehashed black, missing-geometry, wrong-camera, and stale-pixel counterfeits fail",
        {
          negatives: report.negativeControls,
          wrongViewPixelsScope: town
            ? "Correct-metadata wrong-view pixels require the owned starter landmarks; Town is checked by scoped causal edits."
            : "Known green source geometry rejects incorrect pixels even with correct metadata.",
        },
      );

      const staleRevision = await agent(
        "staleCapture",
        await agent("agentExpectation", old.inspection.context),
        request,
      );
      const latest = await inspect();
      const staleGeneration = await agent(
        "staleCapture",
        {
          ...(await agent("agentExpectation", latest.context)),
          generation: latest.context.generation + 1,
        },
        request,
      );
      assert.equal(staleRevision.accepted, false);
      assert.equal(staleGeneration.accepted, false);
      sameContinuity(latest, await inspect());
      record("Stale revision and generation capture requests are rejected", {
        staleRevision,
        staleGeneration,
      });
      const beforeBounds = await inspect();
      const invalidBounds = await execute(
        {
          type: "command",
          command: {
            kind: "entity.transform",
            id: probeId,
            position: [1_000_000, rootY, 0],
          },
        },
        false,
      );
      sameContinuity(beforeBounds, await inspect());
      assert.match(
        invalidBounds.receipt.message,
        /position|coordinate|world|bound|supported|finite/i,
      );
      assert.equal(
        (await fixedCapture("after-invalid-bounds")).metadata.pixelSha256,
        current.metadata.pixelSha256,
      );

      await groundPose(
        town ? [150, 1.62, 155.5] : [0, 1.62, 4.5],
        town ? [150, 1.62, 149] : [0, 1.62, -2],
      );
      const beforeOccupied = await inspect();
      const occupiedEdit = await execute(
        {
          type: "command",
          command: {
            kind: "entity.transform",
            id: probeId,
            position: [
              beforeOccupied.player.position[0],
              beforeOccupied.player.position[1] + rootY,
              beforeOccupied.player.position[2],
            ],
          },
        },
        false,
      );
      sameContinuity(beforeOccupied, await inspect());
      assert.match(
        occupiedEdit.receipt.message,
        /solid|player|wall|overlap|occupied|place|collision|you/i,
      );
      await execute({ type: "camera", ...camera, mode: "fly" });
      assert.equal(
        (await fixedCapture("after-occupied-edit")).metadata.pixelSha256,
        current.metadata.pixelSha256,
      );
      record(
        "Rejected bounds and occupied edits retain the player and working world",
      );

      const beforeCode = await inspect();
      const invalidShader = await execute(
        { type: "shader", source: "this is not valid WGSL" },
        false,
      );
      sameContinuity(beforeCode, await inspect());
      assert.match(
        invalidShader.receipt.message,
        /shader|WGSL|line|parse|syntax|expected|invalid/i,
      );
      assert.equal(
        (await fixedCapture("after-invalid-shader")).metadata.pixelSha256,
        current.metadata.pixelSha256,
      );
      const source = beforeCode.lastWorkingCode?.source;
      assert(
        source?.includes("return vec4(max(color,vec3(0.0)),alpha);"),
        "The known material repair patch needs updating.",
      );
      const repaired = source.replace(
        "return vec4(max(color,vec3(0.0)),alpha);",
        "return vec4(max(color,vec3(0.0))*vec3(0.35,0.7,1.0),alpha);",
      );
      await execute({ type: "shader", source: repaired });
      const repairedImage = await fixedCapture("repaired-shader");
      assert.notEqual(
        repairedImage.metadata.pixelSha256,
        current.metadata.pixelSha256,
      );
      record(
        "A failed WGSL edit retains its image, and a valid repair visibly activates",
        { shaderIdentity: repairedImage.metadata.context.shaderIdentity },
      );
      await execute({ type: "shader", source });
      assert.equal(
        (await fixedCapture("restored-default-code")).metadata.pixelSha256,
        current.metadata.pixelSha256,
      );
      record(
        "Restoring the default code returns the same working image before the longer quest run",
      );

      await groundPose(
        town ? [150, 1.62, 155.5] : [0, 1.62, 4.5],
        town ? [150, 1.62, 149] : [0, 1.62, -2],
      );
      await execute({ type: "game", action: "start" });
      const beforeRange = await inspect();
      await execute({ type: "game", action: "interact" }, false);
      const afterRange = await inspect();
      const rangeOracle = await agent("quest", {
        action: "out-of-range",
        beforePhase: beforeRange.game.phase,
        afterPhase: afterRange.game.phase,
        beforeRevision: beforeRange.context.revision,
        afterRevision: afterRange.context.revision,
        accepted: false,
      });
      assert(rangeOracle.ok, JSON.stringify(rangeOracle));
      const key = await interactAt((await inspect()).game.keyPosition);
      const keyOracle = await agent("quest", {
        action: "collect-key",
        beforePhase: key.before.game.phase,
        afterPhase: key.after.game.phase,
        beforeRevision: key.before.context.revision,
        afterRevision: key.after.context.revision,
        accepted: key.interaction.receipt.ok,
      });
      assert(keyOracle.ok, JSON.stringify(keyOracle));
      const exit = await interactAt((await inspect()).game.exitPosition);
      const exitOracle = await agent("quest", {
        action: "unlock-exit",
        beforePhase: exit.before.game.phase,
        afterPhase: exit.after.game.phase,
        beforeRevision: exit.before.context.revision,
        afterRevision: exit.after.context.revision,
        accepted: exit.interaction.receipt.ok,
      });
      assert(exitOracle.ok, JSON.stringify(exitOracle));
      const beforeReset = await inspect();
      const reset = await execute({ type: "game", action: "reset" });
      const afterReset = await inspect();
      const resetOracle = await agent("quest", {
        action: "reset",
        beforePhase: beforeReset.game.phase,
        afterPhase: afterReset.game.phase,
        beforeRevision: beforeReset.context.revision,
        afterRevision: afterReset.context.revision,
        accepted: reset.receipt.ok,
      });
      assert(resetOracle.ok, JSON.stringify(resetOracle));
      record(
        "Independent quest rules verify range rejection, key collection, exit completion, and reset",
        {
          rangeOracle,
          keyOracle,
          exitOracle,
          resetOracle,
          setup: "Camera-positioned interactions, not a continuous route.",
        },
      );
      await execute({ type: "camera", ...camera, mode: "fly" });
      const finalPages = await inspectAuthored(),
        final = finalPages.inspection;
      report.final = finalState(final);
      report.finalCapture = (await fixedCapture("final")).metadata;
      const log = await agent("exportLog");
      await writeJSON(resolve(output, `${world}-agent-run.json`), log);
      report.log = {
        schema: log.schema,
        entries: log.entries.length,
        accepted: log.entries.filter((entry) => entry.receipt.ok).length,
        rejected: log.entries.filter((entry) => !entry.receipt.ok).length,
        path: `${world}-agent-run.json`,
      };
      record(
        "The bounded replay log retains accepted actions and rejected attempts",
        { ...report.log, enumeration: finalPages.enumeration },
      );
    }
    assert.deepEqual(
      report.errors,
      [],
      "The browser reported an uncaught error.",
    );
    report.elapsedMs = performance.now() - started;
    report.ok = true;
  } catch (error) {
    report.failure = String(error);
    throw error;
  } finally {
    if (page && !page.isClosed()) {
      await agent("close").catch(() => {});
      await page
        .evaluate(() => window.aethyrforgeEditor?.service.close())
        .catch(() => {});
      const deadline = Date.now() + 10_000;
      while (page.workers().length && Date.now() < deadline)
        await page.waitForTimeout(80);
      report.workersAfterClose = page.workers().length;
      if (report.workersAfterClose) {
        report.ok = false;
        report.cleanupErrors = ["A worker survived project close."];
      }
    }
    for (const resource of [context, browser, preview, editor]) {
      try {
        await resource?.close();
      } catch (error) {
        (report.cleanupErrors ??= []).push(String(error));
        report.ok = false;
      }
    }
    report.finishedAt = new Date().toISOString();
    await writeJSON(resolve(output, `${world}-agent-${phase}.json`), report);
    console.log(
      JSON.stringify({
        ok: report.ok,
        phase,
        checks: report.checks.length,
        failure: report.failure,
        elapsedMs: report.elapsedMs,
      }),
    );
  }
}
