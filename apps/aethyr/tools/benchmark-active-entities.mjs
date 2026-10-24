/** Browser CPU benchmark; run through the existing isolated browser wrapper. */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { cpus, platform, release } from "node:os";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";
import {
  balancedOrder,
  distribution,
  createMeasurementReport,
} from "../experiments/c-town/measurement.mjs";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const git = (...args) => execFileSync("git", args, { cwd: root });
const args = new Map();
if (process.argv.includes("--help")) {
  console.log(
    "node tools/benchmark-active-entities.mjs --out DIR [--counts 1000,10000,100000] [--steps 32] [--rounds 4] [--warmup 5] [--budget-ms 10000]",
  );
  process.exit(0);
}
for (let index = 2; index < process.argv.length; index += 2) {
  const name = process.argv[index],
    value = process.argv[index + 1];
  assert(
    [
      "--out",
      "--counts",
      "--steps",
      "--rounds",
      "--warmup",
      "--budget-ms",
    ].includes(name) &&
      value &&
      !value.startsWith("--") &&
      !args.has(name),
    "Choose each supported benchmark option once with a value.",
  );
  args.set(name, value);
}
assert(args.has("--out"), "Choose a private benchmark output directory.");
await mkdir(resolve(args.get("--out")), { recursive: true });
const output = await realpath(resolve(args.get("--out")));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep raw measurements outside the checkout.",
);
const options = {
  counts: (args.get("--counts") ?? "1000,10000,100000").split(",").map(Number),
  steps: Number(args.get("--steps") ?? 32),
  rounds: Number(args.get("--rounds") ?? 4),
  warmupSteps: Number(args.get("--warmup") ?? 5),
  budgetMs: Number(args.get("--budget-ms") ?? 10000),
};
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run through openagents browser run.");
async function sourcePin() {
  const files = [
    "experiments/c-town/active-entities.ts",
    "tools/fixtures/active-entities.ts",
    "tools/fixtures/active-entities.html",
    "tools/benchmark-active-entities.mjs",
    "experiments/c-town/measurement.mjs",
    "experiments/c-town/measurement.d.mts",
    "package-lock.json",
    "tsconfig.json",
    "experiments/c-town/config.ts",
  ];
  const sourceFiles = {};
  for (const path of files.sort())
    sourceFiles[path] = hash(await readFile(resolve(root, path)));
  const untracked = {};
  for (const path of git("ls-files", "--others", "--exclude-standard", "-z")
    .toString("utf8")
    .split("\0")
    .filter(Boolean)
    .sort())
    if (/\.(?:[cm]?[jt]sx?|json|lock|html|css|toml)$/.test(path))
      untracked[path] = hash(await readFile(resolve(root, path)));
  const patch = git("diff", "HEAD", "--binary");
  return {
    revision: git("rev-parse", "HEAD").toString("utf8").trim(),
    dirty: git("status", "--porcelain").length > 0,
    patchSha256: hash(
      Buffer.concat([patch, Buffer.from(JSON.stringify(untracked))]),
    ),
    sourceFiles,
    sourceFilesSha256: hash(JSON.stringify(sourceFiles)),
    untrackedSourceHashes: untracked,
  };
}
const startedAt = new Date().toISOString(),
  source = await sourcePin();
