import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdir,
  mkdtemp,
  readFile,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import test from "node:test";
const { compareTownRenderer } = await import(
  new URL("../tools/compare-town-renderer.mjs", import.meta.url).href
);
const { evaluateFramePacing } = await import(
  new URL("../tools/performance-contract.mjs", import.meta.url).href
);
const { evaluateApplicationPacing } = await import(
  new URL("../tools/application-pacing.mjs", import.meta.url).href
);
const hash = (value: string | Uint8Array) =>
  createHash("sha256").update(value).digest("hex");
const browserModeEvidence = {
  schema: "aethyrforge.browser-mode.v1",
  product: "Chrome/155.0",
  userAgent: "Chrome/155.0",
  commandLineAvailable: true,
  headlessArguments: [],
  mode: "headed",
  scope: "Actual Browser.getVersion and Browser.getBrowserCommandLine",
};
async function fixture(t: any) {
  const root = await mkdtemp(
    join(homedir(), ".openagents/scratch/town-renderer-compare-"),
  );
  t.after(() => rm(root, { recursive: true, force: true }));
  const baseline = join(root, "baseline"),
    candidate = join(root, "candidate");
  await mkdir(baseline);
  await mkdir(candidate);
  return { root, baseline, candidate };
}
function metrics(offset: number, commands = false) {
  const samples = Array.from({ length: 61 }, (_, index) => ({
    frame: index + offset,
    at: 10_000 + (index * 1000) / 60,
    callbackEnteredAt: 10_000 + (index * 1000) / 60 + 0.01,
    elapsedMs: 1000 / 60,
    submitted: true,
    fullGpuMs: 2,
    gpuMs: 1,
    renderCpuMs: 0.5,
    submittedInstances: 108000,
    submittedTriangles: 33000000,
    kernelDroppedSeconds: 0,
    hostClampedSeconds: 0,
    ...(commands
      ? {
          sourceRenderCommands: {
            draws: 12,
            pipelines: 3,
            materials: 5,
            windBindings: 2,
          },
        }
      : {}),
  }));
  return {
    recording: false,
    truncated: false,
    completionTruncated: false,
    completionRange: { firstFrame: offset, lastFrame: offset + 60 },
    observation: { startedAtMs: 10_000, stoppedAtMs: 11_017 },
    samples,
    completedFrames: samples.map((value) => ({
      frame: value.frame,
      submittedAt: value.at + 0.1,
      submitReturnedAt: value.at + 0.2,
      completedAt: value.at + 2.1,
      latencyMs: 2,
      ok: true,
    })),
    resources: {
      renderer: {
        gpuAccountedBytes: 123456,
        sourceSubmission: {
          recordedCommands: 7000,
          bundles: 12,
          executions: 600,
        },
      },
    },
  };
}
async function createRun(
  root: string,
  options: {
    reversed?: boolean;
    commands?: boolean;
    shader?: string;
    keys?: number[];
  } = {},
) {
  const files = {
    "packages/renderer/shaders.ts": hash(options.shader ?? "old"),
  };
  const source = {
    revision: "revision",
    dirty: true,
    files,
    sha256: hash(JSON.stringify(files)),
  };
  const report: any = {
    schema: "aethyrforge.town-swarm-browser.v1",
    ok: false,
    qualified: false,
    errors: [],
    cleanupErrors: [],
    browser: "Chrome/155.0",
    host: { release: "Darwin", cpu: "M5 Max", logicalCpus: 18 },
    configuration: {
      viewport: { width: 1, height: 1 },
      durationMs: 1000,
      warmupMs: 500,
      targetHz: "display",
      minHz: 60,
      instrumented: true,
    },
    sourceBefore: source,
    sourceAfter: source,
    assetClosure: {
      sha256: hash("assets"),
      rawBytes: 123,
      files: 2,
      unchanged: true,
    },
    windows: [],
    captures: [],
  };
  const counts = options.keys ?? (options.reversed ? [100000, 0] : [0, 100000]);
  for (const [index, count] of counts.entries()) {
    const pixels = new Uint8Array([100, 50, 10, 255]);
    const metadata = {
      schema: "aethyrforge.agent.capture.v1",
      context: {
        worldId: "medieval-town-lab",
        sourceIdentity: hash("town"),
        revision: 0,
        generation: options.reversed ? 3 : 2,
        shaderIdentity: hash(options.shader ?? "old"),
      },
      width: 1,
      height: 1,
      time: 0,
      exposure: 1,
      camera: {
        eye: [6, 4, 20],
        target: [6, 4, 19],
        fov: 1,
        up: [0, 1, 0],
        vignette: null,
      },
      actors: {
        sourceIdentity: hash("town"),
        scenario: "night",
        modelIdentity: hash("knights"),
        poseSha256: hash("pose"),
        actorIds: [0, 1, 2, 3, 4, 5, 6, 7],
        count: 8,
        basis: "native-start",
      },
      oracle: { ok: true },
      pixelSha256: hash(pixels),
    };
    const before = { ...metadata, path: `window-${index}-before.rgba` };
    const restored = { ...metadata, path: `window-${index}-restored.rgba` };
    await writeFile(join(root, before.path), pixels);
    await writeFile(join(root, restored.path), pixels);
    const productCounters = {
      before: { kernelDroppedSeconds: 0, hostClampedSeconds: 0 },
      after: { kernelDroppedSeconds: 0, hostClampedSeconds: 0 },
    };
    const applicationOptions = {
      durationMs: 1017,
      warmupMs: 500,
      mode: "diagnostic",
      minHz: 60,
      requireFullGpu: true,
    };
    const measured = metrics(index * 100 + 10, options.commands);
    const tail = measured.completedFrames.at(-1)!;
    tail.latencyMs = 40;
    tail.completedAt = tail.submittedAt + 40;
    const pacing = evaluateFramePacing(measured, applicationOptions);
    const applicationPacing = evaluateApplicationPacing(measured, {
      ...applicationOptions,
      productCounters,
      browserMode: "headed",
    });
    const row = {
      index,
      scenario: "night",
      requestedScenario: "night",
      shot: "spawn",
      route: "static",
      count,
      observationMs: 1017,
      quality: { before, restored, exact: true },
      pacing,
      applicationPacing,
      rawEvidence: `window-${index}.json`,
    };
    await writeFile(
      join(root, row.rawEvidence),
      JSON.stringify({
        schema: "aethyrforge.pacing-window.v1",
        world: "medieval-town",
        ...row,
        quality: { before },
        metrics: measured,
        productCounters,
        applicationOptions,
        browserModeEvidence,
      }),
    );
    report.windows.push(row);
    report.captures.push(before, restored);
  }
  await writeFile(join(root, "town-swarm.json"), JSON.stringify(report));
  return report;
}
async function editReport(root: string, edit: (report: any) => void) {
  const path = join(root, "town-swarm.json");
  const report = JSON.parse(await readFile(path, "utf8"));
  edit(report);
  await writeFile(path, JSON.stringify(report));
}
const failed = (report: any, id: string) =>
  assert.equal(
    report.checks.find((value: any) => value.id === id)?.ok,
    false,
    id,
  );

