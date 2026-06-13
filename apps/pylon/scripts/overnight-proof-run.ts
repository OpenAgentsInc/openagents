#!/usr/bin/env bun
/**
 * M10 overnight unattended proof harness (#4768).
 *
 * Runs an unattended overnight batch of bounded, no-spend coding tasks
 * across BOTH local adapter lanes (Codex, Claude) and BOTH execution
 * surfaces:
 *
 *  - composer surface: the same `runCodexComposerStream` /
 *    `runClaudeComposerStream` path the TUI uses, driven headlessly via the
 *    retained daily-driver proof machinery (`dev-proof-run.ts`, #4847/#4860)
 *    against a dedicated scratch task repository — never the main checkout.
 *  - work-order surface: the full `runNoSpendAssignment` lifecycle (poll →
 *    admit → accept → execute → verify → progress → artifacts → closeout)
 *    with the REAL SDK runners, driven against the local in-process
 *    assignment-API harness from the codex/claude task smokes (#4790
 *    lineage). The live API and SHC lane are intentionally out of scope for
 *    this leg; the deferral is recorded in the summary's deviations.
 *
 * Boundaries: local_bounded execution only, no-spend only, approvals never,
 * network access disabled for Codex sandboxes, acceptEdits for Claude. One
 * task failing never kills the run; failures are retained honestly as typed
 * failure records. Every retained artifact passes the house redaction scan
 * before it is written (raw errors are reduced to a class + digest ref).
 *
 * Outputs (all under --proofs-dir):
 *  - m10-c<N>-<lane>-composer-proof.json   retained daily-driver proofs
 *  - m10-c<N>-<lane>-work-order-proof.json work-order lifecycle proofs
 *  - m10-task-failure-<index>.json         typed failure records
 *  - heartbeats.jsonl                      caller-clocked heartbeat stream
 *  - m10-overnight-summary.json            final run summary
 *
 * Usage:
 *   bun apps/pylon/scripts/overnight-proof-run.ts \
 *     --task-repo <scratch git repo> --proofs-dir <dir> --pylon-home <dir> \
 *     [--cycles <n>] [--sleep-seconds <n>] [--deadline-minutes <n>] \
 *     [--run-id <id>]
 */
import { createHash } from "node:crypto"
import { appendFile, mkdir, writeFile } from "node:fs/promises"
import { join, resolve } from "node:path"
import { runNoSpendAssignment } from "../src/assignment"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { CLAUDE_AGENT_CAPABILITY_REF } from "../src/claude-agent"
import {
  claudeAgentSmokeLease,
  scanRetainedProjection as scanClaudeProjection,
} from "../src/claude-agent-task-smoke"
import { CODEX_AGENT_CAPABILITY_REF } from "../src/codex-agent"
import {
  ciHarness,
  codexAgentSmokeLease,
  scanRetainedProjection as scanCodexProjection,
} from "../src/codex-agent-task-smoke"
import { sendHeartbeat } from "../src/presence"
import { assertPublicProjectionSafe, ensurePylonLocalState } from "../src/state"
import { runProof, scanProofSerialization } from "./dev-proof-run"

export const M10_TASK_SCHEMA = "openagents.pylon.m10_overnight_task.v0.1"
export const M10_FAILURE_SCHEMA = "openagents.pylon.m10_overnight_task_failure.v0.1"
export const M10_HEARTBEAT_SCHEMA = "openagents.pylon.m10_overnight_heartbeat.v0.1"
export const M10_SUMMARY_SCHEMA = "openagents.pylon.m10_overnight_summary.v0.1"

type Lane = "codex" | "claude_agent"
type Surface = "composer" | "work_order"

type OvernightArgs = {
  taskRepo: string
  proofsDir: string
  pylonHome: string
  cycles: number
  sleepSeconds: number
  deadlineMinutes: number
  runId: string
}

type TaskCell = { lane: Lane; surface: Surface }

type TaskOutcome = {
  taskIndex: number
  cycle: number
  lane: Lane
  surface: Surface
  taskRef: string
  outcome: "completed" | "failed"
  resultRef: string | null
  artifactFile: string | null
  startedAt: string
  completedAt: string
}

