import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { Exit, Schema } from "effect";

import {
  IdeBaselineReceiptSchema,
  type IdeBaselineGap,
  type IdeBaselineMetric,
} from "../src/ide/baseline-contract.ts";
import {
  emptyWorkspaceEditorState,
  workspaceEditorRecoverySnapshot,
  withWorkspaceEditorEvent,
  withWorkspaceEditorOpened,
  withWorkspaceEditorOpening,
} from "../src/renderer/workspace-editor.ts";
import { makeTerminalHost, type TerminalBackend } from "../src/terminal-host.ts";
import {
  openWorkspaceDocument,
  saveWorkspaceDocument,
  searchWorkspace,
  workspaceGitDiff,
  workspaceGitStatus,
  workspaceTreePage,
} from "../src/workspace-service.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const defaultOutput = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-00-baseline.json");

type BaselineArgs = Readonly<{
  output: string;
  startupReceipt: string;
  repetitions: number;
  fixtureFiles: number;
}>;

const parseArgs = (argv: ReadonlyArray<string>): BaselineArgs => {
  let output = defaultOutput;
  let startupReceipt = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-00-startup.json");
  let repetitions = 11;
  let fixtureFiles = 2_000;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--out") output = path.resolve(String(argv[++index]));
    else if (argument === "--startup-receipt") startupReceipt = path.resolve(String(argv[++index]));
    else if (argument === "--repetitions")
      repetitions = Math.max(3, Math.trunc(Number(argv[++index])));
    else if (argument === "--fixture-files")
      fixtureFiles = Math.max(100, Math.trunc(Number(argv[++index])));
  }
  return { output, startupReceipt, repetitions, fixtureFiles };
};

const percentile = (values: ReadonlyArray<number>, fraction: number): number => {
  const sorted = [...values].sort((left, right) => left - right);
  const rank = fraction * (sorted.length - 1);
  const low = Math.floor(rank);
  const high = Math.ceil(rank);
  const lowValue = sorted.at(low) ?? 0;
  const highValue = sorted.at(high) ?? lowValue;
  return lowValue + (highValue - lowValue) * (rank - low);
};

const round3 = (value: number): number => Math.round(value * 1_000) / 1_000;

const metricFromSamples = (
  metric: string,
  category: IdeBaselineMetric["category"],
  unit: IdeBaselineMetric["unit"],
  samples: ReadonlyArray<number>,
  sourceRef: string,
  noise: string,
): IdeBaselineMetric => ({
  metric,
  category,
  unit,
  repetitions: samples.length,
  p50: round3(percentile(samples, 0.5)),
  p95: round3(percentile(samples, 0.95)),
  p99: round3(percentile(samples, 0.99)),
  minimum: round3(Math.min(...samples)),
  maximum: round3(Math.max(...samples)),
  sourceRef,
  noise,
});

const timeSamples = (repetitions: number, operation: () => void): ReadonlyArray<number> =>
  Array.from({ length: repetitions }, () => {
    const startedAt = performance.now();
    operation();
    return performance.now() - startedAt;
  });

const makeFixture = (fixtureFiles: number): string => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-ide-baseline-"));
  const directoryCount = 25;
  for (let directoryIndex = 0; directoryIndex < directoryCount; directoryIndex += 1) {
    const directory = path.join(root, `module-${String(directoryIndex).padStart(2, "0")}`);
    mkdirSync(directory);
  }
  for (let fileIndex = 0; fileIndex < fixtureFiles; fileIndex += 1) {
    const directory = path.join(
      root,
      `module-${String(fileIndex % directoryCount).padStart(2, "0")}`,
    );
    writeFileSync(
      path.join(directory, `fixture-${String(fileIndex).padStart(5, "0")}.ts`),
      `export const fixture${fileIndex} = "ide-baseline-needle-${fileIndex}"\n`,
    );
  }
  writeFileSync(path.join(root, "README.md"), "# IDE baseline fixture\n");
  execFileSync("git", ["init", "--quiet"], { cwd: root });
  execFileSync("git", ["add", "."], { cwd: root });
  execFileSync(
    "git",
    [
      "-c",
      "user.name=OpenAgents Baseline",
      "-c",
      "user.email=baseline@openagents.invalid",
      "commit",
      "--quiet",
      "-m",
      "fixture baseline",
    ],
    { cwd: root },
  );
  writeFileSync(path.join(root, "README.md"), "# IDE baseline fixture\nchanged\n");
  return root;
};

