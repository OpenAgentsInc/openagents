import { describe, expect, test } from "bun:test"
import { mkdtempSync } from "node:fs"
import { mkdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  createControlSessionActions,
  type ControlSessionExecutor,
  type ControlSessionExecutorResult,
} from "../src/node/control-sessions"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  runSessionsExec,
  type SessionsExecControl,
} from "../src/node/sessions-exec"
import { createBoundedAutoApprovalPolicy } from "../src/node/auto-approval-policy"
import { PYLON_DEV_CHECK_SCHEMA, type PylonDevCheckProjection } from "../src/dev-loop"

// W-1 (#5377): exercise the blocking run-to-completion driver against a loopback
// control surface backed by a STUB executor — the same way the desktop proofs
// drive the control functions. We wrap `createControlSessionActions(...)` in the
// thin SessionsExecControl the CLI also uses, so the test covers the real
// spawn → poll-to-terminal → artifact path, not a mock of the driver.

async function withFixture<T>(fn: (fixture: {
  proofDir: string
  pylonHome: string
  summary: ReturnType<typeof createBootstrapSummary>
  worktree: string
}) => Promise<T>) {
  const root = mkdtempSync(join(tmpdir(), "pylon-sessions-exec-"))
  try {
    const pylonHome = join(root, "pylon-home")
    const accountHome = join(root, "codex-home")
    const worktree = join(root, "worktree")
    const proofDir = join(root, "proofs")
    await mkdir(pylonHome, { recursive: true })
    await mkdir(accountHome, { recursive: true })
    await mkdir(worktree, { recursive: true })
    await writeFile(
      join(pylonHome, "config.json"),
      `${JSON.stringify({ dev: { accounts: [{ ref: "codex-a", provider: "codex", home: accountHome }] } })}\n`,
    )
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
    return await fn({ proofDir, pylonHome, summary, worktree })
  } finally {
    await rm(root, { recursive: true, force: true })
  }
}

// A dev-check projection the stub executor returns so the session reaches a
// real completed/failed terminal with a changeset + verify outcome.
function devCheck(state: "passed" | "failed"): PylonDevCheckProjection {
  return {
    schema: PYLON_DEV_CHECK_SCHEMA,
    observedAt: new Date().toISOString(),
    action: "check",
    state,
    changeSummary: {
      repo: { state: "ready", rootRef: "root.x", branch: "branch.x", commit: "commit.x" },
      dirty: { state: "dirty", changedCount: 1, stagedCount: 0, unstagedCount: 1, untrackedCount: 0 },
      changedFileRefs: [
        { fileRef: "file.pylon.src_x", status: "modified", area: "pylon.source", extension: "ts" },
      ],
      areaRefs: ["area.pylon.source"],
      blockerRefs: [],
    },
    checkPlan: { state: "ready", commandRefs: ["command.verify"], blockerRefs: [] },
    commandResults: [
      {
        commandRef: "command.verify",
        reasonRef: "check.verify",
        cwdRef: "command.cwd.x",
        argvRef: "command.argv.verify",
        exitCode: state === "passed" ? 0 : 1,
        status: state === "passed" ? "passed" : "failed",
        durationMs: 1,
        stdoutBytes: 0,
        stderrBytes: 0,
        stdoutDigestRef: null,
        stderrDigestRef: null,
      },
    ],
    latestRecordRef: null,
    branchUntouched: true,
    commitUntouched: true,
    pushPerformed: false,
    blockerRefs: [],
  }
}

function executorResult(state: "passed" | "failed"): ControlSessionExecutorResult {
  return {
    commandCount: 1,
    devCheck: devCheck(state),
    editedFileCount: 1,
    eventCount: 2,
    executionMode: "local_bounded",
    externalSessionRef: null,
    responseDigestRef: null,
    totalTokens: 0,
  }
}

// Wrap the in-process control actions in the thin driver control adapter the
// CLI uses, so the test drives the SAME surface.
function controlFrom(actions: ReturnType<typeof createControlSessionActions>): SessionsExecControl {
  return {
    spawn: (cmd) => actions.spawn(cmd as never),
    list: () => actions.list() as never,
    events: (ref) => actions.events(ref) as never,
    artifact: (ref) => actions.artifact(ref) as never,
  }
}

