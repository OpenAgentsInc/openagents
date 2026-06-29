import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { runNoSpendAssignment, type AssignmentClientOptions, type PylonAssignmentLease } from "./assignment.js"
import {
  CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
  CODEX_AGENT_TASK_SCHEMA,
  type CodexAgentRunner,
} from "./codex-agent-executor.js"
import { CODEX_AGENT_CAPABILITY_REF, CODEX_AGENT_SDK_PACKAGE } from "./codex-agent.js"
import { createBootstrapSummary, parseBootstrapArgs } from "./bootstrap.js"
import { sendHeartbeat } from "./presence.js"
import { ensurePylonLocalState } from "./state.js"

/**
 * The bounded local-Codex real-task smoke (issue #4790, epic #4793,
 * promise autopilot.codex_probe_pylon_successor.v1).
 *
 * CI-safe mode drives the full assignment lifecycle — poll, admit
 * (capability-gated), accept, execute, verify, progress, artifacts,
 * closeout — through the worker loop against a local assignment-API
 * harness, with a mock SDK runner standing in for the agent. Live mode
 * (the runbook's operator leg, CX4 #4791) runs the same worker loop
 * against production with the real SDK runner and the owner's own
 * credentials.
 */

export const CODEX_AGENT_TASK_JOB_KIND = "codex_agent_task"

export type CodexAgentTaskSmokeResult = {
  schema: "openagents.pylon.codex_agent_task_smoke.v0.3"
  mode: "ci_safe" | "live"
  ok: boolean
  assignmentRef: string | null
  closeoutStatus: string | null
  closeoutRef: string | null
  resultRefs: string[]
  blockerRefs: string[]
  boundaryChecks: {
    paymentMode: string | null
    settlementState: string | null
    payoutClaimAllowed: boolean | null
    redacted: boolean | null
  }
  redactionScan: { scannedRequestCount: number; violations: string[] }
}

export function codexAgentSmokeLease(
  input: { assignmentRef?: string; leaseRef?: string } = {},
): PylonAssignmentLease {
  const assignmentRef = input.assignmentRef ?? "assignment.public.codex_agent_task.ci_smoke"
  return {
    schema: "openagents.pylon.assignment_lease.v0.3",
    assignmentRef,
    leaseRef: input.leaseRef ?? assignmentRef,
    goal: `goal.public.codex_agent_task.${CODEX_AGENT_SUM_REPAIR_FIXTURE_REF}`,
    paymentMode: "no-spend",
    capabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
    codingAssignment: {
      kind: CODEX_AGENT_TASK_JOB_KIND,
      objective: { objectiveRef: `goal.public.codex_agent_task.${CODEX_AGENT_SUM_REPAIR_FIXTURE_REF}` },
      requiredCapabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
      codex: {
        schema: CODEX_AGENT_TASK_SCHEMA,
        agentKind: "codex_sdk",
        fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
        timeoutSeconds: 300,
      },
    },
    expiresAt: new Date(Date.now() + 15 * 60 * 1000).toISOString(),
  }
}

const REDACTION_PATTERNS: ReadonlyArray<readonly [string, RegExp]> = [
  ["local_user_path", /\/Users\/[a-z0-9_-]+\//i],
  ["local_home_path", /\/home\/[a-z0-9_-]+\//i],
  ["pylon_home_path", /\.pylon\/(cache|state)/i],
  ["codex_home_path", /\.codex\/(auth|config)/i],
  ["instruction_text", /bounded fixture workspace/i],
  ["openai_key_shape", /sk-[a-z0-9_-]{16,}/i],
  ["codex_env_name", /CODEX_API_KEY/],
  ["openai_env_name", /OPENAI_API_KEY/],
  ["bearer_material", /bearer\s+[a-z0-9._-]{8,}/i],
  ["raw_prompt", /raw prompt/i],
  ["provider_payload", /"messages"\s*:\s*\[/],
]

export function scanRetainedProjection(serialized: string): string[] {
  const violations: string[] = []
  for (const [label, pattern] of REDACTION_PATTERNS) {
    if (pattern.test(serialized)) violations.push(`redaction.${label}`)
  }
  return violations
}

const ciFixingRunner: CodexAgentRunner = async (input) => {
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  return { outcome: "completed", turnCount: 1, editedFileCount: 1, commandCount: 1, sessionRef: null }
}

/**
 * Local in-process assignment-API harness (poll/accept/progress/artifacts/
 * closeout/heartbeat). Exported so other no-spend proof harnesses (e.g. the
 * M10 overnight runner) can drive the same work-order lifecycle against a
 * lease-agnostic local server instead of the live API.
 */
export function ciHarness(lease: PylonAssignmentLease) {
  const retained: string[] = []
  const server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url)
      const text = await request.text()
      retained.push(`${url.pathname} ${text}`)
      if (url.pathname.includes("/heartbeat")) {
        const body = text ? (JSON.parse(text) as { pylonRef?: string; sequence?: number }) : {}
        return Response.json({ heartbeatRef: `heartbeat.${body.pylonRef}.${body.sequence}` })
      }
      if (url.pathname.endsWith("/assignments")) {
        return Response.json({
          schema: "openagents.pylon.assignment_poll_response.v0.3",
          assignments: [lease],
        })
      }
      if (url.pathname.endsWith("/accept")) {
        return Response.json({ statusRef: `assignment.accepted.${lease.assignmentRef}` })
      }
      if (url.pathname.endsWith("/progress")) {
        return Response.json({ progressRef: `assignment.progress.${lease.leaseRef}` })
      }
      if (url.pathname.endsWith("/artifacts")) {
        return Response.json({ artifactRef: `assignment.artifacts.${lease.leaseRef}` })
      }
      if (url.pathname.endsWith("/closeout")) {
        return Response.json({ closeoutRef: `assignment.closeout.${lease.leaseRef}` })
      }
      return Response.json({ errorRef: "error.not_found" }, { status: 404 })
    },
  })
  return {
    baseUrl: `http://127.0.0.1:${server.port}`,
    retained,
    stop: () => server.stop(true),
  }
}

