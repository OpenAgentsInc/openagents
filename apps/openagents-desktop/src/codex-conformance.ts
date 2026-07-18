import { createHash } from "node:crypto"

import { bundledCodex01441ProtocolManifest, currentSourceProtocolManifest, type ProtocolMember } from "@openagentsinc/codex-app-server-protocol/parity"

import type { CodexAppServerSupervisorState } from "./codex-app-server-supervisor.ts"
import type { CodexCompatibilityReceipt } from "./codex-native-event-plane.ts"

export type CodexCapabilityDisposition = Readonly<{
  member: string
  direction: ProtocolMember["direction"]
  generation: ProtocolMember["generation"]
  transport: "generated"
  handler: string
  nativeProjection: string
  productPresentation: string
  authority: string
  fixture: string
  realBinaryProof: string
  policy: "supported" | "compatibility-only" | "test-only" | "requires-binary-upgrade"
}>

const classify = (method: string): Readonly<{ owner: string; surface: string; authority: string; smoke: string }> => {
  if (/^(?:account\/|getAuthStatus|config\/|configRequirements\/|experimentalFeature\/|model\/|modelProvider\/|permissionProfile\/)/u.test(method)) return { owner: "CAP-05", surface: "desktop:settings-and-provider-catalog", authority: "account-and-policy-plane", smoke: "codex-control-plane-smoke" }
  if (/^(?:skills\/|hooks\/|hook\/|marketplace\/|plugin\/|app\/|mcpServer)/u.test(method)) return { owner: "CAP-08", surface: "desktop:extension-and-mcp-catalog", authority: "work-context-extension-authority", smoke: "codex-ecosystem-smoke" }
  if (/^(?:fs\/|command\/|fuzzyFileSearch|externalAgentConfig\/|windows|feedback\/)/u.test(method)) return { owner: "CAP-09", surface: "desktop:bounded-host-services", authority: "work-context-host-authority", smoke: "codex-host-services-smoke" }
  if (/^(?:environment\/|process\/|thread\/backgroundTerminals\/|thread\/realtime\/|remoteControl\/|memory\/reset|thread\/(?:increment|decrement)_elicitation)/u.test(method)) return { owner: "CAP-10", surface: "desktop:experimental-supervision", authority: "confirmed-experimental-authority", smoke: "codex-experimental-runtime-smoke" }
  if (/^(?:applyPatchApproval|execCommandApproval|attestation\/|currentTime\/|item\/(?:commandExecution|fileChange|permissions|tool\/requestUserInput|tool\/call)|mcpServer\/elicitation)/u.test(method)) return { owner: "CAP-04", surface: "desktop:reverse-rpc-attention", authority: "exactly-once-reverse-arbiter", smoke: "codex-supervisor-smoke" }
  if (/^(?:thread\/(?:list|search|read|resume|fork|archive|unarchive|unsubscribe|loaded|closed|started|status|name|metadata|settings|memoryMode|goal|turns|items)|getConversationSummary|gitDiffToRemote)/u.test(method)) return { owner: "CAP-03", surface: "desktop:thread-library-and-history", authority: "thread-lifecycle-authority", smoke: "codex-thread-lifecycle-smoke" }
  if (/^(?:review\/|item\/|rawResponse|serverRequest|thread\/compacted|thread\/tokenUsage|turn\/diff|turn\/plan|turn\/completed|turn\/started|error$|warning$|configWarning|deprecationNotice|guardianWarning)/u.test(method)) return { owner: "CAP-06", surface: "desktop:native-turn-item-review-projection", authority: "turn-and-review-authority", smoke: "codex-turn-control-smoke" }
  if (/^(?:turn\/|thread\/compact|thread\/rollback|thread\/realtime|thread\/start)/u.test(method)) return { owner: "CAP-07", surface: "desktop:composer-steer-queue-and-turn-control", authority: "composer-admission-authority", smoke: "codex-turn-control-smoke" }
  if (method === "initialize" || method === "initialized") return { owner: "CAP-01", surface: "desktop:supervisor-handshake", authority: "supervisor-pool-authority", smoke: "codex-supervisor-smoke" }
  if (method.startsWith("mock/")) return { owner: "protocol", surface: "policy:test-only-unavailable", authority: "none", smoke: "not-applicable" }
  return { owner: "CAP-00", surface: "policy:main-private-native-only", authority: "owner-local-policy", smoke: "installed-codex-protocol-smoke" }
}