describe("pylon sessions exec (W-1 run-to-completion)", () => {
  test("spawns, polls to a completed terminal, and returns a structured JSON result with verify outcome + changeset", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async (input) => {
        input.emit({ phase: "composer_event", message: "edited file.pylon.src_x", composerEventIndex: 1 })
        return executorResult("passed")
      }
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const result = await runSessionsExec(controlFrom(actions), {
        adapter: "codex",
        objective: "make the change and verify",
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 25,
      })

      expect(result.schema).toBe("openagents.pylon.sessions_exec_result.v0.1")
      expect(result.ok).toBe(true)
      expect(result.outcome).toBe("completed")
      expect(result.state).toBe("completed")
      expect(result.sessionRef).toStartWith("session.pylon.control.")
      expect(result.resultRef).toStartWith("result.pylon.control_session.")
      expect(result.artifactRef).toStartWith("artifact.pylon.control_session.proof.")
      // Result summary comes from the session's own (redaction-scanned) tail.
      expect(result.resultSummary).toContain("edited file.pylon.src_x")
      // Changeset + verify outcome reused from the retained proof artifact.
      expect(result.verify?.passed).toBe(true)
      expect(result.verify?.commands[0]?.status).toBe("passed")
      expect(result.changeset?.state).toBe("dirty")
      expect(result.changeset?.changedFileRefs.length).toBe(1)
      expect(result.pendingApprovals).toEqual([])
      expect(result.driver.timedOut).toBe(false)
      expect(result.driver.polls).toBeGreaterThan(0)
    })
  })

  test("forwards the requested cloud lane into session.spawn", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async () => executorResult("passed")
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const result = await runSessionsExec(controlFrom(actions), {
        adapter: "codex",
        lane: "cloud-gcp",
        objective: "run on the cloud lane when configured",
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 25,
      })
      expect(result.ok).toBe(true)
      const list = await actions.list()
      expect(list.find((entry) => entry.sessionRef === result.sessionRef)?.lane).toBe("cloud-gcp")
    })
  })

  test("verify failure drives to a failed terminal with ok=false and the failed verify captured", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async () => executorResult("failed")
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const result = await runSessionsExec(controlFrom(actions), {
        adapter: "codex",
        objective: "change that fails verify",
        verify: ["false"],
        worktreePath: worktree,
        pollIntervalMs: 25,
      })

      expect(result.ok).toBe(false)
      expect(result.outcome).toBe("failed")
      expect(result.state).toBe("failed")
      expect(result.errorClass).toBe("verification_failed")
      expect(result.verify?.passed).toBe(false)
      expect(result.verify?.commands[0]?.status).toBe("failed")
    })
  })

  test("executor throw drives to a failed terminal with a typed error class (no raw text)", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async () => {
        throw new Error("raw provider failure sentence must not surface")
      }
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const result = await runSessionsExec(controlFrom(actions), {
        adapter: "codex",
        objective: "change that throws",
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 25,
      })

      expect(result.ok).toBe(false)
      expect(result.outcome).toBe("failed")
      expect(result.state).toBe("failed")
      expect(result.errorClass).toBe("execution_error")
      expect(result.errorDigestRef).toStartWith("digest.pylon.session.error.")
      expect(JSON.stringify(result)).not.toContain("raw provider failure sentence")
    })
  })

  test("a wedged session times out under the driver deadline with outcome=timeout, ok=false", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async (input) =>
        // Never resolves until cancelled — the driver deadline must trip first.
        await new Promise<never>((_resolve, reject) => {
          input.abortSignal.addEventListener("abort", () => reject(new Error("cancelled")), { once: true })
        })
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const result = await runSessionsExec(controlFrom(actions), {
        adapter: "codex",
        objective: "never finishes",
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 10,
        deadlineMs: 80,
      })

      expect(result.ok).toBe(false)
      expect(result.outcome).toBe("timeout")
      expect(result.driver.timedOut).toBe(true)
      // Still non-terminal at the deadline (queued/running).
      expect(["queued", "running"]).toContain(result.state)
      await actions.cancel(result.sessionRef)
    })
  })

  test("a pending approval pauses the driver (default manual) and is surfaced in the result", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async (input) =>
        await new Promise<never>((_resolve, reject) => {
          input.abortSignal.addEventListener("abort", () => reject(new Error("cancelled")), { once: true })
        })
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const control: SessionsExecControl = {
        ...controlFrom(actions),
        approvalsList: async () => ({
          approvals: [{ approvalRef: "approval.x", kind: "labor_first_run" }],
        }),
      }
      const result = await runSessionsExec(control, {
        adapter: "codex",
        objective: "needs an approval",
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 10,
        deadlineMs: 5000,
        onApproval: "manual",
      })

      expect(result.outcome).toBe("approval_required")
      expect(result.ok).toBe(false)
      expect(result.pendingApprovals).toEqual([
        { approvalRef: "approval.x", kind: "labor_first_run", decision: "pause" },
      ])
      // The driver paused well before the deadline rather than blocking.
      expect(result.driver.timedOut).toBe(false)
      await actions.cancel(result.sessionRef)
    })
  })

  test("--on-approval=deny records a deny decision for a pending approval", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async (input) =>
        await new Promise<never>((_resolve, reject) => {
          input.abortSignal.addEventListener("abort", () => reject(new Error("cancelled")), { once: true })
        })
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const resolved: Array<{ approvalRef: string; decision: string }> = []
      const control: SessionsExecControl = {
        ...controlFrom(actions),
        approvalsList: async () => ({ approvals: [{ approvalRef: "approval.y", kind: "spend_gate" }] }),
        approvalsResolve: async (approvalRef, decision) => {
          resolved.push({ approvalRef, decision })
        },
      }
      const result = await runSessionsExec(control, {
        adapter: "codex",
        objective: "needs an approval we deny",
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 10,
        deadlineMs: 5000,
        onApproval: "deny",
      })

      expect(result.pendingApprovals).toEqual([
        { approvalRef: "approval.y", kind: "spend_gate", decision: "deny" },
      ])
      expect(resolved).toEqual([{ approvalRef: "approval.y", decision: "deny" }])
      expect(result.ok).toBe(false)
      await actions.cancel(result.sessionRef)
    })
  })

  test("W-3 plug point: an approvalPolicy callback can approve a pending approval", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      // The session completes normally; the callback approving an approval must
      // NOT block the terminal path. This proves W-3 can plug a real policy in.
      const executor: ControlSessionExecutor = async () => executorResult("passed")
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      let calls = 0
      const resolved: Array<{ approvalRef: string; decision: string }> = []
      const control: SessionsExecControl = {
        ...controlFrom(actions),
        approvalsList: async () => {
          calls += 1
          return calls === 1
            ? { approvals: [{ approvalRef: "approval.z", kind: "bounded_safe" }] }
            : { approvals: [] }
        },
        approvalsResolve: async (approvalRef, decision) => {
          resolved.push({ approvalRef, decision })
        },
      }
      const result = await runSessionsExec(control, {
        adapter: "codex",
        objective: "auto-approved bounded action",
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 10,
        deadlineMs: 5000,
        approvalPolicy: () => "approve",
      })

      expect(result.pendingApprovals).toEqual([
        { approvalRef: "approval.z", kind: "bounded_safe", decision: "approve" },
      ])
      expect(resolved).toEqual([{ approvalRef: "approval.z", decision: "approve" }])
      // An approve does NOT pause: the session is allowed to reach its terminal.
      expect(result.outcome).toBe("completed")
      expect(result.ok).toBe(true)
    })
  })

  // W-3 (#5379): the BOUNDED auto-approve policy driven through the real W-1
  // driver. An in-allow-list, in-scope approval auto-approves and the session
  // reaches its terminal; the audit lands in result.autoApprovals[].
  test("W-3 auto: an allow-listed in-scope approval auto-approves and is audited; session completes", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async () => executorResult("passed")
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      let calls = 0
      const resolved: Array<{ approvalRef: string; decision: string }> = []
      const control: SessionsExecControl = {
        ...controlFrom(actions),
        approvalsList: async () => {
          calls += 1
          return calls === 1
            ? { approvals: [{ approvalRef: "approval.edit", kind: "file_edit", paths: [`${worktree}/src/x.ts`] }] }
            : { approvals: [] }
        },
        approvalsResolve: async (approvalRef, decision) => {
          resolved.push({ approvalRef, decision })
        },
      }
      const auto = createBoundedAutoApprovalPolicy({ scopeRoot: worktree })
      const result = await runSessionsExec(control, {
        adapter: "codex",
        objective: "bounded auto-approved edit",
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 10,
        deadlineMs: 5000,
        onApproval: "auto",
        approvalPolicy: auto.policy,
        approvalAudit: auto.audit,
      })

      expect(result.outcome).toBe("completed")
      expect(result.ok).toBe(true)
      expect(result.pendingApprovals).toEqual([
        { approvalRef: "approval.edit", kind: "file_edit", decision: "approve" },
      ])
      expect(resolved).toEqual([{ approvalRef: "approval.edit", decision: "approve" }])
      expect(result.autoApprovals).toEqual([
        {
          approvalRef: "approval.edit",
          kind: "file_edit",
          category: "allow",
          decision: "approve",
          reason: "auto.allow.allow_listed_kind",
        },
      ])
    })
  })

  // W-3: an out-of-bounds approval (spend) is NOT auto-approved. It denies, the
  // driver stops, and the audit records the deny with a reason. No blanket bypass.
  test("W-3 auto: a spend approval is denied (deny beats allow), audited, and stops the run", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async (input) =>
        await new Promise<never>((_resolve, reject) => {
          input.abortSignal.addEventListener("abort", () => reject(new Error("cancelled")), { once: true })
        })
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const resolved: Array<{ approvalRef: string; decision: string }> = []
      const control: SessionsExecControl = {
        ...controlFrom(actions),
        approvalsList: async () => ({
          approvals: [{ approvalRef: "approval.spend", kind: "spend_gate", prompt: "pay 1000 sats" }],
        }),
        approvalsResolve: async (approvalRef, decision) => {
          resolved.push({ approvalRef, decision })
        },
      }
      const auto = createBoundedAutoApprovalPolicy({ scopeRoot: worktree })
      const result = await runSessionsExec(control, {
        adapter: "codex",
        objective: "out-of-bounds spend must not auto-approve",
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 10,
        deadlineMs: 5000,
        onApproval: "auto",
        approvalPolicy: auto.policy,
        approvalAudit: auto.audit,
      })

      expect(result.ok).toBe(false)
      expect(result.pendingApprovals[0]?.decision).toBe("deny")
      expect(resolved).toEqual([{ approvalRef: "approval.spend", decision: "deny" }])
      expect(result.autoApprovals[0]).toMatchObject({
        approvalRef: "approval.spend",
        category: "deny",
        decision: "deny",
        reason: "auto.deny.spend_or_secret",
      })
      // Audit is projection-safe — no raw prompt text.
      expect(JSON.stringify(result.autoApprovals)).not.toContain("pay 1000 sats")
      await actions.cancel(result.sessionRef)
    })
  })

  // W-3: an out-of-scope path escalates (pause) — the autonomous run stops and
  // reports approval_required rather than touching paths outside the worktree.
  test("W-3 auto: an out-of-scope path escalates to approval_required, audited as escalate", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async (input) =>
        await new Promise<never>((_resolve, reject) => {
          input.abortSignal.addEventListener("abort", () => reject(new Error("cancelled")), { once: true })
        })
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const control: SessionsExecControl = {
        ...controlFrom(actions),
        approvalsList: async () => ({
          approvals: [{ approvalRef: "approval.oos", kind: "file_edit", paths: ["/etc/hosts"] }],
        }),
      }
      const auto = createBoundedAutoApprovalPolicy({ scopeRoot: worktree })
      const result = await runSessionsExec(control, {
        adapter: "codex",
        objective: "out-of-scope edit must escalate",
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 10,
        deadlineMs: 5000,
        onApproval: "auto",
        approvalPolicy: auto.policy,
        approvalAudit: auto.audit,
      })

      expect(result.outcome).toBe("approval_required")
      expect(result.autoApprovals[0]).toMatchObject({
        category: "escalate",
        decision: "pause",
        reason: "auto.escalate.out_of_scope_path",
      })
      await actions.cancel(result.sessionRef)
    })
  })

  // W-3: default (no --on-approval auto) is unchanged — manual still pauses and
  // autoApprovals[] stays empty.
  test("W-3: default manual path is unchanged and autoApprovals[] is empty", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async () => executorResult("passed")
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const result = await runSessionsExec(controlFrom(actions), {
        adapter: "codex",
        objective: "default manual unchanged",
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 25,
      })
      expect(result.outcome).toBe("completed")
      expect(result.autoApprovals).toEqual([])
    })
  })
})
