import type { FleetAccountEntity } from "@openagentsinc/khala-sync"

/**
 * Capacity-aware dispatch account selection (#8389; depends on the
 * `fleet.reportAccountState` capacity fields added in #8302's follow-up —
 * see `fleet-mutators.ts`).
 *
 * Pure selector: given the `fleet_account` post-images currently projected
 * for a fleet run scope, pick the single best account for the NEXT
 * `runtime.startTurn` dispatch. No I/O, no scope/ownership concerns — those
 * stay with the caller (the future Pylon-side consumer of durable
 * `runtime.startTurn` control intents, tracked in #8388).
 *
 * ELIGIBILITY: an account is a dispatch candidate only when ALL hold:
 *   - `readiness === "ready"` (a `cooldown`/`unavailable`/`unknown` account
 *     is excluded even if it still reports leftover `capacityAvailable` —
 *     readiness is the authoritative signal; capacity numbers can lag it).
 *   - `capacityAvailable !== undefined && capacityAvailable > 0`. Missing
 *     capacity is NOT treated as "assume available" (that would dispatch
 *     into unknown-capacity accounts) or as zero-is-fine (same failure
 *     mode as `undefined` — both mean "we don't know this account has a
 *     free slot").
 *   - `options.provider`, when given, equals the account's `provider`
 *     (an account with no reported `provider` never matches a set filter).
 *
 * RANKING among eligible accounts:
 *   1. Highest `capacityAvailable` wins.
 *   2. Tie-break: lowest `capacityBusy + capacityQueued` (least loaded).
 *      Missing `capacityBusy`/`capacityQueued` count as 0 for this sum —
 *      eligibility already required a known `capacityAvailable`, so a
 *      missing load field reads as "no reported load" rather than
 *      "unknown", unlike the eligibility gate above.
 *   3. Tie-break: `accountRefHash` lexicographic ascending, for a fully
 *      deterministic order.
 *
 * ROUND-ROBIN WITHIN A FULL TIE: when the top-ranked group after all three
 * tie-breaks still contains more than one account (i.e. they are
 * indistinguishable — same capacity, same load, and the caller is telling
 * us which hash it dispatched to last), prefer a DIFFERENT member of that
 * tied group over repeating `lastUsedAccountRefHash`, cycling through the
 * group in `accountRefHash` order. This is the "round-robin by available
 * capacity" behavior named in the issue title: capacity naturally breaks
 * most ties across real dispatches (`capacityBusy` changes after each
 * turn), and this covers the residual case where multiple otherwise-equal
 * accounts would otherwise always resolve to the same lowest hash.
 *
 * Returns `undefined` when no account is eligible (empty list, or every
 * account is at zero/unknown capacity or not ready) — callers must not
 * fabricate a fallback account.
 */

export interface SelectDispatchAccountOptions {
  /**
   * The `accountRefHash` most recently dispatched to for this run, if any.
   * Only affects the outcome when the top-ranked accounts are in a full
   * tie (see ROUND-ROBIN above); otherwise the plain capacity/load/hash
   * ranking is unaffected by dispatch history.
   */
  readonly lastUsedAccountRefHash?: string
  /**
   * Restrict eligibility to accounts reporting this exact `provider`
   * (e.g. `"codex"`, `"claude"`) — the dispatch target's lane determines
   * which CLI backs it, so a turn destined for a Codex lane must not land
   * on a Claude account. An account with no reported `provider` is
   * excluded whenever this filter is set, since "unknown provider" is not
   * a safe match. Omit to select across all providers (e.g. when the
   * caller has already pre-filtered `accounts` itself).
   */
  readonly provider?: string
}

const isEligibleAccount = (
  account: FleetAccountEntity,
  options: SelectDispatchAccountOptions,
): boolean =>
  account.readiness === "ready" &&
  account.capacityAvailable !== undefined &&
  account.capacityAvailable > 0 &&
  (options.provider === undefined || account.provider === options.provider)

const loadOf = (account: FleetAccountEntity): number =>
  (account.capacityBusy ?? 0) + (account.capacityQueued ?? 0)

const compareAccounts = (
  a: FleetAccountEntity,
  b: FleetAccountEntity,
): number => {
  // capacityAvailable is guaranteed defined for eligible accounts.
  const capacityDelta = (b.capacityAvailable as number) - (a.capacityAvailable as number)
  if (capacityDelta !== 0) return capacityDelta

  const loadDelta = loadOf(a) - loadOf(b)
  if (loadDelta !== 0) return loadDelta

  return a.accountRefHash < b.accountRefHash
    ? -1
    : a.accountRefHash > b.accountRefHash
      ? 1
      : 0
}

/**
 * The accounts indistinguishable from the top-ranked one on capacity and
 * load — i.e. everything the `accountRefHash` tie-break alone decided
 * between. `ranked` is pre-sorted by `compareAccounts`, so this group is
 * always a prefix of it.
 */
const topTieGroup = (
  ranked: ReadonlyArray<FleetAccountEntity>,
): ReadonlyArray<FleetAccountEntity> => {
  const [best] = ranked
  if (best === undefined) return []
  return ranked.filter(
    (candidate) =>
      candidate.capacityAvailable === best.capacityAvailable &&
      loadOf(candidate) === loadOf(best),
  )
}

export const selectDispatchAccount = (
  accounts: ReadonlyArray<FleetAccountEntity>,
  options: SelectDispatchAccountOptions = {},
): FleetAccountEntity | undefined => {
  const eligible = accounts.filter((account) => isEligibleAccount(account, options))
  if (eligible.length === 0) return undefined

  const ranked = [...eligible].sort(compareAccounts)
  const tieGroup = topTieGroup(ranked)

  if (tieGroup.length <= 1 || options.lastUsedAccountRefHash === undefined) {
    return ranked[0]
  }

  const lastIndex = tieGroup.findIndex(
    (candidate) => candidate.accountRefHash === options.lastUsedAccountRefHash,
  )
  if (lastIndex === -1) {
    // Last dispatch wasn't in this tied group (or there was none) — the
    // plain deterministic order (lowest hash) is already the right pick.
    return tieGroup[0]
  }

  // Cycle to the next tied account, wrapping back to the start.
  return tieGroup[(lastIndex + 1) % tieGroup.length]
}
