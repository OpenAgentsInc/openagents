#!/usr/bin/env bun
import { spawn } from "node:child_process"
import { mkdir, writeFile } from "node:fs/promises"
import { dirname, join, relative, resolve } from "node:path"

export const QA_NIGHTLY_MATRIX_SCHEMA =
  "openagents.khala_code.qa_nightly_matrix.v1"

export const QA_NIGHTLY_DEFAULT_RUNS = 16
export const QA_NIGHTLY_DEFAULT_STEPS = 64
export const QA_NIGHTLY_DEFAULT_STEP_TIMEOUT_MS = 30 * 60 * 1000

export type QaNightlyStepId =
  | "harness-suite"
  | "desktop-verify"
  | "visual-part2-ui"
  | "visual-cockpit"
  | "visual-composer"
  | "monkey-night"
  | "model-based"
  | "property-tier"

export type QaNightlyStep = Readonly<{
  id: QaNightlyStepId
  label: string
  command: readonly string[]
  cwd: string
  expectedArtifactRefs?: readonly string[]
}>

export type QaNightlyStepResult = Readonly<{
  id: QaNightlyStepId
  label: string
  command: readonly string[]
  cwd: string
  durationMs: number
  exitCode: number
  logRef: string
  status: "passed" | "failed" | "timed_out"
}>

export type QaNightlyReport = Readonly<{
  schema: typeof QA_NIGHTLY_MATRIX_SCHEMA
  runId: string
  generatedAt: string
  status: "passed" | "failed"
  steps: readonly QaNightlyStepResult[]
  artifactDir: string
  reportJsonPath: string
  reportMarkdownPath: string
  coverageLedgerPath: string
  issueStatus?: QaNightlyIssueStatus | undefined
}>

export type QaNightlyIssueStatus = Readonly<
  | { status: "disabled"; reason: string }
  | { status: "filed"; issueUrl?: string | undefined }
  | { status: "failed"; reason: string }
>

export type QaNightlyCommandResult = Readonly<{
  durationMs: number
  exitCode: number
  stderr: string
  stdout: string
  timedOut?: boolean | undefined
}>

export type QaNightlyCommandRunner = (
  step: QaNightlyStep,
  timeoutMs: number,
) => Promise<QaNightlyCommandResult>

export type QaNightlyIssueFiler = (
  input: Readonly<{
    bodyPath: string
    report: QaNightlyReport
    title: string
  }>,
) => Promise<QaNightlyIssueStatus>

const sanitizeRunPart = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64) || "unknown"

const nowIso = (): string => new Date().toISOString()