const CYCLE_CELLS: TaskCell[] = [
  { lane: "codex", surface: "composer" },
  { lane: "claude_agent", surface: "composer" },
  { lane: "codex", surface: "work_order" },
  { lane: "claude_agent", surface: "work_order" },
]

// Bounded real doc/test composer task templates. Objectives are retained in
// proofs, so they stay relative-path-only and redaction-pattern free.
const COMPOSER_TASKS: Array<{
  taskRef: string
  objective: (dir: string) => string
  verification: (dir: string) => string[]
}> = [
  {
    taskRef: "task.m10.stats_util",
    objective: (dir) =>
      `In the directory ${dir} (relative to the repository root), create stats.ts ` +
      `exporting mean(values: number[]): number and median(values: number[]): number, ` +
      `plus stats.test.ts covering both functions with bun:test, including an ` +
      `empty-input case. Run \`bun test ${dir}\` and make every test pass. ` +
      `Only create or modify files inside ${dir}.`,
    verification: (dir) => ["bun", "test", dir],
  },
  {
    taskRef: "task.m10.timer_doc",
    objective: (dir) =>
      `In the directory ${dir} (relative to the repository root), create notes.md ` +
      `documenting a small command-line countdown timer tool with exactly three ` +
      `level-2 sections titled "Purpose", "Usage", and "Limits", plus notes.test.ts ` +
      `that reads notes.md with bun:test and asserts all three section headings are ` +
      `present. Run \`bun test ${dir}\` and make every test pass. ` +
      `Only create or modify files inside ${dir}.`,
    verification: (dir) => ["bun", "test", dir],
  },
  {
    taskRef: "task.m10.slug_util",
    objective: (dir) =>
      `In the directory ${dir} (relative to the repository root), create slug.ts ` +
      `exporting slugify(input: string): string that lowercases, trims, collapses ` +
      `whitespace to single hyphens, and strips characters outside [a-z0-9-], plus ` +
      `slug.test.ts covering at least four cases with bun:test. Run ` +
      `\`bun test ${dir}\` and make every test pass. Only create or modify files inside ${dir}.`,
    verification: (dir) => ["bun", "test", dir],
  },
]

const DEVIATIONS = [
  "deviation.m10.shc_lane_deferred_requires_live_scheduled_launch",
  "deviation.m10.web_ui_surface_deferred_requires_live_api",
  "deviation.m10.work_status_cli_surface_deferred_requires_live_api",
  "deviation.m10.work_order_surface_used_local_assignment_harness",
  "deviation.m10.metering_ledger_check_deferred_to_live_lane",
  "deviation.m10.lanes_interpreted_as_local_codex_and_claude_adapters",
]

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function nowIso() {
  return new Date().toISOString()
}

function parseOvernightArgs(argv: string[]): OvernightArgs {
  const usage =
    "usage: overnight-proof-run.ts --task-repo <path> --proofs-dir <path> --pylon-home <path> " +
    "[--cycles <n>] [--sleep-seconds <n>] [--deadline-minutes <n>] [--run-id <id>]"
  let taskRepo: string | null = null
  let proofsDir: string | null = null
  let pylonHome: string | null = null
  let cycles = 8
  let sleepSeconds = 1500
  let deadlineMinutes = 420
  let runId = `m10-${Date.now().toString(36)}`
  for (let index = 0; index < argv.length; index += 2) {
    const arg = argv[index]
    const value = argv[index + 1]
    if (typeof value !== "string") throw new Error(usage)
    if (arg === "--task-repo") taskRepo = resolve(value)
    else if (arg === "--proofs-dir") proofsDir = resolve(value)
    else if (arg === "--pylon-home") pylonHome = resolve(value)
    else if (arg === "--cycles") cycles = boundedInt(value, 1, 50, usage)
    else if (arg === "--sleep-seconds") sleepSeconds = boundedInt(value, 0, 7200, usage)
    else if (arg === "--deadline-minutes") deadlineMinutes = boundedInt(value, 5, 720, usage)
    else if (arg === "--run-id") runId = value
    else throw new Error(usage)
  }
  if (taskRepo === null || proofsDir === null || pylonHome === null) throw new Error(usage)
  return { taskRepo, proofsDir, pylonHome, cycles, sleepSeconds, deadlineMinutes, runId }
}