const report = {
  schema: "aethyrforge.active-entities-browser.v1",
  ok: false,
  startedAt,
  completedAt: null,
  source,
  options,
  browser: null,
  runtime: process.version,
  machine: {
    os: `${platform()} ${release()}`,
    cpuModel: cpus()[0]?.model ?? null,
    logicalCpus: cpus().length,
  },
  fixture: null,
  errors: [],
  warnings: [],
  failure: null,
};
let server, browser, context, page;
try {
  server = await createServer({
    root,
    configFile: false,
    logLevel: "error",
    cacheDir: `node_modules/.vite-active-${process.pid}`,
    server: { host: "127.0.0.1", port: 0, strictPort: false },
  });
  await server.listen();
  browser = await chromium.connectOverCDP(endpoint);
  report.browser = await browser.version();
  context = await browser.newContext({ viewport: { width: 640, height: 360 } });
  page = await context.newPage();
  page.on("pageerror", (error) => report.errors.push(String(error)));
  page.on("console", (message) => {
    if (
      ["warning", "error"].includes(message.type()) &&
      report.warnings.length < 64
    )
      report.warnings.push(message.text().slice(0, 2048));
  });
  await page.goto(
    new URL(
      "/tools/fixtures/active-entities.html",
      server.resolvedUrls.local[0],
    ).href,
    { waitUntil: "domcontentloaded" },
  );
  await page.waitForFunction(() => !!window.activeEntityHarness, null, {
    timeout: 30000,
  });
  report.fixture = await page.evaluate(
    (options) => window.activeEntityHarness.run(options),
    options,
  );
  assert.equal(report.fixture.ok, true);
  assert.deepEqual(
    report.fixture.orders,
    balancedOrder(["off", "on"], options.rounds),
  );
  assert.equal(
    report.fixture.samples.length,
    options.counts.length * options.rounds * 2,
  );
  assert.equal(report.fixture.populations.length, options.counts.length);
  for (const population of report.fixture.populations) {
    assert.equal(population.negatives.length, 5);
    assert(
      population.negatives.every(
        (negative) =>
          negative.rejected &&
          negative.hashRecomputedAfterFault &&
          negative.actualHash !== negative.expectedHash,
      ),
    );
    const samples = report.fixture.samples.filter(
      (sample) => sample.count === population.count,
    );
    assert(
      samples.every(
        (sample) =>
          sample.exact &&
          sample.stateHash === population.referenceHash &&
          sample.positionChangedEntities === population.count,
      ),
    );
    console.log(
      `PASS ${population.count} active entities: ${samples.length} balanced samples, exact independent state/bounds, five negative controls`,
    );
  }
  assert.deepEqual(
    (await sourcePin()).sourceFiles,
    source.sourceFiles,
    "Benchmark source changed during measurement.",
  );
  assert.equal(
    report.errors.length,
    0,
    "The CPU fixture reported an uncaught browser error.",
  );
  report.ok = true;
} catch (error) {
  report.failure = String(error);
  if (page)
    report.fixture = await page
      .evaluate(() => window.activeEntityHarness?.latest())
      .catch(() => null);
  process.exitCode = 1;
} finally {
  await context?.close();
  await browser?.close();
  await server?.close();
  report.completedAt = new Date().toISOString();
  const rawBytes = Buffer.from(JSON.stringify(report, null, 2) + "\n");
  await writeFile(resolve(output, "active-entities-raw.json"), rawBytes);
  const limits = [
    "One local device and browser process; no independent second-device profile has been collected. Contention is retained without filtering slow samples.",
    "This CPU-only fixture uses Vite-transformed TypeScript, not a production-rendered or Wasm gameplay workload. It opens no GPU context and measures no presentation rate.",
    "State verification, reference simulation and hashes are outside the timed batch. Per-step batch averages differ from instrumented individual-step observations.",
    "No browser/process heap peak is available; typed-buffer accounting excludes JavaScript objects, GC, browser, driver and reference-object heap.",
  ];
  const phases = (report.fixture?.populations ?? []).map((population) => {
    const samples = report.fixture.samples.filter(
      (sample) => sample.count === population.count,
    );
    const off = distribution(
      samples
        .filter((sample) => sample.instrumentation === "off")
        .map((sample) => sample.msPerStep),
    );
    const on = distribution(
      samples
        .filter((sample) => sample.instrumentation === "on")
        .map((sample) => sample.msPerStep),
    );
    const frames = samples.flatMap((sample) => sample.frames);
    return {
      id: `active-${population.count}`,
      kind: "simulation-cpu",
      scope: report.fixture.algorithm.scope,
      metrics: {
        count: population.count,
        batchAverageStepMs: { off, on },
        instrumentationOverheadFraction:
          off && on && off.p50 > 0 ? (on.p50 - off.p50) / off.p50 : null,
        instrumentedStepMs: distribution(
          frames.map((frame) => frame.simulationMs),
        ),
        instrumentedBoundsMs: distribution(
          frames.map((frame) => frame.boundsMs),
        ),
        referenceHash: population.referenceHash,
        negativeControlsRejected: population.negatives.length,
        explicitStorage: samples[0]?.storage ?? null,
        missingBrowserHeapPeak: true,
      },
    };
  });
  if (!phases.length)
    phases.push({
      id: "active-failed",
      kind: "simulation-cpu",
      scope: "No complete population was qualified.",
      metrics: { failure: report.failure },
    });
  const build = hash(JSON.stringify(source.sourceFiles));
  const summary = createMeasurementReport({
    id: "active-entities-current-baseline",
    ok: report.ok,
    startedAt,
    finishedAt: report.completedAt,
    environment: { browser: report.browser, os: report.machine.os, gpu: null },
    pins: {
      source: {
        revision: source.revision,
        dirty: source.dirty,
        patchSha256: source.patchSha256,
      },
      assets: {
        sha256: null,
        scope: "No assets; seeded owned CPU entities only.",
      },
      kernel: { sha256: null, abi: null },
      wgsl: { sha256: null, scope: "No renderer or GPU program is loaded." },
      quality: { sha256: hash(JSON.stringify(options)), settings: options },
      build: {
        mode: "vite-development-cpu-fixture",
        sha256: build,
        toolchain: {
          node: process.version,
          packageLockSha256: source.sourceFiles["package-lock.json"],
        },
      },
    },
    cache: {
      state: "not-applicable",
      scope:
        "Fixture code is loaded once before CPU timing. No asset network activity occurs in timed intervals; OS/browser caches are not flushed.",
    },
    warmup: { durationMs: null, iterations: options.warmupSteps },
    trialOrder: report.fixture?.orders ?? [],
    phases,
    evidence: [
      {
        path: "active-entities-raw.json",
        sha256: hash(rawBytes),
        bytes: rawBytes.length,
        schema: report.schema,
        issue: 7,
        limitations: limits,
      },
    ],
    limitations: limits,
    details: {
      sourceFiles: source.sourceFiles,
      sourceFilesSha256: source.sourceFilesSha256,
      algorithm: report.fixture?.algorithm ?? null,
      timing: report.fixture?.timing ?? null,
      sampleCount: report.fixture?.samples?.length ?? 0,
      fixtureElapsedMs: report.fixture?.elapsedMs ?? null,
      machine: report.machine,
      failures: report.failure ? [report.failure] : [],
    },
  });
  await writeFile(
    resolve(output, "active-entities-measurement.json"),
    JSON.stringify(summary, null, 2) + "\n",
  );
  console.log(
    JSON.stringify({
      ok: report.ok,
      samples: report.fixture?.samples?.length ?? 0,
      elapsedMs: report.fixture?.elapsedMs ?? null,
      failure: report.failure,
    }),
  );
}
