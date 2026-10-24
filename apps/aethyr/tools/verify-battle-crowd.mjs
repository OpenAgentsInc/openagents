/** Live battle measurements. Run through openagents browser run; evidence stays private. */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, realpath, writeFile, readFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import {
  distribution,
  summarizeFrames,
} from "../experiments/c-town/measurement.mjs";
import { evaluateFramePacing } from "./performance-contract.mjs";
import {
  battleSourcePin,
  battleAssetPin,
  battleStatusFreshness,
  battleCombatProgress,
} from "./battle-evidence.mjs";
import {
  evaluateApplicationPacing,
  observeApplicationBrowserMode,
} from "./application-pacing.mjs";
import {
  battleParticipationPlan,
  battleRecordingLook,
  NEUTRAL_BATTLE_INPUT,
} from "./battle-shot-plan.mjs";
import {
  battleParticipantWindow,
  battleParticipationAim,
  battleWindowAround60,
  battleAllViewsCoverage,
} from "./battle-window-metrics.mjs";
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
  "Keep battle captures outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run through openagents browser run.");
const url = new URL(process.env.AF_EDITOR_URL || "http://127.0.0.1:55316/");
assert(
  ["http:", "https:"].includes(url.protocol) &&
    ["localhost", "127.0.0.1"].includes(url.hostname) &&
    !url.username &&
    !url.password,
);
const integer = (name, fallback, low, high) => {
  const n = Number(process.env[name] ?? fallback);
  assert(
    Number.isSafeInteger(n) && n >= low && n <= high,
    `${name} is outside its supported range.`,
  );
  return n;
};
const viewport = {
  width: integer("AF_BATTLE_WIDTH", 1280, 640, 3840),
  height: integer("AF_BATTLE_HEIGHT", 720, 360, 2160),
};
const counts = (process.env.AF_BATTLE_COUNTS || "10000").split(",").map(Number);
assert(
  counts.length > 0 &&
    counts.length <= 12 &&
    new Set(counts).size === counts.length &&
    counts.every(
      (n) => Number.isSafeInteger(n) && (n === 0 || n >= 100) && n <= 1000000,
    ),
);
const views = (process.env.AF_BATTLE_VIEWS || "overview,orbit").split(",");
assert(
  views.length &&
    new Set(views).size === views.length &&
    views.every((v) =>
      ["overview", "orbit", "player", "first", "third", "dolly"].includes(v),
    ),
);
const repeats = integer("AF_BATTLE_REPEATS", 2, 1, 4);
const durationMs = integer("AF_BATTLE_WINDOW_MS", 30000, 1000, 60000);
const warmupMs = integer("AF_BATTLE_WARMUP_MS", 5000, 500, 30000);
const instrumentationFlag = process.env.AF_BATTLE_INSTRUMENTED ?? "1";
assert(
  ["0", "1"].includes(instrumentationFlag),
  "Set AF_BATTLE_INSTRUMENTED to 0 or 1.",
);
const instrumented = instrumentationFlag === "1";
const participationFlag = process.env.AF_BATTLE_PARTICIPATE ?? "0";
assert(["0", "1"].includes(participationFlag));
const participationEnabled = participationFlag === "1";
if (participationEnabled)
  assert(
    durationMs >= 2000,
    "Active battle windows need at least two seconds.",
  );
const participatingView = (view) =>
  participationEnabled && ["first", "third"].includes(view);

const mode = process.env.AF_BATTLE_MODE || "diagnostic";
assert(["diagnostic", "qualification"].includes(mode));
if (mode === "qualification")
  assert(durationMs >= 30000 && warmupMs >= 5000 && repeats >= 2);
