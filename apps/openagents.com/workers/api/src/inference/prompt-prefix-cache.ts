// Khala prefix caching as a product feature (book P0-2 / issue #6084).
//
// THE BOOK'S RULE, MADE OPERATIONAL
// ---------------------------------
// Inference Engineering §5.3.1: "Prefix caching works from the start of the
// input sequence until the FIRST non-repeated token ... ensure that novel
// tokens are as late in your context as possible." A single novel token at the
// front voids the whole shared prefix (the model is autoregressive — one
// different token changes the internal representation of everything after it).
//
// Khala coding traffic (khala-code / Autopilot) repeats long STABLE content on
// every call — the identity prompt, the acceptance contract, tool schemas, and
// stable policy blocks — ahead of a small VOLATILE user turn. That is exactly
// the shape prefix caching rewards. This module turns "prompt order is an
// accident" into "prompt order is a deliberate, tested layout" so the shared
// prefix stays cacheable and Fireworks' on-by-default prompt cache (cached input
// billed ~50%) actually hits.
//
// WHAT THIS MODULE OWNS (the book's P0-2 list, items 1-4 + 6 of #6084):
//   1. STABLE LAYOUT. `assembleStablePromptLayout` partitions the outgoing
//      messages into a STABLE prefix (system/identity, tool schemas, acceptance
//      contract, stable policy) followed by VOLATILE content (everything
//      user-specific) — stable first, novel last.
//   2. DETERMINISTIC ORDERING + HASHING. Tool schemas and the acceptance
//      contract are serialized canonically (sorted keys, stable order) so the
//      same logical inputs always produce byte-identical prefix text and the
//      same `stablePrefixHash`.
//   3. CACHE-AFFINITY KEYS. `deriveCacheAffinityKey` composes account / session
//      / codebase into ONE raw key; the gateway records it ONLY as the one-way
//      `hashCacheAffinityKey` digest from `khala-telemetry.ts` (never the raw
//      key). The same module derives the provider session-affinity VALUE.
//   4. PROVIDER SESSION AFFINITY. `sessionAffinityParams` produces the
//      passthrough params (`x-session-affinity` for Fireworks, `user` for the
//      OpenAI-style lanes) the adapters already read, derived from the affinity
//      key so context-heavy coding sessions pin to one cache-warm replica.
//   6. CACHE-AWARE ROUTING is `cache-aware-routing.ts` (a sibling module) which
//      consumes the affinity key produced here.
//
// PURE + framework-agnostic: no Worker, no D1, no Effect runtime, no I/O. The
// route calls these pure functions; everything stays deterministic and testable.
//
// PRIVACY (INVARIANTS: public-safe projection, one-way hashing): the raw
// affinity key (account/session/codebase) NEVER leaves the gateway as anything
// but the FNV-1a digest. The session-affinity VALUE sent to the provider is
// itself a hash of the raw key, so even the upstream provider never sees the raw
// account/session/codebase identifiers — only an opaque correlation token.

import { hashCacheAffinityKey } from './khala-telemetry'
import { type InferenceMessage } from './provider-adapter'

// ---------------------------------------------------------------------------
// Message shape. Type-only import keeps this module runtime-pure while ensuring
// cache-prefix assembly preserves OpenAI tool metadata carried on inference
// messages (assistant `tool_calls`, tool `tool_call_id`, optional names).
// ---------------------------------------------------------------------------

export type PromptMessage = InferenceMessage

// ---------------------------------------------------------------------------
// 1 + 2. Stable prompt layout (stable content first, novel content last).
// ---------------------------------------------------------------------------

// The role a message plays in the cache-stable layout. The STABLE region is the
// shared prefix the cache reuses across every turn of a session/codebase; the
// VOLATILE region is the per-request novel content that voids the prefix from
// its first token onward — so it MUST come last.
export type PromptStability = 'stable' | 'volatile'

