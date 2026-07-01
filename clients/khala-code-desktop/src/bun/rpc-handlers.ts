import { randomUUID } from "node:crypto"

import type {
  KhalaCodexRateLimitProviderStatus,
  KhalaCodexRateLimitResetOutcome,
} from "../shared/codex-rate-limits.js"
import type { CodexAppServerHost } from "./codex-app-server-client.js"
import type { KhalaAppleFmReadiness } from "../shared/apple-fm-readiness.js"
import type { OnDeviceDeciderSelection } from "../shared/on-device-decider.js"
import {
  type KhalaCodeDesktopAppInfo,
  type KhalaCodeDesktopChatTurnEvent,
  type KhalaCodeDesktopCodexAccountsStatus,
  type KhalaCodeDesktopCodexHarnessStatus,
  type KhalaCodeDesktopCodexRateLimitResetResult,
  type KhalaCodeDesktopFleetStatus,
  type KhalaCodeDesktopRPCSchema,
  type KhalaCodeDesktopRuntimeStatus,
} from "../shared/rpc.js"
import { inspectCodexHarnessStatus } from "./codex-harness-status.js"
import {
  consumeKhalaCodexRateLimitResetCredit,
  fetchKhalaCodexRateLimitStatus,
} from "./codex-rate-limits.js"
import {
  khalaCodeDesktopToolCatalog,
  runKhalaCodeDesktopChatTurn,
} from "./khala-chat-runtime.js"
import {
  beginCodexConnect,
  collectCodexAccountEmails,
  ensureLocalPylon,
  inspectCodexFleet,
  type KhalaCodexFleetToolOptions,
  openExternalUrl,
  removeCodexAccount,
} from "./khala-codex-fleet-tools.js"

type ChatEnv = Readonly<Record<string, string | undefined>>
type MaybePromise<T> = T | Promise<T>

export type KhalaCodeDesktopRpcHandlersInput = {
  readonly appleFmReadiness: () => MaybePromise<KhalaAppleFmReadiness>
  readonly codexAppServerHost?: CodexAppServerHost
  readonly codexRateLimitStatus?: () => MaybePromise<KhalaCodexRateLimitProviderStatus>
  readonly codexHarnessStatus?: () => MaybePromise<KhalaCodeDesktopCodexHarnessStatus>
  readonly consumeCodexRateLimitResetCredit?: (input: {
    readonly idempotencyKey: string
  }) => MaybePromise<KhalaCodexRateLimitResetOutcome>
  readonly codexFleetToolOptions?: KhalaCodexFleetToolOptions
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
  harness: KhalaCodeDesktopCodexHarnessStatus,
  env: ChatEnv,
): KhalaCodeDesktopCodexAccountsStatus => {
  const available = harness.available && rateLimits.status === "ok"
  const credentialSource = env.CODEX_HOME?.trim()
    ? "CODEX_HOME" as const
    : "default_home" as const
  const readinessState =
    harness.auth.state !== "ready"
      ? harness.auth.state === "invalid"
        ? "invalid" as const
        : harness.auth.state === "error"
          ? "error" as const
          : "credentials_missing" as const
      : rateLimits.status === "ok"
      ? "ready" as const
      : rateLimits.status === "unavailable"
        ? "credentials_missing" as const
        : "error" as const
  const blockerRefs =
    readinessState === "ready"
      ? []
      : harness.auth.blockerRefs.length > 0
        ? harness.auth.blockerRefs
        : readinessState === "credentials_missing"
          ? ["blocker.codex.credentials_missing"]
          : ["blocker.codex.rate_limit_status_error"]
  const status = available
    ? "ready" as const
    : harness.status === "unavailable" || rateLimits.status === "unavailable"
      ? "unavailable" as const
      : "error" as const

  return {
    ok: true,
    app: "Khala Code Desktop",
    available,
    capability: "codex_accounts",
    observedAt: new Date().toISOString(),
    reason: available
      ? "Codex CLI account is signed in, the harness is ready, and rate-limit windows are available."
      : harness.available
        ? rateLimits.error ?? "Codex account status is unavailable."
        : harness.reason,
    status,
    accounts: [
      {
        provider: "codex",
        accountRef: "default",
        credentialSource,
        homeRef: credentialSource === "CODEX_HOME" ? "env:CODEX_HOME" : "default:~/.codex",
        homeRole: "main_user_codex_home",
        readiness: {
          state: readinessState,
          blockerRefs,
        },
        rateLimits,
      },
    ],
    harness,
    rateLimits,
  }
}

const unavailableRateLimits = (
  error: string,
): KhalaCodexRateLimitProviderStatus => ({
  provider: "codex",
  session: null,
  weekly: null,
  rateLimitResetCredits: null,
  updatedAtIso: new Date().toISOString(),
  error,
  status: "unavailable",
})