const positiveInt = (value: string | undefined, fallback: number): number => {
  const parsed = Number.parseInt(value ?? "", 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

const quote = (args: readonly string[]): string =>
  args.map(arg => (/^[A-Za-z0-9_./:=@+-]+$/.test(arg) ? arg : JSON.stringify(arg))).join(" ")

const toPosix = (path: string): string => path.split("\\").join("/")

const repoRelative = (root: string, path: string): string =>
  toPosix(relative(root, path)) || "."

export const buildQaNightlySteps = (input: Readonly<{
  artifactDir: string
  monkeyRuns?: number | undefined
  monkeySteps?: number | undefined
}>): readonly QaNightlyStep[] => {
  const monkeyRuns = input.monkeyRuns ?? QA_NIGHTLY_DEFAULT_RUNS
  const monkeySteps = input.monkeySteps ?? QA_NIGHTLY_DEFAULT_STEPS
  const monkeyArtifactDir = join(input.artifactDir, "monkey-night")
  return [
    {
      command: ["bun", "run", "--cwd", "packages/khala-qa-harness", "test"],
      cwd: ".",
      id: "harness-suite",
      label: "Khala QA harness suite",
    },
    {
      command: ["bun", "run", "--cwd", "clients/khala-code-desktop", "verify"],
      cwd: ".",
      id: "desktop-verify",
      label: "Khala Code Desktop verify",
    },
    {
      command: ["bun", "run", "--cwd", "clients/khala-code-desktop", "smoke:part2-ui"],
      cwd: ".",
      id: "visual-part2-ui",
      label: "Part 2 UI visual smoke",
    },
    {
      command: ["bun", "run", "--cwd", "clients/khala-code-desktop", "smoke:cockpit-visual"],
      cwd: ".",
      id: "visual-cockpit",
      label: "Fleet cockpit visual smoke",
    },
    {
      command: ["bun", "run", "--cwd", "clients/khala-code-desktop", "smoke:composer-visual"],
      cwd: ".",
      id: "visual-composer",
      label: "Composer visual smoke",
    },
    {
      command: [
        "bun",
        "src/monkey-night.ts",
        "--runs",
        String(monkeyRuns),
        "--steps",
        String(monkeySteps),
        "--artifact-dir",
        monkeyArtifactDir,
      ],
      cwd: "packages/khala-qa-harness",
      expectedArtifactRefs: [
        join("monkey-night", "monkey-night-report.json"),
        join("monkey-night", "monkey-night-coverage-ledger.json"),
      ],
      id: "monkey-night",
      label: `Seeded monkey night (${monkeyRuns * monkeySteps} actions)`,
    },
    {
      command: ["bun", "test", "src/model-based.test.ts"],
      cwd: "packages/khala-qa-harness",
      id: "model-based",
      label: "Model-based tier",
    },
    {
      command: [
        "bun",
        "test",
        "tests/composer-draft-model.property.test.ts",
        "tests/codex-thread-item-projector.property.test.ts",
        "tests/transcript-render.property.test.ts",
      ],
      cwd: "clients/khala-code-desktop",
      id: "property-tier",
      label: "Property tier",
    },
  ] as const
}

export const runSpawnedQaNightlyCommand: QaNightlyCommandRunner = async (
  step,
  timeoutMs,
) => {
  const startedAt = Date.now()
  const [cmd, ...args] = step.command
  if (cmd === undefined) {
    return {
      durationMs: 0,
      exitCode: 1,
      stderr: "empty command",
      stdout: "",
    }
  }

  return new Promise(resolveResult => {
    const child = spawn(cmd, args, {
      cwd: step.cwd,
      detached: true,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    })
    const stdoutChunks: Buffer[] = []
    const stderrChunks: Buffer[] = []
    let timedOut = false
    let killTimer: ReturnType<typeof setTimeout> | undefined

    const killTree = (signal: NodeJS.Signals) => {
      try {
        process.kill(-child.pid, signal)
      } catch {
        child.kill(signal)
      }
    }

    const timer = setTimeout(() => {
      timedOut = true
      killTree("SIGTERM")
      killTimer = setTimeout(() => killTree("SIGKILL"), 5_000)
    }, timeoutMs)

    child.stdout?.on("data", chunk => stdoutChunks.push(Buffer.from(chunk)))
    child.stderr?.on("data", chunk => stderrChunks.push(Buffer.from(chunk)))
    child.on("error", error => {
      clearTimeout(timer)
      if (killTimer !== undefined) clearTimeout(killTimer)
      resolveResult({
        durationMs: Date.now() - startedAt,
        exitCode: 1,
        stderr: error instanceof Error ? error.message : String(error),
        stdout: Buffer.concat(stdoutChunks).toString("utf8"),
      })
    })
    child.on("close", code => {
      clearTimeout(timer)
      if (killTimer !== undefined) clearTimeout(killTimer)
      resolveResult({
        durationMs: Date.now() - startedAt,
        exitCode: timedOut ? 124 : code ?? 1,
        stderr: Buffer.concat(stderrChunks).toString("utf8"),
        stdout: Buffer.concat(stdoutChunks).toString("utf8"),
        timedOut,
      })
    })
  })
}

export const assertQaNightlyReportPublicSafe = (report: QaNightlyReport): void => {
  const text = JSON.stringify(report)
  const unsafe = /\/Users\/|\/home\/|~\/|auth\.json|bearer\s+[a-z0-9._-]+|sk-[a-z0-9]/i
  if (unsafe.test(text)) {
    throw new Error("QA nightly report contains a private path or credential-shaped value")
  }
}

export const renderQaNightlyMarkdown = (report: QaNightlyReport): string => {
  const rows = report.steps
    .map(step => `| ${step.id} | ${step.status} | ${step.exitCode} | ${step.durationMs} | ${step.logRef} |`)
    .join("\n")
  const failed = report.steps.filter(step => step.status !== "passed")
  const failedLines = failed.length === 0
    ? "None."
    : failed.map(step => `- ${step.id}: ${step.status} (${step.logRef})`).join("\n")

  return `# Khala Code QA Nightly Matrix

- Run: \`${report.runId}\`
- Generated: \`${report.generatedAt}\`
- Status: \`${report.status}\`
- Coverage ledger: \`${report.coverageLedgerPath}\`

## Steps

| Step | Status | Exit | Duration ms | Log |
| --- | --- | ---: | ---: | --- |
${rows}

## Failures

${failedLines}
`
}

export const buildQaNightlyFailureIssueBody = (report: QaNightlyReport): string => {
  const failed = report.steps.filter(step => step.status !== "passed")
  return `### Affected surface

Khala Code Desktop fully automated QA cycle, nightly owned-runner matrix.

### Actual behavior

Nightly run \`${report.runId}\` finished with status \`${report.status}\`.

Failed steps:
${failed.map(step => `- \`${step.id}\` ${step.status}, exit ${step.exitCode}, log \`${step.logRef}\``).join("\n")}

### Expected behavior

The full fixture QA matrix should pass: harness suite, desktop verify, visual smokes, monkey night, model-based tier, and property tier.

### Reproduction steps

1. Check out the repository commit used by the owned runner.
2. Run \`bun run qa:nightly\`.
3. Inspect \`${report.reportMarkdownPath}\` and the step logs referenced above.

### Public-safe evidence

- Report JSON: \`${report.reportJsonPath}\`
- Report markdown: \`${report.reportMarkdownPath}\`
- Coverage ledger: \`${report.coverageLedgerPath}\`

### Severity

S1 - supported user or agent path is broken

### Environment

Owned runner nightly matrix. Generated at \`${report.generatedAt}\`.

### Safety and redaction

The report includes public-safe refs, step IDs, exit codes, and artifact paths only. Raw command logs remain in the owned-runner artifact directory and must be redaction-reviewed before external publication.
`
}