function boundedInt(value: string, min: number, max: number, usage: string): number {
  const parsed = Number(value)
  if (!Number.isFinite(parsed) || parsed < min || parsed > max) throw new Error(usage)
  return Math.floor(parsed)
}

/** Combined house redaction gate: daily-driver patterns + both smoke scanners. */
function scanSerialized(serialized: string): string[] {
  return [
    ...scanProofSerialization(serialized),
    ...scanCodexProjection(serialized),
    ...scanClaudeProjection(serialized),
  ]
}

/**
 * Writes a retained artifact only when the redaction gate passes; otherwise
 * retains a refs-only quarantine record so the violation is visible without
 * leaking the violating content.
 */
async function writeRetained(path: string, artifact: unknown): Promise<{ clean: boolean }> {
  const serialized = JSON.stringify(artifact, null, 2)
  const violations = scanSerialized(serialized)
  if (violations.length === 0) {
    await writeFile(path, `${serialized}\n`, "utf8")
    return { clean: true }
  }
  await writeFile(
    path,
    `${JSON.stringify(
      {
        schema: "openagents.pylon.m10_overnight_quarantine.v0.1",
        observedAt: nowIso(),
        state: "quarantined",
        reason: "redaction_scan_failed",
        violationRefs: violations,
        artifactDigestRef: stableRef("digest.m10.quarantined_artifact", serialized),
      },
      null,
      2,
    )}\n`,
    "utf8",
  )
  return { clean: false }
}

function classifyError(error: unknown): { errorClass: string; errorDigestRef: string } {
  const message = error instanceof Error ? `${error.name}: ${error.message}` : String(error)
  const lowered = message.toLowerCase()
  const errorClass = lowered.includes("not ready")
    ? "lane_not_ready"
    : lowered.includes("timeout") || lowered.includes("timed out") || lowered.includes("budget")
      ? "timeout_or_budget"
      : lowered.includes("redaction")
        ? "redaction_gate"
        : lowered.includes("dev check")
          ? "verification_failed"
          : "execution_error"
  return { errorClass, errorDigestRef: stableRef("digest.m10.task_error", message) }
}

async function runGit(cwd: string, args: string[]): Promise<void> {
  const proc = Bun.spawn(["git", ...args], { cwd, stdout: "ignore", stderr: "ignore" })
  await proc.exited
}

class HeartbeatWriter {
  private sequence = 0
  private timer: ReturnType<typeof setInterval> | null = null
  private phase = "starting"
  private lastTaskIndex: number | null = null
  constructor(
    private readonly path: string,
    private readonly runRef: string,
  ) {}

  async record(phase: string, lastTaskIndex: number | null) {
    this.phase = phase
    this.lastTaskIndex = lastTaskIndex
    this.sequence += 1
    const record = {
      schema: M10_HEARTBEAT_SCHEMA,
      runRef: this.runRef,
      sequence: this.sequence,
      observedAt: nowIso(),
      phase,
      lastTaskIndex,
      pidRef: stableRef("ref.m10.pid", String(process.pid)),
    }
    const serialized = JSON.stringify(record)
    if (scanSerialized(serialized).length === 0) {
      await appendFile(this.path, `${serialized}\n`, "utf8")
    }
  }

  start(intervalMs: number) {
    this.timer = setInterval(() => {
      void this.record(this.phase, this.lastTaskIndex)
    }, intervalMs)
  }

  stop() {
    if (this.timer !== null) clearInterval(this.timer)
  }

  get count() {
    return this.sequence
  }
}