test("pairs workload occurrences across reversed order, verifies actual bytes, and never qualifies", async (t) => {
  const paths = await fixture(t);
  await createRun(paths.baseline);
  await createRun(paths.candidate, { reversed: true, shader: "specialized" });
  const report = await compareTownRenderer(paths);
  assert.equal(
    report.ok,
    true,
    JSON.stringify(report.checks.filter((value: any) => !value.ok)),
  );
  assert.equal(report.qualified, false);
  assert.deepEqual(
    report.pairs.map((value: any) => [value.baseline, value.candidate]),
    [
      [0, 1],
      [1, 0],
    ],
  );
  assert.deepEqual(report.changedSourcePaths, ["packages/renderer/shaders.ts"]);
  assert.equal(report.runs[0].recordedOk, false);
  assert.equal(report.runs[0].windows[0].recordedPacingPassed, false);
  assert.equal(
    report.runs[0].windows[0].sourceRenderCommands.values.draws,
    null,
  );
  assert.equal(
    report.runs[0].windows[0].cachedCommandInventory.value.recordedCommands,
    7000,
  );
  assert.equal(report.runs[0].windows[0].stages.renderCpuMs.p50, 0.5);
  const raw = report.runs[0].evidence.find(
    (value: any) => value.path === "window-0.json",
  );
  assert.equal(
    raw.sha256,
    hash(await readFile(join(paths.baseline, "window-0.json"))),
  );
});

test("summarizes actual per-frame counters separately from cache inventory", async (t) => {
  const paths = await fixture(t);
  await createRun(paths.baseline, { commands: true });
  await createRun(paths.candidate, { commands: true });
  const report = await compareTownRenderer(paths);
  assert.equal(report.ok, true);
  assert.equal(
    report.runs[0].windows[0].sourceRenderCommands.values.draws.p50,
    12,
  );
  assert.equal(
    report.runs[0].windows[0].sourceRenderCommands.values.draws.count,
    61,
  );
});