// A message tagged with its stability classification + a deterministic ordinal
// within the stable region. The ordinal pins the stable ordering
// (acceptance-contract → identity → tool-schemas → stable-policy) so two
// requests with the same stable content produce byte-identical prefixes
// regardless of the order the route appended them.
export type ClassifiedPromptMessage = Readonly<{
  message: PromptMessage
  stability: PromptStability
  // Deterministic sort key WITHIN the stable region (lower = earlier). Volatile
  // messages keep their original relative order and always sort after stable.
  stableOrdinal: number
}>

// The canonical ordinal for each kind of stable content. The book's rule is
// "novel tokens as late as possible", so the MOST stable, most-shared content
// gets the lowest ordinal (earliest in the prefix). The acceptance contract and
// identity prompt are identical across every coding session, so they lead; tool
// schemas and stable policy follow; volatile user content is appended last by
// the assembler (never assigned an ordinal here).
export const STABLE_ORDINAL = {
  // The executable acceptance contract — identical for every artifact of a lane
  // (the runner's `window` hooks). The single most-shared block → leads.
  acceptanceContract: 0,
  // The Khala identity system prompt — identical for every khala-* request.
  identity: 1,
  // Tool / function schemas — stable for a given tool set; deterministically
  // serialized so the SAME tool set always yields the SAME bytes.
  toolSchemas: 2,
  // Stable policy blocks (refusal posture, receipt-disclosure rules, ...).
  stablePolicy: 3,
  // Any other system message we cannot positively classify as one of the above.
  // Treated as stable (system content is shared), but ordered after the known
  // blocks so a stray system steer never splits the known stable prefix.
  otherSystem: 4,
} as const

// Markers the gateway stamps onto the system messages it injects, so the
// classifier can recognize them WITHOUT brittle content sniffing. The route
// already injects identity + acceptance-contract system messages; tagging them
// with a stable role keeps classification deterministic and explicit rather than
// a keyword match on prompt text (honors the workspace semantic-routing rule:
// this is structural classification of OUR OWN injected blocks, not user intent).
export type StableBlockKind = keyof typeof STABLE_ORDINAL

// A message the gateway is about to assemble, optionally tagged with the stable
// block it represents. An untagged message is classified by role: `system` →
// `otherSystem` (stable), everything else → volatile.
export type TaggedPromptMessage = Readonly<{
  message: PromptMessage
  // When the gateway KNOWS this is one of its injected stable blocks, it tags it
  // so the assembler orders it canonically. Absent → classified by role.
  stableKind?: StableBlockKind | undefined
}>

// Classify one tagged message into the layout. Tagged stable blocks get their
// canonical ordinal; an untagged `system` message is stable (`otherSystem`);
// any other untagged message is volatile (user-specific, novel content).
const classifyTagged = (
  tagged: TaggedPromptMessage,
  volatileOrder: number,
): ClassifiedPromptMessage => {
  if (tagged.stableKind !== undefined) {
    return {
      message: tagged.message,
      stability: 'stable',
      stableOrdinal: STABLE_ORDINAL[tagged.stableKind],
    }
  }
  if (tagged.message.role === 'system') {
    return {
      message: tagged.message,
      stability: 'stable',
      stableOrdinal: STABLE_ORDINAL.otherSystem,
    }
  }
  return {
    message: tagged.message,
    stability: 'volatile',
    // Volatile messages preserve their original relative order; the +5 offset
    // keeps them strictly after every stable ordinal (max stable ordinal is 4).
    stableOrdinal: 1_000 + volatileOrder,
  }
}

// The assembled, cache-optimal layout: the ordered messages (stable prefix then
// volatile suffix) plus the classification (for telemetry/tests) and the stable
// prefix hash (the deterministic cache key over the shared prefix text).
export type StablePromptLayout = Readonly<{
  // The final ordered messages to send to the provider: stable prefix first,
  // novel/volatile content last (the book's rule).
  messages: ReadonlyArray<PromptMessage>
  // The per-message classification, in final order (stable then volatile).
  classified: ReadonlyArray<ClassifiedPromptMessage>
  // The canonical serialization of the STABLE prefix only (the shared,
  // cacheable bytes). Two requests with identical stable content produce an
  // identical `stablePrefixText`.
  stablePrefixText: string
  // One-way digest of `stablePrefixText` — the deterministic cache key over the
  // shared prefix. Same stable inputs → same hash; any volatile change leaves it
  // UNCHANGED (volatile content never pollutes the prefix or its hash).
  stablePrefixHash: string
}>

