import { describe, expect, test } from "vite-plus/test"

import {
  PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE,
  PUBLIC_TEST_MNEMONIC,
} from "@openagentsinc/sovereign-identity"

import { decodeIdentityStatus } from "./identity-contract.ts"
import { createIdentityHost, deriveNostrPublicIdentity, type IdentityLoader } from "./identity-host.ts"

/**
 * IDR-BS #9103 host tests. They use ONLY IDR-00's published test mnemonic and
 * its frozen public vectors — never a real secret, and never a secret in output.
 */
const loaderFor = (over: Partial<{ source: "rehydrated" | "created"; mnemonic: string; fail: boolean; calls: { n: number } }>): IdentityLoader => ({
  loadOrCreate: async () => {
    if (over.calls) over.calls.n += 1
    if (over.fail) throw new Error("loader boom")
    return { source: over.source ?? "rehydrated", mnemonic: over.mnemonic ?? PUBLIC_TEST_MNEMONIC }
  },
})

describe("identity host (IDR-BS #9103)", () => {
  test("derives the frozen IDR-00 npub + Spark fingerprint from the public test mnemonic", async () => {
    const status = await createIdentityHost(loaderFor({ source: "rehydrated" })).status()
    expect(status.status).toBe("available")
    expect(status.npub).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.npub)
    expect(status.walletFingerprint).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.sparkBip32FingerprintHex)
    expect(status.source).toBe("rehydrated")
    expect(status.profileId).toBe("openagents.legacy_unified_nostr_spark.v1")
  })

  test("the thin Nostr seam derives the frozen npub + pubkey vectors", () => {
    const nostr = deriveNostrPublicIdentity(PUBLIC_TEST_MNEMONIC)
    expect(nostr.npub).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.npub)
    expect(nostr.pubkey).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.nostrPublicKeyHex)
  })

  test("a missing mnemonic (created) passes the source through unchanged", async () => {
    const status = await createIdentityHost(loaderFor({ source: "created" })).status()
    expect(status.source).toBe("created")
    expect(status.status).toBe("available")
  })

  test("the projection carries NO secret material — only public identifiers", async () => {
    const status = await createIdentityHost(loaderFor({})).status()
    const serialized = JSON.stringify(status)
    // The mnemonic must never appear anywhere in the renderer-facing projection.
    expect(serialized.includes(PUBLIC_TEST_MNEMONIC)).toBe(false)
    expect(serialized.includes("abandon")).toBe(false)
    // No nsec / private-key / seed shaped fields.
    expect(serialized.includes("nsec")).toBe(false)
    expect(serialized.includes("privateKey")).toBe(false)
    expect(serialized.includes("mnemonic")).toBe(false)
    expect(serialized.includes("seed")).toBe(false)
    // Only the documented public keys are present.
    expect(Object.keys(status).sort()).toEqual(
      ["npub", "profileId", "schema", "source", "status", "walletFingerprint"],
    )
    // The public projection still decodes against the strict IPC schema.
    expect(decodeIdentityStatus(status)).not.toBeNull()
  })

  test("is fail-soft: a loader error yields the unavailable projection, never a throw", async () => {
    const status = await createIdentityHost(loaderFor({ fail: true })).status()
    expect(status.status).toBe("unavailable")
    expect(status.npub).toBeNull()
    expect(status.walletFingerprint).toBeNull()
    expect(status.source).toBeNull()
  })

  test("memoizes a successful derivation (loader runs once)", async () => {
    const calls = { n: 0 }
    const host = createIdentityHost(loaderFor({ calls }))
    await host.status()
    await host.status()
    expect(calls.n).toBe(1)
  })
})
