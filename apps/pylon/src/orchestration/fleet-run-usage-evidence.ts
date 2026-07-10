import { createHash } from "node:crypto"

import { Schema as S } from "effect"

import type { PylonKhalaCloseoutResult } from "../khala-requester.js"
import { assertPublicProjectionSafe } from "../state.js"

export const PYLON_FLEET_RUN_USAGE_EVIDENCE_SCHEMA =
  "openagents.pylon.fleet_run_usage_evidence.v1" as const

const PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/#=-]{0,255}$/u
const ACCOUNT_REF_HASH_PATTERN = /^account\.pylon\.(?:codex|claude_agent|grok)\.[a-f0-9]{24}$/u
const PYLON_REF_PATTERN = /^[a-z0-9][a-z0-9._:-]{2,119}$/u

const PublicRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(PUBLIC_REF_PATTERN),
)
const AccountRefHash = S.String.check(S.isPattern(ACCOUNT_REF_HASH_PATTERN))
const PylonRef = S.String.check(S.isPattern(PYLON_REF_PATTERN))
const NonNegativeInt = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))
const PositiveInt = S.Number.check(S.isInt(), S.isGreaterThan(0))
const BoundedRefs = S.Array(PublicRef).check(S.isMinLength(1), S.isMaxLength(100))
const NoRefs = S.Array(PublicRef).check(S.isMaxLength(0))

export const PylonFleetRunExactUsageEvidenceSchema = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_USAGE_EVIDENCE_SCHEMA),
  truth: S.Literal("exact"),
  harnessKind: S.Literals(["codex", "claude"]),
  evidenceRef: PublicRef,
  assignmentRef: PublicRef,
  pylonRef: PylonRef,
  provider: S.Literals(["pylon-codex-own-capacity", "pylon-claude-own-capacity"]),
  model: S.Literals(["openagents/pylon-codex", "openagents/pylon-claude"]),
  demandKind: S.Literal("own_capacity"),
  demandSource: S.Literal("khala_coding_delegation"),
  inputTokens: NonNegativeInt,
  outputTokens: NonNegativeInt,
  reasoningTokens: NonNegativeInt,
  cacheReadTokens: NonNegativeInt,
  totalTokens: PositiveInt,
  tokenRows: PositiveInt,
  tokenUsageRefs: BoundedRefs,
  proofRefs: BoundedRefs,
  closeoutChecklistRefs: BoundedRefs,
  proofChecklistRefs: BoundedRefs,
}).pipe(
  S.check(
    S.makeFilter(
      (usage) =>
        usage.totalTokens === usage.inputTokens + usage.outputTokens &&
        usage.reasoningTokens <= usage.outputTokens &&
        usage.cacheReadTokens <= usage.inputTokens &&
        usage.tokenUsageRefs.length >= Math.min(usage.tokenRows, 100) &&
        (usage.harnessKind === "codex"
          ? usage.provider === "pylon-codex-own-capacity" &&
            usage.model === "openagents/pylon-codex"
          : usage.provider === "pylon-claude-own-capacity" &&
            usage.model === "openagents/pylon-claude"),
      { message: "exact FleetRun usage evidence must be internally coherent" },
    ),
  ),
)

export const PylonFleetRunNotMeasuredUsageEvidenceSchema = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_USAGE_EVIDENCE_SCHEMA),
  truth: S.Literal("not_measured"),
  harnessKind: S.Literal("grok"),
  evidenceRef: PublicRef,
  assignmentRef: PublicRef,
  receiptRef: PublicRef,
  tokenUsageRefs: NoRefs,
  caveatRefs: BoundedRefs,
})

export const PylonFleetRunUsageEvidenceSchema = S.Union([
  PylonFleetRunExactUsageEvidenceSchema,
  PylonFleetRunNotMeasuredUsageEvidenceSchema,
])

export type PylonFleetRunExactUsageEvidence =
  typeof PylonFleetRunExactUsageEvidenceSchema.Type
export type PylonFleetRunNotMeasuredUsageEvidence =
  typeof PylonFleetRunNotMeasuredUsageEvidenceSchema.Type
export type PylonFleetRunUsageEvidence = typeof PylonFleetRunUsageEvidenceSchema.Type

export const PylonFleetRunUsageEvidenceCarrierSchema = S.Union([
  S.Struct({
    accountRefHash: S.Null,
    closeoutRef: S.Null,
    usageEvidence: S.Null,
  }),
  S.Struct({
    accountRefHash: AccountRefHash,
    closeoutRef: PublicRef,
    usageEvidence: PylonFleetRunUsageEvidenceSchema,
  }),
])
export type PylonFleetRunUsageEvidenceCarrier = {
  readonly accountRefHash: string | null
  readonly closeoutRef: string | null
  readonly usageEvidence: PylonFleetRunUsageEvidence | null
}

