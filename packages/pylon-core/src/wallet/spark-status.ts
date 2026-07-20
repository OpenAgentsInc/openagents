/**
 * IDR-07 app-side Spark wallet STATUS adapter.
 *
 * The neutral `@openagentsinc/sovereign-identity` package owns the reconciliation
 * SEAM (`SparkComparisonAdapter` / `BreezSeedFingerprintDeriver`), but it must
 * never link a wallet SDK (see that package's `boundary.test.ts`). IDR-07 supplies
 * the REAL app-side Spark adapter behind that seam HERE, in the Pylon app layer.
 *
 * This adapter restores the Spark wallet from the recovered shared root in a
 * STRICTLY status-only posture:
 *
 * - It binds to the recovered `legacy_unified_nostr_spark` profile.
 * - It receives secret material ONLY inside the bounded `RecoveredSecret.use`
 *   scope, delegated to the injected neutral comparison adapter; the mnemonic and
 *   the seed never become a field, a return value, or a logged token.
 * - It OPENS the EXPECTED wallet or FAILS CLOSED. It compares the public wallet
 *   identity derived from the recovered seed against a KNOWN expected public
 *   identity (from the confirmed reconciliation / manifest). A mismatch fails with
 *   `expected_wallet_mismatch`; it NEVER mints a fresh wallet bucket.
 * - It exposes a PUBLIC wallet status only: the wallet public id / fingerprint,
 *   the bound profile, `mode: "status_only"`, and `sendEnabled: false`. There is
 *   NO send path on this surface. A payment/send path is a SEPARATE explicit
 *   authority (a later packet), never admitted here.
 * - A LIVE balance fetch is an ONLINE action. It is NOT part of the offline
 *   status open and is gated behind an explicit owner-attended online flag
 *   (`gateSparkLiveBalance`); the deterministic offline proof is the wallet
 *   IDENTITY match from the recovered seed.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import {
  DERIVATION_PROFILE_ID,
  deriveSovereignIdentityPublic,
  normalizeMnemonic,
  RecoveredSecret,
  rustSparkComparisonAdapter,
  type SparkAdapterFingerprint,
  type SparkAdapterKind,
  type SparkComparisonAdapter,
} from "@openagentsinc/sovereign-identity"
import { Effect, Schema as S } from "effect"

/** The schema id of the public Spark wallet status projection. */
export const SPARK_WALLET_STATUS_SCHEMA_ID = "openagents.pylon.spark_wallet_status.v1" as const

/** The only wallet mode IDR-07 admits. A send mode is a separate explicit authority. */
export const SPARK_WALLET_MODE = "status_only" as const
export type SparkWalletMode = typeof SPARK_WALLET_MODE

/** A lowercase hex string. Public wallet identifiers only — never private material. */
const HexString = S.String.check(S.isPattern(/^[0-9a-f]+$/))
/** A bounded public reference (the frozen derivation profile id). */
const BoundedRef = S.String.check(S.isMaxLength(120), S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]*$/))

/**
 * The PUBLIC status of an opened Spark wallet. Every field is public-safe: it
 * carries the wallet public identifiers, the bound profile, and the posture
 * flags only. It NEVER carries the mnemonic, `nsec`, raw private key, or seed, so
 * a serialized status is safe to log, project to the renderer, or persist.
 */
export const SparkWalletStatus = S.Struct({
  schema: S.Literal(SPARK_WALLET_STATUS_SCHEMA_ID),
  /** The recovered derivation profile this wallet is bound to. */
  profileId: BoundedRef,
  /** The Spark runtime the opening comparison adapter derived under. */
  adapter: S.Literals(["rust_spark", "breez_spark", "ldk"]),
  /** The PUBLIC wallet identifier (the compressed public key hex when present). */
  walletPublicId: HexString,
  /** The PUBLIC wallet fingerprint hex. */
  walletFingerprint: HexString,
  /** Always `status_only` — this surface never sends. */
  mode: S.Literal(SPARK_WALLET_MODE),
  /** True when the EXPECTED wallet was opened (never a new bucket). */
  opened: S.Boolean,
  /** Always `false` — a send path is a separate explicit authority. */
  sendEnabled: S.Literal(false),
  /**
   * The reachability posture. `status_only` is the offline deterministic open;
   * a live balance requires the gated online action and is never reached here.
   */
  reachability: S.Literals(["status_only"]),
})
export interface SparkWalletStatus extends S.Schema.Type<typeof SparkWalletStatus> {}

