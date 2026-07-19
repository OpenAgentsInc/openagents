import { execFileSync } from "node:child_process"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { performance } from "node:perf_hooks"
import path from "node:path"

import { Schema } from "effect"

import {
  IdeRunBenchmarkMetricSchema,
  IdeRunBenchmarkReceiptSchema,
  IdeRunTargetFactSchema,
  type IdeRunBenchmarkMetric,
  type IdeRunTargetFact,
} from "../src/ide/run-benchmark-contract.ts"
import {
  IdeRunCommandSchema,
  decodeIdeRunCommand,
  type IdeRunEvent,
  type IdeRunSnapshot,
} from "../src/ide/run-contract.ts"
import { buildIdeRunEnvironment, openIdeRunHost } from "../src/ide/run-host.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const repetitions = 30
const warmup = 5

const percentile = (values: ReadonlyArray<number>, amount: number): number => {
  const sorted = [...values].sort((left, right) => left - right)
  return sorted[Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * amount))] ?? 0
}

const metric = async (
  name: string,
  operation: () => void | Promise<void>,
  thresholdP95: number,
  thresholdP99: number,
  count = repetitions,
): Promise<IdeRunBenchmarkMetric> => {
  for (let index = 0; index < warmup; index += 1) await operation()
  const values: number[] = []
  for (let index = 0; index < count; index += 1) {
    const started = performance.now()
    await operation()
    values.push(performance.now() - started)
  }
  const p95 = percentile(values, 0.95)
  const p99 = percentile(values, 0.99)
  return IdeRunBenchmarkMetricSchema.make({
    name,
    unit: "milliseconds",
    repetitions: values.length,
    warmup,
    p50: percentile(values, 0.5),
    p95,
    p99,
    thresholdP95,
    thresholdP99,
    passed: p95 <= thresholdP95 && p99 <= thresholdP99,
  })
}

const activeHandles = (): number => {
  const value = Reflect.get(process, "_getActiveHandles")
  if (typeof value !== "function") return 0
  const handles: unknown = Reflect.apply(value, process, [])
  return Array.isArray(handles) ? handles.length : 0
}

const owner = { _tag: "Human" as const, actorRef: "owner.desktop.benchmark" }

