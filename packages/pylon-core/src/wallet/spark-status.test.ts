import { sha256 } from "@noble/hashes/sha256"
import { bytesToHex } from "@noble/hashes/utils"
import {
  type BreezSeedFingerprintDeriver,
  deriveSovereignIdentityPublic,
  ldkSparkComparisonAdapter,
  makeBreezSparkComparisonAdapter,
  PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE,
  PUBLIC_TEST_MNEMONIC,
  RecoveredSecret,
} from "@openagentsinc/sovereign-identity"
import { Effect, Result } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  decodeSparkWalletStatus,
  type ExpectedSparkWallet,
  gateSparkLiveBalance,
  makeSparkStatusAdapter,
  openRecoveredSparkWalletStatus,
  SparkStatusError,
  type SparkWalletStatus,
} from "./spark-status.ts"

/**
 * IDR-07 app-side Spark status adapter tests.
 *
 * SAFETY: LOCAL-ONLY and OFFLINE. Every wallet is opened from the ONE published
 * BIP-39 TEST mnemonic (IDR-00), never a real secret, and the suite reaches no
 * network. The deterministic offline proof is the wallet IDENTITY match from the
 * recovered seed. The tests assert no mnemonic word or seed byte ever appears in
 * a status or an error (the secret-logging tripwire).
 */

/** A second published BIP-39 TEST mnemonic (all-ones entropy). NOT a real secret. */
const SECOND_TEST_MNEMONIC = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong"

/** The frozen expected Rust Spark wallet identity for the fixture seed. */
const expectedRust: ExpectedSparkWallet = {
  adapter: "rust_spark",
  fingerprintHex: PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.sparkBip32FingerprintHex,
  publicKeyHex: PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.sparkPublicKeyHex,
}

/** Build a bounded recovered secret for a public test mnemonic. */
const secretFor = (mnemonic: string): RecoveredSecret =>
  new RecoveredSecret(mnemonic, "plain_mnemonic_file", "1")

const runOpen = (
  adapter: ReturnType<typeof makeSparkStatusAdapter>,
  mnemonic: string,
): Result.Result<SparkWalletStatus, SparkStatusError> =>
  Effect.runSync(Effect.result(adapter.openStatusOnly(secretFor(mnemonic))))

/** Narrow to a success value or throw (test-only). */
const expectSuccess = (
  result: Result.Result<SparkWalletStatus, SparkStatusError>,
): SparkWalletStatus => {
  if (!Result.isSuccess(result)) throw new Error(`expected success, got ${JSON.stringify(result)}`)
  return result.success
}

/** Narrow to a failure or throw (test-only). */
const expectFailure = (
  result: Result.Result<SparkWalletStatus, SparkStatusError>,
): SparkStatusError => {
  if (!Result.isFailure(result)) throw new Error("expected failure, got success")
  return result.failure
}

