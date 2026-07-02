import { existsSync } from "node:fs"
import { readFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import { khalaCodeConfigFromRuntimeEnv } from "./khala-code-config.js"
import { collectKhalaProcessText, spawnKhalaProcess } from "./khala-process.js"

export const KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT =
  "db887d03e1f907467e33271572dffb73bceecd6b"

export const KHALA_CODE_CODEX_PARITY_REFERENCE_LABEL =
  "openai/codex app-server v2 schema at db887d03e1"

export const KHALA_CODE_CODEX_REFERENCE_CHECKOUT_MISSING_BLOCKER_REF =
  "blocker.codex_reference_checkout_missing"

const KHALA_CODE_CODEX_REFERENCE_SCHEMA_DIR =
  "codex-rs/app-server-protocol/schema/typescript"

type KhalaCodeCodexReferenceRootInspectionEnv = Readonly<Record<string, string | undefined>>

export type KhalaCodeCodexReferenceRootStatus =
  | {
      readonly ok: true
      readonly root: string
      readonly status: "ready"
    }
  | {
      readonly blockerRef: typeof KHALA_CODE_CODEX_REFERENCE_CHECKOUT_MISSING_BLOCKER_REF
      readonly ok: false
      readonly reason: string
      readonly status: "blocked"
    }

export type KhalaCodeCodexParityHarness =
  | "codex_wrapper_fixture"
  | "codex_wrapper_live"
  | "legacy_fallback_guard"

export type KhalaCodeCodexParityCoverageRow = Readonly<{
  id: string
  harness: KhalaCodeCodexParityHarness
  testFile: string
  covers: readonly string[]
}>

export const KHALA_CODE_CODEX_PARITY_COVERAGE: readonly KhalaCodeCodexParityCoverageRow[] = [
  {
    id: "app-server-schema-and-methods",
    harness: "codex_wrapper_fixture",
    testFile: "clients/khala-code-desktop/tests/codex-parity-contract.test.ts",
    covers: [
      "codex app-server generated TypeScript schema presence",
      "client request method drift",
      "server request and notification drift",
      "standalone command/process API presence",
    ],
  },
  {
    id: "app-server-effect-service",
    harness: "codex_wrapper_fixture",
    testFile: "clients/khala-code-desktop/tests/codex-app-server-service.test.ts",
    covers: [
      "CodexAppServer Context.Service wrapper",
      "Schema-decoded response and notification boundaries",
      "notification Stream subscriber isolation",
      "timeout interrupt policy and scoped disposal",
    ],
  },
  {
    id: "slash-commands",
    harness: "codex_wrapper_fixture",
    testFile: "clients/khala-code-desktop/tests/codex-slash-commands.test.ts",
    covers: [
      "upstream SlashCommand enum and aliases",
      "visibility gates",
      "availability during active turns and side conversations",
      "dispatch status for every command",
    ],
  },
  {
    id: "app-server-gap-matrix",
    harness: "codex_wrapper_fixture",
    testFile: "clients/khala-code-desktop/tests/codex-app-server-gap-matrix.test.ts",
    covers: [
      "TUI-local app-server gap decisions",
      "slash-command-to-gap rollups",
      "upstream gap IDs and Khala adapter rationales",
    ],
  },
  {
    id: "thread-turn-runtime",
    harness: "codex_wrapper_fixture",
    testFile: "clients/khala-code-desktop/tests/codex-app-server-chat-runtime.test.ts",
    covers: [
      "thread start/resume/list/read/rename/fork/archive/unarchive/delete",
      "turn start/interrupt",
      "Codex notification projection into desktop transcript events",
    ],
  },
  {
    id: "thread-items",
    harness: "codex_wrapper_fixture",
    testFile: "clients/khala-code-desktop/tests/codex-thread-item-projector.test.ts",
    covers: [
      "ThreadItem variants",
      "agent, command, and patch delta families",
      "approval request rendering and resolution",
      "unknown item visibility",
    ],
  },
  {
    id: "approval-responses",
    harness: "codex_wrapper_fixture",
    testFile: "clients/khala-code-desktop/tests/codex-approval-decisions.test.ts",
    covers: [
      "command approval response bodies",
      "file-change approval response bodies",
      "permission grant, session grant, strict-review, decline, and cancel bodies",
    ],
  },
  {
    id: "settings-ecosystem-rate-limits",
    harness: "codex_wrapper_fixture",
    testFile: "clients/khala-code-desktop/tests/rpc-handlers.test.ts",
    covers: [
      "model list",
      "permission profile list",
      "config read/write",
      "feature flags",
      "usage/status",
      "MCP, plugins, skills, apps, and hooks app-server pass-through",
    ],
  },
  {
    id: "fixture-app-server-process",
    harness: "codex_wrapper_fixture",
    testFile: "clients/khala-code-desktop/tests/fixture-codex-app-server.test.ts",
    covers: [
      "spawned fixture Codex app-server stdio process",
      "recorded notification script replay",
      "server-to-client approval request and resolution",
      "background terminal command output",
    ],
  },
  {
    id: "headless-jsonl",
    harness: "codex_wrapper_fixture",
    testFile: "clients/khala-code-desktop/tests/headless.test.ts",
    covers: [
      "headless Codex-backed thread start",
      "JSONL item streaming",
      "structured missing-Codex errors",
      "interrupt smoke hook",
    ],
  },
  {
    id: "legacy-runtime-demotion",
    harness: "legacy_fallback_guard",
    testFile: "clients/khala-code-desktop/tests/khala-chat-runtime.test.ts",
    covers: [
      "legacy Khala-native tools stay behind explicit fallback flags",
      "default catalog remains Codex-wrapper supplemental only",
    ],
  },
  {
    id: "live-app-server-smoke",
    harness: "codex_wrapper_live",
    testFile: "clients/khala-code-desktop/tests/codex-parity-live-smoke.test.ts",
    covers: [
      "skip-safe live smoke guard",
      "explicit live failure when Codex is unavailable",
      "Codex app-server thread start/resume/turn/interrupt lifecycle",
    ],
  },
]

export const KHALA_CODE_CODEX_PARITY_REQUIRED_SCHEMA_FILES = [
  "ClientRequest.ts",
  "ClientNotification.ts",
  "ServerRequest.ts",
  "ServerNotification.ts",
  "v2/ThreadItem.ts",
  "v2/ThreadStartParams.ts",
  "v2/ThreadResumeParams.ts",
  "v2/ThreadForkParams.ts",
  "v2/ThreadArchiveParams.ts",
  "v2/ThreadDeleteParams.ts",
  "v2/ThreadUnarchiveParams.ts",
  "v2/ThreadReadParams.ts",
  "v2/ThreadListParams.ts",
  "v2/TurnStartParams.ts",
  "v2/TurnInterruptParams.ts",
  "v2/CommandExecParams.ts",
  "v2/CommandExecWriteParams.ts",
  "v2/CommandExecResizeParams.ts",
  "v2/CommandExecTerminateParams.ts",
  "v2/CommandExecOutputDeltaNotification.ts",
  "v2/CommandExecutionRequestApprovalResponse.ts",
  "v2/FileChangeRequestApprovalResponse.ts",
  "v2/PermissionsRequestApprovalResponse.ts",
  "v2/ModelListResponse.ts",
  "v2/PermissionProfileListResponse.ts",
  "v2/ConfigReadResponse.ts",
  "v2/ConfigWriteResponse.ts",
  "v2/ExperimentalFeatureListResponse.ts",
  "v2/GetAccountTokenUsageResponse.ts",
] as const

export const KHALA_CODE_CODEX_PARITY_REQUIRED_CLIENT_METHODS = [
  "thread/start",
  "thread/resume",
  "thread/fork",
  "thread/archive",
  "thread/delete",
  "thread/unarchive",
  "thread/name/set",
  "thread/goal/set",
  "thread/goal/get",
  "thread/goal/clear",
  "thread/metadata/update",
  "thread/compact/start",
  "thread/approveGuardianDeniedAction",
  "thread/list",
  "thread/read",
  "thread/inject_items",
  "turn/start",
  "turn/steer",
  "turn/interrupt",
  "review/start",
  "model/list",
  "modelProvider/capabilities/read",
  "permissionProfile/list",
  "experimentalFeature/list",
  "experimentalFeature/enablement/set",
  "config/read",
  "config/value/write",
  "config/batchWrite",
  "configRequirements/read",
  "account/usage/read",
  "account/rateLimits/read",
  "account/rateLimitResetCredit/consume",
  "account/read",
  "command/exec",
  "command/exec/write",
  "command/exec/terminate",
  "command/exec/resize",
  "skills/list",
  "skills/config/write",
  "skills/extraRoots/set",
  "hooks/list",
  "plugin/list",
  "plugin/read",
  "plugin/installed",
  "app/list",
  "mcpServerStatus/list",
  "mcpServer/resource/read",
  "mcpServer/tool/call",
  "mcpServer/oauth/login",
  "config/mcpServer/reload",
  "windowsSandbox/setupStart",
  "windowsSandbox/readiness",
  "externalAgentConfig/detect",
  "externalAgentConfig/import",
  "externalAgentConfig/import/readHistories",
  "fs/readFile",
  "fs/writeFile",
  "fs/getMetadata",
  "fs/readDirectory",
  "feedback/upload",
  "gitDiffToRemote",
  "fuzzyFileSearch",
  "getAuthStatus",
  "getConversationSummary",
  "account/logout",
] as const

export const KHALA_CODE_CODEX_PARITY_REQUIRED_SERVER_REQUESTS = [
  "item/commandExecution/requestApproval",
  "item/fileChange/requestApproval",
  "item/permissions/requestApproval",
  "item/tool/requestUserInput",
  "item/tool/call",
  "mcpServer/elicitation/request",
] as const

export const KHALA_CODE_CODEX_PARITY_REQUIRED_NOTIFICATIONS = [
  "thread/started",
  "thread/status/changed",
  "thread/archived",
  "thread/deleted",
  "thread/unarchived",
  "thread/name/updated",
  "thread/tokenUsage/updated",
  "turn/started",
  "turn/completed",
  "turn/diff/updated",
  "turn/plan/updated",
  "item/started",
  "item/completed",
  "item/agentMessage/delta",
  "item/plan/delta",
  "item/commandExecution/outputDelta",
  "item/commandExecution/terminalInteraction",
  "item/fileChange/patchUpdated",
  "command/exec/outputDelta",
  "process/outputDelta",
  "process/exited",
  "serverRequest/resolved",
  "item/mcpToolCall/progress",
  "skills/changed",
  "mcpServer/oauthLogin/completed",
  "mcpServer/startupStatus/updated",
  "app/list/updated",
] as const

export const KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES = [
  "userMessage",
  "hookPrompt",
  "agentMessage",
  "plan",
  "reasoning",
  "commandExecution",
  "fileChange",
  "mcpToolCall",
  "dynamicToolCall",
  "collabAgentToolCall",
  "subAgentActivity",
  "webSearch",
  "imageView",
  "sleep",
  "imageGeneration",
  "enteredReviewMode",
  "exitedReviewMode",
  "contextCompaction",
] as const

const referenceSchemaRoot = (root: string): string =>
  join(root, KHALA_CODE_CODEX_REFERENCE_SCHEMA_DIR)

const codexReferenceCheckoutMissing = (reason: string): KhalaCodeCodexReferenceRootStatus => ({
  blockerRef: KHALA_CODE_CODEX_REFERENCE_CHECKOUT_MISSING_BLOCKER_REF,
  ok: false,
  reason,
  status: "blocked",
})

export function inspectCodexReferenceRoot(
  startDir = dirname(fileURLToPath(import.meta.url)),
  env: KhalaCodeCodexReferenceRootInspectionEnv = khalaCodeConfigFromRuntimeEnv().env,
): KhalaCodeCodexReferenceRootStatus {
  const explicit = env.KHALA_CODE_CODEX_REFERENCE_ROOT?.trim()
  if (explicit !== undefined && explicit.length > 0) {
    if (existsSync(referenceSchemaRoot(explicit))) {
      return {
        ok: true,
        root: explicit,
        status: "ready",
      }
    }
    return codexReferenceCheckoutMissing(
      `KHALA_CODE_CODEX_REFERENCE_ROOT does not contain ${KHALA_CODE_CODEX_REFERENCE_SCHEMA_DIR}: ${explicit}`,
    )
  }

  let current = startDir
  for (let depth = 0; depth < 10; depth += 1) {
    const candidate = join(current, "projects/repos/codex")
    if (existsSync(referenceSchemaRoot(candidate))) {
      return {
        ok: true,
        root: candidate,
        status: "ready",
      }
    }
    current = dirname(current)
  }
  return codexReferenceCheckoutMissing(
    `Could not locate projects/repos/codex reference checkout with ${KHALA_CODE_CODEX_REFERENCE_SCHEMA_DIR} from ${startDir}`,
  )
}

export function findCodexReferenceRoot(startDir = dirname(fileURLToPath(import.meta.url))): string {
  const status = inspectCodexReferenceRoot(startDir)
  if (status.ok) return status.root
  throw new Error(status.reason)
}

export async function readCodexReferenceCommit(root = findCodexReferenceRoot()): Promise<string> {
  const result = await collectKhalaProcessText(
    spawnKhalaProcess("git", ["-C", root, "rev-parse", "HEAD"]),
  )
  if (result.exitCode !== 0) {
    throw new Error(`git rev-parse failed for Codex reference: ${result.stderr.trim() || `exit ${result.exitCode}`}`)
  }
  return result.stdout.trim()
}

export function codexSchemaPath(root: string, relative: string): string {
  return join(root, "codex-rs/app-server-protocol/schema/typescript", relative)
}

export async function readCodexSchemaFile(root: string, relative: string): Promise<string> {
  return await readFile(codexSchemaPath(root, relative), "utf8")
}

export function extractAppServerMethodsFromGeneratedType(source: string): readonly string[] {
  return [...source.matchAll(/"method":\s*"([^"]+)"/g)].map(match => match[1]!)
}

export function extractThreadItemTypesFromGeneratedType(source: string): readonly string[] {
  return [...source.matchAll(/\{\s*"type":\s*"([^"]+)"/g)].map(match => match[1]!)
}