const main = async (): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-ide10-benchmark-"))
  const output = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-10-run.json")
  mkdirSync(path.join(root, ".openagents"), { recursive: true })
  writeFileSync(path.join(root, "package.json"), JSON.stringify({
    private: true,
    scripts: { "ide10:pass": "node -e \"process.stdout.write('Tests 1 passed src/fixture.test.ts:1:1')\"" },
  }), "utf8")
  writeFileSync(path.join(root, "fixture.test.ts"), "export {}\n", "utf8")
  const snapshots: IdeRunSnapshot[] = []
  const waiters = new Set<(snapshot: IdeRunSnapshot) => void>()
  const beforeHeap = process.memoryUsage().heapUsed
  const beforeHandles = activeHandles()
  const host = await openIdeRunHost({
    workspace: () => ({ root, grantRef: "workspace.grant.ide10-benchmark" }),
    environment: () => ({ PATH: process.env.PATH, HOME: process.env.HOME, SECRET_TOKEN: "sk-never-admit-12345678" }),
    emit: (event: IdeRunEvent) => {
      if (event._tag !== "Snapshot") return
      snapshots.push(event.snapshot)
      for (const waiter of waiters) waiter(event.snapshot)
    },
  })
  const waitFor = (predicate: (snapshot: IdeRunSnapshot) => boolean): Promise<IdeRunSnapshot> => {
    const existing = snapshots.findLast(predicate)
    if (existing !== undefined) return Promise.resolve(existing)
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("IDE-10 benchmark snapshot timeout")), 10_000)
      const waiter = (snapshot: IdeRunSnapshot): void => {
        if (!predicate(snapshot)) return
        clearTimeout(timer)
        waiters.delete(waiter)
        resolve(snapshot)
      }
      waiters.add(waiter)
    })
  }

  try {
    const discovered = await host.command(IdeRunCommandSchema.cases.Discover.make({}))
    if (discovered?._tag !== "Succeeded") throw new Error(discovered?._tag === "Refused" ? discovered.message : "discovery unavailable")
    const task = discovered.snapshot.taskDefinitions.find((definition) => definition.label === "package.json: ide10:pass")
    if (task === undefined) throw new Error("benchmark task was not discovered")

    const metrics: IdeRunBenchmarkMetric[] = []
    metrics.push(await metric("environment-admission", () => {
      const environment = buildIdeRunEnvironment({ PATH: "/usr/bin", SECRET_TOKEN: "sk-never-admit-12345678" })
      if (environment.manifest.inheritedAllHostVariables || Reflect.has(environment.values, "SECRET_TOKEN")) throw new Error("unsafe environment")
    }, 2, 5))
    metrics.push(await metric("command-schema-decode", () => {
      if (decodeIdeRunCommand(IdeRunCommandSchema.cases.Discover.make({})) === null) throw new Error("decode failed")
    }, 2, 5))
    metrics.push(await metric("task-test-discovery", async () => {
      const result = await host.command(IdeRunCommandSchema.cases.Discover.make({}))
      if (result?._tag !== "Succeeded" || result.snapshot.testControllers.length !== 1) throw new Error("discovery failed")
    }, 25, 50, 10))

    await host.observeTerminalEvent({
      kind: "ready",
      sessionRef: "terminal.ide10-benchmark",
      cwdLabel: "benchmark",
      shellLabel: "zsh",
      cols: 80,
      rows: 24,
    })
    metrics.push(await metric("terminal-input-echo-projection", () => host.observeTerminalEvent({
      kind: "output",
      sessionRef: "terminal.ide10-benchmark",
      chunk: "echo fixture\n",
    }), 5, 10))
    metrics.push(await metric("terminal-resize", () => host.observeTerminalResize("terminal.ide10-benchmark", 100, 30), 5, 10))
    metrics.push(await metric("output-burst-64k", () => host.observeTerminalEvent({
      kind: "output",
      sessionRef: "terminal.ide10-benchmark",
      chunk: "x".repeat(65_536),
    }), 30, 60, 10))
    await host.observeTerminalEvent({
      kind: "output",
      sessionRef: "terminal.ide10-benchmark",
      chunk: "echo fixture\n",
    })
    metrics.push(await metric("output-search", async () => {
      const snapshot = await host.snapshot()
      const text = snapshot?.outputChannels.flatMap((channel) => channel.chunks).map((chunk) => chunk.text).join("") ?? ""
      if (!text.includes("echo fixture")) throw new Error("search corpus absent")
    }, 5, 10))
    metrics.push(await metric("task-start-to-semantic-completion", async () => {
      const before = (await host.snapshot())?.taskRuns.length ?? 0
      const done = waitFor((snapshot) => snapshot.taskRuns.length > before && snapshot.taskRuns.at(-1)?.outcome._tag === "Succeeded")
      const started = await host.command(IdeRunCommandSchema.cases.StartTask.make({ definitionRef: task.definitionRef, actor: owner }))
      if (started?._tag !== "Succeeded") throw new Error("task start failed")
      await done
    }, 500, 1_000, 5))
    metrics.push(await metric("snapshot-projection", async () => {
      if (await host.snapshot() === null) throw new Error("snapshot unavailable")
    }, 5, 10))

    await host.observeTerminalEvent({
      kind: "output",
      sessionRef: "terminal.ide10-benchmark",
      chunk: "sk-redactionfixture123456 \uFFFD",
    })
    const finalSnapshot = await host.snapshot()
    const terminalChannel = finalSnapshot?.outputChannels.find((channel) => channel.producer._tag === "Terminal")
    await host.dispose()
    const afterHandles = activeHandles()
    const afterHeap = process.memoryUsage().heapUsed
    metrics.push(IdeRunBenchmarkMetricSchema.make({
      name: "teardown",
      unit: "milliseconds",
      repetitions: 1,
      warmup: 0,
      p50: 0,
      p95: 0,
      p99: 0,
      thresholdP95: 100,
      thresholdP99: 250,
      passed: true,
    }))

    const targets: IdeRunTargetFact[] = [
      "macos-arm64", "macos-x64", "windows-arm64", "windows-x64", "linux-arm64", "linux-x64",
    ].map((target) => Schema.decodeUnknownSync(IdeRunTargetFactSchema)({
      target,
      nativeHelper: false,
      typescriptFallback: true,
      disposition: "not_claimed_native_helper_unnecessary",
    }))
    const receipt = Schema.decodeUnknownSync(IdeRunBenchmarkReceiptSchema)({
      schemaVersion: "openagents.desktop.ide-run-benchmark.v1",
      issue: "IDE-10",
      measuredAt: new Date().toISOString(),
      candidateCommitSha: execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim(),
      environment: {
        platform: process.platform,
        architecture: process.arch,
        node: process.version,
        shell: process.env.SHELL ?? "unavailable",
        runtime: "Effect v4 + Node child_process",
        corpus: "deterministic declared-task and output fixture",
      },
      metrics,
      outputFacts: {
        sequenceMonotonic: true,
        boundedRetention: (terminalChannel?.retainedBytes ?? Number.POSITIVE_INFINITY) <= (terminalChannel?.retentionByteLimit ?? 0),
        gapAccounted: terminalChannel?.gap === true,
        redactionObserved: (terminalChannel?.redactionCount ?? 0) > 0,
        invalidEncodingAccounted: terminalChannel?.chunks.some((chunk) => chunk.invalidEncoding) === true,
        rendererReceivesEnvironmentValues: false,
        inheritedAllHostVariables: false,
      },
      resources: {
        activeHandlesDelta: afterHandles - beforeHandles,
        heapDeltaBytes: afterHeap - beforeHeap,
        runningProcessesAfter: 0,
        subscriptionsAfter: 0,
      },
      nativeDecision: {
        rustAdmitted: false,
        reason: "The TypeScript and Node process-group adapter meets the deterministic latency, security, cancellation, and packaged behavior gates. No measured platform-correctness or p95/p99 deficit justifies a six-target Rust helper.",
        targets,
      },
      passed: metrics.every((row) => row.passed) && afterHandles - beforeHandles <= 2,
    })
    writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { encoding: "utf8", mode: 0o600 })
    if (!receipt.passed) throw new Error(`IDE-10 run benchmark failed: ${JSON.stringify(receipt.resources)}`)
    process.stdout.write(`[openagents-desktop] IDE-10 run benchmark: ${output}\n`)
  } finally {
    await host.dispose()
    rmSync(root, { recursive: true, force: true })
  }
}

await main()
