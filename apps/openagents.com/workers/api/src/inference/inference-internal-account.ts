// Internal/ops account demand-attribution rule for the inference gateway
// (#6298 follow-up).
//
// PROBLEM. #6298 (commit 20cb17eb96) gave captured traces + `token_usage_events`
// rows a `demand_kind`/`demand_source` resolved from the
// `x-openagents-demand-kind`/`x-openagents-demand-source` headers (see
// `requestAttributionFromHeaders` in `chat-completions-routes.ts`). A request
// with NO demand header resolves to `unlabeled`. Our own dogfood — the
// long-running Harbor/Terminal-Bench run, the 15-min heartbeat, the 500-canary —
// all run on ONE internal/ops account but do NOT all send the header (notably the
// Terminal-Bench run predates header-tagging). So that untagged internal traffic
// lands in the external/`unlabeled` corpus and pollutes the trace corpus + the
// demand ledger.
//
// FIX. A DURABLE, header-independent rule: traffic from a configured
// internal/ops account ref is auto-classified `demand_kind=internal` regardless
// of request headers. The allowlist is env-configured
// (`INFERENCE_INTERNAL_ACCOUNT_REFS`, comma-separated account refs) so the ops
// account ref is NOT hardcoded in source — it lives in the worker `vars`. Every
// caller on that account self-classifies as internal WITHOUT sending a header.
//
// SHARED RESOLVER. The chat path resolves ONE `requestAttribution` value and
// hands the SAME value to BOTH the served-tokens recorder (`token_usage_events`)
// AND the trace emitter (`agent_traces.demand_kind`). This rule refines that one
// value AFTER auth (it needs the resolved `accountRef`), so the ledger and the
// trace stay consistent — they classify identically.
//
// PRECEDENCE (never DOWNGRADE a specific internal source):
//   - An explicit header that ALREADY classified the request as `internal` with
//     a specific `demandSource` (e.g. `harbor_terminal_bench` / `heartbeat` /
//     `canary`) KEEPS that specific source — the account rule only ensures the
//     `internal` kind, it must not overwrite the more specific source.
//   - When the account is internal but NO internal-specific header was sent
//     (header-less, or a non-internal header kind), force `demandKind=internal`
//     and default `demandSource` to `internal_account` (the generic ops marker).
//   - A non-internal account is UNAFFECTED: real external users still resolve
//     `external`/`unlabeled` exactly as before.
//
// FAIL-SOFT. A bad/empty/unset allowlist => an EMPTY set => this rule is a pure
// no-op (every request resolves exactly as `requestAttributionFromHeaders`
// produced it). It never throws and never changes a non-internal request.

import { type ServedTokensRequestAttribution } from './served-tokens-recorder'

// The generic ops `demand_source` applied to internal-account traffic that did
// NOT carry a more specific internal-source header. Bounded token (matches the
// `safeAttributionToken` shape used for header sources), so it stores cleanly in
// the same `demand_source` column.
export const INTERNAL_ACCOUNT_DEMAND_SOURCE = 'internal_account' as const

// Parse the comma-separated `INFERENCE_INTERNAL_ACCOUNT_REFS` env var into a
// normalized account-ref set. Whitespace is trimmed and empty entries dropped;
// an unset/blank/all-empty value yields an EMPTY set (the rule no-ops). The refs
// are opaque account refs (e.g. `agent:user_...`); no shape is assumed beyond a
// non-empty trimmed string, so the ops account ref format is not hardcoded.
export const parseInternalAccountRefs = (
  value: string | undefined,
): ReadonlySet<string> => {
  if (value === undefined) {
    return new Set<string>()
  }
  return new Set(
    value
      .split(',')
      .map(ref => ref.trim())
      .filter(ref => ref !== ''),
  )
}

// Refine the header-derived request attribution with the internal-account rule.
//
// `attribution` is the value `requestAttributionFromHeaders` produced (or
// `undefined` when the request sent NO demand headers at all). `accountRef` is
// the authenticated account. `internalAccountRefs` is the env-configured
// allowlist (empty => no-op).
//
// Returns the (possibly refined) attribution. When the account is NOT internal,
// the input is returned unchanged (including `undefined`). When the account IS
// internal, the result always carries `demandKind: 'internal'`, preserving a
// specific internal `demandSource`/`demandClient` from the header and defaulting
// the source to `internal_account` otherwise.
export const applyInternalAccountAttribution = (
  attribution: ServedTokensRequestAttribution | undefined,
  accountRef: string,
  internalAccountRefs: ReadonlySet<string>,
): ServedTokensRequestAttribution | undefined => {
  // Fail-soft no-op: empty allowlist or a non-internal account leaves the
  // header-derived attribution exactly as-is (external users are unaffected).
  if (internalAccountRefs.size === 0 || !internalAccountRefs.has(accountRef)) {
    return attribution
  }

  // The account is internal. Preserve a specific internal source the header
  // already set (never downgrade `harbor_terminal_bench` etc. to the generic
  // marker); otherwise default the source to the generic ops marker. Either way,
  // force `demandKind: 'internal'`.
  const hadInternalHeaderSource =
    attribution?.demandKind === 'internal' &&
    attribution.demandSource !== undefined

  const demandSource = hadInternalHeaderSource
    ? attribution!.demandSource
    : INTERNAL_ACCOUNT_DEMAND_SOURCE

  return {
    demandKind: 'internal',
    demandSource,
    ...(attribution?.demandClient === undefined
      ? {}
      : { demandClient: attribution.demandClient }),
  }
}