const terminalProbeBackend = (): TerminalBackend => ({
  spawn: () => ({
    pid: 1,
    write: () => undefined,
    resize: () => undefined,
    interrupt: () => undefined,
    kill: () => undefined,
    onData: () => undefined,
    onExit: () => undefined,
  }),
});

const relativeReceiptRef = (file: string): string =>
  path.relative(repositoryRoot, file).split(path.sep).join("/");

const startupMetrics = (
  receiptPath: string,
): Readonly<{ metrics: ReadonlyArray<IdeBaselineMetric>; gaps: ReadonlyArray<IdeBaselineGap> }> => {
  try {
    const parsed: unknown = JSON.parse(readFileSync(receiptPath, "utf8"));
    if (typeof parsed !== "object" || parsed === null || !("marks" in parsed))
      throw new Error("missing marks");
    const marks = Reflect.get(parsed, "marks");
    if (typeof marks !== "object" || marks === null) throw new Error("invalid marks");
    const metrics = Object.entries(marks).flatMap(([name, aggregate]) => {
      if (typeof aggregate !== "object" || aggregate === null) return [];
      const p50 = Reflect.get(aggregate, "p50");
      const p95 = Reflect.get(aggregate, "p95");
      const p99 = Reflect.get(aggregate, "p99");
      const minimum = Reflect.get(aggregate, "min");
      const maximum = Reflect.get(aggregate, "max");
      const repetitions = Reflect.get(aggregate, "n");
      if ([p50, p95, p99, minimum, maximum, repetitions].some((value) => typeof value !== "number"))
        return [];
      return [
        {
          metric: `startup.${name}`,
          category: "latency" as const,
          unit: "milliseconds" as const,
          repetitions,
          p50,
          p95,
          p99,
          minimum,
          maximum,
          sourceRef: relativeReceiptRef(receiptPath),
          noise:
            "Real Electron process launches with fresh fixture userData; OS caches and scheduler load are uncontrolled.",
        },
      ];
    });
    return {
      metrics,
      gaps: [
        {
          probe: "startup.chat-only-cost",
          status: "partially_measured",
          reason:
            "The deterministic startup route is measured, but an isolated chat-only versus editor-import differential requires IDE-01 chunking.",
          plannedPacket: "IDE-01",
        },
      ],
    };
  } catch {
    return {
      metrics: [],
      gaps: [
        {
          probe: "startup.app-window-editor-route",
          status: "unmeasured",
          reason:
            "The Electron startup receipt was not supplied; run startup-bench before this harness.",
          plannedPacket: "IDE-07",
        },
      ],
    };
  }
};

