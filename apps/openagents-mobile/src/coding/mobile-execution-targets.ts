import type {
  RuntimeCommandTarget,
} from "@openagentsinc/khala-sync-client"
import { Schema } from "effect"

const AccountUnavailableReason = Schema.Literals([
  "account_exhausted",
  "account_rate_limited",
  "account_requires_reauth",
  "account_unavailable",
])

const AccountSummary = Schema.Struct({
  accountRefHash: Schema.String,
  label: Schema.String,
  ready: Schema.Boolean,
  reason: Schema.optional(AccountUnavailableReason),
})

const AutoFallbackEvent = Schema.Struct({
  type: AccountUnavailableReason,
  targetId: Schema.String,
  nextTargetId: Schema.NullOr(Schema.String),
})

const AutoResolution = Schema.Struct({
  effectiveTargetId: Schema.NullOr(Schema.String),
  usedFallback: Schema.Boolean,
  events: Schema.Array(AutoFallbackEvent),
})

/** The exact authenticated response currently served by the mobile API. */
export const MobileExecutionTargetCatalogSchema = Schema.Struct({
  availableModelIds: Schema.Array(Schema.String),
  availableTargetIds: Schema.Array(Schema.String),
  autoResolution: Schema.NullOr(AutoResolution),
  claudeAccounts: Schema.Array(AccountSummary),
  codexAccounts: Schema.Array(AccountSummary),
  effectiveModelId: Schema.NullOr(Schema.String),
  effectiveTargetId: Schema.NullOr(Schema.String),
  fallback: Schema.Literals([
    "none",
    "no_preference_set",
    "preference_unavailable",
    "default_unavailable",
  ]),
  preferredModelId: Schema.NullOr(Schema.String),
  preferredTargetId: Schema.NullOr(Schema.String),
  updatedAt: Schema.NullOr(Schema.String),
  usedPreference: Schema.Boolean,
})

type MobileExecutionTargetCatalogWire =
  typeof MobileExecutionTargetCatalogSchema.Type

export type MobileExecutionTargetReadiness =
  | "ready"
  | "unavailable"
  | "revoked"
  | "offline"

export type MobileExecutionTargetReason =
  | typeof AccountUnavailableReason.Type
  | "auto_unresolved"
  | "target_not_advertised"

export type MobileExecutionTargetOption = Readonly<{
  targetId: string
  label: string
  accessibilityLabel: string
  providerLabel: "OpenAgents" | "Codex" | "Claude"
  providerRef: string
  modelRef: string
  accountRef?: string
  runtimeTarget: RuntimeCommandTarget
  readiness: MobileExecutionTargetReadiness
  reasonRef?: `reason.${MobileExecutionTargetReason}`
}>

export type MobileExecutionTargetCatalog = Readonly<{
  options: ReadonlyArray<MobileExecutionTargetOption>
  /** Always concrete. The literal `auto` is never exposed here. */
  effectiveTargetId?: string
  preferredTargetId?: string
  notice?: Readonly<{
    kind: "auto_resolved" | "fallback"
    reasonRef: string
  }>
}>

export type MobileExecutionTargetResolution =
  | Readonly<{ state: "ready"; option: MobileExecutionTargetOption }>
  | Readonly<{
      state: "refused"
      reason: MobileExecutionTargetReason
      option?: MobileExecutionTargetOption
    }>

export class MobileExecutionTargetCatalogError extends Error {
  readonly code:
    | "catalog_invalid"
    | "request_failed"
    | "response_invalid"

  constructor(
    code: MobileExecutionTargetCatalogError["code"],
    message: string,
  ) {
    super(message)
    this.name = "MobileExecutionTargetCatalogError"
    this.code = code
  }
}

export type MobileExecutionTargetFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>

const decodeWire = Schema.decodeUnknownSync(
  MobileExecutionTargetCatalogSchema,
  { onExcessProperty: "error" },
)

const accountTargetPattern = /^(codex|claude):[A-Za-z0-9_.:-]{3,128}$/u
const boundedTargetPattern = /^[A-Za-z0-9][A-Za-z0-9._:/-]{0,159}$/u

