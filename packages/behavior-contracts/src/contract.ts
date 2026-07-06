import { Schema as S } from "effect"

export const BehaviorContractSchemaVersion = "openagents.behavior_contracts.v1"

/**
 * Registry lifecycle state for one contract, mirroring the product-promise
 * state discipline: exactly one "good" state (`enforced`), and every other
 * state must be treated as non-green when making claims about the product.
 *
 * - `enforced`   — statement has at least one oracle that runs in a named
 *                  test sweep; deviations fail that sweep.
 * - `pending`    — statement is recorded but its oracle has not landed or is
 *                  not yet wired into a sweep. Never describe pending
 *                  behavior as guaranteed.
 * - `retired`    — statement no longer applies (superseded by a newer
 *                  contract version). Kept for history.
 */
export const BehaviorContractState = S.Literals(["enforced", "pending", "retired"])
export type BehaviorContractState = "enforced" | "pending" | "retired"

/**
 * Where the oracle runs. `test-sweep` means the normal per-package test run
 * (`bun test`, package `verify`, repo `test:*` chain) that gates pushes to
 * main. `nightly` and `manual` are weaker tiers; `unenforced` is an explicit
 * admission that nothing runs.
 */
export const BehaviorContractEnforcementTier = S.Literals([
  "test-sweep",
  "nightly",
  "manual",
  "unenforced",
])
export type BehaviorContractEnforcementTier =
  | "test-sweep"
  | "nightly"
  | "manual"
  | "unenforced"

export const BehaviorContractOracleKind = S.Literals([
  "bun-test",
  "qa-scenario",
  "visual-smoke",
  "manual-check",
  "planned",
])
export type BehaviorContractOracleKind =
  | "bun-test"
  | "qa-scenario"
  | "visual-smoke"
  | "manual-check"
  | "planned"

/** Driver mode vocabulary shared with the khala-qa-harness scenario DSL. */
export const BehaviorContractOracleMode = S.Literals([
  "unit",
  "dom",
  "rpc",
  "vision",
  "headless",
])
export type BehaviorContractOracleMode = "unit" | "dom" | "rpc" | "vision" | "headless"

export const BehaviorContractOracle = S.Struct({
  description: S.String,
  id: S.String,
  kind: BehaviorContractOracleKind,
  mode: BehaviorContractOracleMode,
  /**
   * For `bun-test` oracles: repo-relative path of the test file that encodes
   * the expectation. The file must reference the owning contractId so
   * coverage checking can prove the linkage. For `qa-scenario` oracles: the
   * scenario id in the khala-qa-harness seed corpus.
   */
  ref: S.String,
})
export type BehaviorContractOracle = {
  readonly description: string
  readonly id: string
  readonly kind: BehaviorContractOracleKind
  readonly mode: BehaviorContractOracleMode
  readonly ref: string
}

/**
 * Two-sided seam binding (ST-5 #8511). A seam contract asserts behavior that
 * only exists at the boundary BETWEEN two artifacts — e.g. "a cookie-less
 * bearer client completes a real /api/sync/connect upgrade and reaches
 * live" — where each side can pass its own one-sided suite while the seam is
 * broken (the class of bug that shipped in mobile builds 10-13, and the OTA
 * fingerprint mismatch on 2026-07-06).
 *
 * Convention: a seam contract's contractId carries a `seam` segment
 * (`<area>.seam.<slug>.v<N>`), and the contract MUST name both sides here.
 * The registry validator rejects a `seam`-segment id without this field (and
 * vice versa), and the coverage checker requires an enforced seam contract's
 * `bun-test` oracle ref to be an e2e suite (`*.e2e.*` by repo convention) —
 * a fake-transport unit test on one side can never count as proof that the
 * two real sides meet.
 */
export const BehaviorContractSeam = S.Struct({
  /** Repo-relative path of the client-side artifact the seam binds. */
  client: S.String,
  /** Repo-relative path of the server-side artifact the seam binds. */
  server: S.String,
})
export type BehaviorContractSeam = {
  readonly client: string
  readonly server: string
}

export const BehaviorContractSource = S.Struct({
  /** Where the requirement was stated (e.g. "khala-code-session", "forum", "issue"). */
  channel: S.String,
  statedBy: S.String,
  /** ISO date the requirement was stated. */
  statedOn: S.String,
})
export type BehaviorContractSource = {
  readonly channel: string
  readonly statedBy: string
  readonly statedOn: string
}

export const BehaviorContract = S.Struct({
  /** What enforcing this contract does NOT authorize or imply. */
  authorityBoundary: S.optional(S.String),
  /** Blocker refs that gate `enforced`, promise-registry style. */
  blockerRefs: S.Array(S.String),
  /** Stable dotted versioned id: `<area>.<slug>.v<N>`. */
  contractId: S.String,
  enforcementTier: BehaviorContractEnforcementTier,
  /** Endpoints, tests, docs, receipts, `promise:<id>` / `contract:<id>` cross-refs. */
  evidenceRefs: S.Array(S.String),
  oracles: S.Array(BehaviorContractOracle),
  productArea: S.String,
  /**
   * Required exactly when the contractId carries a `seam` segment: names the
   * client and server artifacts the two-sided seam binds. See
   * {@link BehaviorContractSeam}.
   */
  seam: S.optional(BehaviorContractSeam),
  source: BehaviorContractSource,
  state: BehaviorContractState,
  /** The stated expectation, in the owner's/customer's words, kept verbatim where possible. */
  statement: S.String,
  /** The UI/API surface the contract binds (e.g. "khala-code-desktop"). */
  surface: S.String,
  /** Human-readable description of how the contract is verified end to end. */
  verification: S.String,
})
export type BehaviorContract = {
  readonly authorityBoundary?: string
  readonly blockerRefs: ReadonlyArray<string>
  readonly contractId: string
  readonly enforcementTier: BehaviorContractEnforcementTier
  readonly evidenceRefs: ReadonlyArray<string>
  readonly oracles: ReadonlyArray<BehaviorContractOracle>
  readonly productArea: string
  readonly seam?: BehaviorContractSeam
  readonly source: BehaviorContractSource
  readonly state: BehaviorContractState
  readonly statement: string
  readonly surface: string
  readonly verification: string
}

export const BehaviorContractRegistryDocument = S.Struct({
  contracts: S.Array(BehaviorContract),
  schemaVersion: S.Literal(BehaviorContractSchemaVersion),
  /** Registry version string, `YYYY-MM-DD.N`, bumped on every registry change. */
  version: S.String,
})
export type BehaviorContractRegistryDocument = {
  readonly contracts: ReadonlyArray<BehaviorContract>
  readonly schemaVersion: typeof BehaviorContractSchemaVersion
  readonly version: string
}

export const decodeBehaviorContractRegistryDocument = (
  input: unknown,
): BehaviorContractRegistryDocument =>
  S.decodeUnknownSync(BehaviorContractRegistryDocument)(input) as BehaviorContractRegistryDocument

export const behaviorContractIdPattern = /^[a-z0-9_]+(\.[a-z0-9_]+)+\.v[0-9]+$/u

/**
 * Seam-contract naming convention (ST-5 #8511): a contract is a seam contract
 * exactly when its dotted id carries a standalone `seam` segment
 * (`<area>.seam.<slug>.v<N>`). Seam contracts must carry the `seam` field
 * naming both sides, and the coverage checker holds their enforced `bun-test`
 * oracles to the e2e-suite requirement.
 */
export const isSeamBehaviorContractId = (contractId: string): boolean =>
  contractId.split(".").includes("seam")
