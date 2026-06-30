import { Effect } from "effect"
import type {
  KhalaInteractionChoice,
  KhalaInteractionService,
  KhalaPermissionDecision,
  KhalaPermissionRequest,
  KhalaPermissionService,
  KhalaToolAuthority,
} from "./index.js"

export type KhalaApprovalScope = "session" | "project"

export type KhalaApprovalCacheKey = Readonly<{
  action: KhalaToolAuthority
  projectRef: string
  resourcePattern: string
  scope: KhalaApprovalScope
  sessionId?: string
}>

export interface KhalaApprovalStore {
  readonly allow: (key: KhalaApprovalCacheKey) => Effect.Effect<void, never>
  readonly isAllowed: (key: KhalaApprovalCacheKey) => Effect.Effect<boolean, never>
}

export interface KhalaPermissionPolicyOptions {
  readonly interaction: KhalaInteractionService
  readonly store?: KhalaApprovalStore
}

export function makeInMemoryKhalaApprovalStore(): KhalaApprovalStore {
  const approvals = new Set<string>()
  return {
    allow: key => Effect.sync(() => {
      approvals.add(serializeApprovalKey(key))
    }),
    isAllowed: key => Effect.sync(() => approvals.has(serializeApprovalKey(key))),
  }
}

export function makeKhalaPermissionPolicyService(options: KhalaPermissionPolicyOptions): KhalaPermissionService {
  const store = options.store ?? makeInMemoryKhalaApprovalStore()
  return {
    decide: request =>
      Effect.gen(function* () {
        const cacheKeys = approvalCacheKeysFor(request)
        for (const key of cacheKeys) {
          if (yield* store.isAllowed(key)) return "allow"
        }

        const answer = yield* options.interaction.askUser({
          allowFreeform: false,
          choices: permissionChoices(request),
          invocationId: request.toolCallId,
          khalaSessionId: request.sessionId,
          nonBlocking: false,
          prompt: permissionPrompt(request),
          publicSafe: false,
        }).pipe(
          Effect.catchTag("KhalaToolRuntimeError", () =>
            Effect.succeed({
              events: [],
              reason: "permission_prompt_unavailable",
              requestId: `khala.permission.${request.toolCallId}`,
              status: "unavailable" as const,
            }),
          ),
        )

        if (answer.status !== "answered") return "deny"
        const decision = permissionDecisionFromChoice(answer.answer?.kind === "choice" ? answer.answer.choiceId : undefined)
        if (decision === "always") {
          for (const key of cacheKeys) {
            yield* store.allow(key)
          }
          return "allow"
        }
        return decision
      }),
  }
}

export function approvalCacheKeysFor(request: KhalaPermissionRequest): ReadonlyArray<KhalaApprovalCacheKey> {
  if (request.saveScope === "once") return []
  const scope = request.saveScope
  const projectRef = normalizeProjectRef(request.workingDirectory)
  return resourcePatternsFor(request).map((resourcePattern): KhalaApprovalCacheKey => ({
    action: request.action,
    projectRef,
    resourcePattern,
    scope,
    ...(scope === "session" ? { sessionId: request.sessionId } : {}),
  }))
}

function permissionChoices(request: KhalaPermissionRequest): ReadonlyArray<KhalaInteractionChoice> {
  const choices: KhalaInteractionChoice[] = [
    { description: "Approve this tool call once.", id: "allow", label: "Allow" },
    { description: "Reject this tool call.", id: "deny", label: "Deny" },
  ]
  if (request.saveScope !== "once") {
    choices.push({
      description: `Approve matching ${request.action} requests for this ${request.saveScope}.`,
      id: "always",
      label: "Always Allow",
    })
  }
  return choices
}

function permissionDecisionFromChoice(choiceId: string | undefined): KhalaPermissionDecision {
  if (choiceId === "allow") return "allow"
  if (choiceId === "always") return "always"
  return "deny"
}

function permissionPrompt(request: KhalaPermissionRequest): string {
  const resources = resourcePatternsFor(request).join(", ") || "(no resource)"
  return [
    `Approve ${request.toolName} ${request.action} access?`,
    `Resources: ${resources}`,
    `Scope: ${request.saveScope}`,
  ].join("\n")
}

function resourcePatternsFor(request: KhalaPermissionRequest): ReadonlyArray<string> {
  const resources = request.resources.length > 0 ? request.resources : ["*"]
  return [...new Set(resources.map(normalizeResourcePattern))].sort()
}

function normalizeProjectRef(value: string | undefined): string {
  const trimmed = value?.trim()
  return trimmed === undefined || trimmed.length === 0 ? "(unknown-project)" : trimmed
}

function normalizeResourcePattern(value: string): string {
  const trimmed = value.trim()
  return trimmed.length === 0 ? "(empty-resource)" : trimmed
}

function serializeApprovalKey(key: KhalaApprovalCacheKey): string {
  return JSON.stringify({
    action: key.action,
    projectRef: key.projectRef,
    resourcePattern: key.resourcePattern,
    scope: key.scope,
    sessionId: key.sessionId ?? null,
  })
}
