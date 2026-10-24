/** Record a live battle canvas with wall-clock timestamps. Never uses offline film capture. */
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { randomUUID, createHash } from "node:crypto";
import { createWriteStream } from "node:fs";
import { Transform } from "node:stream";
import { pipeline } from "node:stream/promises";
import {
  mkdir,
  realpath,
  writeFile,
  rename,
  readFile,
  stat,
} from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { distribution } from "../experiments/c-town/measurement.mjs";
import { evaluateFramePacing } from "./performance-contract.mjs";
import {
  battleParticipationPlan,
  battleParticipantEvidence,
  NEUTRAL_BATTLE_INPUT,
  battleRecordingLook,
} from "./battle-shot-plan.mjs";
import {
  battleStatusFreshness,
  battleCombatProgress,
  battleSourcePin,
  battleAssetPin,
} from "./battle-evidence.mjs";
import {
  evaluateApplicationPacing,
  observeApplicationBrowserMode,
} from "./application-pacing.mjs";
function battleOverflowFidelity(before, after, samples) {
  const checks = [
    "gridOverflows",
    "gridOverflowsTotal",
    "projectileOverflows",
  ].map((key) => {
    const first = before?.gpu?.[key],
      last = after?.gpu?.[key];
    const readings = samples.map((sample) => sample.battle?.stats[key]);
    const valid = [first, last, ...readings].every(
      (n) => Number.isSafeInteger(n) && n >= 0,
    );
    const delta = valid ? last - first : null;
    const sampledMaximum =
      valid && readings.length ? Math.max(...readings) : null;
    return {
      id: `${key}.no-loss`,
      before: first ?? null,
      after: last ?? null,
      delta,
      sampledMaximum,
      ok:
        valid &&
        readings.length > 0 &&
        delta === 0 &&
        (key === "gridOverflows"
          ? first === 0 && last === 0 && sampledMaximum === 0
          : sampledMaximum <= first),
    };
  });
  return {
    ok: checks.every((check) => check.ok),
    checks,
    scope:
      "Cumulative grid and projectile counters cover all simulation steps between the direct probes bracketing the timed shot and drain. Grid gauges are also sampled during the shot. Grid overflow omits neighbors, and projectile overflow drops shots.",
  };
}

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set a private AF_CAPTURE_DIR.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep video and evidence outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run through openagents browser run.");
const url = new URL(process.env.AF_EDITOR_URL || "http://127.0.0.1:55316/");
assert(
  ["http:", "https:"].includes(url.protocol) &&
    ["127.0.0.1", "localhost"].includes(url.hostname) &&
    !url.username &&
    !url.password,
);
const probe = process.env.AF_BATTLE_RECORD_PROBE === "1",
  count = Number(process.env.AF_BATTLE_COUNT || 10000),
  seconds = Number(process.env.AF_BATTLE_RECORD_SECONDS || (probe ? 2 : 20)),
  view = process.env.AF_BATTLE_VIEW || "orbit",
  route = process.env.AF_BATTLE_RECORD_ROUTE || "static";
const instrumentationFlag = process.env.AF_BATTLE_RECORD_INSTRUMENTED ?? "0";
assert(
  ["0", "1"].includes(instrumentationFlag),
  "Set AF_BATTLE_RECORD_INSTRUMENTED to 0 or 1.",
);
const instrumented = instrumentationFlag === "1";
const captureHz = Number(process.env.AF_BATTLE_CAPTURE_HZ || 60);
const viewportWidth = Number(process.env.AF_BATTLE_WIDTH || 1280),
  viewportHeight = Number(process.env.AF_BATTLE_HEIGHT || 720);
