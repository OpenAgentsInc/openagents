import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import {
  summarizeFrames,
  distribution,
} from "../experiments/c-town/measurement.mjs";
import { evaluateFramePacing } from "./performance-contract.mjs";
import {
  evaluateApplicationPacing,
  observeApplicationBrowserMode,
  applicationBrowserMode,
} from "./application-pacing.mjs";
import { ENEMY_WORKLOADS, runEnemyWorkload } from "./gameplay-workloads.mjs";

const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const galleryIds = [
  "MT-02",
  "MT-03",
  ...Array.from(
    { length: 14 },
    (_, i) => `MT-${String(i + 5).padStart(2, "0")}`,
  ),
];
const save = (path, value) =>
  writeFile(path, JSON.stringify(value, null, 2) + "\n");

/** Runs in the caller's sole live player context, outside fixed-image captures. */
export async function qualifyPlayer({
  page,
  profile,
  artifact,
  sourceScene,
  directory,
  world,
  output,
}) {
  const mode = process.env.AF_PERFORMANCE_MODE || "qualification";
  assert(["qualification", "diagnostic"].includes(mode));
  const durationMs = Number(
    process.env.AF_RUNTIME_WINDOW_MS ??
      (mode === "qualification" ? 30000 : 10000),
  );
  const warmupMs = Number(process.env.AF_RUNTIME_WARMUP_MS ?? 5000);
  const targetSetting = process.env.AF_RUNTIME_TARGET_HZ ?? "60";
  assert(["60", "display"].includes(targetSetting));
  const targetHz = targetSetting === "60" ? 60 : "display";
  const repeats = Number(process.env.AF_PERFORMANCE_REPEATS ?? 2);
  assert(
    Number.isInteger(durationMs) && durationMs >= 500 && durationMs <= 60000,
  );
  assert(Number.isInteger(warmupMs) && warmupMs >= 500 && warmupMs <= 30000);
  assert(Number.isInteger(repeats) && repeats >= 1 && repeats <= 4);
  const viewports = (process.env.AF_PERFORMANCE_VIEWPORTS || "800x500,1280x720")
    .split(",")
    .map((value) => {
      const match = /^(\d+)x(\d+)$/.exec(value);
      assert(match);
      const [width, height] = match.slice(1).map(Number);
      assert(width >= 320 && width <= 3840 && height >= 200 && height <= 2160);
      return { width, height };
    });
  const town = profile === "town";
  const shots = (
    process.env.AF_PERFORMANCE_SHOTS ||
    (town ? ["spawn", ...galleryIds].join(",") : "spawn")
  ).split(",");
  assert(
    shots.length &&
      new Set(shots).size === shots.length &&
      shots.every((id) => id === "spawn" || (town && galleryIds.includes(id))),
  );
  const routes = (process.env.AF_RUNTIME_ROUTES || "static,sway").split(",");
  assert(
    routes.length &&
      new Set(routes).size === routes.length &&
      routes.every((route) => ["static", "sway"].includes(route)),
  );
  const scenarios = (
    process.env.AF_RUNTIME_SCENARIOS || (town ? "day,night" : "day")
  ).split(",");
  assert(
    scenarios.length &&
      new Set(scenarios).size === scenarios.length &&
      scenarios.every((scenario) => ["day", "night"].includes(scenario)),
  );
  const result = {
    schema: "aethyrforge.player-pacing.v1",
    profile,
    mode,
    qualified: false,
    queueQualified: false,
    presentationMs: null,
    windows: [],
    idleResponsiveness: [],
    coverage: {},
    limitations: [
      "Queue-fence host notifications and all-pass GPU spans do not measure compositor or physical display presentation.",
      "One browser and GPU; source quality and resolution remain explicit. Short diagnostics never establish sustained qualification.",
      "Controller movement alternates bounded intentions. Quest positions are staged through the admitted controller camera API, not a continuous route.",
      "A spell window includes one real preparation/cast and subsequent effects. Persistent destruction is measured separately after effects/debris settle.",
    ],
  };
  result.harnessSources = await Promise.all(
    [
      "player-performance.mjs",
      "gameplay-workloads.mjs",
      "performance-contract.mjs",
      "application-pacing.mjs",
    ].map(async (name) => ({
      path: `tools/${name}`,
      sha256: hash(await readFile(new URL(name, import.meta.url))),
    })),
  );
  result.browserModeEvidence = await observeApplicationBrowserMode(
    page.context().browser(),
  );
  const call = (method, ...args) =>
    page.evaluate(
      ({ method, args }) => window.aethyrforgePlayer[method](...args),
      { method, args },
    );
  const status = () => call("status");
  const until = async (test, label, timeout = 40000) => {
    const deadline = Date.now() + timeout;
    do {
      const value = await test();
      if (value) return value;
      await page.waitForTimeout(80);
    } while (Date.now() < deadline);
    throw Error(label);
  };
  const originalViewport = page.viewportSize();
  const fixtures =
    town && process.env.AF_DRUID_FIXTURES
      ? JSON.parse(await readFile(process.env.AF_DRUID_FIXTURES, "utf8"))
      : null;
  if (fixtures) {
    assert(
      [
        "aethyrforge.druid-browser-fixtures.v1",
        "aethyrforge.druid-performance-fixtures.v1",
      ].includes(fixtures.schema),
    );
    assert.deepEqual(
      fixtures.scene,
      {
        gameplayURL: sourceScene.gameplayURL,
        collisionURL: sourceScene.collisionURL,
      },
      "Player spell probes belong to another asset source.",
    );
    if (fixtures.schema === "aethyrforge.druid-performance-fixtures.v1") {
      const proofBytes = await readFile(fixtures.proof.path);
      assert.equal(
        hash(proofBytes),
        fixtures.proof.sha256,
        "The private probe proof changed.",
      );
      const proof = JSON.parse(proofBytes.toString());
      assert.equal(
        proof.schema,
        "aethyrforge.druid-performance-probe-proof.v1",
      );
      assert.equal(
        proof.kernel.sha256,
        artifact.kernel.sha256,
        "The native probe used a different Wasm module.",
      );
      assert.equal(proof.scene.gameplayURL, sourceScene.gameplayURL);
      assert.equal(proof.scene.collisionURL, sourceScene.collisionURL);
      assert(proof.ownerTriangles > 0);
      for (const probe of Object.values(fixtures.abilities))
        assert.equal(proof.observedAim.nodeId, probe.nodeId);
      result.probeProof = {
        path: fixtures.proof.path,
        sha256: fixtures.proof.sha256,
        scope: proof.scope,
      };
    }
    result.fixtures = {
      schema: fixtures.schema,
      sha256: hash(await readFile(process.env.AF_DRUID_FIXTURES)),
      scene: fixtures.scene,
      sourceId: fixtures.sourceId,
    };
  }
  const castleFixtures =
    town && process.env.AF_CASTLE_FIXTURES
      ? JSON.parse(await readFile(process.env.AF_CASTLE_FIXTURES, "utf8"))
      : null;
  if (castleFixtures) {
    assert.equal(
      castleFixtures.schema,
      "aethyrforge.castle-browser-fixtures.v1",
    );
    const cell = artifact.content.package.cells.find(
      (value) => value.id === artifact.content.entryCell,
    );
    for (const fixture of castleFixtures.scenes) {
      assert(["scene.json", "scene-night.json"].includes(fixture.scene));
      const mode = fixture.scene === "scene.json" ? "day" : "night";
      const scene = JSON.parse(
        await readFile(resolve(directory, "." + cell.scenes[mode]), "utf8"),
      );
      assert.equal(
        scene.gameplayURL,
        fixture.gameplayURL,
        "Castle gameplay source binding changed.",
      );
      assert.equal(
        scene.collisionURL,
        fixture.collisionURL,
        "Castle collision source binding changed.",
      );
      const manifest = JSON.parse(
        await readFile(resolve(directory, "." + scene.gameplayURL), "utf8"),
      );
      const ids = manifest.targetGroups.castle;
      const selectedIds = new Set(ids);
      const objects = manifest.objects.filter((value) =>
        selectedIds.has(value.id),
      );
      assert.equal(ids.length, fixture.castleNodes);
      assert.deepEqual(
        [...new Set(objects.map((value) => value.sourceId))].sort(
          (a, b) => a - b,
        ),
        [...fixture.castleSourceIds].sort((a, b) => a - b),
      );
    }
    result.castleFixtures = {
      schema: castleFixtures.schema,
      sha256: hash(await readFile(process.env.AF_CASTLE_FIXTURES)),
      scenes: castleFixtures.scenes.map((value) => ({
        scene: value.scene,
        gameplayURL: value.gameplayURL,
        collisionURL: value.collisionURL,
        castleNodes: value.castleNodes,
      })),
    };
  }
  const selectedFeatures = (
    process.env.AF_PLAYER_PERFORMANCE_FEATURES ||
    [
      "render",
      "controller",
      "quest",
      "spells",
      "destruction",
      "castle",
      ...ENEMY_WORKLOADS,
    ].join(",")
  ).split(",");
  assert(
    selectedFeatures.length &&
      new Set(selectedFeatures).size === selectedFeatures.length &&
      selectedFeatures.every((value) =>
        [
          "render",
          "controller",
          "quest",
          "spells",
          "destruction",
          "castle",
          ...ENEMY_WORKLOADS,
        ].includes(value),
      ),
  );
  async function collect({
    id,
    route = "static",
    viewport,
    round,
    prepare,
    activity,
    prepareAfterWarmup = true,
  }) {
    await page.bringToFront();
    await prepare();
    await call("pause", false);
    await call("benchmark", {
      op: "start",
      route,
      instrumented: true,
      targetHz,
      capacity: 20000,
    });
    await page.waitForTimeout(warmupMs);
    await call("benchmark", { op: "stop", drain: true });
    if (prepareAfterWarmup) await prepare();
    const before = await status();
    const windowStart = await call("benchmark", {
      op: "start",
      route,
      instrumented: true,
      targetHz,
      capacity: 20000,
    });
    const action = activity
      ? await activity(durationMs)
      : await page.waitForTimeout(durationMs);
    const metrics = await call("benchmark", { op: "stop", drain: true });
    await page.waitForTimeout(180);
    const final = await call("benchmark", { op: "read", drain: true });
    const after = await status();
    const scenario = final.resources.environment.mode;
    const measuredMs =
      final.observation.stoppedAtMs - final.observation.startedAtMs;
    const pacing = evaluateFramePacing(final, {
      durationMs: measuredMs,
      warmupMs,
      mode,
      requireFullGpu: true,
    });
    const productCounters = {
      before: windowStart.resources.frameCounters,
      after: metrics.resources.frameCounters,
    };
    const applicationPacing = evaluateApplicationPacing(final, {
      warmupMs,
      mode,
      requireFullGpu: true,
      productCounters,
      browserMode: applicationBrowserMode(result.browserModeEvidence),
    });
    const summary = summarizeFrames(final.samples);
    summary.callbackScope = final.sampleScope ?? "legacy-callback-samples";
    summary.renderAttempts = final.samples.length;
    summary.rawCallbacks = final.callbackCounts?.rawCallbacks ?? null;
    summary.intentionalPacedCallbacks =
      final.callbackCounts?.intentionalPacedCallbacks ?? null;
    summary.targetHz = final.targetHz ?? "display";
    assert.equal(final.resources.renderer.width, viewport.width);
    assert.equal(final.resources.renderer.height, viewport.height);
    assert(
      final.samples.length > 1,
      "The live player produced no usable frame window.",
    );
    const path = resolve(
      output,
      `${profile}-pacing-window-${String(result.windows.length).padStart(3, "0")}.json`,
    );
    await save(path, {
      schema: "aethyrforge.pacing-window.v1",
      host: "player",
      profile,
      id,
      scenario,
      route,
      viewport,
      round,
      metrics: final,
      productCounters,
      browserModeEvidence: result.browserModeEvidence,
      applicationOptions: { mode, warmupMs, minHz: 60, requireFullGpu: true },
      activity: action ?? null,
    });
    const bytes = await readFile(path);
    result.windows.push({
      id,
      scenario,
      route,
      viewport,
      round,
      observationMs: measuredMs,
      summary,
      pacing,
      applicationPacing,
      productCounters,
      activity: action ?? null,
      before: {
        sourceIdentity: before.sourceIdentity,
        player: before.state.player,
        gameplay: boundedGameplay(before.state.gameplay),
      },
      after: {
        sourceIdentity: after.sourceIdentity,
        player: after.state.player,
        gameplay: boundedGameplay(after.state.gameplay),
      },
      resources: metrics.resources,
      rawEvidence: {
        path,
        sha256: hash(bytes),
        bytes: bytes.length,
        schema: "aethyrforge.pacing-window.v1",
      },
    });
    assert.equal(after.sourceIdentity, before.sourceIdentity);
    console.log(
      `PLAYER PACING ${profile}/${id}/${route}/${viewport.width}x${viewport.height}/${round}: completed=${pacing.summary.completionRateHz}Hz qualified=${pacing.qualified}`,
    );
  }
  const spawnEye = world?.spawn.position.map(
    (value, i) => value + (i === 1 ? 1.62 : 0),
  );
  const spawnTarget = spawnEye?.map((value, i) => value + (i === 2 ? -5 : 0));
  const spawn = async (mode = "fly") => {
    await call("restoreGameplay");
    await call("setCamera", spawnEye, spawnTarget, mode);
  };
  const sourceSpawn = async (scenario) => {
    await call("restoreGameplay");
    await call("referenceShot", scenario === "day" ? "MT-02" : "MT-11");
    await call(
      "setCamera",
      sourceScene.spawn.position,
      sourceScene.spawn.target,
      "fly",
    );
  };
  async function sampleSpell(duration, ability) {
    const before = await status();
    const started = performance.now();
    const cast = await call("cast", ability);
    const samples = [];
    while (performance.now() - started < duration) {
      const value = await status();
      samples.push({
        atMs: performance.now() - started,
        ...boundedGameplay(value.state.gameplay),
      });
      await page.waitForTimeout(
        Math.min(250, Math.max(0, duration - (performance.now() - started))),
      );
    }
    const after = await status();
    assert(
      after.state.gameplay.castCount > before.state.gameplay.castCount,
      "The real spell did not enter the Rust simulation.",
    );
    assert(
      samples.some(
        (value) => value.effects > 0 || value.debris > 0 || value.destroyed > 0,
      ),
      "The spell window contained no reported impact/effect/destruction.",
    );
    return { ability, castId: cast.gameplay?.castCount ?? null, samples };
  }
  try {
    if (profile === "empty") {
      for (const viewport of viewports) {
        await page.setViewportSize(viewport);
        for (let round = 0; round < repeats; round++) {
          await page.waitForTimeout(warmupMs);
          const samples = await page.evaluate(async (duration) => {
            const values = [];
            const start = performance.now();
            await new Promise((resolve) => {
              const tick = (now) => {
                values.push(now);
                if (now - start < duration) requestAnimationFrame(tick);
                else resolve();
              };
              requestAnimationFrame(tick);
            });
            return values;
          }, durationMs);
          const intervals = samples
            .slice(1)
            .map((value, index) => value - samples[index]);
          const cadence = distribution(intervals);
          const ok =
            samples.length >= Math.floor((durationMs * 60) / 1000) - 1 &&
            cadence.p95 <= 17.5 &&
            cadence.p99 <= 20 &&
            cadence.max <= 33.34 &&
            intervals.filter((value) => value > 25).length / intervals.length <=
              0.001;
          result.idleResponsiveness.push({
            viewport,
            round,
            callbacks: samples.length,
            durationMs,
            warmupMs,
            samples,
            cadence,
            ok,
            scope:
              "Browser animation-callback responsiveness of an idle single-clear player. No continuously rendered engine FPS claim.",
          });
        }
      }
      result.coverage = {
        emptyIdle: true,
        continuousRendering: "not-applicable",
      };
      result.ok = result.idleResponsiveness.every((row) => row.ok);
      result.applicationPerformance = {
        schema: "aethyrforge.application-matrix.v1",
        mode,
        qualified: false,
        browserMode: applicationBrowserMode(result.browserModeEvidence),
        compositorPresentation: null,
        physicalPresentation: null,
        scope:
          "Idle browser/UI responsiveness only; continuous application rendering is not applicable. The suite independently checks duration, warmup, viewports, and repeats.",
      };
      return result;
    }
    const initial = await status();
    assert.equal(
      initial.renderer.gpuTimingStatus?.scope,
      "render-compute-pass-envelope",
      "Build this player with VITE_PLAYER_DIAGNOSTICS=1 on an adapter with timestamp queries before full-GPU qualification.",
    );
    result.capabilities = {
      character: artifact.profile === "character",
      quest: Boolean(world.game),
      destruction: initial.state.gameplay.objectCount > 0,
      castle: (initial.state.gameplay.castle?.total ?? 0) > 0,
      enemies: (initial.state.gameplay.enemies?.total ?? 0) > 0,
    };
    if (result.capabilities.enemies) {
      assert(sourceScene.enemyCharacterURL && sourceScene.enemyPatrolsURL);
      const bytes = await readFile(
        resolve(directory, "." + sourceScene.enemyPatrolsURL),
      );
      const routes = JSON.parse(bytes.toString());
      assert.equal(routes.schema, "aethyrforge.enemy-patrols.v1");
      for (const scenario of ["day", "night"])
        assert.equal(routes.scenarios[scenario].length, 8);
      result.enemySource = {
        characterURL: sourceScene.enemyCharacterURL,
        patrolsURL: sourceScene.enemyPatrolsURL,
        patrolsSha256: hash(bytes),
        actorIds: Object.fromEntries(
          ["day", "night"].map((scenario) => [
            scenario,
            routes.scenarios[scenario].map((value) => value.id),
          ]),
        ),
      };
    }
    if (
      result.capabilities.destruction &&
      (selectedFeatures.includes("spells") ||
        selectedFeatures.includes("destruction"))
    )
      assert(
        fixtures,
        "A gameplay-enabled Town needs source-bound AF_DRUID_FIXTURES for spell/destruction qualification.",
      );
    for (const viewport of viewports) {
      await page.setViewportSize(viewport);
      await call("pause", false);
      await until(async () => {
        const state = await status();
        return (
          state.renderer.width === viewport.width &&
          state.renderer.height === viewport.height
        );
      }, "The player did not apply the declared resolution.");
      if (selectedFeatures.includes("render"))
        for (const shot of shots)
          for (const scenario of town && shot === "spawn" ? scenarios : [null])
            for (const route of routes)
              for (let round = 0; round < repeats; round++)
                await collect({
                  id: `view:${shot}`,
                  route,
                  viewport,
                  round,
                  prepare: () =>
                    town
                      ? shot === "spawn"
                        ? sourceSpawn(scenario)
                        : call("referenceShot", shot)
                      : spawn(),
                });
      if (selectedFeatures.includes("controller"))
        for (const perspective of artifact.profile === "character"
          ? ["first", "third"]
          : ["first"])
          for (let round = 0; round < repeats; round++)
            await collect({
              id: `controller:${perspective}`,
              viewport,
              round,
              prepare: async () => {
                await spawn("walk");
                await call("perspective", perspective);
                await page.waitForTimeout(400);
              },
              activity: async (duration) => {
                const start = performance.now();
                const samples = [];
                let sign = 1;
                try {
                  while (performance.now() - start < duration) {
                    await call("input", { right: sign * 0.5 });
                    sign *= -1;
                    const state = await status();
                    samples.push({
                      atMs: performance.now() - start,
                      position: state.state.player.position,
                      grounded: state.state.player.grounded,
                    });
                    await page.waitForTimeout(
                      Math.min(
                        400,
                        Math.max(0, duration - (performance.now() - start)),
                      ),
                    );
                  }
                } finally {
                  await call("clearInput");
                }
                return {
                  intentions: "Alternating half-strength lateral movement",
                  samples,
                };
              },
            });
      if (selectedFeatures.includes("quest") && world.game)
        for (let round = 0; round < repeats; round++)
          await collect({
            id: "quest:staged-key-exit-reset",
            viewport,
            round,
            prepare: () => spawn("walk"),
            activity: async (duration) => {
              const start = performance.now();
              let completed = 0;
              while (performance.now() - start < duration) {
                await call("reset");
                for (const point of [
                  world.game.key.position,
                  world.game.exit.position,
                ]) {
                  const eye = [point[0], point[1] + 1.62, point[2]];
                  await call(
                    "setCamera",
                    eye,
                    [eye[0], eye[1], eye[2] - 2],
                    "walk",
                  );
                  await call("interact");
                }
                assert(
                  (await status()).game.completed,
                  "The staged quest did not complete.",
                );
                completed++;
                await page.waitForTimeout(
                  Math.min(
                    600,
                    Math.max(0, duration - (performance.now() - start)),
                  ),
                );
              }
              return {
                completed,
                route:
                  "Staged admitted camera positions, not continuous navigation",
              };
            },
          });
      if (result.capabilities.enemies) {
        const adapter = {
          wait: (ms) => page.waitForTimeout(ms),
          state: async () => {
            const value = await status();
            return {
              enemies: value.state.gameplay.enemies,
              renderer: value.enemyRender,
              revision: value.state.revision,
              player: value.state.player.position,
              gameplay: {
                castCount: value.state.gameplay.castCount,
                destroyedCount: value.state.gameplay.destroyedCount,
                effects: value.state.gameplay.effects.length,
                debris: value.state.gameplay.debris.length,
                pendingImpacts: value.state.gameplay.pendingImpacts,
                activeCast: Boolean(value.state.gameplay.activeCast),
              },
            };
          },
          restore: () => call("restoreGameplay"),
          point: (target, eye) => call("setCamera", eye, target, "fly"),
          cast: async (ability, expected = true) => {
            if (expected) {
              const value = await call("cast", ability);
              return { ok: true, castCount: value.gameplay?.castCount ?? null };
            }
            let error;
            try {
              await call("cast", ability);
            } catch (value) {
              error = String(value);
            }
            assert(
              error,
              "A real sky miss was accepted by the published player.",
            );
            return { ok: false, message: error };
          },
        };
        for (const scenario of scenarios)
          for (const feature of ENEMY_WORKLOADS.filter((value) =>
            selectedFeatures.includes(value),
          ))
            for (let round = 0; round < repeats; round++)
              await collect({
                id: `${feature}:${scenario}`,
                viewport,
                round,
                prepare: async () => {
                  await call("restoreGameplay");
                  await call("scenario", scenario);
                  await call(
                    "referenceShot",
                    scenario === "day" ? "MT-02" : "MT-11",
                  );
                  const enemies = (await status()).state.gameplay.enemies;
                  assert.equal(enemies.total, 8);
                  assert.deepEqual(
                    enemies.actors.map((actor) => actor.id),
                    result.enemySource.actorIds[scenario],
                  );
                },
                activity: (duration) =>
                  runEnemyWorkload(adapter, feature, duration),
              });
      }
      if (result.capabilities.destruction && fixtures) {
        await call("restoreGameplay");
        await call("scenario", "day");
        if (selectedFeatures.includes("spells"))
          for (const ability of [
            "lightning_bolt",
            "meteor_shower",
            "thunderbolt",
          ]) {
            const probe = fixtures.abilities[ability];
            assert(probe, `Missing ${ability} source fixture.`);
            for (let round = 0; round < repeats; round++)
              await collect({
                id: `spell:${ability}`,
                viewport,
                round,
                prepare: async () => {
                  await call("restoreGameplay");
                  await call("setCamera", probe.eye, probe.target, "fly");
                  const aim = await call("aim");
                  assert.equal(
                    aim?.nodeId,
                    probe.nodeId,
                    "The source-bound spell ray no longer selects its expected cell.",
                  );
                },
                activity: (duration) => sampleSpell(duration, ability),
              });
          }
        if (selectedFeatures.includes("destruction")) {
          const probe = fixtures.abilities.lightning_bolt;
          for (let round = 0; round < repeats; round++)
            await collect({
              id: "destruction:persistent-settled-rubble",
              viewport,
              round,
              prepare: async () => {
                await call("restoreGameplay");
                await call("setCamera", probe.eye, probe.target, "fly");
                for (
                  let n = 0;
                  n < (probe.maxCasts || 4) &&
                  !(await status()).state.gameplay.destroyedCount;
                  n++
                ) {
                  await call("cast", "lightning_bolt");
                  await page.waitForTimeout(500);
                }
                await until(async () => {
                  const state = (await status()).state.gameplay;
                  return (
                    state.destroyedCount > 0 &&
                    state.debris.length === 0 &&
                    state.effects.length === 0 &&
                    !state.pendingImpacts
                  );
                }, "The damaged source did not settle before the persistent-rubble window.");
              },
            });
        }
      }
      if (result.capabilities.castle && selectedFeatures.includes("castle")) {
        assert(
          castleFixtures,
          "A Castle-enabled artifact needs exact source-bound AF_CASTLE_FIXTURES.",
        );
        for (const fixture of castleFixtures.scenes) {
          const scenario = fixture.scene === "scene.json" ? "day" : "night";
          const setup = async () => {
            await call("restoreGameplay");
            await call("scenario", scenario);
            await call("referenceShot", scenario === "day" ? "MT-02" : "MT-11");
            await call(
              "setCamera",
              fixture.overview.eye,
              fixture.overview.target,
              "fly",
            );
            assert.equal(
              (await status()).state.gameplay.castle.total,
              fixture.castleNodes,
            );
          };
          for (let round = 0; round < repeats; round++) {
            await collect({
              id: `castle:${scenario}:prepare-active`,
              viewport,
              round,
              prepare: setup,
              activity: (duration) => sampleSpell(duration, "destroy_castle"),
            });
            await collect({
              id: `castle:${scenario}:cancel-and-restore`,
              viewport,
              round,
              prepare: setup,
              activity: async (duration) => {
                const started = performance.now();
                const phases = [];
                const mark = async (phase) =>
                  phases.push({
                    phase,
                    at: await page.evaluate(() => performance.now()),
                    state: boundedGameplay((await status()).state.gameplay),
                  });
                await mark("preparation-start");
                const casting = call("cast", "destroy_castle").then(
                  (value) => ({
                    ok: true,
                    value: boundedGameplay(value.gameplay),
                  }),
                  (error) => ({ ok: false, error: String(error) }),
                );
                await page.waitForTimeout(50);
                await call("cancelCast");
                const canceled = await casting;
                await mark("preparation-canceled");
                await call("restoreGameplay");
                await call("cast", "destroy_castle");
                await until(
                  async () => (await status()).state.gameplay.castle.active,
                  "The real Castle barrage never became active.",
                );
                await mark("barrage-active");
                await page.waitForTimeout(500);
                await call("cancelCast");
                await mark("barrage-canceled");
                await call("restoreGameplay");
                await mark("restored");
                assert.equal((await status()).state.gameplay.destroyedCount, 0);
                await page.waitForTimeout(
                  Math.max(0, duration - (performance.now() - started)),
                );
                return { canceled, phases };
              },
            });
            await collect({
              id: `castle:${scenario}:complete-settled-rubble`,
              viewport,
              round,
              prepareAfterWarmup: false,
              prepare: async () => {
                await setup();
                await call("cast", "destroy_castle");
                await until(
                  async () => {
                    const value = (await status()).state.gameplay;
                    return (
                      value.castle.complete &&
                      !value.castle.active &&
                      !value.pendingImpacts &&
                      value.debris.length === 0 &&
                      value.effects.length === 0
                    );
                  },
                  "The complete Castle destruction did not settle.",
                  180000,
                );
                const value = (await status()).state.gameplay;
                assert.equal(value.castle.remaining, 0);
                assert(
                  value.castle.rubbleCount >=
                    fixture.castleFragments.minimumPersistent,
                  "The measured completed Castle has no expected persistent source rubble.",
                );
              },
            });
          }
        }
      }
    }
    result.coverage = {
      sourceViews: !town || galleryIds.every((id) => shots.includes(id)),
      townSpawn:
        !town ||
        ["day", "night"].every((scenario) =>
          ["static", "sway"].every((route) =>
            ["800x500", "1280x720"].every(
              (viewport) =>
                new Set(
                  result.windows
                    .filter(
                      (row) =>
                        row.id === "view:spawn" &&
                        row.scenario === scenario &&
                        row.route === route &&
                        `${row.viewport.width}x${row.viewport.height}` ===
                          viewport,
                    )
                    .map((row) => row.round),
                ).size >= 2,
            ),
          ),
        ),
      viewports: ["800x500", "1280x720"].every((id) =>
        viewports.some((value) => `${value.width}x${value.height}` === id),
      ),
      repeats: repeats >= 2,
      routes: ["static", "sway"].every((value) => routes.includes(value)),
      features: [
        "render",
        "controller",
        "quest",
        "spells",
        "destruction",
      ].every((value) => selectedFeatures.includes(value)),
      enemies:
        !town ||
        (result.capabilities.enemies &&
          ENEMY_WORKLOADS.every((value) => selectedFeatures.includes(value))),
      castle:
        !result.capabilities.castle ||
        Boolean(castleFixtures && selectedFeatures.includes("castle")),
    };
    if (result.capabilities.castle && !result.coverage.castle)
      result.limitations.push(
        "This artifact includes a castle, but no source-bound castle performance fixture was provided. All-product coverage remains incomplete.",
      );
    result.queueQualified =
      mode === "qualification" &&
      Object.values(result.coverage).every(Boolean) &&
      result.windows.every((value) => value.pacing.queueQualified);
    result.qualified =
      result.queueQualified &&
      result.windows.every((value) => value.pacing.qualified);
    result.applicationPerformance = {
      schema: "aethyrforge.application-matrix.v1",
      mode,
      qualified:
        mode === "qualification" &&
        Object.values(result.coverage).every(Boolean) &&
        result.windows.length > 0 &&
        result.windows.every((row) => row.applicationPacing.windowQualified),
      coverage: { ...result.coverage },
      browserMode: applicationBrowserMode(result.browserModeEvidence),
      compositorPresentation: null,
      physicalPresentation: null,
      failedWindows: result.windows
        .filter((row) => !row.applicationPacing.ok)
        .map((row) => ({
          id: row.id,
          scenario: row.scenario,
          route: row.route,
          viewport: row.viewport,
          round: row.round,
          checks: row.applicationPacing.checks.filter(
            (check) => check.required && !check.ok,
          ),
        })),
      scope:
        "Local published-player application matrix. The outer artifact/quality/cleanup report and full product suite are additionally required; strict v1 flags are unchanged.",
    };
    result.ok =
      mode === "diagnostic"
        ? result.windows.every((value) => value.pacing.ok)
        : result.qualified;
    return result;
  } finally {
    await call("benchmark", { op: "stop", drain: true }).catch(() => {});
    await call("clearInput").catch(() => {});
    await call("restoreGameplay").catch(() => {});
    if (town) await call("referenceShot", "MT-02").catch(() => {});
    await call("pause", true).catch(() => {});
    await page.setViewportSize(originalViewport);
    await save(resolve(output, `${profile}-player-pacing.json`), result);
  }
}
function boundedGameplay(value) {
  return {
    castCount: value?.castCount ?? 0,
    destroyed: value?.destroyedCount ?? 0,
    debris: value?.debris?.length ?? 0,
    effects: value?.effects?.length ?? 0,
    pendingImpacts: value?.pendingImpacts ?? 0,
    castleActive: value?.castle?.active ?? false,
    enemies: value?.enemies
      ? {
          total: value.enemies.total,
          alive: value.enemies.alive,
          killed: value.enemies.killed,
        }
      : null,
    castle: value?.castle
      ? {
          total: value.castle.total,
          remaining: value.castle.remaining,
          destroyed: value.castle.destroyed,
          complete: value.castle.complete,
          active: value.castle.active,
          rubbleCount: value.castle.rubbleCount,
        }
      : null,
  };
}
