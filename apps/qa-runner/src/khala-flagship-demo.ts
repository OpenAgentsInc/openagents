#!/usr/bin/env bun
// Khala Code flagship QA demo (#8026).
//
// This composes the headed packaged-app AX smoke with a native-demo distiller:
// the real Electrobun window is launched in fixture/no-spend mode, AX +
// screenshot evidence is inspected for an explicitly seeded bug marker, and a
// committed deterministic regression test is emitted from the public-safe
// evidence report. The generic browser distiller is still used to grade the
// session trace; the native emitter writes the regression that pins this demo's
// AX evidence contract.

import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";

import { assessCandidate, distill } from "./distiller";
import {
  runKhalaDesktopHeadedNativeSmoke,
  type KhalaPackagedNativeSmokeOptions,
  type KhalaPackagedNativeSmokeOutcome,
} from "./khala-desktop-backend";
import { assertPublicSafeResult } from "./result";
import {
  makeSessionTrace,
  shortHash,
  type KhalaSessionTrace,
  type SessionBeat,
} from "./session-trace";
import { makeTarget } from "./target";

export const KHALA_FLAGSHIP_DEMO_REPORT_SCHEMA =
  "openagents.qa_runner.khala_code_flagship_demo.v1" as const;

const FLAGSHIP_TRACE_FILE = "khala-flagship-session-trace.json";
const FLAGSHIP_REPORT_FILE = "khala-flagship-demo-report.json";
const DEFAULT_SEEDED_BUG_ID = "khala-code-packaged-seeded-bug";
const DEFAULT_SEEDED_BUG_LABEL = "Khala Code packaged seeded bug";
const DEFAULT_SEEDED_BUG_TEXT =
  "Seeded bug: packaged Khala Code fixture response is rendered";

export interface KhalaFlagshipSeededBugSpec {
  readonly id?: string;
  readonly label?: string;
  readonly observedText?: string;
}

export interface KhalaFlagshipDemoOptions extends KhalaPackagedNativeSmokeOptions {
  readonly emitPath?: string;
  readonly seededBug?: KhalaFlagshipSeededBugSpec;
}

export interface KhalaFlagshipDemoReport {
  readonly schemaVersion: typeof KHALA_FLAGSHIP_DEMO_REPORT_SCHEMA;
  readonly status: "pass" | "fail";
  readonly target: {
    readonly name: string;
    readonly baseUrl: string;
  };
  readonly nativeSmoke: {
    readonly result: string;
    readonly smokeReport: string;
    readonly status: "pass" | "fail";
    readonly screenshots: ReadonlyArray<string>;
    readonly visualBaselines: ReadonlyArray<{
      readonly id: string;
      readonly status: string;
      readonly delta?: string;
    }>;
  };
  readonly seededBug: {
    readonly id: string;
    readonly label: string;
    readonly verdict: "found" | "not_found";
    readonly observedTextHash: string;
    readonly evidence: {
      readonly axTree: string;
      readonly screenshots: ReadonlyArray<string>;
    };
  };
  readonly distilledRegression: {
    readonly assertionCount: number;
    readonly candidateAdmissible: boolean;
    readonly reasons: ReadonlyArray<string>;
    readonly sourceDigest: string;
    readonly testRef: string;
    readonly verificationClass: string;
  };
}

export interface KhalaFlagshipDemoOutcome {
  readonly native: KhalaPackagedNativeSmokeOutcome;
  readonly regressionPath: string;
  readonly report: KhalaFlagshipDemoReport;
  readonly reportPath: string;
  readonly trace: KhalaSessionTrace;
  readonly tracePath: string;
}

const defaultRegressionPath = (): string =>
  resolve(
    import.meta.dir,
    "../generated/khala-code-packaged-seeded-bug.e2e.test.ts",
  );

const normalizeSeededBug = (
  input: KhalaFlagshipSeededBugSpec | undefined,
): Required<KhalaFlagshipSeededBugSpec> => ({
  id: input?.id ?? DEFAULT_SEEDED_BUG_ID,
  label: input?.label ?? DEFAULT_SEEDED_BUG_LABEL,
  observedText: input?.observedText ?? DEFAULT_SEEDED_BUG_TEXT,
});

const artifactRef = (artifactDir: string, path: string): string => {
  const ref = relative(artifactDir, path);
  return ref.startsWith("..") ? basename(path) : ref;
};