const sha = (b) => createHash("sha256").update(b).digest("hex");
const report = {
  schema: "aethyrforge.battle-crowd-browser.v1",
  runId: randomUUID(),
  startedAt: new Date().toISOString(),
  ok: false,
  revision: execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: root,
    encoding: "utf8",
  }).trim(),
  dirty: !!execFileSync("git", ["status", "--porcelain"], {
    cwd: root,
    encoding: "utf8",
  }).trim(),
  kernelSha256: sha(await readFile(resolve(root, "public/kernel.wasm"))),
  url: url.href,
  configuration: {
    viewport: [viewport.width, viewport.height],
    deviceScaleFactor: 1,
    counts,
    views,
    repeats,
    durationMs,
    warmupMs,
    mode,
    instrumented,
    activeParticipation: participationEnabled,
    participantAttack: participationEnabled ? "ranged" : null,
    thirdPersonResetAim: participationEnabled
      ? battleParticipationAim("third")
      : null,
    experimentalMeanTargetHz: 58,
    seed: 1,
    targetHz: "display",
  },
  windows: [],
  rawWindows: [],
  checks: [],
  captures: [],
  errors: [],
  console: [],
  failed: [],
  limitations: [
    "This is a device-local GPU battle experiment, not 10,000 authoritative multiplayer sessions.",
    "The actor pool includes living fighters and held corpses with explicit local replenishment. Alive/moving/attacking populations are reported throughout.",
    "Submissions and queue-fence callbacks are not compositor or physical display presentation. Full GPU timings exclude uploads/copies outside measured passes and composition.",
    "Opt-in AF_BATTLE_PARTICIPATE=1 applies real bounded movement, turn, ranged attack, camera-look and respawn commands during first/third-person windows; standing windows cannot pass the separate all-view mean target.",
    "This verifier performs no video encoding. Recording overhead is measured separately by record-battle.mjs.",
    "Qualification applies only to this run's exact source and asset pins. A source change requires new qualification; earlier runs remain separate evidence for their prior code.",
    "Pool capacity means simulated actors. First, third and dolly shots report their submitted subset separately and cannot establish full-army visibility capacity through culling alone.",
    instrumented
      ? "Instrumentation records CPU stages and GPU timestamp spans and adds measurement overhead. This is not the uninstrumented production rendering path."
      : "CPU stage and GPU timestamp instrumentation is disabled. Qualification covers actual application/queue cadence and simulation continuity; unavailable GPU timings do not support a GPU-budget claim.",
  ],
};
report.sourcePin = await battleSourcePin(root);
report.assetPin = await battleAssetPin(
  url.href,
  process.env.AF_BATTLE_ASSET_ROOT || process.env.AETHYRFORGE_PRIVATE_ASSETS,
);
if (counts.includes(0))
  report.limitations.push(
    "The zero-crowd baseline uses the actual player camera because Stop disables the spectator camera. It is not a camera-matched crowd-only comparison.",
  );
const browser = await chromium.connectOverCDP(endpoint);
report.browserMode = await observeApplicationBrowserMode(browser);
const context = await browser.newContext({
  viewport,
  deviceScaleFactor: 1,
  serviceWorkers: "block",
});
const page = await context.newPage();
page.setDefaultTimeout(20000);
let closing = false,
  measuring = false,
  activeRaw;