/** A public-safe decoder for the boundary. It never throws. */
const decodeStatusExit = S.decodeUnknownExit(SparkWalletStatus)
export const decodeSparkWalletStatus = (value: unknown): SparkWalletStatus | null => {
  const decoded = decodeStatusExit(value)
  return decoded._tag === "Success" ? decoded.value : null
}

/**
 * A typed Spark status failure. It carries the coarse reason and the adapter kind
 * ONLY. It never carries the mnemonic, seed, or any private material, so a logged
 * error can never leak the secret.
 *
 * - `expected_wallet_mismatch`: the recovered seed derived a DIFFERENT public
 *   wallet than the known expected one. Fail closed; never mint a new bucket.
 * - `derive_failed`: the injected comparison adapter could not derive.
 * - `deferred`: the comparison adapter is a deferred seam (LDK); it cannot open.
 * - `online_action_gated`: a live-balance/online action was requested without the
 *   explicit owner-attended online flag.
 */
export class SparkStatusError extends S.TaggedErrorClass<SparkStatusError>()(
  "pylon-core.SparkStatusError",
  {
    reason: S.Literals([
      "expected_wallet_mismatch",
      "derive_failed",
      "deferred",
      "online_action_gated",
    ]),
    adapter: S.Literals(["rust_spark", "breez_spark", "ldk"]),
  },
) {}

/**
 * A KNOWN expected public wallet identity. In production it comes from the IDR-04
 * confirmed reconciliation result (`GroupedIdentity.sparkFingerprints`) or the
 * IDR-05 public manifest. The adapter opens THIS wallet or fails closed.
 */
export interface ExpectedSparkWallet {
  readonly adapter: SparkAdapterKind
  readonly fingerprintHex: string
  readonly publicKeyHex?: string
}

/** Options for the app-side Spark status adapter. */
export interface SparkStatusAdapterOptions {
  /**
   * The neutral IDR-04 comparison adapter that derives the PUBLIC wallet identity
   * inside the bounded secret scope. Defaults to the offline-exact Rust Spark
   * reference. The real Breez Spark SDK (online) is injected explicitly through
   * `makeBreezSparkComparisonAdapter` at a composition root when that gated online
   * path is admitted.
   */
  readonly comparisonAdapter?: SparkComparisonAdapter
  /** The KNOWN expected public wallet identity to match. */
  readonly expected: ExpectedSparkWallet
  /** The bound public profile id. Defaults to the frozen shared-root profile. */
  readonly profileId?: string
}

/** The app-side Spark status adapter surface. It opens status-only; it never sends. */
export interface SparkStatusAdapter {
  /** The Spark runtime this adapter opens under. */
  readonly adapter: SparkAdapterKind
  /**
   * Open the EXPECTED wallet in STATUS-ONLY mode from the bounded recovered
   * secret. It fails closed on a mismatch and never mints a new wallet bucket.
   */
  readonly openStatusOnly: (
    secret: RecoveredSecret,
  ) => Effect.Effect<SparkWalletStatus, SparkStatusError>
}

/** Whether a derived public fingerprint IS the expected wallet identity. */
const matchesExpected = (
  derived: SparkAdapterFingerprint,
  expected: ExpectedSparkWallet,
): boolean => {
  if (derived.adapter !== expected.adapter) return false
  if (derived.fingerprintHex !== expected.fingerprintHex) return false
  // When both sides expose a public key, it must match too.
  if (
    derived.publicKeyHex !== undefined &&
    expected.publicKeyHex !== undefined &&
    derived.publicKeyHex !== expected.publicKeyHex
  ) {
    return false
  }
  return true
}

/** Build the public-safe status for an opened wallet. Public identifiers only. */
const buildStatus = (fp: SparkAdapterFingerprint, profileId: string): SparkWalletStatus => ({
  schema: SPARK_WALLET_STATUS_SCHEMA_ID,
  profileId,
  adapter: fp.adapter,
  walletPublicId: fp.publicKeyHex ?? fp.fingerprintHex,
  walletFingerprint: fp.fingerprintHex,
  mode: SPARK_WALLET_MODE,
  opened: true,
  sendEnabled: false,
  reachability: "status_only",
})