async function runComposerTask(input: {
  args: OvernightArgs
  cycle: number
  lane: Lane
  taskIndex: number
}): Promise<{ taskRef: string; resultRef: string | null; artifactFile: string; ok: boolean }> {
  const template = COMPOSER_TASKS[(input.cycle - 1) % COMPOSER_TASKS.length]!
  const laneDir = input.lane === "codex" ? "codex" : "claude"
  const dir = `tasks/c${input.cycle}/${laneDir}`
  const proof = await runProof({
    adapter: input.lane,
    accountRef: null,
    codexHome: null,
    claudeConfigDir: null,
    proofOutput: null,
    cwd: input.args.taskRepo,
    issueRefs: ["OpenAgentsInc/openagents#4768"],
    objective: template.objective(dir),
    promptFile: null,
    timeoutSeconds: 600,
    verificationArgv: template.verification(dir),
  })
  const fileName = `m10-c${input.cycle}-${laneDir}-composer-proof.json`
  const written = await writeRetained(join(input.args.proofsDir, fileName), proof)
  const ok = written.clean && proof.devCheck.state === "passed"
  return {
    taskRef: template.taskRef,
    resultRef: proof.executor.responseDigestRef,
    artifactFile: fileName,
    ok,
  }
}

async function runWorkOrderTask(input: {
  args: OvernightArgs
  cycle: number
  lane: Lane
  taskIndex: number
}): Promise<{ taskRef: string; resultRef: string | null; artifactFile: string; ok: boolean }> {
  const laneDir = input.lane === "codex" ? "codex" : "claude"
  const assignmentRef =
    input.lane === "codex"
      ? `assignment.public.codex_agent_task.m10_overnight_c${input.cycle}`
      : `assignment.public.claude_agent_task.m10_overnight_c${input.cycle}`
  const lease =
    input.lane === "codex"
      ? codexAgentSmokeLease({ assignmentRef })
      : claudeAgentSmokeLease({ assignmentRef })
  const harness = ciHarness(lease)
  const observedAt = nowIso()
  try {
    const summary = createBootstrapSummary(
      parseBootstrapArgs(["--display-name", "M10 Overnight Proof"]),
      { ...Bun.env, PYLON_HOME: input.args.pylonHome },
    )
    const state = await ensurePylonLocalState(summary)
    await writeFile(
      state.paths.runtimeState,
      `${JSON.stringify({
        lifecycle: "assignment-ready",
        displayName: "M10 Overnight Proof",
        resourceMode: "background_20",
        capabilityRefs: [CODEX_AGENT_CAPABILITY_REF, CLAUDE_AGENT_CAPABILITY_REF],
        blockerRefs: [],
        updatedAt: nowIso(),
      })}\n`,
    )
    await sendHeartbeat(summary, { baseUrl: harness.baseUrl })

    // Real probe + real SDK runners: no runner/probe injection here. The
    // executor runs the lease's bounded fixture task with the local
    // subscription-backed lane and verifies it with the fixture's real
    // verification command before closing out.
    const run = await runNoSpendAssignment(summary, { baseUrl: harness.baseUrl })

    const closeout = "closeout" in run && run.closeout !== undefined ? run.closeout : null
    const closeoutReceipt =
      "closeoutReceipt" in run && run.closeoutReceipt !== undefined ? run.closeoutReceipt : null
    const artifact = {
      schema: M10_TASK_SCHEMA,
      runRef: stableRef("run.m10.overnight", input.args.runId),
      taskIndex: input.taskIndex,
      cycle: input.cycle,
      lane: input.lane,
      surface: "work_order" as const,
      observedAt,
      completedAt: nowIso(),
      executionMode: "local_bounded" as const,
      paymentMode: "no-spend" as const,
      assignmentApi: "local_harness" as const,
      ok: run.ok === true,
      assignmentRef: "lease" in run && run.lease !== undefined ? run.lease.assignmentRef : null,
      closeoutStatus: closeout?.status ?? null,
      closeoutRef:
        closeoutReceipt !== null && typeof closeoutReceipt === "object" && "closeoutRef" in closeoutReceipt
          ? String(closeoutReceipt.closeoutRef)
          : null,
      receiptRefs: closeout?.receiptRefs ?? [],
      proofRefs: closeout?.proofRefs ?? [],
      resultRefs: closeout?.resultRefs ?? [],
      blockerRefs: closeout?.blockerRefs ?? [],
      boundaryChecks: {
        paymentMode: closeout?.paymentMode ?? null,
        settlementState: closeout?.settlementState ?? null,
        payoutClaimAllowed: closeout?.payoutClaimAllowed ?? null,
        redacted: closeout?.redacted ?? null,
      },
      harnessRequestCount: harness.retained.length,
      redactionScan: { state: "clean" as const },
      deviations: ["deviation.m10.work_order_surface_used_local_assignment_harness"],
    }
    assertPublicProjectionSafe(artifact)
    const fileName = `m10-c${input.cycle}-${laneDir}-work-order-proof.json`
    const written = await writeRetained(join(input.args.proofsDir, fileName), artifact)
    const ok =
      written.clean &&
      run.ok === true &&
      closeout?.status === "accepted" &&
      closeout.payoutClaimAllowed === false &&
      closeout.settlementState === "not_applicable"
    return {
      taskRef: input.lane === "codex" ? "task.m10.codex_work_order_fixture" : "task.m10.claude_work_order_fixture",
      resultRef: artifact.closeoutRef,
      artifactFile: fileName,
      ok,
    }
  } finally {
    harness.stop()
  }
}