// STABLE-region join separator. A literal record separator so the canonical
// prefix text is unambiguous (a message body cannot forge a boundary).
const PREFIX_SEPARATOR = ''

// Canonically serialize the stable prefix messages into deterministic text. The
// messages are already sorted by `stableOrdinal`; each is rendered `role:content`
// and joined by the record separator. This is the byte-stable shared-prefix
// representation the hash is taken over.
const serializeStablePrefix = (
  stable: ReadonlyArray<ClassifiedPromptMessage>,
): string =>
  stable
    .map(entry => `${entry.message.role}:${entry.message.content}`)
    .join(PREFIX_SEPARATOR)

// Assemble the cache-optimal prompt layout from the gateway's tagged messages.
//
// DETERMINISM CONTRACT (the load-bearing invariant): for a fixed set of stable
// tagged blocks, the stable prefix text + hash are IDENTICAL regardless of the
// order the route appended them, and are UNAFFECTED by any volatile content. A
// stable sort by `stableOrdinal` keeps equal-ordinal blocks in input order
// (stable sort) so duplicate-kind blocks never reorder nondeterministically.
export const assembleStablePromptLayout = (
  tagged: ReadonlyArray<TaggedPromptMessage>,
): StablePromptLayout => {
  let volatileOrder = 0
  const classified = tagged.map(entry => {
    if (entry.stableKind === undefined && entry.message.role !== 'system') {
      const result = classifyTagged(entry, volatileOrder)
      volatileOrder += 1
      return result
    }
    return classifyTagged(entry, volatileOrder)
  })

  // Stable sort by ordinal (Array.prototype.sort is stable in modern engines;
  // we also tie-break by original index to be explicit and engine-independent).
  const withIndex = classified.map((entry, index) => ({ entry, index }))
  withIndex.sort((a, b) => {
    const byOrdinal = a.entry.stableOrdinal - b.entry.stableOrdinal
    return byOrdinal !== 0 ? byOrdinal : a.index - b.index
  })
  const ordered = withIndex.map(item => item.entry)

  const stable = ordered.filter(entry => entry.stability === 'stable')
  const stablePrefixText = serializeStablePrefix(stable)

  return {
    classified: ordered,
    messages: ordered.map(entry => entry.message),
    stablePrefixHash: hashStablePrefix(stablePrefixText),
    stablePrefixText,
  }
}