async function persistRaw(phase) {
  if (!activeRaw) return;
  activeRaw.phase = phase;
  activeRaw.updatedAt = new Date().toISOString();
  await writeFile(
    resolve(output, activeRaw.file),
    JSON.stringify(activeRaw, null, 2) + "\n",
  );
}
page.on("pageerror", (e) => report.errors.push(String(e)));
page.on("console", (m) => {
  if (["error", "warning"].includes(m.type()))
    report.console.push({ type: m.type(), text: m.text().slice(0, 4096) });
});
page.on("requestfailed", (r) => {
  if (!closing)
    report.failed.push({ url: r.url(), error: r.failure()?.errorText });
});
page.on("response", (r) => {
  if (!closing && r.status() >= 400)
    report.failed.push({ url: r.url(), status: r.status() });
});
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
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
    const result = await predicate(s);
    if (result) return result === true ? s : result;
    await sleep(80);
  }
  throw Error(`${label}: ${JSON.stringify(await status())}`);
}
async function action(action) {
  const id = await page.evaluate(
    (action) => window.aethyrforge.battle(action),
    action,
  );
  const receipt = await until(
    async () => (await snapshot()).receipts.find((r) => r.requestId === id),
    "The battle action did not finish",
    30000,
  );
  assert.equal(receipt.ok, true, receipt.message);
  return receipt;
}
async function restoreThirdPersonAim(view) {
  if (view !== "third") return null;
  const controls = { ...NEUTRAL_BATTLE_INPUT, ...battleParticipationAim(view) };
  const receipt = await action({
    op: "input",
    ...controls,
    attackKind: "ranged",
  });
  return { controls, receipt };
}
async function capture(name) {
  const path = resolve(output, `${name}.png`);
  await page.screenshot({ path });
  report.captures.push(path);
}
function validateProbe(p, count) {
  assert(
    p && p.count === count && p.finite === true,
    "The actual GPU battle probe is missing or invalid",
  );
  assert(/^[a-f0-9]{64}$/.test(p.sampleSha256));
  assert(Number.isSafeInteger(p.tick) && p.tick >= 0);
  assert(
    Array.isArray(p.samples) && p.samples.length > 0 && p.samples.length <= 32,
  );
  assert.equal(new Set(p.samples.map((s) => s.id)).size, p.samples.length);
  for (const s of p.samples) {
    assert(Number.isSafeInteger(s.id) && s.id >= 0 && s.id < count);
    assert(["paladin", "sorceror", "zombie", "warrok"].includes(s.kind));
    assert([0, 1].includes(s.team));
    assert(
      Array.isArray(s.position) &&
        s.position.length === 3 &&
        s.position.every((n) => Number.isFinite(n) && Math.abs(n) < 20000),
    );
    assert(
      Number.isFinite(s.health) &&
        s.health >= 0 &&
        s.health <= s.maxHealth &&
        s.maxHealth > 0,
    );
    assert(["idle", "walk", "attack", "death"].includes(s.state));
  }
  assert.equal(p.gpu.alive + p.gpu.dead, count);
  assert.equal(p.gpu.visible + p.gpu.culled, count);
  assert.equal(p.gpu.near + p.gpu.far, p.gpu.visible);
  assert.equal(p.gpu.invalid, 0);
  assert.equal(
    p.gpu.gridOverflows,
    0,
    "Neighbor-grid overflow would change the combat workload",
  );
}
const check = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
try {
  await page.goto(url.href);
  report.observedViewport = await page.evaluate(() => ({
    width: window.innerWidth,
    height: window.innerHeight,
    devicePixelRatio: window.devicePixelRatio,
  }));
  assert.deepEqual(
    report.observedViewport,
    { ...viewport, devicePixelRatio: 1 },
    "The actual browser viewport or DPR differs from the requested workload.",
  );
  await until(
    (s) => s.phase === "ready" && s.mmorpg,
    "The prepared battle world did not open",
    150000,
  );
  await page
    .locator("#mmo-name")
    .fill(`Battle-${Date.now().toString(36).slice(-6)}`);
  await page.getByRole("button", { name: "Enter world", exact: true }).click();
  await until(
    (s) => s.mmorpg?.connection === "online",
    "The Paladin did not join",
  );
  await page
    .locator(".mmo-login")
    .waitFor({ state: "detached", timeout: 3000 });
  await page.getByRole("button", { name: "Battle Lab", exact: true }).click();
  for (const count of counts)
    for (const view of count === 0 ? ["player"] : views)
      for (let repeat = 0; repeat < repeats; repeat++) {
        const index = report.windows.length;
        activeRaw = {
          schema: "aethyrforge.battle-window-raw.v1",
          index,
          count,
          view,
          repeat,
          instrumented,
          file: `battle-window-${report.runId}-${index}-${count}-${view}-r${repeat}-raw.json`,
          sourceSha256: report.sourcePin.sha256,
          assetSha256: report.assetPin.sha256,
          startedAt: new Date().toISOString(),
        };
        report.rawWindows.push(activeRaw.file);
        await persistRaw("setup");
        if (count === 0) {
          await action({ op: "stop" });
          await until(
            (s) => !s.battle?.enabled,
            "The zero-crowd baseline did not stop the battle",
          );
        } else {
          await action({ op: "start", count, seed: 1 });
          if (view === "first" || view === "third")
            await action({ op: "join", view, class: "paladin", team: 0 });
          await action({ op: "camera", view });
          await until(
            (s) =>
              s.battle?.enabled &&
              s.battle.count === count &&
              s.battle.camera === view,
            "The battle/camera did not activate",
          );
        }
        await sleep(warmupMs);
        if (participatingView(view)) {
          await action({ op: "respawn" });
          await until(
            (s) => s.battle?.participant?.ready && s.battle.participant.alive,
            "The measured participant did not return ready",
          );
          await action({ op: "weapon", attackKind: "ranged" });
          activeRaw.participationAim = await restoreThirdPersonAim(view);
        }
        const beforeMetrics = await benchmark({
          op: count ? "readBattle" : "read",
        });
        const beforeProbe = beforeMetrics.battleProbe ?? null;
        activeRaw.beforeMetrics = beforeMetrics;
        activeRaw.beforeProbe = beforeProbe;
        await persistRaw("before-probe-read");
        if (count) validateProbe(beforeProbe, count);
        await capture(`battle-${index}-${count}-${view}-before`);
        await page.bringToFront();
        const started = await benchmark({
          op: "start",
          route: "static",
          capacity: 20000,
          instrumented,
          targetHz: "display",
        });
        measuring = true;
        activeRaw.started = started;
        await persistRaw("measurement-started");
        const samples = (activeRaw.statusSamples = []),
          inputEvents = (activeRaw.inputEvents = []),
          participantEvents = (activeRaw.participantEvents = []),
          inputPlan = participatingView(view)
            ? battleParticipationPlan(durationMs / 1000)
            : [],
          begin = performance.now();
        activeRaw.inputPlan = inputPlan;
        let lastInputAt = -Infinity,
          lastLookElapsed = 0,
          priorAlive;
        while (performance.now() - begin < durationMs) {
          const s = await status();
          assert.equal(s.phase, "ready");
          if (count) assert.equal(s.battle?.count, count);
          else assert.equal(Boolean(s.battle?.enabled), false);
          samples.push({
            wallMs: performance.now() - begin,
            frame: s.frame,
            fps: s.fps,
            battle: structuredClone(s.battle),
            runtimeMetrics: s.runtimeMetrics,
          });
          if (participatingView(view)) {
            const atMs = performance.now() - begin,
              actor = s.battle?.participant;
            assert(
              actor?.active,
              "The measured battle participant disappeared",
            );
            if (priorAlive === true && !actor.alive)
              participantEvents.push({
                kind: "death-observed",
                atMs,
                participant: structuredClone(actor),
              });
            priorAlive = actor.alive;
            if (actor.ready && !actor.alive) {
              participantEvents.push({
                kind: "respawn-request",
                atMs,
                participant: structuredClone(actor),
              });
              const receipt = await action({ op: "respawn" });
              participantEvents.push({
                kind: "respawn-receipt",
                atMs: performance.now() - begin,
                receipt,
              });
              if (view === "third") {
                const aim = await restoreThirdPersonAim(view);
                participantEvents.push({
                  kind: "respawn-aim",
                  atMs: performance.now() - begin,
                  ...aim,
                });
              }
            } else if (atMs - lastInputAt >= 250) {
              const phase = inputPlan.findLast((p) => p.atMs <= atMs);
              const controls =
                actor.ready && actor.alive
                  ? {
                      ...(phase?.input ?? NEUTRAL_BATTLE_INPUT),
                      ...battleRecordingLook(
                        durationMs / 1000,
                        lastLookElapsed,
                        atMs,
                        view,
                      ),
                    }
                  : NEUTRAL_BATTLE_INPUT;
              const receipt = await action({
                op: "input",
                ...controls,
                attackKind: "ranged",
              });
              inputEvents.push({ atMs, controls, receipt });
              lastInputAt = atMs;
              lastLookElapsed = atMs;
            }
          }
          await sleep(250);
        }
        if (participatingView(view))
          await action({
            op: "input",
            ...NEUTRAL_BATTLE_INPUT,
            attackKind: "ranged",
          });
        const stopped = await benchmark({ op: "stop", drain: true });
        measuring = false;
        activeRaw.stopped = stopped;
        const afterMetrics = await benchmark({
          op: count ? "readBattle" : "read",
        });
        const afterProbe = afterMetrics.battleProbe ?? null;
        activeRaw.afterMetrics = afterMetrics;
        activeRaw.afterProbe = afterProbe;
        await persistRaw("after-probe-read");
        const statusFreshness = count
          ? battleStatusFreshness(samples, {
              stoppedSimulationSteps:
                stopped.resources.battle?.stats.simulationSteps,
              directAfterTick: afterProbe?.tick,
            })
          : null;
        const combatProgress = count
          ? battleCombatProgress(beforeProbe, afterProbe, count)
          : null;
        const overflowFidelity = count
          ? battleOverflowFidelity(beforeProbe, afterProbe, samples)
          : null;
        activeRaw.statusFreshness = statusFreshness;
        activeRaw.combatProgress = combatProgress;
        activeRaw.overflowFidelity = overflowFidelity;
        await persistRaw("status-freshness-evaluated");
        let changed = 0;
        if (count) {
          validateProbe(afterProbe, count);
          assert(
            statusFreshness.ok,
            `Population status samples are stale or incomplete: ${JSON.stringify(statusFreshness)}`,
          );
          assert(
            combatProgress.ok,
            `Direct GPU probes do not show complete advancing combat: ${JSON.stringify(combatProgress)}`,
          );
          assert(afterProbe.tick > beforeProbe.tick);
          assert.notEqual(afterProbe.sampleSha256, beforeProbe.sampleSha256);
          assert(
            afterProbe.gpu.attacks > beforeProbe.gpu.attacks &&
              afterProbe.gpu.hits > beforeProbe.gpu.hits,
            "The battle must include real attacks and damage",
          );
          assert(
            afterProbe.gpu.deaths > beforeProbe.gpu.deaths,
            "The battle must include real deaths",
          );
          if (durationMs >= 10000)
            assert(
              afterProbe.gpu.respawned > beforeProbe.gpu.respawned,
              "Sustained pool load needs its declared local replenishment",
            );
          const byId = new Map(beforeProbe.samples.map((s) => [s.id, s]));
          changed = afterProbe.samples.filter((s) => {
            const b = byId.get(s.id);
            return (
              b &&
              (Math.hypot(...s.position.map((n, i) => n - b.position[i])) >
                0.001 ||
                s.health !== b.health)
            );
          }).length;
          assert(
            changed > 0,
            "Sampled GPU actors must actually move or change HP",
          );
        }
        const observedMs =
          stopped.observation.stoppedAtMs - stopped.observation.startedAtMs;
        const productCounters = {
          before: started.resources.frameCounters,
          after: stopped.resources.frameCounters,
        };
        const opts = {
          mode,
          minHz: 60,
          warmupMs,
          durationMs: observedMs,
          requireFullGpu: instrumented,
        };
        const pacing = evaluateFramePacing(stopped, opts);
        const applicationPacing = evaluateApplicationPacing(stopped, {
          ...opts,
          productCounters,
          browserMode: report.browserMode.mode,
        });
        const droppedBefore = started.resources.battle?.stats.droppedSeconds;
        const droppedAfter = stopped.resources.battle?.stats.droppedSeconds;
        const battleContinuity = {
          droppedSecondsBefore: droppedBefore ?? null,
          droppedSecondsAfter: droppedAfter ?? null,
          noDroppedTime:
            !count ||
            (Number.isFinite(droppedBefore) &&
              Number.isFinite(droppedAfter) &&
              droppedAfter === droppedBefore),
        };
        const start = stopped.observation.startedAtMs;
        const bins = (events) =>
          Array.from(
            { length: Math.floor(observedMs / 1000) },
            (_, i) =>
              events.filter(
                (at) => at >= start + i * 1000 && at < start + (i + 1) * 1000,
              ).length,
          );
        const row = {
          index,
          rawEvidence: activeRaw.file,
          count,
          view,
          repeat,
          instrumented,
          warmupMs,
          requestedDurationMs: durationMs,
          observedMs,
          productCounters,
          pacing,
          applicationPacing,
          battleContinuity,
          statusFreshness,
          combatProgress,
          overflowFidelity,
          frames: summarizeFrames(stopped.samples),
          perSecond: {
            accepted: bins(
              stopped.completedFrames.map((f) => f.submitReturnedAt),
            ),
            completed: bins(stopped.completedFrames.map((f) => f.completedAt)),
          },
          populations: Object.fromEntries(
            [
              "alive",
              "dead",
              "visible",
              "near",
              "far",
              "moving",
              "attacking",
              "deaths",
              "respawned",
              "triangles",
              "gpuBytes",
            ].map((key) => [
              key,
              distribution(samples.map((s) => s.battle?.stats[key] ?? null)),
            ]),
          ),
          visibilityScope: samples.every(
            (s) => (s.battle?.stats.visible ?? 0) >= count * 0.95,
          )
            ? "at-least-95-percent-of-pool"
            : "camera-subset-of-simulated-pool",
          beforeProbe,
          afterProbe,
          changedSampleActors: changed,
          inputPlan,
          inputEvents,
          participantEvents,
          participantEvidence: participatingView(view)
            ? battleParticipantWindow(samples, participantEvents)
            : null,
          gpuStageSnapshot: instrumented
            ? {
                timingStatus: stopped.resources.renderer.gpuTimingStatus,
                scope:
                  "Latest measured GPU frame only. Pass spans may overlap; do not sum them or call these stage percentiles.",
              }
            : null,
          statusSamples: samples,
          metrics: stopped,
        };
        row.around60 = battleWindowAround60(row);
        activeRaw.result = {
          pacing,
          applicationPacing,
          battleContinuity,
          statusFreshness,
          combatProgress,
          overflowFidelity,
          participantEvidence: row.participantEvidence,
          around60: row.around60,
        };
        await persistRaw("evaluated");
        report.windows.push(row);
        await writeFile(
          resolve(output, `battle-window-${index}.json`),
          JSON.stringify(row, null, 2) + "\n",
        );
        check(
          count
            ? `Real ${count}-actor ${view} battle advances combat, death and GPU poses`
            : "Zero-crowd player-camera baseline completed",
          {
            pacing: pacing.ok,
            applicationPacing: applicationPacing.ok,
            overflowFidelity: overflowFidelity?.ok ?? true,
            around60: row.around60.ok,
            activeParticipation: row.participantEvidence?.ok ?? null,
            index,
          },
        );
        await capture(`battle-${index}-${count}-${view}-after`);
        await action({ op: "stop" });
        activeRaw = undefined;
      }
  const grouped = counts.map((count) => ({
    count,
    windows: report.windows.filter((w) => w.count === count),
  }));
  report.largestPassingPool = Math.max(
    0,
    ...grouped
      .filter(
        (g) =>
          g.count > 0 &&
          g.windows.length === views.length * repeats &&
          g.windows.every(
            (w) =>
              w.pacing.ok &&
              w.applicationPacing.ok &&
              w.battleContinuity.noDroppedTime &&
              w.statusFreshness.ok &&
              w.combatProgress.ok &&
              w.overflowFidelity.ok,
          ),
      )
      .map((g) => g.count),
  );
  report.qualified = mode === "qualification" && report.largestPassingPool > 0;
  report.around60AllViews = counts
    .filter((count) => count > 0)
    .map((count) => battleAllViewsCoverage(report.windows, count));
  report.largestAround60AllViewsPool = Math.max(
    0,
    ...report.around60AllViews
      .filter((result) => result.ok)
      .map((result) => result.count),
  );
  report.activeParticipationFidelity =
    participationEnabled &&
    report.windows.filter((w) => ["first", "third"].includes(w.view)).length >
      0 &&
    report.windows
      .filter((w) => ["first", "third"].includes(w.view))
      .every((w) => w.participantEvidence?.ok === true);
  report.overflowFidelity = report.windows.every(
    (w) => !w.count || w.overflowFidelity.ok,
  );
  report.qualificationScope = instrumented
    ? "application/queue cadence and GPU timestamp budget"
    : "application/queue cadence; GPU timestamp budget unavailable";
  assert.equal(report.errors.length, 0, JSON.stringify(report.errors));
  assert.equal(report.failed.length, 0, JSON.stringify(report.failed));
  report.ok = true;
  console.log(
    JSON.stringify({
      functionalPass: true,
      activeParticipationPass: report.activeParticipationFidelity,
      largestPassingPool: report.largestPassingPool,
      qualified: report.qualified,
      largestAround60AllViewsPool: report.largestAround60AllViewsPool,
      windows: report.windows.length,
    }),
  );
} catch (error) {
  if (activeRaw) {
    activeRaw.error = { error: String(error), stack: error.stack };
    if (measuring) {
      try {
        activeRaw.stopped = await benchmark({ op: "stop", drain: true });
        measuring = false;
      } catch (stopError) {
        activeRaw.stopError = String(stopError);
      }
    }
    await persistRaw("failed");
  }
  report.errors.push({ error: String(error), stack: error.stack });
  try {
    report.failureStatus = await status();
    await capture("battle-error");
  } catch {}
  console.error(error);
  process.exitCode = 1;
} finally {
  if (measuring) await benchmark({ op: "stop", drain: true }).catch(() => {});
  await page
    .evaluate(() => window.aethyrforge?.battle({ op: "stop" }))
    .catch(() => {});
  closing = true;
  await context.close().catch(() => {});
  await browser.close();
  try {
    report.sourcePinAfter = await battleSourcePin(root);
    report.assetPinAfter = await battleAssetPin(
      url.href,
      process.env.AF_BATTLE_ASSET_ROOT ||
        process.env.AETHYRFORGE_PRIVATE_ASSETS,
    );
    report.sourcesUnchanged =
      report.sourcePin.sha256 === report.sourcePinAfter.sha256;
    report.assetsUnchanged =
      report.assetPin.sha256 === report.assetPinAfter.sha256 &&
      report.assetPin.manifest.sha256 ===
        report.assetPinAfter.manifest.sha256 &&
      report.assetPin.scene.sha256 === report.assetPinAfter.scene.sha256;
    if (!report.sourcesUnchanged || !report.assetsUnchanged) {
      report.ok = false;
      process.exitCode = 1;
      report.errors.push("Source or private assets changed during the run.");
    }
  } catch (error) {
    report.ok = false;
    process.exitCode = 1;
    report.errors.push(`Final evidence pin: ${error}`);
  }
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, "battle-crowd-browser.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
}