export function createKhalaCodeDesktopRpcRequestHandlers(
  input: KhalaCodeDesktopRpcHandlersInput,
): KhalaCodeDesktopRPCSchema["requests"] {
  const codexHarnessStatus = async (): Promise<KhalaCodeDesktopCodexHarnessStatus> =>
    input.codexHarnessStatus?.() ??
    inspectCodexHarnessStatus({ env: input.env as NodeJS.ProcessEnv })

  const codexAccountsStatus = async (): Promise<KhalaCodeDesktopCodexAccountsStatus> => {
    const harness = await codexHarnessStatus()
    if (!harness.available) {
      return codexStatusFromRateLimits(unavailableRateLimits(harness.reason), harness, input.env)
    }
    const rateLimits = await (input.codexRateLimitStatus?.() ??
      fetchKhalaCodexRateLimitStatus({ env: input.env as NodeJS.ProcessEnv }))
    return codexStatusFromRateLimits(rateLimits, harness, input.env)
  }

  return {
    async appInfo() {
      return appInfo()
    },
    async appleFmReadiness() {
      return input.appleFmReadiness()
    },
    async codexAppServerRestart() {
      return input.codexAppServerHost?.restart() ?? {
        ok: false,
        action: "restart",
        changed: false,
        status: {
          ok: true,
          app: "Khala Code Desktop",
          adapterVersion: "unconfigured",
          codexCommand: "codex",
          codexHome: "",
          diagnostics: [],
          initialized: false,
          initializeResult: null,
          lastError: "Codex app-server host is not configured.",
          pendingRequestCount: 0,
          pid: null,
          state: "errored",
          transport: "stdio",
        },
        error: "Codex app-server host is not configured.",
      }
    },
    async codexAppServerStart() {
      return input.codexAppServerHost?.start() ?? {
        ok: false,
        action: "start",
        changed: false,
        status: {
          ok: true,
          app: "Khala Code Desktop",
          adapterVersion: "unconfigured",
          codexCommand: "codex",
          codexHome: "",
          diagnostics: [],
          initialized: false,
          initializeResult: null,
          lastError: "Codex app-server host is not configured.",
          pendingRequestCount: 0,
          pid: null,
          state: "errored",
          transport: "stdio",
        },
        error: "Codex app-server host is not configured.",
      }
    },
    async codexAppServerStatus() {
      return input.codexAppServerHost?.status() ?? {
        ok: true,
        app: "Khala Code Desktop",
        adapterVersion: "unconfigured",
        codexCommand: "codex",
        codexHome: "",
        diagnostics: [],
        initialized: false,
        initializeResult: null,
        lastError: "Codex app-server host is not configured.",
        pendingRequestCount: 0,
        pid: null,
        state: "errored",
        transport: "stdio",
      }
    },
    async codexAppServerStop() {
      return input.codexAppServerHost?.stop() ?? {
        ok: true,
        action: "stop",
        changed: false,
        status: {
          ok: true,
          app: "Khala Code Desktop",
          adapterVersion: "unconfigured",
          codexCommand: "codex",
          codexHome: "",
          diagnostics: [],
          initialized: false,
          initializeResult: null,
          lastError: "Codex app-server host is not configured.",
          pendingRequestCount: 0,
          pid: null,
          state: "stopped",
          transport: "stdio",
        },
      }
    },
    async codexAccountsStatus() {
      return codexAccountsStatus()
    },
    async codexFleetStatus(): Promise<KhalaCodeDesktopFleetStatus> {
      const fleet = await inspectCodexFleet(
        { includeProcesses: true, startPylon: false },
        { ...input.codexFleetToolOptions, env: input.env as NodeJS.ProcessEnv },
      )
      const emails = await collectCodexAccountEmails(
        fleet.accounts.map(account => account.accountRef),
        { env: input.env },
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
          capacity: account.capacity,
          email: emails[account.accountRef] ?? null,
        })),
        activeAssignments: fleet.activeAssignments.map(marker => ({
          assignmentRef: marker.assignmentRef,
          elapsedMs: marker.elapsedMs,
          issueRef: marker.issueRef,
          tokenRate: marker.tokenRate,
          updatedAt: marker.updatedAt,
        })),
        tokenRate: fleet.tokenRate,
        processes: fleet.processes.map(process => ({
          pid: process.pid,
          parentPid: process.parentPid,
          elapsed: process.elapsed,
        })),
      }
    },
    async codexHarnessStatus() {
      return codexHarnessStatus()
    },
    async codingStatus() {
      const harness = await codexHarnessStatus()
      if (!harness.available) {
        return runtimeStatus({
          available: false,
          capability: "coding",
          reason: harness.reason,
          status: harness.status,
        })
      }
      return runtimeStatus({
        available: true,
        capability: "coding",
        reason: "Khala Code coding is gated on the local Codex harness.",
        status: "ready",
      })
    },
    async onDeviceDeciderStatus() {
      return input.onDeviceDeciderStatus()
    },
    async connectCodexAccount(accountRef: string) {
      const result = await beginCodexConnect(accountRef, { env: input.env })
      if (result.verificationUrl !== null) openExternalUrl(result.verificationUrl)
      return result
    },
    async openExternalUrl(url: string) {
      return openExternalUrl(url)
    },
    async removeCodexAccount(accountRef: string) {
      return removeCodexAccount(accountRef, { env: input.env })
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
