#!/usr/bin/env bun
import { spawn } from "node:child_process"
import { readdir, readFile, mkdir, writeFile } from "node:fs/promises"
import { basename, dirname, join, relative, resolve } from "node:path"

import {
  KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
  createEmptyKhalaCodeQaCoverageLedger,
  khalaCodeQaCoverageFrontierReport,
  mergeKhalaCodeQaCoverageLedgers,
  type KhalaCodeQaCoverageFrontierReport,
  type KhalaCodeQaCoverageLedger,
} from "../packages/khala-qa-harness/src/index.js"

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
  attempts: readonly QaNightlyStepAttemptResult[]
  durationMs: number
  exitCode: number
  logRef: string
  status: "passed" | "failed" | "timed_out" | "flaky"
}>

export type QaNightlyStepAttemptResult = Readonly<{
  attempt: 1 | 2
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
  coverageLedgerSourcePaths: readonly string[]
  coverageFrontierReportPath: string
  coverageSteeringInputPath: string
  quarantineLedgerPath: string
  issueStatus?: QaNightlyIssueStatus | undefined
  quarantineIssueStatus?: QaNightlyIssueStatus | undefined
  zeroCoverageIssueStatus?: QaNightlyIssueStatus | undefined
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

export type QaNightlyCoverageSteeringInput = Readonly<{
  schema: "openagents.khala_code.coverage_frontier_steering_input.v1"
  generatedAt: string
  frontierReportPath: string
  frontierRefs: readonly string[]
  missingCounts: Readonly<Record<keyof KhalaCodeQaCoverageFrontierReport["missing"], number>>
  zeroForAWeekRefs: readonly string[]
}>

export type QaNightlyCoverageArtifacts = Readonly<{
  frontierReport: KhalaCodeQaCoverageFrontierReport
  frontierReportPath: string
  sourceLedgerPaths: readonly string[]
  steeringInput: QaNightlyCoverageSteeringInput
  steeringInputPath: string
  unionLedger: KhalaCodeQaCoverageLedger
  unionLedgerPath: string
}>

export type QaNightlyFlakeQuarantineEntry = Readonly<{
  schema: "openagents.khala_code.qa_flake_quarantine_entry.v1"
  command: readonly string[]
  cwd: string
  evidenceRefs: readonly string[]
  firstAttempt: QaNightlyStepAttemptResult
  generatedAt: string
  reason: "failed_then_passed_retry"
  retryAttempt: QaNightlyStepAttemptResult
  runId: string
  stepId: QaNightlyStepId
  stepLabel: string
}>

export type QaNightlyFlakeQuarantineLedger = Readonly<{
  schema: "openagents.khala_code.qa_flake_quarantine_ledger.v1"
  entries: readonly QaNightlyFlakeQuarantineEntry[]
  generatedAt: string
  policy: "retry_once_then_quarantine_no_silent_green"
  runId: string
}>

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

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)

const sorted = (values: Iterable<string>): readonly string[] => [...new Set(values)].sort()

const isoDate = (value: string): string => value.slice(0, 10)

const addDaysIso = (date: string, days: number): string => {
  const timestamp = Date.parse(`${date}T00:00:00.000Z`)
  if (!Number.isFinite(timestamp)) return date
  return new Date(timestamp + days * 24 * 60 * 60 * 1000).toISOString().slice(0, 10)
}

const readJsonFile = async (path: string): Promise<unknown | undefined> => {
  try {
    return JSON.parse(await readFile(path, "utf8"))
  } catch {
    return undefined
  }
}

const walkFiles = async (root: string): Promise<readonly string[]> => {
  let entries: Awaited<ReturnType<typeof readdir>>
  try {
    entries = await readdir(root, { withFileTypes: true })
  } catch {
    return []
  }

  const files: string[] = []
  for (const entry of entries) {
    const path = join(root, entry.name)
    if (entry.isDirectory()) {
      files.push(...await walkFiles(path))
    } else if (entry.isFile()) {
      files.push(path)
    }
  }
  return files.sort((left, right) => left.localeCompare(right))
}

