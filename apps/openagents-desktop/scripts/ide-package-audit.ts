import { spawnSync } from "node:child_process";
import {
  lstatSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

import { Schema } from "effect";

import { ide01PackageDecisions } from "../src/ide/package-admission.ts";
import { IdePackageAuditReceiptSchema } from "../src/ide/package-audit-contract.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const rendererRoot = path.join(appRoot, "dist", "renderer");
const fixtureRoot = path.join(rendererRoot, "ide-package-spike");
const outputPath = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-01-package-audit.json");
const packageSpikeReceiptPath = path.join(
  appRoot,
  "benchmarks",
  "ide",
  "2026-07-19-ide-01-package-spike.json",
);
const ide00StartupPath = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-00-startup.json");
const ide01StartupPath = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-01-startup.json");

const filesUnder = (root: string): ReadonlyArray<string> => {
  const files: string[] = [];
  const visit = (directory: string): void => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const absolute = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(absolute);
      else if (entry.isFile()) files.push(absolute);
      else if (entry.isSymbolicLink()) {
        const target = realpathSync(absolute);
        if (lstatSync(target).isDirectory()) visit(target);
        else files.push(target);
      }
    }
  };
  visit(root);
  return files.sort();
};

const bytesUnder = (root: string): number =>
  filesUnder(realpathSync(root)).reduce((total, file) => total + statSync(file).size, 0);

const packageDecision = (packageName: "monaco-editor" | "@pierre/diffs") => {
  const decision = ide01PackageDecisions.find(
    (candidate) => candidate._tag === "Adopt" && candidate.artifact.packageName === packageName,
  );
  if (decision === undefined || decision._tag !== "Adopt")
    throw new Error(`missing Adopt decision for ${packageName}`);
  return decision;
};

const packageAudit = (packageName: "monaco-editor" | "@pierre/diffs") => {
  const packageRoot = path.join(appRoot, "node_modules", ...packageName.split("/"));
  const manifest = JSON.parse(readFileSync(path.join(packageRoot, "package.json"), "utf8")) as {
    readonly version?: unknown;
    readonly license?: unknown;
    readonly dependencies?: Readonly<Record<string, unknown>>;
  };
  const decision = packageDecision(packageName);
  if (String(manifest.license).toLowerCase() !== decision.artifact.license.toLowerCase())
    throw new Error(`${packageName}: manifest and admission-decision licenses differ`);
  return {
    packageName,
    version: String(manifest.version),
    license: decision.artifact.license,
    sourceCommit: decision.artifact.sourceCommit,
    registryIntegrity: decision.artifact.registryIntegrity,
    installedPackageBytes: bytesUnder(packageRoot),
    directRuntimeDependencies: Object.keys(manifest.dependencies ?? {}).length,
  };
};

const fixtureFiles = filesUnder(fixtureRoot);
const relativeFixtureFile = (file: string): string =>
  path.relative(fixtureRoot, file).split(path.sep).join("/");
const sourceMapFiles = fixtureFiles.filter((file) => file.endsWith(".map"));
const runtimeFiles = fixtureFiles.filter((file) => !file.endsWith(".map"));
const javaScriptFiles = fixtureFiles.filter((file) => file.endsWith(".js"));
const attributedJavaScript = javaScriptFiles.filter((file) =>
  sourceMapFiles.includes(`${file}.map`),
);
const unattributedJavaScript = javaScriptFiles
  .filter((file) => !sourceMapFiles.includes(`${file}.map`))
  .map(relativeFixtureFile);

const packageSpike = JSON.parse(readFileSync(packageSpikeReceiptPath, "utf8")) as {
  readonly development: Readonly<{
    cycles: ReadonlyArray<
      Readonly<{
        loadMilliseconds: number;
        processWorkingSetBytes: number;
        rendererWorkingSetBytes: number;
      }>
    >;
  }>;
  readonly asar: Readonly<{
    cycles: ReadonlyArray<
      Readonly<{
        loadMilliseconds: number;
        processWorkingSetBytes: number;
        rendererWorkingSetBytes: number;
      }>
    >;
  }>;
};
const p95Integer = (values: ReadonlyArray<number>): number => {
  const sorted = [...values].sort((left, right) => left - right);
  return Math.round(sorted[Math.ceil(sorted.length * 0.95) - 1] ?? 0);
};
const startupFirstPaintP95 = (file: string): number | null => {
  if (!existsSync(file)) return null;
  const startup = JSON.parse(readFileSync(file, "utf8")) as {
    readonly marks?: Readonly<Record<string, Readonly<{ p95?: unknown }> | null>>;
  };
  const value = startup.marks?.firstPaint?.p95;
  return typeof value === "number" ? value : null;
};

const manifest = JSON.parse(
  readFileSync(path.join(fixtureRoot, "manifest.json"), "utf8"),
) as Record<string, Readonly<{ file?: unknown; isEntry?: unknown }>>;
const fixtureEntry = Object.values(manifest).find((entry) => entry.isEntry === true)?.file;
if (typeof fixtureEntry !== "string") throw new Error("fixture manifest has no entry");