const repoRelativeRef = (path: string): string => {
  const appRoot = resolve(import.meta.dir, "..");
  const ref = relative(appRoot, path);
  return ref.startsWith("..") ? basename(path) : ref;
};

const readAxTreeText = (path: string): string =>
  existsSync(path) ? readFileSync(path, "utf8") : "";

const buildFlagshipTrace = (input: {
  readonly found: boolean;
  readonly native: KhalaPackagedNativeSmokeOutcome;
  readonly seededBug: Required<KhalaFlagshipSeededBugSpec>;
}): KhalaSessionTrace => {
  const beats: SessionBeat[] = [
    {
      contentRef: `sha256:${shortHash(input.seededBug.label)}`,
      kind: "chat_turn",
      role: "assistant",
    },
    {
      action: "readText",
      kind: "browser",
      status: "ok",
      targetHint: "native AX tree",
    },
    {
      action: "screenshot",
      kind: "browser",
      status: input.native.result.status === "pass" ? "ok" : "failed",
      targetHint: "packaged seeded bug evidence",
    },
    {
      action: "assert",
      kind: "browser",
      status: input.found ? "ok" : "failed",
      targetHint: `AX tree contains "${input.seededBug.observedText}"`,
    },
    {
      kind: "verdict",
      verificationClass:
        input.found && input.native.result.status === "pass" ? "test_passed" : "failed",
    },
  ];
  return makeSessionTrace({
    beats,
    goal:
      "Verify the Khala Code packaged native demo finds the seeded bug from AX and screenshot evidence.",
    inputs: [
      {
        description: "the headed packaged Khala Code fixture app",
        name: "target",
        type: "NativeDesktopTarget",
      },
      {
        description: "the public-safe seeded bug marker to hunt",
        name: "seeded_bug_marker",
        type: "string",
      },
    ],
    model: "openagents/khala",
    outputs: [
      {
        description: "whether the seeded bug was observed in the AX evidence",
        name: "seeded_bug_found",
        type: "boolean",
      },
    ],
    receipts: ["khala-code-flagship-demo-report"],
    target: input.native.result.target,
  });
};

export const renderKhalaFlagshipRegressionSource = (
  report: KhalaFlagshipDemoReport,
): string => {
  const embeddedReport = JSON.stringify(report, null, 2);
  return `// GENERATED by the qa-runner Khala Code flagship demo distiller.
// Do not hand-edit; re-run \`bun run --cwd apps/qa-runner khala:flagship-demo\`.
//
// Deterministic regression distilled from the headed packaged-app AX demo.
// It pins the public-safe evidence contract: the agent found the seeded bug
// from native AX/screenshot evidence, and the distillation was admissible.

import { describe, expect, test } from "bun:test";
import { decodeKhalaFlagshipDemoReport } from "../src/khala-flagship-demo";

const report = ${embeddedReport} as const;

describe("distilled: Khala Code packaged seeded-bug hunt", () => {
  test("pins the headed native seeded-bug evidence contract", () => {
    const decoded = decodeKhalaFlagshipDemoReport(report);
    expect(decoded.status).toBe("pass");
    expect(decoded.seededBug.verdict).toBe("found");
    expect(decoded.nativeSmoke.status).toBe("pass");
    expect(decoded.nativeSmoke.screenshots.length).toBeGreaterThanOrEqual(1);
    expect(decoded.distilledRegression.assertionCount).toBeGreaterThanOrEqual(1);
    expect(decoded.distilledRegression.candidateAdmissible).toBe(true);
  });
});
`;
};

export function decodeKhalaFlagshipDemoReport(
  value: unknown,
): KhalaFlagshipDemoReport {
  assertPublicSafeResult(value);
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Khala flagship demo report must be an object");
  }
  const report = value as Partial<KhalaFlagshipDemoReport>;
  if (report.schemaVersion !== KHALA_FLAGSHIP_DEMO_REPORT_SCHEMA) {
    throw new Error("Khala flagship demo report schema mismatch");
  }
  if (report.status !== "pass" && report.status !== "fail") {
    throw new Error("Khala flagship demo report status mismatch");
  }
  if (
    report.seededBug?.verdict !== "found" &&
    report.seededBug?.verdict !== "not_found"
  ) {
    throw new Error("Khala flagship demo seeded bug verdict mismatch");
  }
  if (
    typeof report.distilledRegression?.assertionCount !== "number" ||
    typeof report.distilledRegression?.candidateAdmissible !== "boolean" ||
    typeof report.distilledRegression?.testRef !== "string"
  ) {
    throw new Error("Khala flagship demo regression metadata mismatch");
  }
  return report as KhalaFlagshipDemoReport;
}