assert(
  Number.isSafeInteger(viewportWidth) &&
    viewportWidth >= 640 &&
    viewportWidth <= 3840,
);
assert(
  Number.isSafeInteger(viewportHeight) &&
    viewportHeight >= 360 &&
    viewportHeight <= 2160,
);
assert(
  [60, 120].includes(captureHz),
  "Choose a 60 or 120 Hz capture request rate.",
);
assert(Number.isSafeInteger(count) && count >= 100 && count <= 1000000);
assert(Number.isFinite(seconds) && seconds >= 2 && seconds <= 60);
assert(
  ["overview", "orbit", "player", "first", "third", "dolly"].includes(view),
);
assert(
  ["static", "orbit-player", "participation"].includes(route),
  "Set AF_BATTLE_RECORD_ROUTE to static, orbit-player, or participation.",
);
const initialView = route === "orbit-player" ? "orbit" : view;
const participantClass = process.env.AF_BATTLE_CLASS || "paladin";
const attackKind = process.env.AF_BATTLE_ATTACK_KIND || "melee";
const respawnFlag = process.env.AF_BATTLE_RECORD_RESPAWN ?? "1";
const lookFlag = process.env.AF_BATTLE_RECORD_LOOK ?? "0";
assert(["paladin", "sorceror"].includes(participantClass));
assert(["melee", "ranged"].includes(attackKind));
assert(["0", "1"].includes(respawnFlag));
assert(["0", "1"].includes(lookFlag));
assert(
  lookFlag === "0" || participatingView(initialView),
  "Camera sweeps require first or third person.",
);
function participatingView(value) {
  return value === "first" || value === "third";
}
assert(
  route !== "participation" || ["first", "third"].includes(initialView),
  "Participation shots use first or third person.",
);
const participating = ["first", "third"].includes(initialView);
const inputPlan =
  route === "participation" ? battleParticipationPlan(seconds) : [];
const WARMUP_MS = 5000,
  MAX_BYTES = 256 * 1024 * 1024,
  token = randomUUID();
const report = {
  schema: "aethyrforge.battle-live-recording.v1",
  startedAt: new Date().toISOString(),
  ok: false,
  qualified: false,
  url: url.href,
  probe,
  count,
  view: initialView,
  requestedView: view,
  route,
  seconds,
  configuration: {
    viewport: [viewportWidth, viewportHeight],
    deviceScaleFactor: 1,
    instrumented,
    targetHz: "display",
    route,
    initialView,
    warmupMs: WARMUP_MS,
    captureRequestHz: captureHz,
    playerCutFraction: route === "orbit-player" ? 0.6 : null,
    participantClass: participating ? participantClass : null,
    attackKind: participating ? attackKind : null,
    respawnOnDeath: participating && respawnFlag === "1",
    cameraLookSweep: lookFlag === "1",
    cameraInitialPitchInput:
      lookFlag === "1" ? (initialView === "third" ? 0.17 : -0.02) : 0,
    inputPlan,
  },
  recordingMethod:
    "Live transferred preview canvas captureStream + MediaRecorder; native wall-clock timestamps; no synthetic frames or timestamp rewriting",
  windows: [],
  captures: [],
  errors: [],
  console: [],
  failed: [],
  limitations: [
    "Canvas video excludes the React HUD. Browser/codec selection does not prove hardware acceleration.",
    "Render submissions, video frames and physical display presentation are separate observations.",
    "The requested pool size is the simulated population. Drawn counts are observed separately for each camera and can be a subset in the close player view.",
    "Direct combat probes bracket the timed window and include bridge/drain time outside it. Timed status samples and participant events retain their own clocks. A single recorded cohort never establishes sustained qualification.",
    instrumented
      ? "CPU stages and GPU timestamp spans are instrumented in both comparison windows and add measurement overhead."
      : "Both comparison windows use the production rendering path with CPU stage and GPU timestamp instrumentation disabled. Missing GPU timings are reported as unavailable, without a GPU-budget claim.",
  ],
};
const browser = await chromium.connectOverCDP(endpoint);
report.browserMode = await observeApplicationBrowserMode(browser);
const context = await browser.newContext({
  viewport: { width: viewportWidth, height: viewportHeight },
  deviceScaleFactor: 1,
  serviceWorkers: "block",
});
const page = await context.newPage();
page.setDefaultTimeout(20000);
let closing = false,
  measuring = false,
  frame,
  previewOrigin,
  videoPath,
  partialPath,
  recording,
  received = false,
  server;
page.on("pageerror", (e) => report.errors.push(String(e)));
page.on("console", (m) => {
  if (["warning", "error"].includes(m.type()))
    report.console.push({ type: m.type(), text: m.text().slice(0, 4096) });
});
page.on("requestfailed", (r) => {
  if (!closing)
    report.failed.push({ url: r.url(), error: r.failure()?.errorText });
});
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const status = async () => (await snapshot()).status;
const benchmark = (options) =>
  page.evaluate(
    (options) => window.aethyrforge.cTownBenchmark(options),
    options,
  );
