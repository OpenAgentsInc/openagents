import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { Worker } from "node:worker_threads";

import { Effect, Exit, Schema } from "effect";

import {
  IdeLanguageCapabilitySchema,
  IdeLanguageRequestRefSchema,
  IdeLanguageRequestSchema,
  IdeLanguageStopRequestSchema,
} from "../src/ide/language-contract.ts";
import { IdeLanguageBenchmarkReceiptSchema, type IdeLanguageBenchmarkReceipt } from "../src/ide/language-benchmark-contract.ts";
import { makeIdeLanguageRequestFixture } from "../src/ide/language-fixture.ts";
import { makeIdeLanguageService } from "../src/ide/language-service.ts";
import { makeIdeLanguageWorkerProvider } from "../src/ide/language-worker-provider.ts";

const percentile = (values: ReadonlyArray<number>, amount: number): number => {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * amount))] ?? 0;
};

const metric = (values: ReadonlyArray<number>) => ({
  p50Ms: percentile(values, .5),
  p95Ms: percentile(values, .95),
  p99Ms: percentile(values, .99),
  maxMs: Math.max(...values),
});

const main = async (): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-ide-06-language-"));
  const src = path.join(root, "src");
  mkdirSync(src, { recursive: true });
  const fileCount = 150;
  let sourceBytes = 0;
  for (let index = 0; index < fileCount; index += 1) {
    const content = `export interface Value${index} { readonly ordinal: ${index}; readonly label: string }\nexport const value${index}: Value${index} = { ordinal: ${index}, label: \"value-${index}\" };\n`;
    sourceBytes += Buffer.byteLength(content);
    writeFileSync(path.join(src, `value-${index}.ts`), content, "utf8");
  }
  const activeContent = "import { value0 } from './value-0.js';\nexport const answer: number = value0.label;\nexport function describe(input: string): string { return `${input}:${value0.ordinal}`; }\n";
  sourceBytes += Buffer.byteLength(activeContent);
  writeFileSync(path.join(src, "index.ts"), activeContent, "utf8");

  const workers = new Set<Worker>();
  let workersStarted = 0;
  const workerUrl = new URL("../dist/workers/language-utility-worker.js", import.meta.url);
  const provider = makeIdeLanguageWorkerProvider(root, workerUrl, url => {
    const worker = new Worker(url);
    workersStarted += 1;
    workers.add(worker);
    worker.once("exit", () => workers.delete(worker));
    return worker;
  });
  const service = await Effect.runPromise(makeIdeLanguageService(provider));
  const base = makeIdeLanguageRequestFixture("benchmark");
  const capabilities = IdeLanguageCapabilitySchema.literals;
  const requestFor = (ordinal: number, capability: typeof IdeLanguageCapabilitySchema.Type) => IdeLanguageRequestSchema.make({
    ...base,
    requestRef: IdeLanguageRequestRefSchema.make(`ide.language-request.benchmark.${capability}.${ordinal}`),
    capability,
    content: activeContent,
    requestedAt: new Date().toISOString(),
    query: capability === "rename_preview" ? "renamedAnswer"
      : capability === "completion_resolve" ? "value0"
      : capability === "workspace_symbols" ? "value"
      : null,
    timeoutMs: 10_000,
  });

  try {
    const firstStarted = performance.now();
    const first = await Effect.runPromise(service.request(requestFor(0, "diagnostics")));
    const firstDiagnosticsMs = performance.now() - firstStarted;
    if (first.state._tag !== "Complete" || !first.items.some(item => item._tag === "Diagnostic")) {
      throw new Error("The real TypeScript worker did not return the expected diagnostic.");
    }
    const ready = await Effect.runPromise(service.snapshot());
    if (ready._tag !== "Ready") throw new Error("The project language service did not become ready.");

    const samples = 12;
    const diagnostics: number[] = [];
    const symbols: number[] = [];
    for (let index = 1; index <= samples; index += 1) {
      let started = performance.now();
      await Effect.runPromise(service.request(requestFor(index, "diagnostics")));
      diagnostics.push(performance.now() - started);
      started = performance.now();
      const result = await Effect.runPromise(service.request(requestFor(index, "document_symbols")));
      symbols.push(performance.now() - started);
      if (!result.items.some(item => item._tag === "Symbol")) throw new Error("Document symbol corpus was empty.");
    }
    for (let index = 0; index < capabilities.length; index += 1) {
      const capability = capabilities[index]!;
      const result = await Effect.runPromise(service.request(requestFor(500 + index, capability)));
      if (result.capability !== capability || result.state._tag === "Unavailable") {
        throw new Error(`Capability ${capability} did not complete through the real provider.`);
      }
    }

    const cancellationRuns = Array.from({ length: 100 }, (_, index) =>
      Effect.runPromise(service.request(requestFor(1_000 + index, "diagnostics"))));
    const cancellationResults = await Promise.all(cancellationRuns);
    const committed = cancellationResults.filter(result => result.state._tag === "Complete").length;

    const crashedWorker = [...workers].at(-1);
    if (crashedWorker === undefined) throw new Error("No worker existed for the crash fixture.");
    await crashedWorker.terminate();
    const restartStarted = performance.now();
    const crashExit = await Effect.runPromiseExit(service.request(requestFor(2_000, "diagnostics")));
    if (Exit.isSuccess(crashExit)) throw new Error("A dead provider was not surfaced to supervision.");
    const recovered = await Effect.runPromise(service.request(requestFor(2_001, "diagnostics")));
    const restartLatencyMs = performance.now() - restartStarted;
    const recoveredSnapshot = await Effect.runPromise(service.snapshot());
    if (recovered.state._tag !== "Complete" || recoveredSnapshot._tag !== "Ready") {
      throw new Error("The supervised provider did not recover.");
    }

    const stopped = await Effect.runPromise(service.stop(IdeLanguageStopRequestSchema.make({
      schemaVersion: "openagents.desktop.ide-language-stop.v1",
      grantRef: base.grantRef,
      reason: "project_closed",
    })));
    if (stopped._tag !== "Stopped") throw new Error("Language service did not reach Stopped.");
    await new Promise(resolve => setTimeout(resolve, 20));
    const diagnosticsMetric = metric(diagnostics);
    const symbolsMetric = metric(symbols);
    const receipt: IdeLanguageBenchmarkReceipt = Schema.decodeUnknownSync(IdeLanguageBenchmarkReceiptSchema)({
      schemaVersion: "openagents.desktop.ide-language-benchmark.v1",
      issue: "IDE-06",
      generatedAt: new Date().toISOString(),
      runtime: {
        node: process.version,
        platform: process.platform,
        arch: process.arch,
        providerVersion: ready.providerVersion,
        executable: ready.executable,
        placement: "project_local",
      },
      corpus: { files: fileCount + 1, sourceBytes, samples, capabilitiesExercised: capabilities },
      latency: { firstDiagnosticsMs, diagnostics: diagnosticsMetric, documentSymbols: symbolsMetric },
      cancellationFence: { scheduled: 100, committed, superseded: 100 - committed },
      restart: {
        crashObserved: true,
        recoveredServiceGeneration: recoveredSnapshot.serviceGeneration,
        restartLatencyMs,
      },
      resources: {
        workersStarted,
        activeWorkersAfter: workers.size,
        pendingRequestsAfter: stopped.activeRequests,
      },
      budgets: {
        firstDiagnosticsMs: 4_000,
        diagnosticsP95Ms: 750,
        documentSymbolsP95Ms: 750,
        restartMs: 4_000,
        passed: firstDiagnosticsMs <= 4_000 && diagnosticsMetric.p95Ms <= 750 &&
          symbolsMetric.p95Ms <= 750 && restartLatencyMs <= 4_000 && committed === 1 && workers.size === 0,
      },
      offline: { remoteRequests: 0 },
    });
    const output = path.resolve(import.meta.dirname, "../benchmarks/ide/2026-07-19-ide-06-language.json");
    writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
    if (!receipt.budgets.passed) throw new Error(`IDE-06 language budgets failed: ${JSON.stringify(receipt)}`);
    process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
  } finally {
    for (const worker of workers) await worker.terminate().catch(() => undefined);
    rmSync(root, { recursive: true, force: true });
  }
};

await main();