describe("IDR-07 app-side Spark status adapter", () => {
  test("opens the EXPECTED wallet in status-only mode from the recovered seed", () => {
    const adapter = makeSparkStatusAdapter({ expected: expectedRust })
    const status = expectSuccess(runOpen(adapter, PUBLIC_TEST_MNEMONIC))
    // It opened THE expected wallet — the frozen public identifiers.
    expect(status.opened).toBe(true)
    expect(status.adapter).toBe("rust_spark")
    expect(status.walletFingerprint).toBe(
      PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.sparkBip32FingerprintHex,
    )
    expect(status.walletPublicId).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.sparkPublicKeyHex)
    // Bound to the recovered shared-root profile.
    expect(status.profileId).toBe("openagents.legacy_unified_nostr_spark.v1")
    // STATUS-ONLY, no send path admitted.
    expect(status.mode).toBe("status_only")
    expect(status.sendEnabled).toBe(false)
    expect(status.reachability).toBe("status_only")
    // The public projection decodes against its own strict schema.
    expect(decodeSparkWalletStatus(status)).not.toBeNull()
  })

  test("FAILS CLOSED on a wrong seed — no wallet bucket, no mint", () => {
    const adapter = makeSparkStatusAdapter({ expected: expectedRust })
    const error = expectFailure(runOpen(adapter, SECOND_TEST_MNEMONIC))
    expect(error).toBeInstanceOf(SparkStatusError)
    expect(error.reason).toBe("expected_wallet_mismatch")
    // The recovered public identity of the wrong seed differs from the expected
    // wallet, so the adapter refuses rather than opening a fresh bucket.
    const wrong = deriveSovereignIdentityPublic(SECOND_TEST_MNEMONIC)
    expect(wrong.sparkBip32FingerprintHex).not.toBe(expectedRust.fingerprintHex)
  })

  test("a deferred (LDK) comparison adapter can never open a wallet", () => {
    const adapter = makeSparkStatusAdapter({
      comparisonAdapter: ldkSparkComparisonAdapter,
      expected: { adapter: "ldk", fingerprintHex: "00000000" },
    })
    expect(expectFailure(runOpen(adapter, PUBLIC_TEST_MNEMONIC)).reason).toBe("deferred")
  })

  test("opens under an injected Breez seam and matches its expected fingerprint", () => {
    // A deterministic OFFLINE stand-in for the bounded Breez SDK deriver. The real
    // online Breez SDK link is a deferred owner-attended step.
    const testBreezDeriver: BreezSeedFingerprintDeriver = {
      deriveSeedFingerprint: (seed) =>
        Effect.succeed({
          fingerprintHex: bytesToHex(sha256(seed).slice(0, 4)),
          publicKeyHex: bytesToHex(sha256(seed)),
        }),
    }
    const breezAdapter = makeBreezSparkComparisonAdapter(testBreezDeriver)
    // Derive the expected Breez fingerprint the same bounded way, from the seed.
    const expectedBreez = Effect.runSync(
      breezAdapter.deriveFingerprint(secretFor(PUBLIC_TEST_MNEMONIC)),
    )
    const adapter = makeSparkStatusAdapter({
      comparisonAdapter: breezAdapter,
      expected: {
        adapter: "breez_spark",
        fingerprintHex: expectedBreez.fingerprintHex,
        publicKeyHex: expectedBreez.publicKeyHex,
      },
    })
    expect(expectSuccess(runOpen(adapter, PUBLIC_TEST_MNEMONIC)).adapter).toBe("breez_spark")
  })

  test("the convenience open derives the expected wallet from the recovered seed", () => {
    const status = openRecoveredSparkWalletStatus(PUBLIC_TEST_MNEMONIC)
    expect(status).not.toBeNull()
    expect(status?.mode).toBe("status_only")
    expect(status?.walletFingerprint).toBe(
      PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.sparkBip32FingerprintHex,
    )
  })

  test("tripwire: no mnemonic word or seed byte appears in a status or an error", () => {
    const adapter = makeSparkStatusAdapter({ expected: expectedRust })
    const ok = expectSuccess(runOpen(adapter, PUBLIC_TEST_MNEMONIC))
    const bad = expectFailure(runOpen(adapter, SECOND_TEST_MNEMONIC))
    for (const serialized of [JSON.stringify(ok), JSON.stringify(bad), String(bad)]) {
      expect(serialized.includes("abandon")).toBe(false)
      expect(serialized.includes("about")).toBe(false)
      expect(serialized.includes("zoo")).toBe(false)
      expect(serialized.includes("mnemonic")).toBe(false)
      expect(serialized.includes("nsec")).toBe(false)
      expect(serialized.includes("seed")).toBe(false)
    }
  })

  describe("live balance is an online action — gated, never run offline", () => {
    const status = openRecoveredSparkWalletStatus(PUBLIC_TEST_MNEMONIC) as SparkWalletStatus

    const runGate = (online: boolean, ownerAttended: boolean): SparkStatusError => {
      const result = Effect.runSync(
        Effect.result(gateSparkLiveBalance(status, { online, ownerAttended })),
      )
      if (!Result.isFailure(result)) throw new Error("expected the gate to fail")
      return result.failure
    }

    test("fails closed without the owner-attended online flags", () => {
      expect(runGate(false, false).reason).toBe("online_action_gated")
    })

    test("even when gated on, the real online SDK link stays deferred", () => {
      expect(runGate(true, true).reason).toBe("deferred")
    })
  })
})