export const fileQaNightlyFailureIssueWithGh: QaNightlyIssueFiler = async ({
  bodyPath,
  report,
  title,
}) => {
  const gh = spawn("gh", [
    "issue",
    "create",
    "--title",
    title,
    "--body-file",
    bodyPath,
    "--label",
    "bug",
    "--label",
    "qa",
  ], {
    stdio: ["ignore", "pipe", "pipe"],
  })
  const stdoutChunks: Buffer[] = []
  const stderrChunks: Buffer[] = []
  gh.stdout?.on("data", chunk => stdoutChunks.push(Buffer.from(chunk)))
  gh.stderr?.on("data", chunk => stderrChunks.push(Buffer.from(chunk)))
  const exitCode = await new Promise<number>(resolveExit => {
    gh.on("close", code => resolveExit(code ?? 1))
    gh.on("error", () => resolveExit(1))
  })
  if (exitCode !== 0) {
    return {
      reason: Buffer.concat(stderrChunks).toString("utf8").trim() || `gh exited ${exitCode}`,
      status: "failed",
    }
  }
  const issueUrl = Buffer.concat(stdoutChunks).toString("utf8").trim().split(/\s+/)[0]
  return {
    issueUrl: issueUrl === "" ? undefined : issueUrl,
    status: "filed",
  }
}

