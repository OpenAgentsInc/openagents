import { randomUUID } from "node:crypto"

import type {
  KhalaCodexRateLimitProviderStatus,
  KhalaCodexRateLimitResetOutcome,
} from "../shared/codex-rate-limits.js"
import type { KhalaAppleFmReadiness } from "../shared/apple-fm-readiness.js"
import type { OnDeviceDeciderSelection } from "../shared/on-device-decider.js"
import {
  type KhalaCodeDesktopAppInfo,
  type KhalaCodeDesktopChatTurnEvent,
  type KhalaCodeDesktopCodexAccountsStatus,
  type KhalaCodeDesktopCodexRateLimitResetResult,
  type KhalaCodeDesktopFleetStatus,
  type KhalaCodeDesktopRPCSchema,
  type KhalaCodeDesktopRuntimeStatus,
} from "../shared/rpc.js"
import {
  consumeKhalaCodexRateLimitResetCredit,
  fetchKhalaCodexRateLimitStatus,
} from "./codex-rate-limits.js"
import {
  khalaCodeDesktopToolCatalog,
  runKhalaCodeDesktopChatTurn,
} from "./khala-chat-runtime.js"
import { ensureLocalPylon, inspectCodexFleet } from "./khala-codex-fleet-tools.js"

type ChatEnv = Readonly<Record<string, string | undefined>>
type MaybePromise<T> = T | Promise<T>

export type KhalaCodeDesktopRpcHandlersInput = {
  readonly appleFmReadiness: () => MaybePromise<KhalaAppleFmReadiness>
  readonly codexRateLimitStatus?: () => MaybePromise<KhalaCodexRateLimitProviderStatus>
  readonly consumeCodexRateLimitResetCredit?: (input: {
    readonly idempotencyKey: string
  }) => MaybePromise<KhalaCodexRateLimitResetOutcome>
  readonly env: ChatEnv
  readonly emitChatTurnEvent?: (event: KhalaCodeDesktopChatTurnEvent) => void
  readonly onDeviceDeciderStatus: () => MaybePromise<OnDeviceDeciderSelection>
  readonly workingDirectory: string
}

const appInfo = (): KhalaCodeDesktopAppInfo => ({
  ok: true,
  app: "Khala Code Desktop",
  observedAt: new Date().toISOString(),
})

const runtimeStatus = (input: {
  readonly available: boolean
  readonly capability: KhalaCodeDesktopRuntimeStatus["capability"]
  readonly reason: string
  readonly status: KhalaCodeDesktopRuntimeStatus["status"]
}): KhalaCodeDesktopRuntimeStatus => ({
  ok: true,
  app: "Khala Code Desktop",
  available: input.available,
  capability: input.capability,
  observedAt: new Date().toISOString(),
  reason: input.reason,
  status: input.status,
})

const codexStatusFromRateLimits = (
  rateLimits: KhalaCodexRateLimitProviderStatus,
  env: ChatEnv,
): KhalaCodeDesktopCodexAccountsStatus => {
  const available = rateLimits.status === "ok"
  const credentialSource = env.CODEX_HOME?.trim()
    ? "CODEX_HOME" as const
    : "default_home" as const
  const readinessState =
    rateLimits.status === "ok"
      ? "ready" as const
      : rateLimits.status === "unavailable"
        ? "credentials_missing" as const
        : "error" as const
  const blockerRefs =
    readinessState === "ready"
      ? []
      : readinessState === "credentials_missing"
        ? ["blocker.codex.credentials_missing"]
        : ["blocker.codex.rate_limit_status_error"]
  const status = available
    ? "ready" as const
    : rateLimits.status === "unavailable"
      ? "unavailable" as const
      : "error" as const

  return {
    ok: true,
    app: "Khala Code Desktop",
    available,
    capability: "codex_accounts",
    observedAt: new Date().toISOString(),
    reason: available
      ? "Codex CLI account is signed in and rate-limit windows are available."
      : rateLimits.error ?? "Codex account status is unavailable.",
    status,
    accounts: [
      {
        provider: "codex",
        accountRef: "default",
        credentialSource,
        homeRef: credentialSource === "CODEX_HOME" ? "env:CODEX_HOME" : "default:~/.codex",
        readiness: {
          state: readinessState,
          blockerRefs,
        },
        rateLimits,
      },
    ],
    rateLimits,
  }
}