export async function runKhalaCodeFlagshipDemo(
  input: KhalaFlagshipDemoOptions,
): Promise<KhalaFlagshipDemoOutcome> {
  const artifactDir = resolve(input.artifactDir);
  const regressionPath = resolve(input.emitPath ?? defaultRegressionPath());
  const seededBug = normalizeSeededBug(input.seededBug);
  mkdirSync(artifactDir, { recursive: true });
  mkdirSync(dirname(regressionPath), { recursive: true });

  const native = await runKhalaDesktopHeadedNativeSmoke({
    ...input,
    artifactDir,
  });
  const axTreeText = readAxTreeText(native.axTreePath);
  const found =
    native.result.status === "pass" && axTreeText.includes(seededBug.observedText);
  const trace = buildFlagshipTrace({ found, native, seededBug });
  const distilled = distill(trace);
  const assessment = assessCandidate(distilled, trace);
  const report: KhalaFlagshipDemoReport = {
    distilledRegression: {
      assertionCount: distilled.emitters.e2e.assertionCount,
      candidateAdmissible: assessment.admissible,
      reasons: assessment.reasons,
      sourceDigest: trace.digest,
      testRef: repoRelativeRef(regressionPath),
      verificationClass: distilled.verificationClass,
    },
    nativeSmoke: {
      result: "result.json",
      screenshots: native.result.artifacts.screenshots,
      smokeReport: artifactRef(artifactDir, native.smokeReportPath),
      status: native.result.status,
      visualBaselines: native.visualBaselines.map((result) => ({
        ...(result.delta === undefined ? {} : { delta: result.delta }),
        id: result.id,
        status: result.status,
      })),
    },
    schemaVersion: KHALA_FLAGSHIP_DEMO_REPORT_SCHEMA,
    seededBug: {
      evidence: {
        axTree: artifactRef(artifactDir, native.axTreePath),
        screenshots: native.result.artifacts.screenshots,
      },
      id: seededBug.id,
      label: seededBug.label,
      observedTextHash: `sha256:${shortHash(seededBug.observedText)}`,
      verdict: found ? "found" : "not_found",
    },
    status: found && assessment.admissible ? "pass" : "fail",
    target: native.result.target,
  };
  decodeKhalaFlagshipDemoReport(report);

  const tracePath = join(artifactDir, FLAGSHIP_TRACE_FILE);
  const reportPath = join(artifactDir, FLAGSHIP_REPORT_FILE);
  writeFileSync(tracePath, `${JSON.stringify(trace, null, 2)}\n`);
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(regressionPath, renderKhalaFlagshipRegressionSource(report));

  return { native, regressionPath, report, reportPath, trace, tracePath };
}

interface CliArgs {
  readonly appPath?: string;
  readonly appProcessName?: string;
  readonly baselineDir?: string;
  readonly blessBaselines?: boolean;
  readonly composerSelector?: string;
  readonly emit?: string;
  readonly help?: boolean;
  readonly hotbarSelector?: string;
  readonly out?: string;
  readonly prompt?: string;
  readonly requireBaselines?: boolean;
  readonly seededBugText?: string;
  readonly sendSelector?: string;
}