async function until(predicate, label, timeout = 15000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    const s = await status();
    if (s.phase === "error") throw Error(s.message);
    const r = await predicate(s);
    if (r) return r === true ? s : r;
    await sleep(80);
  }
  throw Error(`${label}: ${JSON.stringify(await status())}`);
}
async function action(action) {
  const id = await page.evaluate((a) => window.aethyrforge.battle(a), action);
  const result = await until(
    async () => (await snapshot()).receipts.find((r) => r.requestId === id),
    "The battle action did not finish",
    30000,
  );
  assert.equal(result.ok, true, result.message);
  return result;
}
async function prepareCohort() {
  const resetReceipt = await action({ op: "reset" });
  let joinReceipt;
  if (participating) {
    joinReceipt = await action({
      op: "join",
      class: participantClass,
      team: 0,
      view: initialView,
    });
    await action({ op: "weapon", attackKind });
    await action({ op: "input", ...NEUTRAL_BATTLE_INPUT, attackKind });
  }
  await action({ op: "camera", view: initialView });
  const warmupStarted = performance.now();
  await sleep(WARMUP_MS);
  const warmupReady = await until(
    (s) =>
      s.phase === "ready" &&
      s.battle?.enabled &&
      s.battle.count === count &&
      s.battle.camera === initialView &&
      Number.isSafeInteger(s.battle.stats.sampleSerial) &&
      s.battle.stats.sampleSerial > 0 &&
      Number.isSafeInteger(s.battle.stats.simulationTicks) &&
      s.battle.stats.simulationTicks > 0 &&
      (!participating || s.battle.participant?.active === true),
    "The recording camera and fresh GPU counters did not become ready",
  );
  const warmup = {
    plannedMs: WARMUP_MS,
    observedMs: performance.now() - warmupStarted,
    frame: warmupReady.frame,
    sampleSerial: warmupReady.battle.stats.sampleSerial,
    simulationTicks: warmupReady.battle.stats.simulationTicks,
    participant: warmupReady.battle.participant ?? null,
    participantEpoch: warmupReady.battle.stats.participantEpoch ?? null,
  };
  const probeRequestedEpochMs = performance.timeOrigin + performance.now();
  const beforeProbeMetrics = await benchmark({ op: "readBattle" });
  const probeReceivedEpochMs = performance.timeOrigin + performance.now();
  let ready = warmupReady,
    respawn = null;
  if (participating) {
    const previousEpoch = warmupReady.battle.stats.participantEpoch;
    assert(
      Number.isSafeInteger(previousEpoch) && previousEpoch >= 0,
      "The warmed-up participant GPU epoch is unavailable.",
    );
    const requestedEpochMs = performance.timeOrigin + performance.now();
    const receipt = await action({ op: "respawn" });
    ready = await until(
      (s) =>
        s.phase === "ready" &&
        s.battle?.enabled &&
        s.battle.count === count &&
        s.battle.camera === initialView &&
        s.battle.participant?.active === true &&
        s.battle.participant.ready === true &&
        s.battle.participant.alive === true &&
        s.battle.participant.health > 0 &&
        Number.isSafeInteger(s.battle.stats.participantEpoch) &&
        s.battle.stats.participantEpoch > previousEpoch,
      "The fresh participant life was not acknowledged by the GPU",
    );
    respawn = {
      receipt,
      requestedEpochMs,
      acknowledgedEpochMs: performance.timeOrigin + performance.now(),
      previousGpuEpoch: previousEpoch,
      acknowledgedGpuEpoch: ready.battle.stats.participantEpoch,
      participant: ready.battle.participant,
      frame: ready.frame,
      scope:
        "A real Respawn after camera warmup; ready requires the worker's reset-epoch acknowledgement, plus a newer observed GPU epoch and a live positive-health participant.",
    };
  }
  const cameraInput =
    lookFlag === "1"
      ? await action({
          op: "input",
          ...NEUTRAL_BATTLE_INPUT,
          attackKind,
          lookY: initialView === "third" ? 0.17 : -0.02,
        })
      : null;
  return {
    resetReceipt,
    cameraInput,
    joinReceipt: joinReceipt ?? null,
    warmup,
    freshLife: respawn,
    beforeProbe: beforeProbeMetrics.battleProbe,
    beforeProbeClock: {
      probeRequestedEpochMs,
      probeReceivedEpochMs,
      scope:
        "Direct global combat probe after warmup and before the fresh participant life. Cumulative combat counters survive Respawn; the reset-epoch acknowledgement follows this probe.",
    },
    warmupMs: WARMUP_MS,
    observedWarmupMs: warmup.observedMs,
    camera: ready.battle.camera,
    count: ready.battle.count,
    sampleSerial: ready.battle.stats.sampleSerial,
    simulationTicks: ready.battle.stats.simulationTicks,
    frame: ready.frame,
    playerPosition: ready.mmorpg?.player?.position ?? null,
    participant: ready.battle.participant ?? null,
  };
}
async function measure(label, setup, media) {
  const beforeProbe = setup.beforeProbe;
  const mediaStart = await media?.start();
  const startRequestedEpochMs = performance.timeOrigin + performance.now();
  const before = await benchmark({
    op: "start",
    route: "static",
    capacity: 20000,
    instrumented,
    targetHz: "display",
  });
  const startReceivedEpochMs = performance.timeOrigin + performance.now();
  measuring = true;
  const start = performance.now(),
    samples = [],
    transitions = [],
    inputEvents = [],
    participantEvents = [];
  let lastInputAt = -Infinity,
    lastLookElapsed = 0,
    priorAlive;
  const cutAtMs = route === "orbit-player" ? seconds * 1000 * 0.6 : null;
  while (performance.now() - start < seconds * 1000) {
    if (
      cutAtMs !== null &&
      transitions.length === 0 &&
      performance.now() - start >= cutAtMs
    ) {
      const requestedAtMs = performance.now() - start;
      const receipt = await action({ op: "camera", view: "player" });
      const shown = await until(
        (s) => s.battle?.camera === "player",
        "The player camera did not activate",
      );
      transitions.push({
        from: "orbit",
        to: "player",
        plannedAtMs: cutAtMs,
        requestedAtMs,
        observedAtMs: performance.now() - start,
        frame: shown.frame,
        playerPosition: shown.mmorpg?.player?.position ?? null,
        battle: shown.battle,
        receipt,
        clock: "Node performance.now relative to the benchmark-start receipt",
      });
    }
    const s = await status();
    assert.equal(s.phase, "ready");
    assert.equal(s.battle?.count, count);
    const actor = s.battle?.participant;
    if (participating) {
      assert(actor?.active, "The local battle participant became inactive.");
      if (priorAlive !== undefined && actor.alive !== priorAlive)
        participantEvents.push({
          atMs: performance.now() - start,
          kind: actor.alive ? "alive" : "death",
          participant: structuredClone(actor),
        });
      priorAlive = actor.alive;
      if (!actor.alive && respawnFlag === "1") {
        const atMs = performance.now() - start;
        const receipt = await action({ op: "respawn" });
        participantEvents.push({
          atMs,
          kind: "respawn-request",
          receipt,
          participant: structuredClone(actor),
        });
      }
      if (performance.now() - start - lastInputAt >= 250) {
        const elapsed = performance.now() - start;
        const phase = inputPlan.findLast((phase) => phase.atMs <= elapsed);
        let input = actor.alive
          ? (phase?.input ?? NEUTRAL_BATTLE_INPUT)
          : NEUTRAL_BATTLE_INPUT;
        if (lookFlag === "1" && actor.alive)
          input = {
            ...input,
            ...battleRecordingLook(
              seconds,
              lastLookElapsed,
              elapsed,
              initialView,
            ),
          };
        lastLookElapsed = elapsed;
        const receipt = await action({ op: "input", ...input, attackKind });
        lastInputAt = performance.now() - start;
        inputEvents.push({
          requestedAtMs: elapsed,
          receivedAtMs: lastInputAt,
          input,
          receipt,
          participant: structuredClone(actor),
        });
      }
    }
    samples.push({
      at: performance.now() - start,
      frame: s.frame,
      battle: s.battle,
      fps: s.fps,
    });
    const elapsed = performance.now() - start;
    const untilCut =
      cutAtMs !== null && transitions.length === 0 ? cutAtMs - elapsed : 250;
    await sleep(Math.max(1, Math.min(250, untilCut, seconds * 1000 - elapsed)));
  }
  const mediaStop = await media?.stop();
  const stopRequestedEpochMs = performance.timeOrigin + performance.now();
  const after = await benchmark({ op: "stop", drain: true });
  const stopReceivedEpochMs = performance.timeOrigin + performance.now();
  measuring = false;
  if (participating)
    await action({ op: "input", ...NEUTRAL_BATTLE_INPUT, attackKind });
  const afterProbeMetrics = await benchmark({ op: "readBattle" });
  const afterProbe = afterProbeMetrics.battleProbe;
  const statusFreshness = battleStatusFreshness(samples, {
    stoppedSimulationSteps: after.resources.battle?.stats.simulationSteps,
    directAfterTick: afterProbe?.tick,
  });
  const combatProgress = battleCombatProgress(beforeProbe, afterProbe, count);
  const overflowFidelity = battleOverflowFidelity(
    beforeProbe,
    afterProbe,
    samples,
  );
  const effectCounters = [
    "rangedShots",
    "projectileImpacts",
    "projectileHits",
    "projectileOverflows",
    "siegeShots",
    "siegeHits",
    "participantHits",
    "participantKills",
    "participantAttacks",
    "participantDamage",
    "participantMeleeHits",
  ];
  const timedCombatEffects = Object.fromEntries(
    effectCounters.map((key) => {
      const first = samples[0]?.battle.stats[key],
        last = samples.at(-1)?.battle.stats[key];
      return [
        key,
        {
          first: first ?? null,
          last: last ?? null,
          delta:
            Number.isFinite(first) && Number.isFinite(last)
              ? last - first
              : null,
        },
      ];
    }),
  );
  assert.equal(
    transitions.length,
    cutAtMs === null ? 0 : 1,
    "The configured live camera route did not complete.",
  );
  const observedMs =
    after.observation.stoppedAtMs - after.observation.startedAtMs;
  const productCounters = {
    before: before.resources.frameCounters,
    after: after.resources.frameCounters,
  };
  const options = {
    mode: "diagnostic",
    durationMs: observedMs,
    warmupMs: WARMUP_MS,
    minHz: 60,
    requireFullGpu: instrumented,
  };
  const row = {
    label,
    instrumented,
    route,
    setup,
    viewTransitions: transitions,
    inputEvents,
    participantEvents,
    participantEvidence: participating
      ? {
          ...battleParticipantEvidence(samples, participantEvents),
          first: samples[0]?.battle.participant ?? null,
          last: samples.at(-1)?.battle.participant ?? null,
          positions: samples.map((sample) => ({
            atMs: sample.at,
            participant: sample.battle.participant,
          })),
          scope:
            "Actual local participant snapshots during this timed shot; respawns and input receipts are explicit events. No participant pose or health was injected.",
        }
      : null,
    clocks: {
      node: {
        startRequestedEpochMs,
        startReceivedEpochMs,
        stopRequestedEpochMs,
        stopReceivedEpochMs,
      },
      mediaStart: mediaStart?.clock ?? null,
      mediaStop: mediaStop ?? null,
      offsets: mediaStart
        ? {
            benchmarkStartRequestAfterMediaStartMs:
              startRequestedEpochMs - mediaStart.clock.startRequestedEpochMs,
            benchmarkStartReceiptAfterMediaStartMs:
              startReceivedEpochMs - mediaStart.clock.startRequestedEpochMs,
            mediaStopAfterBenchmarkStartReceiptMs:
              mediaStop.epochMs - startReceivedEpochMs,
            benchmarkStopRequestAfterMediaStopMs:
              stopRequestedEpochMs - mediaStop.epochMs,
          }
        : null,
      benchmarkWorker: after.observation,
      scope:
        "Media uses the preview performance clock plus its timeOrigin. Benchmark worker times are monotonic in a separate clock; Node epoch request/receipt brackets bound bridge latency, rather than claiming exact frame alignment.",
    },
    beforeProbe,
    afterProbe,
    statusFreshness,
    combatProgress,
    overflowFidelity,
    timedCombatEffects,
    combatEffectsScope:
      "First/last asynchronously sampled GPU counters inside the timed shot; these exclude later direct-probe advancement. Full probes separately retain sampled real projectile and participant state. Gauge counts such as active projectiles are retained in statusSamples.",
    observedMs,
    productCounters,
    pacing: evaluateFramePacing(after, options),
    applicationPacing: evaluateApplicationPacing(after, {
      ...options,
      productCounters,
      browserMode: report.browserMode.mode,
    }),
    populations: Object.fromEntries(
      ["alive", "dead", "visible", "moving", "attacking"].map((k) => [
        k,
        distribution(samples.map((s) => s.battle.stats[k])),
      ]),
    ),
    populationsByView: Object.fromEntries(
      [...new Set(samples.map((s) => s.battle.camera))].map((camera) => [
        camera,
        Object.fromEntries(
          ["alive", "dead", "visible", "moving", "attacking", "triangles"].map(
            (key) => [
              key,
              distribution(
                samples
                  .filter((s) => s.battle.camera === camera)
                  .map((s) => s.battle.stats[key]),
              ),
            ],
          ),
        ),
      ]),
    ),
    statusSamples: samples,
    metrics: after,
  };
  report.windows.push(row);
  await writeFile(
    resolve(output, `recording-${label}-window.json`),
    JSON.stringify(row, null, 2) + "\n",
  );
  assert(
    statusFreshness.ok,
    `Recorded population samples are stale or incomplete: ${JSON.stringify(statusFreshness)}`,
  );
  assert(
    combatProgress.ok,
    `Direct GPU probes do not show complete advancing combat: ${JSON.stringify(combatProgress)}`,
  );
  assert(
    overflowFidelity.ok,
    `The recording lost neighbors or projectile shots: ${JSON.stringify(overflowFidelity)}`,
  );
  if (route === "participation")
    assert(
      row.participantEvidence.activeParticipation,
      `The local participant did not both travel and attack during the shot: ${JSON.stringify(row.participantEvidence)}`,
    );
  if (route === "participation")
    assert(
      (timedCombatEffects.participantAttacks.delta ?? 0) > 0,
      "The GPU did not report a real local-participant attack during the shot.",
    );
  console.log(`Completed ${label} live window`);
  return row;
}
try {
  report.sourcePin = await battleSourcePin(root);
  if (!probe)
    report.assetPin = await battleAssetPin(
      url.href,
      process.env.AF_BATTLE_ASSET_ROOT,
    );
  await page.goto(url.href);
  await until(
    (s) => s.phase === "ready" && s.mmorpg,
    "The grass world did not open",
    150000,
  );
  frame = await (
    await page.locator("#preview-frame").elementHandle()
  ).contentFrame();
  assert(frame);
  previewOrigin = new URL(frame.url()).origin;
  if (!probe) {
    await page
      .locator("#mmo-name")
      .fill(`Film-${Date.now().toString(36).slice(-6)}`);
    await page
      .getByRole("button", { name: "Enter world", exact: true })
      .click();
    await until(
      (s) => s.mmorpg?.connection === "online",
      "The Paladin did not join",
    );
    await page
      .locator(".mmo-login")
      .waitFor({ state: "detached", timeout: 3000 });
    await action({ op: "start", count, seed: 1 });
    await action({ op: "camera", view: initialView });
    await measure("unrecorded", await prepareCohort());
  }
  server = createServer(async (req, res) => {
    if (req.headers.origin !== previewOrigin || req.url !== `/${token}`) {
      res.writeHead(403);
      res.end();
      return;
    }
    res.setHeader("Access-Control-Allow-Origin", previewOrigin);
    res.setHeader("Vary", "Origin");
    if (req.method === "OPTIONS") {
      res.setHeader("Access-Control-Allow-Methods", "POST");
      res.setHeader("Access-Control-Allow-Headers", "Content-Type");
      res.writeHead(204);
      res.end();
      return;
    }
    if (req.method !== "POST" || received || !partialPath) {
      res.writeHead(409);
      res.end();
      return;
    }
    received = true;
    const sink = createWriteStream(partialPath, { flags: "wx" });
    let bytes = 0;
    try {
      await pipeline(
        req,
        new Transform({
          transform(chunk, _encoding, done) {
            bytes += chunk.length;
            done(
              bytes > MAX_BYTES
                ? Error("The live video exceeded 256 MiB")
                : null,
              chunk,
            );
          },
        }),
        sink,
      );
      assert(bytes > 0);
      await rename(partialPath, videoPath);
      report.uploadBytes = bytes;
      res.writeHead(204);
      res.end();
    } catch (error) {
      sink.destroy();
      report.errors.push(String(error));
      if (!res.destroyed) {
        res.writeHead(500);
        res.end("The video could not be saved.");
      }
    }
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const upload = `http://127.0.0.1:${server.address().port}/${token}`;
  const recordedSetup = !probe ? await prepareCohort() : undefined;
  const recordingPlan = await frame.evaluate(() => {
    const mimeType = [
      "video/mp4;codecs=avc1.64002A",
      "video/mp4",
      "video/webm;codecs=vp8",
      "video/webm",
    ].find((value) => MediaRecorder.isTypeSupported(value));
    if (!mimeType)
      throw Error("This browser has no supported live video codec.");
    return { mimeType };
  });
  const startRecording = async () => {
    recording = await frame.evaluate(
      async ({ upload, maxBytes, maxMs, captureHz }) => {
        const canvas = document.querySelector("#world");
        if (
          !(canvas instanceof HTMLCanvasElement) ||
          typeof canvas.captureStream !== "function" ||
          typeof MediaRecorder === "undefined"
        )
          throw Error("This browser cannot capture the live canvas.");
        const selected = [
          "video/mp4;codecs=avc1.64002A",
          "video/mp4",
          "video/webm;codecs=vp8",
          "video/webm",
        ].find((t) => MediaRecorder.isTypeSupported(t));
        if (!selected)
          throw Error("This browser has no supported live video codec.");
        // A higher request rate can catch real paints between capture ticks.
        // Rendering stays display-driven; capture adds no authored frames.
        const stream = canvas.captureStream(captureHz),
          track = stream.getVideoTracks()[0];
        if (!track) throw Error("The canvas produced no live video track.");
        track.contentHint = "motion";
        const settings = track.getSettings(),
          recorder = new MediaRecorder(stream, {
            mimeType: selected,
            videoBitsPerSecond: 8000000,
          });
        const chunks = [];
        let bytes = 0,
          started,
          stopRequested,
          timer,
          resolveStop,
          rejectStop,
          failure,
          events = 0;
        const done = new Promise((resolve, reject) => {
          resolveStop = resolve;
          rejectStop = reject;
        });
        void done.catch(() => {});
        const cleanup = () => {
          clearTimeout(timer);
          stream.getTracks().forEach((t) => t.stop());
        };
        recorder.ondataavailable = (event) => {
          if (event.data.size) {
            bytes += event.data.size;
            if (bytes > maxBytes) {
              failure = Error("The live video exceeded its memory budget.");
              if (recorder.state !== "inactive") recorder.stop();
              return;
            }
            chunks.push(event.data);
            events++;
          }
        };
        recorder.onerror = (event) => {
          failure = event.error || Error("The live recorder stopped.");
          cleanup();
          rejectStop(failure);
        };
        recorder.onstop = async () => {
          const stoppedAt = performance.now();
          cleanup();
          try {
            if (failure) throw failure;
            const blob = new Blob(chunks, { type: recorder.mimeType });
            if (!blob.size)
              throw Error(
                "The transferred canvas produced an empty live video.",
              );
            const response = await fetch(upload, {
              method: "POST",
              headers: { "Content-Type": recorder.mimeType },
              body: blob,
            });
            if (!response.ok) throw Error("The live video could not upload.");
            resolveStop({
              wallMs: stoppedAt - started,
              requestedCaptureMs:
                stopRequested === undefined ? null : stopRequested - started,
              uploadMs: performance.now() - stoppedAt,
              clock: {
                timeOrigin: performance.timeOrigin,
                startRequestedAtMs: started,
                startRequestedEpochMs: performance.timeOrigin + started,
                stopRequestedAtMs: stopRequested ?? null,
                stopRequestedEpochMs:
                  stopRequested === undefined
                    ? null
                    : performance.timeOrigin + stopRequested,
                stopEventAtMs: stoppedAt,
                stopEventEpochMs: performance.timeOrigin + stoppedAt,
              },
              bytes: blob.size,
              chunks: events,
              mimeType: recorder.mimeType,
              videoBitsPerSecond: recorder.videoBitsPerSecond,
            });
          } catch (error) {
            rejectStop(error);
          } finally {
            chunks.length = 0;
          }
        };
        const requestStop = () => {
          if (stopRequested === undefined) stopRequested = performance.now();
          if (recorder.state !== "inactive") recorder.stop();
          return {
            atMs: stopRequested,
            epochMs: performance.timeOrigin + stopRequested,
          };
        };
        started = performance.now();
        recorder.start(500);
        const startReturned = performance.now();
        timer = setTimeout(() => {
          failure = Error("Live recording exceeded its time limit.");
          requestStop();
        }, maxMs);
        window.__battleLiveRecording = {
          requestStop,
          completion: () => done,
          stop() {
            requestStop();
            return done;
          },
        };
        return {
          clock: {
            timeOrigin: performance.timeOrigin,
            startRequestedAtMs: started,
            startRequestedEpochMs: performance.timeOrigin + started,
            startReturnedAtMs: startReturned,
            startReturnedEpochMs: performance.timeOrigin + startReturned,
          },
          mimeType: recorder.mimeType,
          requestedFps: captureHz,
          videoBitsPerSecond: recorder.videoBitsPerSecond,
          canvas: [canvas.width, canvas.height],
          track: {
            width: settings.width,
            height: settings.height,
            frameRate: settings.frameRate,
            resizeMode: settings.resizeMode,
          },
        };
      },
      { upload, maxBytes: MAX_BYTES, maxMs: (seconds + 15) * 1000, captureHz },
    );
    report.recording = recording;
    return recording;
  };

  videoPath = resolve(
    output,
    `battle-live.${recordingPlan.mimeType.includes("mp4") ? "mp4" : "webm"}`,
  );
  partialPath = videoPath + ".partial";
  for (const path of [videoPath, partialPath]) {
    try {
      await stat(path);
      throw Error("Choose a fresh output directory for the live video.");
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
  }
  const stopRecording = () =>
    frame.evaluate(() => window.__battleLiveRecording.requestStop());
  if (!probe) {
    await measure("recorded", recordedSetup, {
      start: startRecording,
      stop: stopRecording,
    });
  } else {
    await startRecording();
    await sleep(seconds * 1000);
    await stopRecording();
  }
  report.recordingResult = await frame.evaluate(() =>
    window.__battleLiveRecording.completion(),
  );
  assert(received);
  assert(report.uploadBytes > 0);
  report.videoPath = videoPath;
  report.videoSha256 = createHash("sha256")
    .update(await readFile(videoPath))
    .digest("hex");
  const info = JSON.parse(
    execFileSync(
      "ffprobe",
      [
        "-v",
        "error",
        "-count_frames",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=codec_name,width,height,r_frame_rate,avg_frame_rate,duration,nb_read_frames:format=duration,size",
        "-of",
        "json",
        videoPath,
      ],
      { encoding: "utf8" },
    ),
  );
  const pts = JSON.parse(
    execFileSync(
      "ffprobe",
      [
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "frame=best_effort_timestamp_time",
        "-of",
        "json",
        videoPath,
      ],
      { encoding: "utf8", maxBuffer: 8 * 1024 * 1024 },
    ),
  );
  report.ffprobe = info;
  const stream = info.streams[0];
  assert.equal(stream.width, recording.canvas[0]);
  assert.equal(stream.height, recording.canvas[1]);
  const times = pts.frames.map((f) => Number(f.best_effort_timestamp_time));
  assert(
    times.length >= 2 &&
      times.every((t, i) => Number.isFinite(t) && (!i || t > times[i - 1])),
  );
  const videoSeconds = Number(info.format.duration || stream.duration);
  assert(videoSeconds >= seconds * 0.8 && videoSeconds <= seconds + 10);
  report.videoFrameCadence = {
    frames: times.length,
    durationSeconds: videoSeconds,
    decodedFramesPerSecond: times.length / videoSeconds,
    intervalMs: distribution(
      times.slice(1).map((t, i) => (t - times[i]) * 1000),
    ),
    timestamps: times,
  };
  const picture = resolve(output, "battle-live-frame.png");
  execFileSync("ffmpeg", [
    "-hide_banner",
    "-loglevel",
    "error",
    "-threads",
    "2",
    "-i",
    videoPath,
    "-ss",
    String(Math.min(2, videoSeconds / 2)),
    "-frames:v",
    "1",
    picture,
  ]);
  report.captures.push(picture);
  if (!probe) {
    await page.screenshot({ path: resolve(output, "battle-live-hud.png") });
    report.captures.push(resolve(output, "battle-live-hud.png"));
    report.finalBattle = await status();
  }
  assert.equal(report.errors.length, 0, JSON.stringify(report.errors));
  assert.equal(report.failed.length, 0, JSON.stringify(report.failed));
  report.sourceAfter = await battleSourcePin(root);
  report.sourceUnchanged =
    report.sourcePin.sha256 === report.sourceAfter.sha256;
  assert(
    report.sourceUnchanged,
    "Source files changed during the live recording.",
  );
  if (!probe) {
    report.assetAfter = await battleAssetPin(
      url.href,
      process.env.AF_BATTLE_ASSET_ROOT,
    );
    report.assetsUnchanged =
      report.assetPin.sha256 === report.assetAfter.sha256;
    assert(
      report.assetsUnchanged,
      "Battle assets changed during the live recording.",
    );
  }
  report.ok = true;
  console.log(
    JSON.stringify({
      ok: true,
      videoPath,
      decodedFrames: times.length,
      videoSeconds,
      decodedFps: times.length / videoSeconds,
    }),
  );
} catch (error) {
  report.errors.push({ error: String(error), stack: error.stack });
  try {
    report.failureStatus = await status();
  } catch {}
  console.error(error);
  process.exitCode = 1;
} finally {
  if (frame)
    await frame
      .evaluate(() => window.__battleLiveRecording?.requestStop())
      .catch(() => {});
  if (participating)
    await page
      .evaluate(
        (input) => window.aethyrforge?.battle({ op: "input", ...input }),
        NEUTRAL_BATTLE_INPUT,
      )
      .catch(() => {});
  if (measuring) await benchmark({ op: "stop", drain: true }).catch(() => {});
  if (frame)
    await frame
      .evaluate(() => window.__battleLiveRecording?.stop())
      .catch(() => {});
  if (!probe)
    await page
      .evaluate(() => window.aethyrforge?.battle({ op: "stop" }))
      .catch(() => {});
  closing = true;
  await context.close().catch(() => {});
  await browser.close();
  if (server) await new Promise((r) => server.close(r));
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, "battle-live-recording.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
}