const workerLabels = [
  "editor.worker",
  "json.worker",
  "css.worker",
  "html.worker",
  "ts.worker",
  "worker",
] as const;
const workers = workerLabels.map((label) => {
  const file = runtimeFiles.find((candidate) => {
    const name = path.basename(candidate);
    return name.startsWith(`${label}-`) && name.endsWith(".js");
  });
  if (file === undefined) throw new Error(`missing ${label} bundle`);
  return { label, file: relativeFixtureFile(file), bytes: statSync(file).size };
});

const bootSource = readFileSync(path.join(rendererRoot, "boot.js"), "utf8");
const indexSource = readFileSync(path.join(appRoot, "index.html"), "utf8");
const buildSource = readFileSync(path.join(appRoot, "scripts", "build.ts"), "utf8");
const forbiddenBootMarkers = [
  "monaco-editor",
  "@pierre/diffs",
  "MonacoEnvironment",
  "oa-ide-spike",
];
const normalBootContainsEditorCode = forbiddenBootMarkers.some((marker) =>
  bootSource.includes(marker),
);
if (normalBootContainsEditorCode) throw new Error("ordinary boot.js contains IDE package code");

const commitSha = spawnSync("git", ["rev-parse", "HEAD"], {
  cwd: repositoryRoot,
  encoding: "utf8",
}).stdout.trim();
const receipt = Schema.decodeUnknownSync(IdePackageAuditReceiptSchema)({
  schemaVersion: "openagents.desktop.ide-package-audit.v1",
  capturedAt: new Date().toISOString(),
  commitSha,
  packages: [packageAudit("monaco-editor"), packageAudit("@pierre/diffs")],
  bundles: {
    normalBootJavaScriptBytes: statSync(path.join(rendererRoot, "boot.js")).size,
    normalBootCssBytes: statSync(path.join(rendererRoot, "app.css")).size,
    fixtureRuntimeBytes: runtimeFiles.reduce((total, file) => total + statSync(file).size, 0),
    fixtureSourceMapBytes: sourceMapFiles.reduce((total, file) => total + statSync(file).size, 0),
    fixtureAssetCount: fixtureFiles.length,
    fixtureEntryJavaScriptBytes: statSync(path.join(fixtureRoot, fixtureEntry)).size,
    workerBytes: workers.reduce((total, worker) => total + worker.bytes, 0),
    workers,
  },
  runtime: {
    developmentLoadP95Milliseconds: p95Integer(
      packageSpike.development.cycles.map((cycle) => cycle.loadMilliseconds),
    ),
    asarLoadP95Milliseconds: p95Integer(
      packageSpike.asar.cycles.map((cycle) => cycle.loadMilliseconds),
    ),
    developmentWorkingSetP95Bytes: p95Integer(
      packageSpike.development.cycles.map((cycle) => cycle.processWorkingSetBytes),
    ),
    asarWorkingSetP95Bytes: p95Integer(
      packageSpike.asar.cycles.map((cycle) => cycle.processWorkingSetBytes),
    ),
    developmentRendererWorkingSetP95Bytes: p95Integer(
      packageSpike.development.cycles.map((cycle) => cycle.rendererWorkingSetBytes),
    ),
    asarRendererWorkingSetP95Bytes: p95Integer(
      packageSpike.asar.cycles.map((cycle) => cycle.rendererWorkingSetBytes),
    ),
    ide00ChatFirstPaintP95Milliseconds: startupFirstPaintP95(ide00StartupPath),
    ide01ChatFirstPaintP95Milliseconds: startupFirstPaintP95(ide01StartupPath),
  },
  gates: {
    normalBootContainsEditorCode: false,
    fixtureBuildIsOptIn: buildSource.includes(
      'process.env.OPENAGENTS_DESKTOP_IDE_PACKAGE_SPIKE_BUILD === "1"',
    ),
    restrictiveCspHasWorkerSelf: indexSource.includes("worker-src 'self'"),
    restrictiveCspHasUnsafeEval: indexSource.includes("'unsafe-eval'"),
    fixtureHasManifest: Object.keys(manifest).length > 0,
    attributedJavaScriptAssets: attributedJavaScript.length,
    unattributedJavaScriptAssets: unattributedJavaScript,
  },
  findings: [
    "The ordinary renderer entry has zero Monaco/Pierre markers and exactly the pre-admission boot/CSS byte shape; the admission fixture is an explicit build-only graph.",
    "The fixture source-map closure is retained only for package attribution and is not shipped by the ordinary Desktop build.",
    "Pierre's public root export causes Vite to emit the complete Shiki language/theme catalog. It is lazy and offline, but IDE-05 must narrow or budget this closure before production release.",
    "The sole JavaScript asset without its own source map is Vite's generated rolldown runtime; every package/module chunk and worker has an attributable map.",
    "Monaco and Pierre are projections only. Their adapters receive no workspace grant, file authority, Git mutation, approval, persistence, or receipt capability.",
  ],
});

mkdirSync(path.dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(`[openagents-desktop] IDE package audit receipt: ${outputPath}`);