// One-way digest of the stable prefix text. Reuses FNV-1a (the same family as
// the telemetry cache-affinity hash) so the cache-key digest is a stable,
// public-safe correlation token — NOT a secret, and never reversible to the
// prompt text. Neutral prefix so the digest is self-describing in telemetry.
export const hashStablePrefix = (stablePrefixText: string): string => {
  let hash = 0x811c9dc5
  for (let index = 0; index < stablePrefixText.length; index += 1) {
    hash ^= stablePrefixText.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return `prefix:fnv1a32:${hash.toString(16).padStart(8, '0')}`
}

// ---------------------------------------------------------------------------
// 2. Deterministic tool-schema serialization (stable serialization → stable
// prefix → stable cache key).
// ---------------------------------------------------------------------------

// Canonically serialize an arbitrary JSON-able value with SORTED object keys, so
// two semantically-identical tool schemas that differ only in key order produce
// byte-identical text. Arrays preserve order (order is semantic for a tool list
// the caller fixes); objects sort keys (key order is NOT semantic in JSON).
export const canonicalJson = (value: unknown): string => {
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value) ?? 'null'
  }
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(',')}]`
  }
  const record = value as Record<string, unknown>
  const keys = Object.keys(record).sort()
  const entries = keys.map(
    key => `${JSON.stringify(key)}:${canonicalJson(record[key])}`,
  )
  return `{${entries.join(',')}}`
}

// Deterministically serialize a list of tool/function schemas into ONE stable
// system-message body. The tools are sorted by a stable identity (a `name` /
// `function.name` field when present, else their canonical JSON) so the SAME tool
// set always yields the SAME bytes regardless of the order the caller listed
// them — keeping the tool-schema block a stable prefix contributor.
export const serializeToolSchemas = (
  tools: ReadonlyArray<unknown>,
): string => {
  const toolName = (tool: unknown): string => {
    if (tool !== null && typeof tool === 'object') {
      const record = tool as Record<string, unknown>
      if (typeof record['name'] === 'string') {
        return record['name']
      }
      const fn = record['function']
      if (fn !== null && typeof fn === 'object') {
        const fnName = (fn as Record<string, unknown>)['name']
        if (typeof fnName === 'string') {
          return fnName
        }
      }
    }
    return canonicalJson(tool)
  }
  const sorted = [...tools].sort((a, b) =>
    toolName(a) < toolName(b) ? -1 : toolName(a) > toolName(b) ? 1 : 0,
  )
  return sorted.map(canonicalJson).join('\n')
}

// ---------------------------------------------------------------------------
// 3. Cache-affinity keys (account / session / codebase → ONE raw key).
// ---------------------------------------------------------------------------

// The dimensions that determine which cache-warm lane/replica a follow-up turn
// should pin to. The more of these two consecutive requests share, the more of
// the prefix the provider cache can reuse:
//   - account  : the authenticated account (always present).
//   - session  : a client-supplied conversation/session id (multi-turn chat).
//   - codebase : a client-supplied codebase/repo ref (codebase Q&A; the book's
//                "asking multiple questions about a codebase" case).
// Only `account` is required; the others sharpen affinity when present.
export type CacheAffinityDimensions = Readonly<{
  account: string
  session?: string | undefined
  codebase?: string | undefined
}>

// Field separator for the composed raw key. A unit separator so a dimension
// value cannot forge a boundary into the next field.
const KEY_FIELD_SEPARATOR = ''

// Compose the cache-affinity dimensions into ONE raw key string. Deterministic:
// the same dimensions always produce the same raw key. This raw key is what
// `hashCacheAffinityKey` (telemetry) digests for the public-safe record AND what
// `sessionAffinityParams` digests for the provider header — the raw key itself
// is NEVER stored, sent, or logged.
//
// Absent optional dimensions are rendered as empty fields (not omitted) so the
// key shape is fixed and `account-only` vs `account+session` never collide.
export const deriveCacheAffinityKey = (
  dimensions: CacheAffinityDimensions,
): string =>
  [
    `acct=${dimensions.account}`,
    `sess=${dimensions.session ?? ''}`,
    `code=${dimensions.codebase ?? ''}`,
  ].join(KEY_FIELD_SEPARATOR)

// ---------------------------------------------------------------------------
// 4. Provider session affinity (x-session-affinity + OpenAI `user`).
// ---------------------------------------------------------------------------

// The passthrough-param keys the adapters already read for session affinity:
//   - `x-session-affinity` : Fireworks replica-pinning header (fireworks-adapter
//                            reads it from passthroughParams → request header).
//   - `user`               : OpenAI-style stable end-user id (the passthrough /
//                            OpenAI lanes forward it).
// Both carry the SAME opaque value derived from the affinity key, so a session's
// follow-up turns pin to the same cache-warm replica across whichever lane serves.
export const SESSION_AFFINITY_PARAM = 'x-session-affinity'
export const OPENAI_USER_PARAM = 'user'

// The opaque session-affinity VALUE sent to providers. It is a one-way digest of
// the raw affinity key, so the provider receives only an opaque correlation
// token — never the raw account/session/codebase identifiers (privacy: the
// upstream provider learns nothing about who the account is, only that two
// requests belong together). Reuses the telemetry hash so the value the provider
// pins on and the hash the receipt records share the same derivation.
export const deriveSessionAffinityValue = (rawKey: string): string =>
  hashCacheAffinityKey(rawKey)

// Build the session-affinity passthrough params for a derived affinity value.
// The route MERGES these into the request's `passthroughParams` so the adapters
// pick them up. A caller-supplied `user`/`x-session-affinity` is OVERRIDDEN by
// the gateway-derived value (the gateway owns affinity so a session pins
// deterministically, not by a client's stray value).
export const sessionAffinityParams = (
  affinityValue: string,
): Readonly<Record<string, string>> => ({
  [OPENAI_USER_PARAM]: affinityValue,
  [SESSION_AFFINITY_PARAM]: affinityValue,
})

// ---------------------------------------------------------------------------
// 5. Cached-token telemetry + the totalTokens reconciliation.
// ---------------------------------------------------------------------------

// Provider-reported token usage as the adapters normalize it (a subset of
// InferenceUsage — kept structural so this module stays a pure leaf).
export type ProviderUsage = Readonly<{
  promptTokens: number
  completionTokens: number
  totalTokens: number
  // Cached input tokens where the provider reports a cached dimension
  // (Fireworks `prompt_tokens_details.cached_tokens`, Anthropic
  // `cache_read_input_tokens`, Gemini `cachedContentTokenCount`). Undefined when
  // the provider does not report it → honestly `not_measured` downstream.
  cachedPromptTokens?: number | undefined
}>

// The reconciled view of a provider usage object. Explains the live discrepancy
// where `totalTokens` (e.g. 679) does NOT equal `promptTokens + completionTokens`
// (e.g. 347 + 20 = 367).
//
// WHY THE GAP IS REAL, NOT A BUG: the served models behind Khala bill tokens
// beyond the visible prompt + completion:
//   - Gemini (khala-mini): `totalTokenCount` includes THINKING / tool-use tokens
//     not surfaced in `promptTokenCount` / `candidatesTokenCount`
//     (vertex-gemini-adapter parseUsage documents this — it trusts the provider
//     totalTokenCount rather than recomputing, to avoid under-billing thoughts).
//   - Reasoning lanes similarly bill internal reasoning tokens.
// So `total - (prompt + completion)` is a REAL billed dimension (reasoning /
// thinking / tool-use), reported HONESTLY here as `unaccountedTokens` rather than
// silently dropped or treated as an error. Telemetry records the provider's
// authoritative `totalTokens` (receipt-first) and discloses the reconciliation.
export type ReconciledUsage = Readonly<{
  promptTokens: number
  completionTokens: number
  // The provider's AUTHORITATIVE total (receipt-first). This — not
  // prompt+completion — is what metering + telemetry record.
  totalTokens: number
  // Cached input tokens (subset of promptTokens that hit the prompt cache),
  // undefined when the provider does not report a cached dimension.
  cachedPromptTokens: number | undefined
  // total − (prompt + completion). Non-negative and typically the model's
  // reasoning / thinking / tool-use billed tokens. `0` when the provider's total
  // is exactly prompt+completion (no hidden dimension).
  unaccountedTokens: number
  // True when the provider's total exceeds prompt+completion (a hidden billed
  // dimension exists). Lets telemetry/callers SEE the gap is expected, not a
  // miscount.
  hasUnaccountedTokens: boolean
}>

// Reconcile a provider usage object. Trusts the provider's `totalTokens` as
// authoritative (receipt-first; never recompute it as prompt+completion, which
// would under-count billed reasoning/thinking/tool-use tokens). When the
// provider's total is LESS than prompt+completion (a malformed/degenerate
// response), the unaccounted delta floors at 0 (never negative) and the total is
// left as the provider reported it (we do not fabricate a corrected total).
export const reconcileUsageTokens = (
  usage: ProviderUsage,
): ReconciledUsage => {
  const visible = usage.promptTokens + usage.completionTokens
  const unaccounted = Math.max(0, usage.totalTokens - visible)
  return {
    cachedPromptTokens: usage.cachedPromptTokens,
    completionTokens: usage.completionTokens,
    hasUnaccountedTokens: unaccounted > 0,
    promptTokens: usage.promptTokens,
    totalTokens: usage.totalTokens,
    unaccountedTokens: unaccounted,
  }
}

// Re-export the telemetry hash so callers that already import this module can
// produce the public-safe cache-affinity digest from the SAME place they derive
// the raw key (one canonical hashing path; no parallel hash).
export { hashCacheAffinityKey }