test("detects one changed channel despite forged exact flags and matching capture metadata hashes", async (t) => {
  const paths = await fixture(t);
  await createRun(paths.baseline);
  await createRun(paths.candidate);
  const changed = new Uint8Array([100, 51, 10, 255]);
  await writeFile(join(paths.candidate, "window-0-before.rgba"), changed);
  const report = await compareTownRenderer(paths);
  failed(report, "candidate.window-0.before-digest");
  failed(report, "pair.0.0.before");
  assert.equal(report.pairs[0].quality[0].changedPixels, 1);
  assert.equal(report.pairs[0].quality[0].maxChannelError, 1);
});

test("rejects mismatched camera or native actor pose even when pixels match", async (t) => {
  const paths = await fixture(t);
  await createRun(paths.baseline);
  await createRun(paths.candidate);
  await editReport(paths.candidate, (report) => {
    report.windows[0].quality.before.camera.eye = [7, 4, 20];
    report.windows[1].quality.before.actors.poseSha256 = hash("different pose");
  });
  const report = await compareTownRenderer(paths);
  failed(report, "pair.0.0.before");
  failed(report, "pair.1.1.before");
  assert.equal(report.pairs[0].quality[0].exact, true);
});

test("retains unmatched rows and cannot pass partial pairs", async (t) => {
  const paths = await fixture(t);
  await createRun(paths.baseline);
  await createRun(paths.candidate, { keys: [0] });
  const report = await compareTownRenderer(paths);
  failed(report, "pairing.complete");
  assert.equal(report.pairs.length, 1);
  assert.equal(report.unmatched.length, 1);
});

test("invalidates forged pacing, changed sources, asset mismatch, and failed cleanup", async (t) => {
  const paths = await fixture(t);
  await createRun(paths.baseline);
  await createRun(paths.candidate);
  await editReport(paths.candidate, (report) => {
    report.windows[0].pacing.ok = true;
    report.sourceAfter = {
      ...report.sourceAfter,
      sha256: hash("changed source"),
    };
    report.assetClosure.sha256 = hash("other assets");
    report.cleanupErrors.push("worker remains");
  });
  const report = await compareTownRenderer(paths);
  failed(report, "candidate.window-0.pacing-recomputed");
  failed(report, "candidate.source-stable");
  failed(report, "candidate.source-after-digest");
  failed(report, "candidate.errors");
  failed(report, "asset-closure");
});

test("rejects raw-window identity changes and missing restored images without losing other pairs", async (t) => {
  const paths = await fixture(t);
  await createRun(paths.baseline);
  await createRun(paths.candidate);
  const path = join(paths.candidate, "window-0.json");
  const raw = JSON.parse(await readFile(path, "utf8"));
  raw.count = 200000;
  await writeFile(path, JSON.stringify(raw));
  await rm(join(paths.candidate, "window-1-restored.rgba"));
  const report = await compareTownRenderer(paths);
  failed(report, "candidate.window-0.raw-identity");
  failed(report, "candidate.window-1.restored-readable");
  assert.equal(report.pairs.length, 2);
  assert.equal(report.qualified, false);
});

test("cannot read evidence through traversal or a symlink outside its run directory", async (t) => {
  const paths = await fixture(t);
  await createRun(paths.baseline);
  await createRun(paths.candidate);
  await symlink(
    join(paths.baseline, "window-0-before.rgba"),
    join(paths.candidate, "escaped.rgba"),
  );
  await editReport(paths.candidate, (report) => {
    report.windows[0].quality.before.path = "../baseline/window-0-before.rgba";
    report.windows[1].quality.before.path = "escaped.rgba";
  });
  const report = await compareTownRenderer(paths);
  failed(report, "candidate.window-0.before-readable");
  failed(report, "candidate.window-1.before-readable");
});

test("keeps malformed stage and command measurements unavailable and reports their failures", async (t) => {
  const paths = await fixture(t);
  await createRun(paths.baseline);
  await createRun(paths.candidate, { commands: true });
  const path = join(paths.candidate, "window-0.json");
  const raw = JSON.parse(await readFile(path, "utf8"));
  raw.metrics.samples[0].renderCpuMs = -1;
  raw.metrics.samples[0].sourceRenderCommands.draws = -2;
  raw.metrics.resources.renderer.renderCommands = {
    scope: "all-world-commands",
    draws: 12,
    pipelines: 3,
    materials: 5,
    windBindings: 2,
  };
  await writeFile(path, JSON.stringify(raw));
  const report = await compareTownRenderer(paths);
  failed(report, "candidate.window-0.stage-renderCpuMs");
  failed(report, "candidate.window-0.commands-draws");
  failed(report, "candidate.window-0.last-source-command-scope");
  assert.equal(report.runs[1].windows[0].stages.renderCpuMs, null);
  assert.equal(
    report.runs[1].windows[0].sourceRenderCommands.values.draws,
    null,
  );
  assert.equal(report.pairs.length, 2);
});