const key = (member: ProtocolMember): string => `${member.direction}\0${member.method}`
const bundledKeys = new Set(bundledCodex01441ProtocolManifest.members.map(key))
export const codexDisposition = (member: ProtocolMember): CodexCapabilityDisposition => {
  const family = classify(member.method)
  const policy = member.method.startsWith("mock/") ? "test-only" : member.generation !== "upstream-generated" ? "compatibility-only" : bundledKeys.has(key(member)) ? "supported" : "requires-binary-upgrade"
  return { member: member.method, direction: member.direction, generation: member.generation, transport: "generated", handler: `${family.owner}:${policy}`, nativeProjection: member.direction === "server-notification" ? "bounded-native-envelope" : member.direction === "server-request" ? "exactly-once-reverse-decision" : "typed-request-response", productPresentation: family.surface, authority: family.authority, fixture: member.fixture, realBinaryProof: policy === "requires-binary-upgrade" ? "pending-compatible-binary" : family.smoke, policy }
}

const percentage = (values: ReadonlyArray<boolean>): number => values.length === 0 ? 100 : Number((100 * values.filter(Boolean).length / values.length).toFixed(2))
export const makeCodexConformanceReport = () => {
  const selected = currentSourceProtocolManifest.members.map(codexDisposition)
  const bundled = bundledCodex01441ProtocolManifest.members.map(codexDisposition)
  const coverage = (rows: ReadonlyArray<CodexCapabilityDisposition>) => ({
    transport: percentage(rows.map(value => value.transport === "generated")),
    handlerDisposition: percentage(rows.map(value => value.handler.length > 0)),
    nativeProjection: percentage(rows.map(value => value.nativeProjection.length > 0)),
    productPresentation: percentage(rows.map(value => /^(?:desktop:|policy:)/u.test(value.productPresentation))),
    authority: percentage(rows.map(value => value.authority.length > 0)),
    fixture: percentage(rows.map(value => value.fixture.length > 0)),
    realBinaryFamily: percentage(rows.map(value => value.realBinaryProof !== "pending-compatible-binary")),
  })
  return {
    schema: "openagents.codex_conformance.v1" as const,
    generatedAt: "generated-from-reviewed-manifests",
    selectedTarget: { identity: currentSourceProtocolManifest.identity, counts: currentSourceProtocolManifest.counts, coverage: coverage(selected), members: selected },
    bundledReleaseTarget: { identity: bundledCodex01441ProtocolManifest.identity, counts: bundledCodex01441ProtocolManifest.counts, schemaSha256: bundledCodex01441ProtocolManifest.generatedSchemaSha256, coverage: coverage(bundled), members: bundled },
    crossSurface: { desktop: "typed-projections-and-intents", web: "policy:no-local-app-server-authority", mobile: "policy:no-local-app-server-authority", operator: "diagnostic-counts-and-state-only", composer: "shared-effect-admission-matrix" },
  }
}
export type CodexConformanceReport = ReturnType<typeof makeCodexConformanceReport>

export const evaluateCodexReleaseGate = (input: Readonly<{
  supervisorStates: ReadonlyArray<CodexAppServerSupervisorState>
  compatibilityReceipts: ReadonlyArray<CodexCompatibilityReceipt>
  binaryManifestMatches: boolean
  familySmokesPass: boolean
  reverseRequestsSettled: boolean
  queueJournalHealthy: boolean
  recoveryHealthy: boolean
  overloadObserved: boolean
}>): Readonly<{ releasable: boolean; blockers: ReadonlyArray<string>; diagnosticRef: string }> => {
  const blockers: string[] = []
  if (!input.binaryManifestMatches) blockers.push("binary_manifest_mismatch")
  if (!input.familySmokesPass) blockers.push("family_smoke_failure")
  if (input.compatibilityReceipts.length > 0) blockers.push("protocol_decode_drift")
  if (input.supervisorStates.some(state => state.status !== "ready")) blockers.push("supervisor_not_ready")
  if (!input.reverseRequestsSettled) blockers.push("reverse_request_unsettled")
  if (!input.queueJournalHealthy) blockers.push("queue_journal_corrupt")
  if (!input.recoveryHealthy) blockers.push("recovery_corrupt")
  if (input.overloadObserved) blockers.push("transport_overload")
  return { releasable: blockers.length === 0, blockers, diagnosticRef: `diagnostic.${createHash("sha256").update(JSON.stringify(blockers)).digest("hex").slice(0, 24)}` }
}

export const publicCodexDiagnostics = (input: Readonly<{ states: ReadonlyArray<CodexAppServerSupervisorState>; receipts: ReadonlyArray<CodexCompatibilityReceipt> }>) => ({
  connections: input.states.map(state => ({ status: state.status, generation: state.generation })),
  compatibility: input.receipts.map(receipt => ({ direction: receipt.direction, method: receipt.method, reason: receipt.reason, occurrences: receipt.occurrences })),
})