const main = (): void => {
  const args = parseArgs(process.argv.slice(2));
  const fixtureRoot = makeFixture(args.fixtureFiles);
  const grantRef = "workspace.grant.ide-baseline";
  const metrics: IdeBaselineMetric[] = [];
  const rawSamples: Record<string, ReadonlyArray<number>> = {};
  const addLatency = (
    name: string,
    sourceRef: string,
    noise: string,
    operation: () => void,
  ): void => {
    const samples = timeSamples(args.repetitions, operation);
    rawSamples[name] = samples.map(round3);
    metrics.push(metricFromSamples(name, "latency", "milliseconds", samples, sourceRef, noise));
  };

  try {
    addLatency(
      "workspace.tree.first-page",
      "apps/openagents-desktop/src/workspace-service.ts#workspaceTreePage",
      "Warm filesystem cache is not flushed; the first sample is retained in raw results.",
      () => {
        workspaceTreePage({ root: fixtureRoot, grantRef, directoryRef: "", limit: 80 });
      },
    );
    addLatency(
      "workspace.tree.large-repository-traversal",
      "apps/openagents-desktop/src/workspace-service.ts#workspaceTreePage",
      "Traverses every fixture directory page; filesystem cache and directory enumeration vary by host.",
      () => {
        for (let index = 0; index < 25; index += 1) {
          workspaceTreePage({
            root: fixtureRoot,
            grantRef,
            directoryRef: `module-${String(index).padStart(2, "0")}`,
            limit: 100,
          });
        }
      },
    );
    addLatency(
      "workspace.search.path",
      "apps/openagents-desktop/src/workspace-service.ts#searchWorkspace",
      "Bounded deterministic fixture; filesystem cache is uncontrolled.",
      () => {
        searchWorkspace({ root: fixtureRoot, grantRef, query: "fixture-019", mode: "path" });
      },
    );
    addLatency(
      "workspace.search.content",
      "apps/openagents-desktop/src/workspace-service.ts#searchWorkspace",
      "Git candidate enumeration and filesystem cache are host-dependent.",
      () => {
        searchWorkspace({ root: fixtureRoot, grantRef, query: "needle-1999", mode: "content" });
      },
    );
    addLatency(
      "workspace.document.open",
      "apps/openagents-desktop/src/workspace-service.ts#openWorkspaceDocument",
      "4 KiB-or-smaller UTF-8 fixture on local temporary storage.",
      () => {
        openWorkspaceDocument(fixtureRoot, grantRef, {
          grantRef,
          pathRef: "module-00/fixture-00000.ts",
        });
      },
    );

    const opened = openWorkspaceDocument(fixtureRoot, grantRef, {
      grantRef,
      pathRef: "module-00/fixture-00000.ts",
    });
    if (opened.state !== "available") throw new Error("baseline document fixture unavailable");
    let editorState = withWorkspaceEditorOpened(
      withWorkspaceEditorOpening(emptyWorkspaceEditorState(), opened.document.pathRef),
      opened.document.pathRef,
      opened,
    );
    addLatency(
      "editor.textarea.edit-reducer",
      "apps/openagents-desktop/src/renderer/workspace-editor.ts#withWorkspaceEditorEvent",
      "Reducer-only timing excludes DOM input-to-paint.",
      () => {
        editorState = withWorkspaceEditorEvent(editorState, {
          type: "change",
          value: `${opened.document.content}// edit ${performance.now()}\n`,
        });
      },
    );
    addLatency(
      "editor.recovery-snapshot",
      "apps/openagents-desktop/src/renderer/workspace-editor.ts#workspaceEditorRecoverySnapshot",
      "In-memory schema-ready recovery projection; persistence flush is excluded.",
      () => {
        workspaceEditorRecoverySnapshot(editorState);
      },
    );
    addLatency(
      "workspace.document.save",
      "apps/openagents-desktop/src/workspace-service.ts#saveWorkspaceDocument",
      "Atomic local file replacement; preparation writes are excluded.",
      () => {
        const current = openWorkspaceDocument(fixtureRoot, grantRef, {
          grantRef,
          pathRef: "module-00/fixture-00000.ts",
        });
        if (current.state !== "available") throw new Error("save fixture unavailable");
        saveWorkspaceDocument(fixtureRoot, grantRef, {
          grantRef,
          pathRef: current.document.pathRef,
          content: current.document.content,
          expectedRevisionRef: current.document.revisionRef,
        });
      },
    );
    addLatency(
      "workspace.document.conflict-detection",
      "apps/openagents-desktop/src/workspace-service.ts#saveWorkspaceDocument",
      "Conflict is detected against an intentionally stale revision.",
      () => {
        saveWorkspaceDocument(fixtureRoot, grantRef, {
          grantRef,
          pathRef: opened.document.pathRef,
          content: opened.document.content,
          expectedRevisionRef: opened.document.revisionRef,
        });
      },
    );
    addLatency(
      "git.status",
      "apps/openagents-desktop/src/workspace-service.ts#workspaceGitStatus",
      "Spawns fixed git porcelain command against a 2,000-file fixture.",
      () => {
        workspaceGitStatus(fixtureRoot);
      },
    );
    addLatency(
      "git.diff",
      "apps/openagents-desktop/src/workspace-service.ts#workspaceGitDiff",
      "Spawns bounded numstat and unified diff commands.",
      () => {
        workspaceGitDiff(fixtureRoot, path.join(fixtureRoot, "README.md"));
      },
    );
    addLatency(
      "terminal.typed-host-startup-teardown",
      "apps/openagents-desktop/src/terminal-host.ts#makeTerminalHost",
      "Injected deterministic backend measures host lifecycle only, not PTY process spawn.",
      () => {
        const host = makeTerminalHost({
          backend: terminalProbeBackend(),
          workspace: () => ({ root: fixtureRoot, grantRef }),
          mutationAuthority: {
            authorize: currentGrantRef => ({
              _tag: "Permitted",
              permit: Object.freeze({
                _tag: "LocalOnly",
                key: `local:${currentGrantRef}`,
                grantRef: currentGrantRef,
                sessionRef: "session.ide-baseline",
                workContextRef: "work-context.ide-baseline",
                attachmentRef: null,
                generation: null,
                targetRef: null,
              }),
            }),
            reauthorize: permit => permit.grantRef === grantRef,
          },
          emit: () => undefined,
        });
        host.create({ sessionRef: "terminal.idebaseline" });
        host.dispose();
      },
    );

    const resourceNoise =
      "Process-wide Node harness observation after repeated fixture operations; not split by Electron renderer/main.";
    const rssSamples = Array.from({ length: args.repetitions }, () => process.memoryUsage().rss);
    rawSamples["resource.node-rss"] = rssSamples;
    metrics.push(
      metricFromSamples(
        "resource.node-rss",
        "resource",
        "bytes",
        rssSamples,
        "node:process.memoryUsage",
        resourceNoise,
      ),
    );
    const descriptorSamples = Array.from({ length: args.repetitions }, () => {
      try {
        return readdirSync("/dev/fd").length;
      } catch {
        return 0;
      }
    });
    rawSamples["resource.open-file-descriptors"] = descriptorSamples;
    metrics.push(
      metricFromSamples(
        "resource.open-file-descriptors",
        "resource",
        "count",
        descriptorSamples,
        "/dev/fd count (value only; paths never emitted)",
        resourceNoise,
      ),
    );

    const startup = startupMetrics(args.startupReceipt);
    metrics.push(...startup.metrics);
    const gaps: IdeBaselineGap[] = [
      ...startup.gaps,
      {
        probe: "finder.cold-open",
        status: "unmeasured",
        reason:
          "Requires packaged macOS Finder document registration and cold-launch automation; no headless surrogate is claimed.",
        plannedPacket: "IDE-07",
      },
      {
        probe: "editor.input-to-paint",
        status: "partially_measured",
        reason:
          "Reducer latency is measured; compositor paint and native keyboard delivery require the IDE-01 real Chromium editor harness.",
        plannedPacket: "IDE-01",
      },
      {
        probe: "workspace.search-cancellation",
        status: "partially_measured",
        reason:
          "Search latency and deterministic worker cancellation tests exist, but this receipt does not time real worker cancellation settlement.",
        plannedPacket: "IDE-02",
      },
      {
        probe: "resources.renderer-main-workers-watchers-subscriptions",
        status: "partially_measured",
        reason:
          "Node RSS and descriptor count are measured; process-separated Electron and repeated project open/close telemetry require IDE-07 packaging.",
        plannedPacket: "IDE-07",
      },
      {
        probe: "terminal.real-pty-startup-teardown",
        status: "partially_measured",
        reason:
          "Typed host lifecycle is measured with an injected backend; real process-tree receipts remain in the terminal test suite and are not timed here.",
        plannedPacket: "IDE-04",
      },
    ];

    const rawOutput = args.output.replace(/\.json$/u, ".raw.json");
    mkdirSync(path.dirname(args.output), { recursive: true });
    writeFileSync(
      rawOutput,
      JSON.stringify(
        {
          schema: "openagents.desktop.ide-baseline-raw.v1",
          publicSafe: true,
          samples: rawSamples,
        },
        null,
        2,
      ),
    );

    const receipt = {
      schemaVersion: "openagents.desktop.ide-baseline.v1",
      environment: {
        capturedAt: new Date().toISOString(),
        commitSha: execFileSync("git", ["rev-parse", "HEAD"], {
          cwd: repositoryRoot,
          encoding: "utf8",
        }).trim(),
        platform: process.platform,
        architecture: process.arch,
        nodeVersion: process.version,
        electronVersion: process.versions.electron ?? null,
        fixtureFiles: args.fixtureFiles,
        repetitions: args.repetitions,
        mode: "public-safe deterministic local fixture",
      },
      metrics,
      gaps,
      rawResultRefs: [
        relativeReceiptRef(rawOutput),
        ...(startup.metrics.length > 0 ? [relativeReceiptRef(args.startupReceipt)] : []),
      ],
      assertions: [
        "Fixture paths, repository contents, environment values, and descriptor targets are not emitted.",
        "Every measured series reports p50, p95, p99, minimum, maximum, and repetitions.",
        "Unmeasured real UI, Finder, PTY, worker cancellation, and split-process resource states remain explicit gaps.",
        "This baseline is a comparison input and makes no Monaco, Zed-quality, or Cursor-parity claim.",
      ],
    };
    const decoded = Schema.decodeUnknownExit(IdeBaselineReceiptSchema)(receipt);
    if (Exit.isFailure(decoded)) throw new Error(String(decoded.cause));
    writeFileSync(args.output, JSON.stringify(decoded.value, null, 2));
    console.log(`[ide-baseline] PASS — ${metrics.length} metrics, ${gaps.length} explicit gaps`);
    console.log(`[ide-baseline] receipt ${relativeReceiptRef(args.output)}`);
    console.log(`[ide-baseline] raw ${relativeReceiptRef(rawOutput)}`);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
};

main();