async function main() {
  const args = parseOvernightArgs(Bun.argv.slice(2))
  const startedAt = nowIso()
  const deadlineAtMs = Date.now() + args.deadlineMinutes * 60 * 1000
  const runRef = stableRef("run.m10.overnight", args.runId)
  await mkdir(args.proofsDir, { recursive: true })
  await mkdir(args.pylonHome, { recursive: true })

  const heartbeat = new HeartbeatWriter(join(args.proofsDir, "heartbeats.jsonl"), runRef)
  heartbeat.start(120_000)
  await heartbeat.record("starting", null)

  // Start receipt to the (non-retained) run log.
  process.stdout.write(
    `${JSON.stringify({
      message: "M10 overnight unattended proof run starting",
      runId: args.runId,
      runRef,
      startedAt,
      cycles: args.cycles,
      sleepSeconds: args.sleepSeconds,
      deadlineMinutes: args.deadlineMinutes,
      lanes: ["codex", "claude_agent"],
      surfaces: ["composer", "work_order"],
      taskRepo: args.taskRepo,
      proofsDir: args.proofsDir,
    })}\n`,
  )

  const outcomes: TaskOutcome[] = []
  let taskIndex = 0
  let stoppedEarly: string | null = null

  for (let cycle = 1; cycle <= args.cycles; cycle += 1) {
    for (const cell of CYCLE_CELLS) {
      if (Date.now() > deadlineAtMs) {
        stoppedEarly = "deadline_reached"
        break
      }
      taskIndex += 1
      const taskStartedAt = nowIso()
      await heartbeat.record(`task_${taskIndex}_${cell.lane}_${cell.surface}_running`, taskIndex)
      process.stdout.write(
        `${JSON.stringify({ message: "task starting", taskIndex, cycle, ...cell, at: taskStartedAt })}\n`,
      )
      try {
        const result =
          cell.surface === "composer"
            ? await runComposerTask({ args, cycle, lane: cell.lane, taskIndex })
            : await runWorkOrderTask({ args, cycle, lane: cell.lane, taskIndex })
        outcomes.push({
          taskIndex,
          cycle,
          lane: cell.lane,
          surface: cell.surface,
          taskRef: result.taskRef,
          outcome: result.ok ? "completed" : "failed",
          resultRef: result.resultRef,
          artifactFile: result.artifactFile,
          startedAt: taskStartedAt,
          completedAt: nowIso(),
        })
        process.stdout.write(
          `${JSON.stringify({ message: "task finished", taskIndex, ok: result.ok, artifactFile: result.artifactFile, at: nowIso() })}\n`,
        )
      } catch (error) {
        const { errorClass, errorDigestRef } = classifyError(error)
        const fileName = `m10-task-failure-${taskIndex}.json`
        await writeRetained(join(args.proofsDir, fileName), {
          schema: M10_FAILURE_SCHEMA,
          runRef,
          taskIndex,
          cycle,
          lane: cell.lane,
          surface: cell.surface,
          observedAt: nowIso(),
          errorClass,
          errorDigestRef,
        })
        outcomes.push({
          taskIndex,
          cycle,
          lane: cell.lane,
          surface: cell.surface,
          taskRef: `task.m10.failed_${cell.surface}`,
          outcome: "failed",
          resultRef: errorDigestRef,
          artifactFile: fileName,
          startedAt: taskStartedAt,
          completedAt: nowIso(),
        })
        process.stdout.write(
          `${JSON.stringify({ message: "task failed", taskIndex, errorClass, error: String(error), at: nowIso() })}\n`,
        )
      }
      await heartbeat.record(`task_${taskIndex}_${cell.lane}_${cell.surface}_done`, taskIndex)
    }
    if (stoppedEarly !== null) break
    // Keep the scratch task repo tidy between cycles so each composer task
    // starts from a committed state.
    await runGit(args.taskRepo, ["add", "-A"])
    await runGit(args.taskRepo, ["commit", "-m", `m10 overnight cycle ${cycle} task output`])
    if (cycle < args.cycles && args.sleepSeconds > 0) {
      if (Date.now() + args.sleepSeconds * 1000 > deadlineAtMs) {
        stoppedEarly = "deadline_reached"
        break
      }
      await heartbeat.record(`sleeping_after_cycle_${cycle}`, taskIndex)
      await new Promise((resolveSleep) => setTimeout(resolveSleep, args.sleepSeconds * 1000))
    }
  }

  const completedAt = nowIso()
  const byCell = (lane: Lane, surface: Surface) =>
    outcomes.filter((outcome) => outcome.lane === lane && outcome.surface === surface)
  const summary = {
    schema: M10_SUMMARY_SCHEMA,
    runRef,
    issueRefs: ["OpenAgentsInc/openagents#4768"],
    supervision: {
      label: "unattended_overnight_owner_directed_batch",
      ownerDirected: true,
      humanInLoopDuringRun: false,
      executionMode: "local_bounded" as const,
      paymentMode: "no-spend" as const,
    },
    startedAt,
    completedAt,
    stoppedEarly,
    plan: {
      cycles: args.cycles,
      cellsPerCycle: CYCLE_CELLS.length,
      lanes: ["codex", "claude_agent"],
      surfaces: ["composer", "work_order"],
      sleepSeconds: args.sleepSeconds,
      deadlineMinutes: args.deadlineMinutes,
    },
    counts: {
      tasksAttempted: outcomes.length,
      tasksCompleted: outcomes.filter((outcome) => outcome.outcome === "completed").length,
      tasksFailed: outcomes.filter((outcome) => outcome.outcome === "failed").length,
      heartbeatsWritten: heartbeat.count,
      byCell: {
        codexComposer: byCell("codex", "composer").length,
        claudeComposer: byCell("claude_agent", "composer").length,
        codexWorkOrder: byCell("codex", "work_order").length,
        claudeWorkOrder: byCell("claude_agent", "work_order").length,
      },
    },
    tasks: outcomes,
    redactionScan: { state: "clean" as const, appliedToEveryArtifact: true },
    deviations: DEVIATIONS,
  }
  await heartbeat.record("writing_summary", taskIndex)
  heartbeat.stop()
  await writeRetained(join(args.proofsDir, "m10-overnight-summary.json"), summary)
  await appendFile(
    join(args.proofsDir, "heartbeats.jsonl"),
    `${JSON.stringify({
      schema: M10_HEARTBEAT_SCHEMA,
      runRef,
      sequence: heartbeat.count + 1,
      observedAt: nowIso(),
      phase: "finished",
      lastTaskIndex: taskIndex,
      pidRef: stableRef("ref.m10.pid", String(process.pid)),
    })}\n`,
    "utf8",
  )
  process.stdout.write(
    `${JSON.stringify({
      message: "M10 overnight run finished",
      runRef,
      tasksAttempted: summary.counts.tasksAttempted,
      tasksCompleted: summary.counts.tasksCompleted,
      tasksFailed: summary.counts.tasksFailed,
      stoppedEarly,
      completedAt,
    })}\n`,
  )
}

if (import.meta.main) {
  await main()
}
