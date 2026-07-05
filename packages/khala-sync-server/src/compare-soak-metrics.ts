/**
 * Compare-mode soak observability (#8282 shared follow-up, filed against the
 * #8336 entitlements and #8361 supervision blockers).
 *
 * PROBLEM: proving a `compare` read (D1 serves, Postgres shadow-read
 * compared, mismatches logged via the existing typed diagnostics —
 * `khala_sync_*_compare_mismatch`, `khala_sync_*_dual_write_failed`, etc.)
 * is safe to flip to real Postgres serving requires a genuine multi-hour (or
 * longer) soak with ZERO mismatches. Until now the only way to observe that
 * was a `wrangler tail` piped to one agent's terminal for the length of a
 * single session — not a real soak, invisible once the session ends, and
 * silently vacuous for near-zero-traffic domains (a "clean" tail on zero
 * requests proves nothing).
 *
 * THIS MODULE is additive, NOT a replacement: it does not touch the
 * existing per-call diagnostic events. It is a second, durable, aggregable
 * layer — one Cloudflare Analytics Engine data point per compare-mode read
 * — that a query script (see khala-sync-server/scripts/query-compare-soak.ts)
 * can later answer "how many compare-mode reads has domain X served in the
 * last N hours, how many mismatched, and is that zero or NON-EXISTENT
 * (vacuous)?" for.
 *
 * FAIL-SOFT CONTRACT (load-bearing): `record()` must NEVER throw, block, or
 * slow the real read/response path it instruments. A missing dataset
 * binding, an Analytics Engine outage, or a malformed sample all degrade to
 * a silent no-op — never a delayed or failed compare-mode read.
 *
 * Cloudflare Workers Analytics Engine schema reminder (there is no fixed
 * schema — every dataset is just up to 20 `blobs` (strings), up to 20
 * `doubles` (numbers), and up to 1 `index` per data point, queryable later
 * via the Analytics Engine SQL API as `blob1`, `blob2`, ..., `double1`,
 * `double2`, ..., `index1`, `timestamp`):
 *
 *   blob1    domain slug (also the index, so the sampler always keeps at
 *            least one sample per domain even on a sampled dataset)
 *   blob2    read-kind / op / table (bounded to the domain's own read-kind
 *            vocabulary, e.g. `freeTierKeyExists`, `omni_public_proof_bundles:readById`)
 *   blob3    outcome ("match" | "mismatch" | "error")
 *   double1  1 (constant) — SUM(double1) is the total compare-mode read count
 *   double2  1 iff outcome === "match", else 0
 *   double3  1 iff outcome === "mismatch", else 0
 *   double4  1 iff outcome === "error" (the shadow Postgres read itself
 *            failed — not comparable, but still real traffic worth counting
 *            so a domain never reads as vacuous just because its shadow
 *            reads are erroring)
 *   index1   domain slug (see blob1)
 */

export type CompareSoakOutcome = "match" | "mismatch" | "error"

export type CompareSoakSample = Readonly<{
  /** Domain slug, e.g. "entitlements_gate", "entitlements_non_gate", "supervision", "artanis". */
  domain: string
  /** Read-kind / op / table identifier within the domain. */
  readKind: string
  outcome: CompareSoakOutcome
}>

/**
 * Structural subset of Cloudflare's `AnalyticsEngineDataset` binding type —
 * defined locally (rather than depending on `@cloudflare/workers-types` from
 * this Node-runnable package) so this module stays portable and trivially
 * testable with a plain object double. Mirrors the real
 * `writeDataPoint(event?: AnalyticsEngineDataPoint): void` shape (optional
 * param, plain mutable arrays, nullable blob/index elements) closely enough
 * that a real Cloudflare `AnalyticsEngineDataset` binding satisfies this
 * interface structurally with no cast at the call site.
 */
export interface CompareSoakAnalyticsDataset {
  writeDataPoint(event?: {
    indexes?: Array<string | null>
    blobs?: Array<string | null>
    doubles?: Array<number>
  }): void
}

export type CompareSoakMetrics = Readonly<{
  record: (sample: CompareSoakSample) => void
}>

// Analytics Engine hard limits: blobs are capped at 5120 bytes combined and
// index at 96 bytes; domain/read-kind strings are always short internal
// identifiers, but clip defensively so a future typo can never throw.
const MAX_INDEX_LENGTH = 96
const MAX_BLOB_LENGTH = 512

const clip = (value: string, max: number): string =>
  value.length > max ? value.slice(0, max) : value

/**
 * Build the fail-soft recorder. Pass `dataset` as `undefined` when the
 * `analytics_engine_datasets` binding is absent (local dev, tests, or a
 * Worker env that has not been granted the binding yet) — `record()` then
 * becomes a true no-op, so every call site can wire this in unconditionally.
 */
export const makeCompareSoakMetrics = (
  dataset: CompareSoakAnalyticsDataset | undefined,
): CompareSoakMetrics => {
  if (dataset === undefined) {
    return { record: () => {} }
  }
  return {
    record: (sample: CompareSoakSample) => {
      try {
        const domain = clip(sample.domain, MAX_INDEX_LENGTH)
        dataset.writeDataPoint({
          blobs: [domain, clip(sample.readKind, MAX_BLOB_LENGTH), sample.outcome],
          doubles: [
            1,
            sample.outcome === "match" ? 1 : 0,
            sample.outcome === "mismatch" ? 1 : 0,
            sample.outcome === "error" ? 1 : 0,
          ],
          indexes: [domain],
        })
      } catch {
        // Fail-soft by construction — see module doc. Never rethrow, never
        // let an Analytics Engine fault reach the caller.
      }
    },
  }
}

/** The no-op recorder, exported directly for call sites that want an
 * explicit "metrics disabled" value without threading `undefined` through
 * `makeCompareSoakMetrics`. */
export const noopCompareSoakMetrics: CompareSoakMetrics = {
  record: () => {},
}
