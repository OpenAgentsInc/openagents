/** Run through openagents browser run. Raw samples and licensed captures stay outside Git. */
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdir,
  readFile,
  realpath,
  readdir,
  writeFile,
} from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import {
  archiveEvidence,
  createMeasurementReport,
  validateMeasurementReport,
} from "../experiments/c-town/measurement.mjs";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
const options = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  const name = process.argv[i],
    value = process.argv[i + 1];
  assert(
    ["--out", "--town", "--routes", "--cpu-budget-ms"].includes(name) &&
      value &&
      !options.has(name),
    "Use each named benchmark option once with a value.",
  );
  options.set(name, value);
}
assert(options.has("--out"), "Choose an external output directory.");
await mkdir(resolve(options.get("--out")), { recursive: true });
const output = await realpath(resolve(options.get("--out")));
assert(
  output !== root && !output.startsWith(root + sep),
  "Keep benchmark data outside Git.",
);
assert(
  (await readdir(output)).length === 0,
  "Choose a fresh empty output directory so a failed run cannot reuse old evidence.",
);
assert(
  process.env.OPENAGENTS_CHROME_WS || process.env.OPENAGENTS_CHROME_PORT,
  "Run through openagents browser run.",
);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const readJSON = async (path) => JSON.parse(await readFile(path, "utf8"));
const saveJSON = (path, value) =>
  writeFile(path, JSON.stringify(value, null, 2) + "\n");
const startedAt = new Date().toISOString(),
  started = performance.now();
const stages = [];
let failure;
async function child(name, script, args = [], env = {}) {
  const directory = resolve(output, name);
  await mkdir(directory, { recursive: true });
  const stage = { name, startedAt: new Date().toISOString(), ok: false };
  stages.push(stage);
  const at = performance.now();
  try {
    await new Promise((done, reject) => {
      const process = spawn(
        globalThis.process.execPath,
        [resolve(root, script), ...args],
        {
          cwd: root,
          env: { ...globalThis.process.env, AF_CAPTURE_DIR: directory, ...env },
          stdio: "inherit",
        },
      );
      process.once("error", reject);
      process.once("exit", (code, signal) =>
        code === 0
          ? done()
          : reject(new Error(`${name} failed (${signal ?? code}).`)),
      );
    });
    stage.ok = true;
  } finally {
    stage.elapsedMs = performance.now() - at;
    stage.finishedAt = new Date().toISOString();
  }
}
const evidencePaths = [2, 3, 4, 5, 6].map((n) =>
  resolve(root, `experiments/kaigen/results/issue-${n}.json`),
);
try {
  await child("cpu", "tools/benchmark-active-entities.mjs", [
    "--out",
    resolve(output, "cpu"),
    "--budget-ms",
    options.get("--cpu-budget-ms") ?? "30000",
  ]);
  for (const world of [
    "starter",
    ...(options.has("--town") ? ["medieval-town"] : []),
  ])
    await child(world, "tools/benchmark-runtime.mjs", [], {
      AF_VERIFY_WORLD: world,
      AF_RUNTIME_ROUTES: options.get("--routes") ?? "static,sway",
      AF_MEASUREMENT_EVIDENCE: JSON.stringify(evidencePaths),
      ...(world === "medieval-town"
        ? { AETHYRFORGE_PRIVATE_ASSETS: await realpath(options.get("--town")) }
        : {}),
    });
} catch (error) {
  failure = String(error);
}