export const runQaNightlyMatrix = async (input: Readonly<{
  artifactRoot?: string | undefined
  commandRunner?: QaNightlyCommandRunner | undefined
  env?: Readonly<Record<string, string | undefined>> | undefined
  issueFiler?: QaNightlyIssueFiler | undefined
  now?: (() => string) | undefined
  root?: string | undefined
  stepTimeoutMs?: number | undefined
}> = {}): Promise<QaNightlyReport> => {
  const env = input.env ?? process.env
  const root = resolve(input.root ?? process.cwd())
  const generatedAt = input.now?.() ?? nowIso()
  const runId = `khala-code-qa-nightly-${sanitizeRunPart(generatedAt.replace(/[:]/g, ""))}`
  const artifactRoot = input.artifactRoot ?? env.OA_QA_NIGHTLY_ARTIFACT_DIR ?? join(root, "var", "qa-nightly")
  const artifactDir = join(artifactRoot, runId)
  const commandRunner = input.commandRunner ?? runSpawnedQaNightlyCommand
  const timeoutMs =
    input.stepTimeoutMs ?? positiveInt(env.OA_QA_NIGHTLY_STEP_TIMEOUT_MS, QA_NIGHTLY_DEFAULT_STEP_TIMEOUT_MS)
  const monkeyRuns = positiveInt(env.OA_QA_NIGHTLY_MONKEY_RUNS, QA_NIGHTLY_DEFAULT_RUNS)
  const monkeySteps = positiveInt(env.OA_QA_NIGHTLY_MONKEY_STEPS, QA_NIGHTLY_DEFAULT_STEPS)
  const steps = buildQaNightlySteps({ artifactDir, monkeyRuns, monkeySteps })
  const results: QaNightlyStepResult[] = []

  await mkdir(artifactDir, { recursive: true })
  for (const step of steps) {
    const logPath = join(artifactDir, "logs", `${step.id}.log`)
    await mkdir(dirname(logPath), { recursive: true })
    const absoluteStep = {
      ...step,
      cwd: resolve(root, step.cwd),
    }
    const result = await commandRunner(absoluteStep, timeoutMs)
    const status = result.timedOut === true
      ? "timed_out"
      : result.exitCode === 0
        ? "passed"
        : "failed"
    await writeFile(
      logPath,
      [
        `$ ${quote(step.command)}`,
        `cwd: ${step.cwd}`,
        `status: ${status}`,
        `exitCode: ${result.exitCode}`,
        `durationMs: ${result.durationMs}`,
        "",
        "## stdout",
        result.stdout,
        "",
        "## stderr",
        result.stderr,
        "",
      ].join("\n"),
    )
    results.push({
      command: step.command,
      cwd: step.cwd,
      durationMs: result.durationMs,
      exitCode: result.exitCode,
      id: step.id,
      label: step.label,
      logRef: repoRelative(root, logPath),
      status,
    })
  }

  const reportJsonPath = join(artifactDir, "qa-nightly-report.json")
  const reportMarkdownPath = join(artifactDir, "qa-nightly-report.md")
  const issueBodyPath = join(artifactDir, "qa-nightly-failure-issue.md")
  const coverageLedgerPath = join(artifactDir, "monkey-night", "monkey-night-coverage-ledger.json")
  const reportBase: QaNightlyReport = {
    artifactDir: repoRelative(root, artifactDir),
    coverageLedgerPath: repoRelative(root, coverageLedgerPath),
    generatedAt,
    reportJsonPath: repoRelative(root, reportJsonPath),
    reportMarkdownPath: repoRelative(root, reportMarkdownPath),
    runId,
    schema: QA_NIGHTLY_MATRIX_SCHEMA,
    status: results.every(step => step.status === "passed") ? "passed" : "failed",
    steps: results,
  }

  const issueStatus = await (async (): Promise<QaNightlyIssueStatus | undefined> => {
    if (reportBase.status === "passed") {
      return { reason: "nightly matrix passed", status: "disabled" }
    }
    if (env.OA_QA_NIGHTLY_FILE_ISSUE !== "1") {
      return { reason: "OA_QA_NIGHTLY_FILE_ISSUE is not set", status: "disabled" }
    }
    const body = buildQaNightlyFailureIssueBody(reportBase)
    await writeFile(issueBodyPath, body)
    return (input.issueFiler ?? fileQaNightlyFailureIssueWithGh)({
      bodyPath: issueBodyPath,
      report: reportBase,
      title: `[Bug]: Khala Code QA nightly failed ${reportBase.runId}`,
    })
  })()

  const report: QaNightlyReport = {
    ...reportBase,
    ...(issueStatus === undefined ? {} : { issueStatus }),
  }
  assertQaNightlyReportPublicSafe(report)
  await writeFile(reportJsonPath, `${JSON.stringify(report, null, 2)}\n`)
  await writeFile(reportMarkdownPath, renderQaNightlyMarkdown(report))
  return report
}

if (import.meta.main) {
  const report = await runQaNightlyMatrix()
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`)
  process.exit(report.status === "passed" ? 0 : 1)
}