export function createKhalaCodeDesktopRpcRequestHandlers(
  input: KhalaCodeDesktopRpcHandlersInput,
): KhalaCodeDesktopRPCSchema["requests"] {
  const codexAccountsStatus = async (): Promise<KhalaCodeDesktopCodexAccountsStatus> => {
    const rateLimits = await (input.codexRateLimitStatus?.() ??
      fetchKhalaCodexRateLimitStatus({ env: input.env as NodeJS.ProcessEnv }))
    return codexStatusFromRateLimits(rateLimits, input.env)
  }

  return {
    async appInfo() {
      return appInfo()
    },
    async appleFmReadiness() {
      return input.appleFmReadiness()
    },
    async codexAccountsStatus() {
      return codexAccountsStatus()
    },
    async codexFleetStatus(): Promise<KhalaCodeDesktopFleetStatus> {
      const fleet = await inspectCodexFleet(
        { includeProcesses: true, startPylon: false },
        { env: input.env as NodeJS.ProcessEnv },
      )
      return {
        ok: fleet.ensure.ok,
        observedAt: fleet.observedAt,
        pylon: {
          status: fleet.ensure.status,
          pylonRef: fleet.ensure.pylonRef,
          message: fleet.ensure.message,
        },
        availableCodexAssignments: fleet.availableCodexAssignments,
        maxCodexAssignments: fleet.maxCodexAssignments,
        accounts: fleet.accounts.map(account => ({
          accountRef: account.accountRef,
          provider: account.provider,
          readiness: account.readiness,
          quotaState: account.quotaState,
          accountKey: account.accountKey,
        })),
        activeAssignments: fleet.activeAssignments.map(marker => ({
          assignmentRef: marker.assignmentRef,
          issueRef: marker.issueRef,
          updatedAt: marker.updatedAt,
        })),
        processes: fleet.processes.map(process => ({
          pid: process.pid,
          parentPid: process.parentPid,
          elapsed: process.elapsed,
        })),
      }
    },
    async codingStatus() {
      return runtimeStatus({
        available: true,
        capability: "coding",
        reason: "Khala Code chat and owner-local tools are served by this desktop process.",
        status: "ready",
      })
    },
    async onDeviceDeciderStatus() {
      return input.onDeviceDeciderStatus()
    },
    async pylonStatus() {
      const status = await ensureLocalPylon({
        start: false,
        timeoutMs: 10_000,
        waitMs: 0,
      }, {
        env: input.env,
      })
      return runtimeStatus({
        available: status.ok,
        capability: "pylon",
        reason: status.ok
          ? status.message
          : `${status.message}${status.unavailableReason ? ` ${status.unavailableReason}` : ""}`,
        status: status.ok ? "ready" : "unavailable",
      })
    },
    async submitChatMessage(request) {
      return runKhalaCodeDesktopChatTurn({
        env: input.env,
        ...(input.emitChatTurnEvent === undefined ? {} : { onEvent: input.emitChatTurnEvent }),
        request,
        workingDirectory: input.workingDirectory,
      })
    },
    async consumeCodexRateLimitResetCredit(): Promise<KhalaCodeDesktopCodexRateLimitResetResult> {
      const observedAt = new Date().toISOString()
      const idempotencyKey = randomUUID()
      try {
        const outcome = await (input.consumeCodexRateLimitResetCredit?.({
          idempotencyKey,
        }) ??
          consumeKhalaCodexRateLimitResetCredit({
            env: input.env as NodeJS.ProcessEnv,
            idempotencyKey,
          }))
        return {
          ok: true,
          observedAt,
          outcome,
          status: await codexAccountsStatus(),
        }
      } catch (error) {
        return {
          ok: false,
          observedAt,
          outcome: null,
          status: await codexAccountsStatus(),
          error: error instanceof Error ? error.message : String(error),
        }
      }
    },
    async tokenAccountingStatus() {
      return runtimeStatus({
        available: false,
        capability: "token_accounting",
        reason: "Token accounting is handled by hosted OpenAgents when a cloud credential is configured.",
        status: "not_configured",
      })
    },
    async toolCatalog() {
      return khalaCodeDesktopToolCatalog()
    },
  }
}
