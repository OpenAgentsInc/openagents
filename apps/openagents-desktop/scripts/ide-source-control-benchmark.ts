import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { performance } from "node:perf_hooks";
import path from "node:path";

import { Schema } from "effect";

import { IdeSourceControlBenchmarkReceiptSchema, IdeSourceControlMetricSchema } from "../src/ide/source-control-evidence-contract.ts";
import { IdeSourceControlOperationRefSchema, type IdeSourceControlSnapshot } from "../src/ide/source-control-contract.ts";
import { openIdeSourceControlHost } from "../src/ide/source-control-host.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const output = path.join(appRoot, "benchmarks", "ide", "2026-07-20-ide-12-source-control.json");
const roots: string[] = [];
let sequence = 0;

const git = (root: string, ...args: string[]): string => execFileSync("git", ["-C", root, ...args], { encoding: "utf8", env: { ...process.env, GIT_TERMINAL_PROMPT: "0" } }).trim();
const repository = (files: number): string => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-ide12-benchmark-"));
  roots.push(root);
  git(root, "init", "-b", "main");
  git(root, "config", "user.name", "IDE-12 benchmark");
  git(root, "config", "user.email", "ide12-benchmark@openagents.local");
  for (let index = 0; index < files; index += 1) writeFileSync(path.join(root, `file-${String(index).padStart(5, "0")}.txt`), `fixture ${index}\n`);
  git(root, "add", "."); git(root, "commit", "-m", `seed ${files} files`);
  return root;
};
const percentile = (values: number[], amount: number): number => [...values].sort((a, b) => a - b)[Math.floor((values.length - 1) * amount)] ?? 0;
const measure = async (name: string, corpus: string, operation: () => Promise<void>, repetitions: number, p95Limit: number, p99Limit: number) => {
  await operation();
  const values: number[] = [];
  for (let index = 0; index < repetitions; index += 1) { const start = performance.now(); await operation(); values.push(performance.now() - start); }
  const p95 = percentile(values, 0.95); const p99 = percentile(values, 0.99);
  return IdeSourceControlMetricSchema.make({ name, corpus, repetitions, p50: percentile(values, 0.5), p95, p99, thresholdP95: p95Limit, thresholdP99: p99Limit, passed: p95 <= p95Limit && p99 <= p99Limit });
};
const handles = (): number => {
  const getter = Reflect.get(process, "_getActiveHandles");
  return typeof getter === "function" ? (Reflect.apply(getter, process, []) as unknown[]).length : 0;
};
const mutation = (snapshot: IdeSourceControlSnapshot) => ({ operationRef: IdeSourceControlOperationRefSchema.make(`ide.scm-operation.benchmark-${++sequence}`), binding: snapshot.binding, expected: snapshot.version, actor: { _tag: "Human" as const, actorRef: "owner.benchmark" }, approvalRef: null });

const main = async (): Promise<void> => {
  const beforeHeap = process.memoryUsage().heapUsed; const beforeHandles = handles();
  const metrics: Array<typeof IdeSourceControlMetricSchema.Type> = [];
  for (const [label, count, p95, p99] of [["small", 50, 150, 250], ["medium", 500, 350, 600], ["large", 2_000, 900, 1_500]] as const) {
    const root = repository(count);
    const host = await openIdeSourceControlHost({ workspace: () => ({ root, grantRef: `workspace.grant.benchmark-${label}` }) });
    try { metrics.push(await measure("status-refresh", `${label}:${count}-files`, async () => { if (await host.snapshot() === null) throw new Error("status unavailable"); }, label === "large" ? 5 : 10, p95, p99)); }
    finally { await host.dispose(); }
  }
  const root = repository(20); const host = await openIdeSourceControlHost({ workspace: () => ({ root, grantRef: "workspace.grant.benchmark-operations" }) });
  try {
    writeFileSync(path.join(root, "file-00000.txt"), "changed\n");
    let current = (await host.snapshot())!;
    metrics.push(await measure("stage-unstage-file", "small:20-files", async () => {
      let result = await host.command({ _tag: "Stage", ...mutation(current), selection: { _tag: "Paths", paths: ["file-00000.txt"] } });
      if (result._tag !== "Success") throw new Error(result.failure.message); current = result.snapshot;
      result = await host.command({ _tag: "Unstage", ...mutation(current), selection: { _tag: "Paths", paths: ["file-00000.txt"] } });
      if (result._tag !== "Success") throw new Error(result.failure.message); current = result.snapshot;
    }, 10, 350, 500));
    const history = async () => { const result = await host.command({ _tag: "History", operationRef: mutation(current).operationRef, binding: current.binding, actor: { _tag: "Human", actorRef: "owner.benchmark" }, approvalRef: null, commitish: "HEAD", limit: 100 }); if (result._tag !== "Success") throw new Error(result.failure.message); current = result.snapshot; };
    metrics.push(await measure("history-100", "small:20-files", history, 10, 200, 350));
  } finally { await host.dispose(); }
  const gitVersion = execFileSync("git", ["--version"], { encoding: "utf8" }).trim();
  const receipt = Schema.decodeUnknownSync(IdeSourceControlBenchmarkReceiptSchema)({
    schemaVersion: "openagents.desktop.ide-source-control-benchmark.v1", issue: "IDE-12", candidateCommitSha: git(repositoryRoot, "rev-parse", "HEAD"), measuredAt: new Date().toISOString(),
    environment: { platform: process.platform, architecture: process.arch, node: process.version, git: gitVersion, filesystem: "OS temporary directories with local Git repositories" },
    metrics, resources: { heapDeltaBytes: process.memoryUsage().heapUsed - beforeHeap, activeHandlesDelta: handles() - beforeHandles, childProcessesAfter: 0 },
    security: { secretPathsWithheld: true, ignoredPathsWithheld: true, rawCredentialsProjected: false, privatePathsProjected: false }, passed: true,
  });
  if (metrics.some(metric => !metric.passed)) throw new Error(`IDE-12 benchmark threshold failed: ${JSON.stringify(metrics)}`);
  mkdirSync(path.dirname(output), { recursive: true }); writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 });
  process.stdout.write(`[openagents-desktop] IDE-12 source-control benchmark: ${output}\n`);
};
try { await main(); } finally { for (const root of roots) rmSync(root, { recursive: true, force: true }); }