function smokeResult(
  mode: "ci_safe" | "live",
  run: Awaited<ReturnType<typeof runNoSpendAssignment>>,
  redactionScan: CodexAgentTaskSmokeResult["redactionScan"],
): CodexAgentTaskSmokeResult {
  const closeout = "closeout" in run ? run.closeout : undefined
  const closeoutReceipt = "closeoutReceipt" in run ? run.closeoutReceipt : undefined
  const ok =
    run.ok === true &&
    closeout?.status === "accepted" &&
    closeout.payoutClaimAllowed === false &&
    closeout.settlementState === "not_applicable" &&
    redactionScan.violations.length === 0
  return {
    schema: "openagents.pylon.codex_agent_task_smoke.v0.3",
    mode,
    ok,
    assignmentRef: "lease" in run && run.lease !== undefined ? run.lease.assignmentRef : null,
    closeoutStatus: closeout?.status ?? null,
    closeoutRef: closeoutReceipt?.closeoutRef ?? null,
    resultRefs: closeout?.resultRefs ?? [],
    blockerRefs: closeout?.blockerRefs ?? [],
    boundaryChecks: {
      paymentMode: closeout?.paymentMode ?? null,
      settlementState: closeout?.settlementState ?? null,
      payoutClaimAllowed: closeout?.payoutClaimAllowed ?? null,
      redacted: closeout?.redacted ?? null,
    },
    redactionScan,
  }
}

/**
 * CI-safe leg: local harness, mock runner, ready probe. Proves the full
 * worker-loop lifecycle and the retained-projection redaction discipline
 * without credentials, network, or spend.
 */
export async function runCodexAgentTaskCiSmoke(): Promise<CodexAgentTaskSmokeResult> {
  const home = await mkdtemp(join(tmpdir(), "pylon-codex-agent-smoke-"))
  const lease = codexAgentSmokeLease()
  const harness = ciHarness(lease)
  try {
    const summary = createBootstrapSummary(
      parseBootstrapArgs(["--display-name", "Codex Agent Smoke"]),
      { PYLON_HOME: home },
      "darwin",
    )
    const state = await ensurePylonLocalState(summary)
    await writeFile(
      state.paths.runtimeState,
      `${JSON.stringify({
        lifecycle: "assignment-ready",
        displayName: "Codex Agent Smoke",
        resourceMode: "background_20",
        capabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
        blockerRefs: [],
        updatedAt: new Date().toISOString(),
      })}\n`,
    )
    await sendHeartbeat(summary, { baseUrl: harness.baseUrl })

    const run = await runNoSpendAssignment(summary, {
      baseUrl: harness.baseUrl,
      codexAgentRunner: ciFixingRunner,
      codexAgentProbe: {
        env: { CODEX_API_KEY: "ci-smoke-key-shape" },
        platform: "darwin",
        codexCliLoginPresent: false,
        importer: async (specifier: string) => {
          if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
          return {}
        },
      },
    })

    const closeout = "closeout" in run ? run.closeout : {}
    const serialized = `${harness.retained.join("\n")}\n${JSON.stringify(closeout)}`
    const redactionScan = {
      scannedRequestCount: harness.retained.length,
      violations: scanRetainedProjection(serialized),
    }
    return smokeResult("ci_safe", run, redactionScan)
  } finally {
    harness.stop()
    await rm(home, { recursive: true, force: true })
  }
}

/**
 * Live leg: the installed binary's worker loop against a real deployment,
 * real probe, real SDK runner, the owner's own credentials. Requires a
 * dispatched codex_agent_task assignment for this Pylon (see the worker
 * dispatch script and docs/codex-agent-task-smoke.md).
 */
export async function runCodexAgentTaskLiveSmoke(
  options: Pick<AssignmentClientOptions, "agentToken" | "baseUrl">,
): Promise<CodexAgentTaskSmokeResult> {
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
  const run = await runNoSpendAssignment(summary, { ...options })
  const serialized = JSON.stringify("closeout" in run ? run.closeout : {})
  const redactionScan = { scannedRequestCount: 1, violations: scanRetainedProjection(serialized) }
  return smokeResult("live", run, redactionScan)
}