const artifacts = [];
for (const [directory, file] of [
  ["cpu", "active-entities-measurement.json"],
  ["starter", "starter-runtime-measurement.json"],
  ...(options.has("--town")
    ? [["medieval-town", "medieval-town-runtime-measurement.json"]]
    : []),
]) {
  try {
    const path = resolve(output, directory, file),
      bytes = await readFile(path);
    const report = validateMeasurementReport(JSON.parse(bytes));
    artifacts.push({ path, bytes, report });
    assert.equal(report.ok, true, `Child measurement did not pass: ${path}`);
  } catch (error) {
    if (!failure) failure = String(error);
  }
}
const manifest = {
  schema: "aethyrforge.benchmark-suite.v1",
  ok: !failure,
  startedAt,
  finishedAt: new Date().toISOString(),
  elapsedMs: performance.now() - started,
  stages,
  artifacts: artifacts.map(({ path, bytes, report }) => ({
    path,
    sha256: hash(bytes),
    schema: report.schema,
    ok: report.ok,
  })),
  ...(failure ? { failure } : {}),
  scope:
    "CPU-only fixture completes before sequential owned/Town GPU worlds. Dedicated Chrome process persists; context/worker/server cleanup remains each child tool's responsibility. Prior workstream evidence retains its original source and measurement boundaries.",
};
await saveJSON(resolve(output, "suite-native.json"), manifest);
if (!failure) {
  const integrated = artifacts.filter(({ report }) =>
    report.phases.some((phase) => phase.kind === "integrated"),
  );
  const base = integrated[0]?.report ?? artifacts[0].report;
  const prior = await Promise.all(
    evidencePaths.map(async (path, index) => ({
      path: `experiments/kaigen/results/issue-${index + 2}.json`,
      bytes: new Uint8Array(await readFile(path)),
      issue: index + 2,
      limitations: [
        "Historical native evidence, qualified only for its recorded source, device, quality and timing boundaries; it is not a paired current-source comparison.",
      ],
    })),
  );
  const pins = structuredClone(base.pins);
  pins.assets = {
    sha256: hash(
      JSON.stringify(integrated.map(({ report }) => report.pins.assets)),
    ),
    scope:
      "Composite owned and optional private Town asset pins; individual runs retain their precise scopes.",
  };
  pins.source.suite = {
    revision: execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
    }).trim(),
    fileSha256: hash(await readFile(fileURLToPath(import.meta.url))),
  };
  const common = createMeasurementReport({
    id: "kaigen-representative-suite",
    ok: true,
    startedAt,
    finishedAt: manifest.finishedAt,
    environment: base.environment,
    pins,
    cache: {
      state: "mixed",
      scope:
        "A dedicated Chrome process persists across CPU and sequential GPU checks; fresh-context and same-context reload observations are labeled by each run. Preflight asset hashing reads the entire selected closure and warms the OS file cache. OS, driver, shader and Wasm caches are not reset.",
    },
    warmup: { durationMs: null, iterations: null },
    trialOrder: stages.map(({ name }) => [name]),
    phases: artifacts.map(({ path, bytes, report }) => ({
      id: report.id,
      kind: report.phases.some((phase) => phase.kind === "integrated")
        ? "integrated"
        : "simulation-cpu",
      scope:
        "Native shared report referenced exactly; its phase boundaries, raw distributions, validation and limitations remain authoritative.",
      metrics: {
        sha256: hash(bytes),
        path: resolve(path).slice(output.length + 1),
        phases: report.phases,
      },
    })),
    evidence: archiveEvidence(prior),
    limitations: [
      "One Mac/device profile. An independent second device has not been measured; no broad hardware or language conclusion is supported.",
      "The active transform workload is a fixed CPU micro-workload, not 100k complete game entities.",
      "Callbacks, submissions, main-HDR GPU timing and presentation are distinct. Displayed frame rate and isolated GPU upload time are unavailable.",
      "Optional timer overhead is compared above the shared recorder and normal product counters; the observer's total cost is not eliminated.",
      "Historical streaming/layout/worker/agent/player evidence is linked by exact bytes and its own source, not relabeled as a current paired comparison.",
      "Complete raw samples, failures and licensed captures remain in the external output directories.",
    ],
  });
  await saveJSON(resolve(output, "suite-measurement.json"), common);
}
console.log(
  JSON.stringify({
    ok: !failure,
    stages: stages.map(({ name, ok, elapsedMs }) => ({ name, ok, elapsedMs })),
    ...(failure ? { failure } : {}),
  }),
);
if (failure) throw new Error(failure);