const isCoverageLedger = (value: unknown): value is KhalaCodeQaCoverageLedger =>
  isRecord(value) &&
  value.schema === "khala_code_qa_coverage_ledger.v1" &&
  typeof value.generatedAt === "string" &&
  Array.isArray(value.runIds) &&
  isRecord(value.rpcMethods) &&
  isRecord(value.slashCommands)

const isCoverageFrontierReport = (value: unknown): value is KhalaCodeQaCoverageFrontierReport =>
  isRecord(value) &&
  value.schema === "khala_code_qa_coverage_frontier.v1" &&
  typeof value.generatedAt === "string" &&
  isRecord(value.missing)

export const khalaCoverageFrontierRefs = (
  frontier: KhalaCodeQaCoverageFrontierReport,
): readonly string[] => sorted([
  ...frontier.missing.rpcMethods.map(item => `rpcMethods:${item}`),
  ...frontier.missing.hotbarPanels.map(item => `hotbarPanels:${item}`),
  ...frontier.missing.slashCommands.map(item => `slashCommands:${item}`),
  ...frontier.missing.selectors.map(item => `selectors:${item}`),
  ...frontier.missing.settingsKeys.map(item => `settingsKeys:${item}`),
  ...frontier.missing.approvalDecisionKinds.map(item => `approvalDecisionKinds:${item}`),
  ...frontier.missing.threadItemVariants.map(item => `threadItemVariants:${item}`),
])

export const computeZeroForConsecutiveCoverageDays = (
  entries: ReadonlyArray<Readonly<{
    generatedAt: string
    refs: readonly string[]
  }>>,
  options: Readonly<{
    days?: number | undefined
    latestDate?: string | undefined
  }> = {},
): readonly string[] => {
  const days = options.days ?? 7
  const latestDate = options.latestDate ?? entries.map(entry => isoDate(entry.generatedAt)).sort().at(-1)
  if (latestDate === undefined) return []

  const refsByDate = new Map<string, Set<string>>()
  for (const entry of [...entries].sort((left, right) => left.generatedAt.localeCompare(right.generatedAt))) {
    refsByDate.set(isoDate(entry.generatedAt), new Set(entry.refs))
  }

  const requiredDates = Array.from({ length: days }, (_, index) => addDaysIso(latestDate, -index))
  const latestRefs = refsByDate.get(requiredDates[0] ?? "")
  if (latestRefs === undefined) return []

  let intersection = new Set(latestRefs)
  for (const date of requiredDates) {
    const refs = refsByDate.get(date)
    if (refs === undefined) return []
    intersection = new Set([...intersection].filter(ref => refs.has(ref)))
  }

  return sorted(intersection)
}

export const collectQaNightlyCoverageLedgerPaths = async (
  input: Readonly<{
    artifactRoot: string
    currentCoverageLedgerPath: string
  }>,
): Promise<readonly string[]> => {
  const paths = new Set<string>()
  for (const path of await walkFiles(input.artifactRoot)) {
    const name = basename(path)
    if (name.endsWith("coverage-ledger.json") && name !== "coverage-union-ledger.json") {
      paths.add(path)
    }
  }
  paths.add(input.currentCoverageLedgerPath)
  return [...paths].sort((left, right) => left.localeCompare(right))
}

const collectQaNightlyCoverageFrontierReportPaths = async (
  artifactRoot: string,
): Promise<readonly string[]> =>
  (await walkFiles(artifactRoot))
    .filter(path => basename(path) === "coverage-frontier-report.json")
    .sort((left, right) => left.localeCompare(right))

const readCoverageLedgers = async (
  paths: readonly string[],
): Promise<ReadonlyArray<{ ledger: KhalaCodeQaCoverageLedger; path: string }>> => {
  const ledgers: Array<{ ledger: KhalaCodeQaCoverageLedger; path: string }> = []
  for (const path of paths) {
    const value = await readJsonFile(path)
    if (isCoverageLedger(value)) ledgers.push({ ledger: value, path })
  }
  return ledgers
}

const readCoverageFrontierHistory = async (
  artifactRoot: string,
): Promise<ReadonlyArray<{ frontier: KhalaCodeQaCoverageFrontierReport; path: string }>> => {
  const frontiers: Array<{ frontier: KhalaCodeQaCoverageFrontierReport; path: string }> = []
  for (const path of await collectQaNightlyCoverageFrontierReportPaths(artifactRoot)) {
    const value = await readJsonFile(path)
    if (isCoverageFrontierReport(value)) frontiers.push({ frontier: value, path })
  }
  return frontiers
}