function parseArgs(argv: ReadonlyArray<string>): CliArgs {
  const args: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]!;
    if (arg === "--") continue;
    if (arg === "--help" || arg === "-h") {
      args.help = true;
      continue;
    }
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = argv[i + 1];
    if (next !== undefined && !next.startsWith("--")) {
      args[key] = next;
      i += 1;
    } else {
      args[key] = true;
    }
  }
  return {
    ...(typeof args["app-path"] === "string" ? { appPath: args["app-path"] } : {}),
    ...(typeof args["app-process-name"] === "string" ? { appProcessName: args["app-process-name"] } : {}),
    ...(typeof args["baseline-dir"] === "string" ? { baselineDir: args["baseline-dir"] } : {}),
    ...(args["bless-baselines"] === true ? { blessBaselines: true } : {}),
    ...(typeof args["composer-selector"] === "string" ? { composerSelector: args["composer-selector"] } : {}),
    ...(typeof args.emit === "string" ? { emit: args.emit } : {}),
    ...(args.help === true ? { help: true } : {}),
    ...(typeof args["hotbar-selector"] === "string" ? { hotbarSelector: args["hotbar-selector"] } : {}),
    ...(typeof args.out === "string" ? { out: args.out } : {}),
    ...(typeof args.prompt === "string" ? { prompt: args.prompt } : {}),
    ...(args["require-baselines"] === true ? { requireBaselines: true } : {}),
    ...(typeof args["seeded-bug-text"] === "string" ? { seededBugText: args["seeded-bug-text"] } : {}),
    ...(typeof args["send-selector"] === "string" ? { sendSelector: args["send-selector"] } : {}),
  };
}

function usage(): void {
  console.log(`Usage:
  QA_NATIVE_DESKTOP=1 bun run src/khala-flagship-demo.ts -- --out ./runs/khala-flagship-demo --seeded-bug-text "<public marker>"

Options:
  --out <dir>                  Artifact directory (default: ./runs/khala-flagship-demo)
  --emit <path>                Committed regression path (default: generated/khala-code-packaged-seeded-bug.e2e.test.ts)
  --app-path <path>            Explicit Khala Code.app path; otherwise auto-discovered
  --app-process-name <name>    macOS AX process name (default: bundle name without .app)
  --baseline-dir <dir>         Visual baseline store
  --bless-baselines            Bless visual baselines while running
  --require-baselines          Fail when screenshots have no baseline
  --seeded-bug-text <text>     Public-safe AX text marker the agent must find
  --hotbar-selector <selector> AX selector or point:x,y fallback for the hotbar target
  --composer-selector <sel>    AX selector or point:x,y fallback for the composer
  --send-selector <selector>   AX selector or point:x,y fallback for send
  --prompt <text>              Fixture text typed into the composer; not written to public reports
`);
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  if (args.help === true) {
    usage();
    return;
  }

  const baselineDir = resolve(
    args.baselineDir ??
      process.env.KHALA_CODE_VISUAL_BASELINE_DIR ??
      join(import.meta.dir, "../../../clients/khala-code-desktop/visual-baselines"),
  );
  const selectors = {
    ...(args.hotbarSelector === undefined ? {} : { hotbar: args.hotbarSelector }),
    ...(args.composerSelector === undefined ? {} : { composer: args.composerSelector }),
    ...(args.sendSelector === undefined ? {} : { send: args.sendSelector }),
  };
  const outcome = await runKhalaCodeFlagshipDemo({
    artifactDir: resolve(args.out ?? "./runs/khala-flagship-demo"),
    ...(args.appPath === undefined ? {} : { appPath: args.appPath }),
    ...(args.appProcessName === undefined ? {} : { appProcessName: args.appProcessName }),
    ...(args.emit === undefined ? {} : { emitPath: args.emit }),
    ...(args.prompt === undefined ? {} : { promptText: args.prompt }),
    ...(args.seededBugText === undefined
      ? {}
      : { seededBug: { observedText: args.seededBugText } }),
    ...(Object.keys(selectors).length === 0 ? {} : { selectors }),
    target: makeTarget({ name: "khala-code-packaged", baseUrl: "khala-desktop://packaged" }),
    visualBaseline: {
      baselineDir,
      bless:
        args.blessBaselines === true ||
        process.env.KHALA_CODE_VISUAL_BASELINE_BLESS === "1",
      requireBaseline:
        args.requireBaselines === true ||
        process.env.KHALA_CODE_VISUAL_BASELINE_REQUIRE === "1",
    },
  });

  console.log("=== Khala Code flagship seeded-bug demo ===");
  console.log("status:", outcome.report.status);
  console.log("report:", outcome.reportPath);
  console.log("trace:", outcome.tracePath);
  console.log("regression:", outcome.regressionPath);
  console.log("seeded bug:", outcome.report.seededBug.verdict);
  console.log(
    "visual baselines:",
    JSON.stringify(outcome.report.nativeSmoke.visualBaselines),
  );
  process.exit(outcome.report.status === "pass" ? 0 : 1);
}

if (import.meta.main) {
  await main();
}