const assertBoundedTargetId = (targetId: string): string => {
  const value = targetId.trim()
  if (
    !boundedTargetPattern.test(value)
  ) {
    throw new MobileExecutionTargetCatalogError(
      "catalog_invalid",
      "Execution-target catalog contains an unsupported target id.",
    )
  }
  return value
}

const reasonReadiness = (
  ready: boolean,
  reason: typeof AccountUnavailableReason.Type | undefined,
): MobileExecutionTargetReadiness => {
  if (ready) return "ready"
  if (reason === "account_requires_reauth") return "revoked"
  if (reason === "account_unavailable") return "offline"
  return "unavailable"
}

const reasonRef = (
  ready: boolean,
  reason: typeof AccountUnavailableReason.Type | undefined,
): `reason.${MobileExecutionTargetReason}` | undefined =>
  ready ? undefined : `reason.${reason ?? "account_unavailable"}`

const accountOption = (
  provider: "claude" | "codex",
  account: typeof AccountSummary.Type,
): MobileExecutionTargetOption => {
  const targetId = assertBoundedTargetId(
    `${provider}:${account.accountRefHash}`,
  )
  if (!accountTargetPattern.test(targetId)) {
    throw new MobileExecutionTargetCatalogError(
      "catalog_invalid",
      "Execution-target catalog contains an invalid account ref.",
    )
  }
  const readiness = reasonReadiness(account.ready, account.reason)
  const providerLabel = provider === "codex" ? "Codex" : "Claude"
  return {
    targetId,
    label: account.label,
    accessibilityLabel: `${account.label}, ${providerLabel}, ${readiness}`,
    providerLabel,
    providerRef: provider === "codex"
      ? "provider.openai.codex"
      : "provider.anthropic.claude",
    modelRef: provider === "codex"
      ? "model.gpt-5.6-sol"
      : "model.claude-fable-5",
    accountRef: account.accountRefHash,
    runtimeTarget: {
      lane: provider === "codex" ? "codex_app_server" : "claude_pylon",
      executionTargetId: targetId,
    },
    readiness,
    ...reasonRef(account.ready, account.reason) === undefined
      ? {}
      : { reasonRef: reasonRef(account.ready, account.reason) },
  }
}

const hostedOption = (targetId: "gemini" | "khala"): MobileExecutionTargetOption => {
  const modelLabel = targetId === "khala" ? "Khala" : "Gemini"
  return {
    targetId,
    label: modelLabel,
    accessibilityLabel: `${modelLabel}, OpenAgents, ready`,
    providerLabel: "OpenAgents",
    providerRef: "provider.openagents.hosted",
    modelRef: "model.gemini-3.5-flash",
    runtimeTarget: { lane: "hosted_khala", executionTargetId: targetId },
    readiness: "ready",
  }
}

const concreteTargetId = (
  wire: MobileExecutionTargetCatalogWire,
  targetId: string | null,
): string | undefined => {
  if (targetId === null) return undefined
  const bounded = assertBoundedTargetId(targetId)
  if (bounded !== "auto") return bounded
  const resolved = wire.autoResolution?.effectiveTargetId
  if (resolved === null || resolved === undefined || resolved === "auto") {
    return undefined
  }
  return assertBoundedTargetId(resolved)
}