const stableRef = (prefix: string, seed: string): string =>
  `${prefix}.${createHash("sha256").update(seed).digest("hex").slice(0, 24)}`

const decodeEvidence = (value: unknown): PylonFleetRunUsageEvidence => {
  const evidence = S.decodeUnknownSync(PylonFleetRunUsageEvidenceSchema)(value, {
    onExcessProperty: "error",
  })
  assertPublicProjectionSafe(evidence, "pylonFleetRunUsageEvidence")
  return evidence
}

const exactProviderFor = (
  harnessKind: "codex" | "claude",
): {
  readonly model: "openagents/pylon-codex" | "openagents/pylon-claude"
  readonly provider: "pylon-codex-own-capacity" | "pylon-claude-own-capacity"
} => harnessKind === "codex"
  ? {
      model: "openagents/pylon-codex",
      provider: "pylon-codex-own-capacity",
    }
  : {
      model: "openagents/pylon-claude",
      provider: "pylon-claude-own-capacity",
    }

export function exactPylonFleetRunUsageEvidence(input: {
  readonly accountRefHash: string
  readonly assignmentRef: string
  readonly closeout: PylonKhalaCloseoutResult
  readonly harnessKind: "codex" | "claude"
  readonly pylonRef: string
}): PylonFleetRunUsageEvidenceCarrier & {
  readonly accountRefHash: string
  readonly closeoutRef: string
  readonly usageEvidence: PylonFleetRunExactUsageEvidence
} {
  const expectedAccountPrefix = input.harnessKind === "codex"
    ? "account.pylon.codex."
    : "account.pylon.claude_agent."
  if (
    !ACCOUNT_REF_HASH_PATTERN.test(input.accountRefHash) ||
    !input.accountRefHash.startsWith(expectedAccountPrefix)
  ) {
    throw new Error("Pylon FleetRun exact account custody ref is invalid")
  }
  const { closeout } = input
  const expected = exactProviderFor(input.harnessKind)
  const usage = closeout.proof.tokenUsage
  const statusUsage = closeout.status.tokenUsage
  const statusPolicy = closeout.status.closeoutPolicy
  const proofPolicy = closeout.proof.closeoutPolicy
  const closeoutChecklistRefs = closeout.closeoutChecklist.items.map(item => item.ref)
  const proofChecklistRefs = closeout.proof.proofChecklist.items.map(item => item.ref)
  if (
    closeout.assignmentRef !== input.assignmentRef ||
    closeout.status.assignmentRef !== input.assignmentRef ||
    closeout.proof.assignmentRef !== input.assignmentRef ||
    closeout.status.pylonRef !== input.pylonRef ||
    closeout.proof.pylonRef !== input.pylonRef ||
    closeout.status.owner.agentUserRef !== closeout.proof.owner.agentUserRef ||
    closeout.status.owner.openauthUserRef !== closeout.proof.owner.openauthUserRef ||
    !closeout.closeoutChecklist.ok ||
    !closeout.proof.proofChecklist.ok ||
    closeout.closeoutChecklist.items.some(item => !item.ok) ||
    closeout.proof.proofChecklist.items.some(item => !item.ok) ||
    closeout.status.lifecycle.closeoutRefs.length === 0 ||
    closeout.status.lifecycle.proofRefs.length === 0 ||
    closeout.status.progress.state !== "closed_out" ||
    !closeout.status.progress.closeoutReady ||
    !closeout.status.progress.hasTokenUsage ||
    statusPolicy?.source !== "worker_closeout_event" ||
    proofPolicy?.source !== "worker_closeout_event" ||
    statusPolicy.paymentMode !== "no-spend" ||
    proofPolicy.paymentMode !== "no-spend" ||
    statusPolicy.settlementState !== "not_applicable" ||
    proofPolicy.settlementState !== "not_applicable" ||
    statusPolicy.payoutClaimAllowed !== false ||
    proofPolicy.payoutClaimAllowed !== false ||
    usage.provider !== expected.provider ||
    usage.model !== expected.model ||
    usage.usageTruth !== "exact" ||
    usage.demandKind !== "own_capacity" ||
    usage.demandSource !== "khala_coding_delegation" ||
    usage.rowCount <= 0 ||
    usage.totalTokens <= 0 ||
    usage.totalTokens !== usage.inputTokens + usage.outputTokens ||
    usage.reasoningTokens > usage.outputTokens ||
    usage.cacheReadTokens > usage.inputTokens ||
    usage.refs.length < Math.min(usage.rowCount, 100) ||
    statusUsage.status !== "recorded" ||
    statusUsage.provider !== usage.provider ||
    statusUsage.model !== usage.model ||
    statusUsage.usageTruth !== usage.usageTruth ||
    statusUsage.demandKind !== usage.demandKind ||
    statusUsage.demandSource !== usage.demandSource ||
    statusUsage.rowCount !== usage.rowCount ||
    statusUsage.inputTokens !== usage.inputTokens ||
    statusUsage.outputTokens !== usage.outputTokens ||
    statusUsage.reasoningTokens !== usage.reasoningTokens ||
    statusUsage.cacheReadTokens !== usage.cacheReadTokens ||
    statusUsage.totalTokens !== usage.totalTokens ||
    statusUsage.refs.length !== usage.refs.length ||
    statusUsage.refs.some((ref, index) => ref !== usage.refs[index]) ||
    closeoutChecklistRefs.length === 0 ||
    proofChecklistRefs.length === 0
  ) {
    throw new Error("Pylon FleetRun exact usage evidence is incomplete or mismatched")
  }
  const evidence = decodeEvidence({
    schema: PYLON_FLEET_RUN_USAGE_EVIDENCE_SCHEMA,
    truth: "exact",
    harnessKind: input.harnessKind,
    evidenceRef: stableRef(
      "evidence.public.pylon.fleet_run.exact",
      `${input.assignmentRef}:${usage.refs.join(":")}:${usage.totalTokens}`,
    ),
    assignmentRef: input.assignmentRef,
    pylonRef: input.pylonRef,
    provider: usage.provider,
    model: usage.model,
    demandKind: usage.demandKind,
    demandSource: usage.demandSource,
    inputTokens: usage.inputTokens,
    outputTokens: usage.outputTokens,
    reasoningTokens: usage.reasoningTokens,
    cacheReadTokens: usage.cacheReadTokens,
    totalTokens: usage.totalTokens,
    tokenRows: usage.rowCount,
    tokenUsageRefs: usage.refs,
    proofRefs: closeout.status.lifecycle.proofRefs,
    closeoutChecklistRefs,
    proofChecklistRefs,
  })
  if (evidence.truth !== "exact") {
    throw new Error("Pylon FleetRun exact evidence decoded to the wrong variant")
  }
  return {
    accountRefHash: input.accountRefHash,
    closeoutRef: closeout.status.lifecycle.closeoutRefs[0]!,
    usageEvidence: evidence,
  }
}

