import type { KhalaCodeDesktopFleetStatus } from "../../shared/rpc"

// A live-SHAPED fleet status fixture (mirrors the exact `KhalaCodeDesktopFleetStatus`
// RPC shape produced by `pylon-service.ts` / `fleet-run-supervisor.ts`). Used by
// the dev-only cockpit page and the render/dispatch tests. Public-safe: no
// emails, tokens, or local paths.
export const enCockpitFixtureStatus: KhalaCodeDesktopFleetStatus = {
  ok: true,
  observedAt: "2026-07-08T20:00:00.000Z",
  pylon: {
    status: "online",
    pylonRef: "pylon.local.cockpit",
    message: "online",
  },
  availableCodexAssignments: 1,
  maxCodexAssignments: 3,
  tokenRate: {
    activeAdjustedTokensPerMinute: null,
    completedStatus: "not_measured",
    completedTokenRows: null,
    completedTokensPerMinute: null,
    inFlightTokens: null,
    inFlightTokensPerMinute: null,
    source: "unavailable",
    unavailableReason: null,
  },
  accounts: [
    {
      accountRef: "codex",
      provider: "codex",
      readiness: "ready",
      quotaState: "available",
      accountKey: "account_key_public",
      capacity: null,
      email: null,
    },
    {
      accountRef: "claude",
      provider: "claude_agent",
      readiness: "ready",
      quotaState: "available",
      accountKey: "account_key_public_claude",
      capacity: null,
      email: null,
    },
    {
      accountRef: "codex-2",
      provider: "codex",
      readiness: "credentials_missing",
      quotaState: null,
      accountKey: null,
      capacity: null,
      email: null,
    },
  ],
  activeAssignments: [
    {
      assignmentRef: "assignment.public.one",
      runRef: "run.public.alpha",
      elapsedMs: 183000,
      issueRef: "github.issue.openagents.8586",
      workerSession: {
        approvalState: "approval_required",
        blockerRefs: [],
        closeoutStatus: null,
        executionRuntime: "codex_harness",
        homeRole: "pylon_isolated_worker_codex_home",
        queuePolicy: {
          admission: "pylon_capacity_gate",
          cooldown: "ready",
          refill: "pylon_presence_heartbeat",
          queued: null,
        },
        reviewState: "active",
        role: "swarm_worker_codex_session",
        transcriptRef: null,
      },
      tokenRate: {
        source: "unavailable",
        status: "not_measured",
        tokenCountKind: null,
        tokens: null,
        tokensPerMinute: null,
      },
      updatedAt: "2026-07-08T20:01:00.000Z",
    },
    {
      assignmentRef: "assignment.public.two",
      runRef: "run.public.alpha",
      elapsedMs: 42000,
      issueRef: "github.issue.openagents.8574",
      tokenRate: {
        source: "unavailable",
        status: "not_measured",
        tokenCountKind: null,
        tokens: null,
        tokensPerMinute: null,
      },
      updatedAt: "2026-07-08T20:01:30.000Z",
    },
  ],
  processes: [
    {
      pid: "200",
      parentPid: "199",
      elapsed: "00:03:03",
    },
  ],
}