export const emitQaNightlyCoverageArtifacts = async (input: Readonly<{
  artifactDir: string
  artifactRoot: string
  currentCoverageLedgerPath: string
  generatedAt: string
  root: string
  runId: string
}>): Promise<QaNightlyCoverageArtifacts> => {
  const coverageDir = join(input.artifactDir, "coverage")
  const unionLedgerPath = join(coverageDir, "coverage-union-ledger.json")
  const frontierReportPath = join(coverageDir, "coverage-frontier-report.json")
  const steeringInputPath = join(coverageDir, "coverage-frontier-steering.json")
  await mkdir(coverageDir, { recursive: true })

  const ledgerPaths = await collectQaNightlyCoverageLedgerPaths({
    artifactRoot: input.artifactRoot,
    currentCoverageLedgerPath: input.currentCoverageLedgerPath,
  })
  const loadedLedgers = await readCoverageLedgers(ledgerPaths)
  const unionLedger = loadedLedgers.length === 0
    ? createEmptyKhalaCodeQaCoverageLedger({ generatedAt: input.generatedAt, runId: input.runId })
    : mergeKhalaCodeQaCoverageLedgers(loadedLedgers.map(item => item.ledger))

  const initialFrontier = khalaCodeQaCoverageFrontierReport({
    generatedAt: input.generatedAt,
    ledger: unionLedger,
    manifest: KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
  })
  const historicalFrontiers = await readCoverageFrontierHistory(input.artifactRoot)
  const zeroForAWeek = computeZeroForConsecutiveCoverageDays([
    ...historicalFrontiers.map(({ frontier }) => ({
      generatedAt: frontier.generatedAt,
      refs: khalaCoverageFrontierRefs(frontier),
    })),
    {
      generatedAt: initialFrontier.generatedAt,
      refs: khalaCoverageFrontierRefs(initialFrontier),
    },
  ], {
    latestDate: isoDate(input.generatedAt),
  })
  const frontierReport = khalaCodeQaCoverageFrontierReport({
    generatedAt: input.generatedAt,
    ledger: unionLedger,
    manifest: KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
    zeroForAWeek,
  })
  const steeringInput: QaNightlyCoverageSteeringInput = {
    frontierRefs: khalaCoverageFrontierRefs(frontierReport),
    frontierReportPath: repoRelative(input.root, frontierReportPath),
    generatedAt: input.generatedAt,
    missingCounts: {
      approvalDecisionKinds: frontierReport.missing.approvalDecisionKinds.length,
      hotbarPanels: frontierReport.missing.hotbarPanels.length,
      rpcMethods: frontierReport.missing.rpcMethods.length,
      selectors: frontierReport.missing.selectors.length,
      settingsKeys: frontierReport.missing.settingsKeys.length,
      slashCommands: frontierReport.missing.slashCommands.length,
      threadItemVariants: frontierReport.missing.threadItemVariants.length,
    },
    schema: "openagents.khala_code.coverage_frontier_steering_input.v1",
    zeroForAWeekRefs: frontierReport.zeroForAWeekIssueCandidates,
  }

  await writeFile(unionLedgerPath, `${JSON.stringify(unionLedger, null, 2)}\n`)
  await writeFile(frontierReportPath, `${JSON.stringify(frontierReport, null, 2)}\n`)
  await writeFile(steeringInputPath, `${JSON.stringify(steeringInput, null, 2)}\n`)

  return {
    frontierReport,
    frontierReportPath,
    sourceLedgerPaths: loadedLedgers.map(item => item.path),
    steeringInput,
    steeringInputPath,
    unionLedger,
    unionLedgerPath,
  }
}

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
  const sourceLedgers = report.coverageLedgerSourcePaths.length === 0
    ? "None; emitted an empty baseline union ledger."
    : report.coverageLedgerSourcePaths.map(path => `- \`${path}\``).join("\n")
  const zeroCoverage = report.zeroCoverageIssueStatus === undefined
    ? "Not evaluated."
    : report.zeroCoverageIssueStatus.status === "filed"
      ? `Filed: ${report.zeroCoverageIssueStatus.issueUrl ?? "issue URL unavailable"}`
      : `${report.zeroCoverageIssueStatus.status}: ${report.zeroCoverageIssueStatus.reason}`
  const quarantine = report.quarantineIssueStatus === undefined
    ? "Not evaluated."
    : report.quarantineIssueStatus.status === "filed"
      ? `Filed: ${report.quarantineIssueStatus.issueUrl ?? "issue URL unavailable"}`
      : `${report.quarantineIssueStatus.status}: ${report.quarantineIssueStatus.reason}`

  return `# Khala Code QA Nightly Matrix

- Run: \`${report.runId}\`
- Generated: \`${report.generatedAt}\`
- Status: \`${report.status}\`
- Flake quarantine ledger: \`${report.quarantineLedgerPath}\`
- Coverage union ledger: \`${report.coverageLedgerPath}\`
- Coverage frontier report: \`${report.coverageFrontierReportPath}\`
- Explorer steering input: \`${report.coverageSteeringInputPath}\`

## Steps

| Step | Status | Exit | Duration ms | Log |
| --- | --- | ---: | ---: | --- |
${rows}

## Failures

${failedLines}

## Flake Quarantine

Policy: retry once, quarantine pass-after-fail, never silently retry green.

Quarantine issue status: ${quarantine}

## Coverage Frontier

Source ledgers:

${sourceLedgers}

Zero-for-seven-days issue status: ${zeroCoverage}
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
- Flake quarantine ledger: \`${report.quarantineLedgerPath}\`
- Coverage union ledger: \`${report.coverageLedgerPath}\`
- Coverage frontier report: \`${report.coverageFrontierReportPath}\`
- Explorer steering input: \`${report.coverageSteeringInputPath}\`

### Severity

S1 - supported user or agent path is broken

### Environment

Owned runner nightly matrix. Generated at \`${report.generatedAt}\`.

### Safety and redaction

The report includes public-safe refs, step IDs, exit codes, and artifact paths only. Raw command logs remain in the owned-runner artifact directory and must be redaction-reviewed before external publication.
`
}