export function pylonGrokUsageEvidenceRefs(assignmentRef: string): {
  readonly closeoutRef: string
  readonly receiptRef: string
} {
  if (!PUBLIC_REF_PATTERN.test(assignmentRef)) {
    throw new Error("Pylon Grok usage evidence assignment ref is invalid")
  }
  return {
    closeoutRef: stableRef("closeout.public.pylon.grok", assignmentRef),
    receiptRef: stableRef("receipt.public.pylon.grok", assignmentRef),
  }
}

export function notMeasuredPylonFleetRunUsageEvidence(input: {
  readonly accountRefHash: string
  readonly assignmentRef: string
  readonly closeoutRef: string
  readonly receiptRef: string
}): PylonFleetRunUsageEvidenceCarrier & {
  readonly accountRefHash: string
  readonly closeoutRef: string
  readonly usageEvidence: PylonFleetRunNotMeasuredUsageEvidence
} {
  if (!/^account\.pylon\.grok\.[a-f0-9]{24}$/u.test(input.accountRefHash)) {
    throw new Error("Pylon FleetRun Grok account custody ref is invalid")
  }
  if (!/^assignment\.pylon\.grok\.[a-f0-9]{24}$/u.test(input.assignmentRef)) {
    throw new Error("Pylon FleetRun Grok assignment custody ref is invalid")
  }
  const stableEvidenceRefs = pylonGrokUsageEvidenceRefs(input.assignmentRef)
  if (
    input.closeoutRef !== stableEvidenceRefs.closeoutRef ||
    input.receiptRef !== stableEvidenceRefs.receiptRef
  ) {
    throw new Error("Pylon FleetRun Grok evidence refs are not stable")
  }
  const evidence = decodeEvidence({
    schema: PYLON_FLEET_RUN_USAGE_EVIDENCE_SCHEMA,
    truth: "not_measured",
    harnessKind: "grok",
    evidenceRef: stableRef(
      "evidence.public.pylon.fleet_run.not_measured",
      `${input.assignmentRef}:${input.receiptRef}:${input.closeoutRef}`,
    ),
    assignmentRef: input.assignmentRef,
    receiptRef: input.receiptRef,
    tokenUsageRefs: [],
    caveatRefs: ["caveat.pylon.fleet_run.grok_usage_not_measured"],
  })
  if (evidence.truth !== "not_measured") {
    throw new Error("Pylon FleetRun not-measured evidence decoded to the wrong variant")
  }
  return {
    accountRefHash: input.accountRefHash,
    closeoutRef: input.closeoutRef,
    usageEvidence: evidence,
  }
}