const catalogFromWire = (
  wire: MobileExecutionTargetCatalogWire,
): MobileExecutionTargetCatalog => {
  const advertised = new Set(wire.availableTargetIds.map(assertBoundedTargetId))
  for (const modelId of wire.availableModelIds) {
    if (modelId.trim().length === 0 || modelId.length > 160) {
      throw new MobileExecutionTargetCatalogError(
        "catalog_invalid",
        "Execution-target catalog contains an invalid model id.",
      )
    }
  }

  const options: MobileExecutionTargetOption[] = []
  if (advertised.has("khala")) options.push(hostedOption("khala"))
  if (advertised.has("gemini")) options.push(hostedOption("gemini"))

  for (const account of wire.codexAccounts) {
    const option = accountOption("codex", account)
    if (!advertised.has(option.targetId)) {
      throw new MobileExecutionTargetCatalogError(
        "catalog_invalid",
        "Codex account is not present in the advertised target set.",
      )
    }
    options.push(option)
  }
  for (const account of wire.claudeAccounts) {
    const option = accountOption("claude", account)
    if (!advertised.has(option.targetId)) {
      throw new MobileExecutionTargetCatalogError(
        "catalog_invalid",
        "Claude account is not present in the advertised target set.",
      )
    }
    options.push(option)
  }

  const effectiveTargetId = concreteTargetId(wire, wire.effectiveTargetId)
  const preferredTargetId = concreteTargetId(wire, wire.preferredTargetId)
  const knownIds = new Set(options.map(option => option.targetId))
  if (effectiveTargetId !== undefined && !knownIds.has(effectiveTargetId)) {
    throw new MobileExecutionTargetCatalogError(
      "catalog_invalid",
      "The effective execution target is not selectable.",
    )
  }

  return {
    options,
    ...(effectiveTargetId === undefined ? {} : { effectiveTargetId }),
    ...(preferredTargetId === undefined || !knownIds.has(preferredTargetId)
      ? {}
      : { preferredTargetId }),
    ...(wire.effectiveTargetId === "auto"
      ? {
          notice: effectiveTargetId === undefined
            ? { kind: "fallback" as const, reasonRef: "reason.auto_unresolved" }
            : { kind: "auto_resolved" as const, reasonRef: `target.${effectiveTargetId}` },
        }
      : wire.fallback === "none"
        ? {}
        : {
            notice: {
              kind: "fallback" as const,
              reasonRef: `reason.${wire.fallback}`,
            },
          }),
  }
}

export const decodeMobileExecutionTargetCatalog = (
  input: unknown,
): MobileExecutionTargetCatalog => {
  try {
    return catalogFromWire(decodeWire(input))
  } catch (error) {
    if (error instanceof MobileExecutionTargetCatalogError) throw error
    throw new MobileExecutionTargetCatalogError(
      "catalog_invalid",
      "Execution-target catalog failed strict validation.",
    )
  }
}

export const resolveMobileExecutionTargetOption = (
  catalog: MobileExecutionTargetCatalog,
  targetId: string,
): MobileExecutionTargetResolution => {
  const concrete = targetId === "auto" ? catalog.effectiveTargetId : targetId
  if (concrete === undefined || concrete === "auto") {
    return { state: "refused", reason: "auto_unresolved" }
  }
  const option = catalog.options.find(candidate => candidate.targetId === concrete)
  if (option === undefined) {
    return { state: "refused", reason: "target_not_advertised" }
  }
  if (option.readiness !== "ready") {
    const reason = option.reasonRef?.slice("reason.".length) as
      | MobileExecutionTargetReason
      | undefined
    return {
      state: "refused",
      reason: reason ?? "account_unavailable",
      option,
    }
  }
  return { state: "ready", option }
}

export const fetchMobileExecutionTargetCatalog = async (input: Readonly<{
  baseUrl: string
  token: string
  fetch?: MobileExecutionTargetFetch
}>): Promise<MobileExecutionTargetCatalog> => {
  const fetchImpl = input.fetch ?? globalThis.fetch
  let response: Response
  try {
    response = await fetchImpl(
      new URL("/api/mobile/model-preference", input.baseUrl),
      {
        method: "GET",
        headers: {
          accept: "application/json",
          authorization: `Bearer ${input.token}`,
        },
      },
    )
  } catch {
    throw new MobileExecutionTargetCatalogError(
      "request_failed",
      "Execution-target catalog request failed.",
    )
  }

  if (!response.ok) {
    throw new MobileExecutionTargetCatalogError(
      "request_failed",
      `Execution-target catalog request returned HTTP ${response.status}.`,
    )
  }

  let body: unknown
  try {
    body = await response.json()
  } catch {
    throw new MobileExecutionTargetCatalogError(
      "response_invalid",
      "Execution-target catalog response was not JSON.",
    )
  }
  return decodeMobileExecutionTargetCatalog(body)
}