/**
 * Build the app-side Spark status adapter over an injected neutral comparison
 * adapter and a known expected wallet identity.
 */
export const makeSparkStatusAdapter = (
  options: SparkStatusAdapterOptions,
): SparkStatusAdapter => {
  const comparison = options.comparisonAdapter ?? rustSparkComparisonAdapter
  const profileId = options.profileId ?? DERIVATION_PROFILE_ID
  const expected = options.expected
  return {
    adapter: comparison.kind,
    openStatusOnly: Effect.fn("Pylon.SparkStatusAdapter.openStatusOnly")(function* (secret) {
      // A deferred seam (LDK) can never open the status-only wallet.
      if (comparison.availability === "deferred") {
        return yield* new SparkStatusError({ reason: "deferred", adapter: comparison.kind })
      }
      // Derive the PUBLIC wallet identity inside the bounded secret scope. The
      // mnemonic and seed stay inside the neutral adapter's `use` callback.
      const derived = yield* comparison
        .deriveFingerprint(secret)
        .pipe(
          Effect.mapError(
            (error) => new SparkStatusError({ reason: "derive_failed", adapter: error.adapter }),
          ),
        )
      // FAIL CLOSED: open THE EXPECTED wallet or nothing. A mismatch NEVER
      // creates a new wallet bucket.
      if (!matchesExpected(derived, expected)) {
        return yield* new SparkStatusError({
          reason: "expected_wallet_mismatch",
          adapter: comparison.kind,
        })
      }
      return buildStatus(derived, profileId)
    }),
  }
}

/**
 * Open the recovered Spark wallet in STATUS-ONLY mode from a mnemonic, binding to
 * the frozen shared-root profile and proving the EXPECTED wallet opens.
 *
 * The expected public wallet identity is derived from the SAME recovered seed
 * (the recovered root IS the expected wallet), so a healthy rehydrate opens
 * status-only and a corrupted seed fails closed. This is the non-blocking probe
 * the Desktop Boot Sequence (#9103) surfaces: the mnemonic lives only inside this
 * bounded scope and the return is the PUBLIC status. It returns `null` on any
 * failure, so a caller never has to handle a secret-bearing error.
 */
export const openRecoveredSparkWalletStatus = (mnemonic: string): SparkWalletStatus | null =>
  Effect.runSync(
    Effect.gen(function* () {
      const normalized = normalizeMnemonic(mnemonic)
      const pub = deriveSovereignIdentityPublic(normalized)
      const expected: ExpectedSparkWallet = {
        adapter: "rust_spark",
        fingerprintHex: pub.sparkBip32FingerprintHex,
        publicKeyHex: pub.sparkPublicKeyHex,
      }
      const adapter = makeSparkStatusAdapter({ expected })
      const secret = new RecoveredSecret(normalized, "plain_mnemonic_file", "1")
      return yield* adapter.openStatusOnly(secret)
    }).pipe(Effect.catch(() => Effect.succeed<SparkWalletStatus | null>(null))),
  )

// ---------------------------------------------------------------------------
// Live balance — an ONLINE action, gated. Never run in the offline suite.
// ---------------------------------------------------------------------------

/** The gate for a live-balance / online Spark action. */
export interface SparkLiveBalanceGate {
  /** True only when a network-reachable environment is admitted. */
  readonly online: boolean
  /** True only during an explicit owner-attended run. */
  readonly ownerAttended: boolean
}

/**
 * Gate a live-balance (online) Spark action. A live balance fetch reaches the
 * network and the real Breez Spark SDK, so it is NEVER part of the offline
 * status open and NEVER runs in the automated suite. Without both the `online`
 * and `ownerAttended` flags it fails `online_action_gated`. Even when both flags
 * are set the real SDK link is a DEFERRED online step recorded as a NEEDS-OWNER
 * item, so this fails `deferred` rather than silently reaching the network.
 */
export const gateSparkLiveBalance = Effect.fn("Pylon.SparkStatusAdapter.gateSparkLiveBalance")(
  function* (status: SparkWalletStatus, gate: SparkLiveBalanceGate) {
    if (!gate.online || !gate.ownerAttended) {
      return yield* new SparkStatusError({ reason: "online_action_gated", adapter: status.adapter })
    }
    // The real online Breez Spark SDK link is a deferred owner-attended step.
    return yield* new SparkStatusError({ reason: "deferred", adapter: status.adapter })
  },
)