export const buildQaNightlyZeroCoverageIssueBody = (
  report: QaNightlyReport,
  frontier: KhalaCodeQaCoverageFrontierReport,
): string => {
  const candidates = frontier.zeroForAWeekIssueCandidates
  return `### Affected surface

Khala Code Desktop fully automated QA coverage frontier.

### Actual behavior

Nightly run \`${report.runId}\` found coverage classes that remained at zero for seven consecutive coverage days:
${candidates.map(candidate => `- \`${candidate}\``).join("\n")}

### Expected behavior

The explorer should eventually exercise each manifest coverage class, or the missing class should be explicitly owned with a new scenario, selector, RPC action, or model-boundary exception.

### Reproduction steps

1. Check out the repository commit used by the owned runner.
2. Run \`bun run qa:nightly\`.
3. Inspect \`${report.coverageFrontierReportPath}\` and \`${report.coverageSteeringInputPath}\`.

### Public-safe evidence

- Report JSON: \`${report.reportJsonPath}\`
- Coverage union ledger: \`${report.coverageLedgerPath}\`
- Coverage frontier report: \`${report.coverageFrontierReportPath}\`
- Explorer steering input: \`${report.coverageSteeringInputPath}\`

### Severity

S3 - coverage gap with automated steering impact

### Environment

Owned runner nightly matrix. Generated at \`${report.generatedAt}\`.

### Safety and redaction

The frontier contains coverage class names and repo-relative artifact refs only; it does not include account data, secrets, or raw command logs.
`
}

export const buildQaNightlyFlakeQuarantineIssueBody = (
  report: QaNightlyReport,
  ledger: QaNightlyFlakeQuarantineLedger,
): string => {
  const entries = ledger.entries
  return `### Affected surface

Khala Code Desktop fully automated QA nightly flake quarantine.

### Actual behavior

Nightly run \`${report.runId}\` observed one or more steps fail once and pass on the single retry. These are quarantined bugs, not green retries:
${entries.map(entry =>
  `- \`${entry.stepId}\`: first ${entry.firstAttempt.status} exit ${entry.firstAttempt.exitCode} (\`${entry.firstAttempt.logRef}\`), retry passed (\`${entry.retryAttempt.logRef}\`)`
).join("\n")}

### Expected behavior

The first attempt should pass deterministically. A pass-after-fail must remain tracked until the underlying product or harness bug is fixed and the quarantine entry is removed.

### Reproduction steps

1. Check out the repository commit used by the owned runner.
2. Run \`bun run qa:nightly\`.
3. Inspect \`${report.quarantineLedgerPath}\` and the first-attempt/retry logs listed above.

### Public-safe evidence

- Report JSON: \`${report.reportJsonPath}\`
- Report markdown: \`${report.reportMarkdownPath}\`
- Quarantine ledger: \`${report.quarantineLedgerPath}\`
${entries.flatMap(entry => entry.evidenceRefs.map(ref => `- Evidence ref: \`${ref}\``)).join("\n")}

### Severity

S2 - intermittent failure in the automated QA gate

### Environment

Owned runner nightly matrix. Generated at \`${report.generatedAt}\`.

### Safety and redaction

The quarantine ledger contains step IDs, commands, exit codes, and public-safe artifact refs only. Raw logs remain in the owned-runner artifact directory and must be redaction-reviewed before external publication.
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
  const quarantineEntries: QaNightlyFlakeQuarantineEntry[] = []

  await mkdir(artifactDir, { recursive: true })
  const runStepAttempt = async (
    step: QaNightlyStep,
    absoluteStep: QaNightlyStep,
    attempt: 1 | 2,
  ): Promise<QaNightlyStepAttemptResult> => {
    const logPath = join(artifactDir, "logs", attempt === 1 ? `${step.id}.log` : `${step.id}.retry.log`)
    await mkdir(dirname(logPath), { recursive: true })
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
        `attempt: ${attempt}`,
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
    return {
      attempt,
      durationMs: result.durationMs,
      exitCode: result.exitCode,
      logRef: repoRelative(root, logPath),
      status,
    }
  }

  for (const step of steps) {
    const absoluteStep = {
      ...step,
      cwd: resolve(root, step.cwd),
    }
    const firstAttempt = await runStepAttempt(step, absoluteStep, 1)
    const attempts: QaNightlyStepAttemptResult[] = [firstAttempt]
    if (firstAttempt.status !== "passed") {
      attempts.push(await runStepAttempt(step, absoluteStep, 2))
    }
    const lastAttempt = attempts.at(-1) ?? firstAttempt
    const status: QaNightlyStepResult["status"] = firstAttempt.status === "passed"
      ? "passed"
      : lastAttempt.status === "passed"
        ? "flaky"
        : lastAttempt.status
    const evidenceRefs = [
      firstAttempt.logRef,
      ...(attempts.length > 1 ? [lastAttempt.logRef] : []),
      ...(step.expectedArtifactRefs ?? []).map(ref => repoRelative(root, join(artifactDir, ref))),
    ]
    if (status === "flaky") {
      quarantineEntries.push({
        command: step.command,
        cwd: step.cwd,
        evidenceRefs,
        firstAttempt,
        generatedAt,
        reason: "failed_then_passed_retry",
        retryAttempt: lastAttempt,
        runId,
        schema: "openagents.khala_code.qa_flake_quarantine_entry.v1",
        stepId: step.id,
        stepLabel: step.label,
      })
    }
    results.push({
      attempts,
      command: step.command,
      cwd: step.cwd,
      durationMs: attempts.reduce((sum, attempt) => sum + attempt.durationMs, 0),
      exitCode: status === "flaky" ? firstAttempt.exitCode : lastAttempt.exitCode,
      id: step.id,
      label: step.label,
      logRef: status === "flaky" ? firstAttempt.logRef : lastAttempt.logRef,
      status,
    })
  }

  const reportJsonPath = join(artifactDir, "qa-nightly-report.json")
  const reportMarkdownPath = join(artifactDir, "qa-nightly-report.md")
  const issueBodyPath = join(artifactDir, "qa-nightly-failure-issue.md")
  const zeroCoverageIssueBodyPath = join(artifactDir, "qa-nightly-zero-coverage-issue.md")
  const quarantineIssueBodyPath = join(artifactDir, "qa-nightly-flake-quarantine-issue.md")
  const quarantineLedgerPath = join(artifactDir, "quarantine", "flake-quarantine-ledger.json")
  const quarantineLedger: QaNightlyFlakeQuarantineLedger = {
    entries: quarantineEntries,
    generatedAt,
    policy: "retry_once_then_quarantine_no_silent_green",
    runId,
    schema: "openagents.khala_code.qa_flake_quarantine_ledger.v1",
  }
  await mkdir(dirname(quarantineLedgerPath), { recursive: true })
  await writeFile(quarantineLedgerPath, `${JSON.stringify(quarantineLedger, null, 2)}\n`)
  const monkeyCoverageLedgerPath = join(artifactDir, "monkey-night", "monkey-night-coverage-ledger.json")
  const coverageArtifacts = await emitQaNightlyCoverageArtifacts({
    artifactDir,
    artifactRoot,
    currentCoverageLedgerPath: monkeyCoverageLedgerPath,
    generatedAt,
    root,
    runId,
  })
  const reportBase: QaNightlyReport = {
    artifactDir: repoRelative(root, artifactDir),
    coverageFrontierReportPath: repoRelative(root, coverageArtifacts.frontierReportPath),
    coverageLedgerPath: repoRelative(root, coverageArtifacts.unionLedgerPath),
    coverageLedgerSourcePaths: coverageArtifacts.sourceLedgerPaths.map(path => repoRelative(root, path)),
    coverageSteeringInputPath: repoRelative(root, coverageArtifacts.steeringInputPath),
    generatedAt,
    quarantineLedgerPath: repoRelative(root, quarantineLedgerPath),
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
  const quarantineIssueStatus = await (async (): Promise<QaNightlyIssueStatus | undefined> => {
    if (quarantineLedger.entries.length === 0) {
      return { reason: "no pass-after-fail retries were observed", status: "disabled" }
    }
    if (env.OA_QA_NIGHTLY_FILE_QUARANTINE_ISSUE !== "1") {
      return { reason: "OA_QA_NIGHTLY_FILE_QUARANTINE_ISSUE is not set", status: "disabled" }
    }
    const body = buildQaNightlyFlakeQuarantineIssueBody(reportBase, quarantineLedger)
    await writeFile(quarantineIssueBodyPath, body)
    return (input.issueFiler ?? fileQaNightlyFailureIssueWithGh)({
      bodyPath: quarantineIssueBodyPath,
      report: reportBase,
      title: `[Bug]: Khala Code QA flake quarantined ${quarantineLedger.entries[0]?.stepId ?? reportBase.runId}`,
    })
  })()
  const zeroCoverageIssueStatus = await (async (): Promise<QaNightlyIssueStatus | undefined> => {
    if (coverageArtifacts.frontierReport.zeroForAWeekIssueCandidates.length === 0) {
      return { reason: "no coverage class has been zero for seven consecutive days", status: "disabled" }
    }
    if (env.OA_QA_NIGHTLY_FILE_COVERAGE_ISSUE !== "1") {
      return { reason: "OA_QA_NIGHTLY_FILE_COVERAGE_ISSUE is not set", status: "disabled" }
    }
    const body = buildQaNightlyZeroCoverageIssueBody(reportBase, coverageArtifacts.frontierReport)
    await writeFile(zeroCoverageIssueBodyPath, body)
    return (input.issueFiler ?? fileQaNightlyFailureIssueWithGh)({
      bodyPath: zeroCoverageIssueBodyPath,
      report: reportBase,
      title: `[Bug]: Khala Code QA coverage stayed zero ${coverageArtifacts.frontierReport.zeroForAWeekIssueCandidates[0]}`,
    })
  })()

  const report: QaNightlyReport = {
    ...reportBase,
    ...(issueStatus === undefined ? {} : { issueStatus }),
    ...(quarantineIssueStatus === undefined ? {} : { quarantineIssueStatus }),
    ...(zeroCoverageIssueStatus === undefined ? {} : { zeroCoverageIssueStatus }),
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
