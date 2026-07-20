import { Schema } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  deriveIdentityRef,
  PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE,
  PUBLIC_TEST_MNEMONIC,
  resolveLocalIdentityPublic,
  WebPublicIdentity,
  webSignerBridgeFromSigner,
} from "@openagentsinc/sovereign-identity"
import { deriveNip06Identity } from "@openagentsinc/pylon-core/shared/nostr-identity"

import { projectDesktopIdentity } from "./desktop-identity.ts"

/**
 * IDR-08 ONE-IDENTITY PROOF.
 *
 * The whole point of IDR-08 is that Pylon and Desktop stop deriving identity in
 * parallel and both CONSUME the one shared `@openagentsinc/sovereign-identity`
 * service — so one mnemonic yields ONE `identityRef` and ONE `npub` across every
 * admitted local surface, and the web surface receives no raw key.
 *
 * LOCAL-ONLY, fixture-only (IDR-00 public mnemonic). No network, no Keychain, no
 * secret in output.
 */
describe("IDR-08 one identity across Pylon + Desktop from the shared service", () => {
  // The shared service is the source of truth for the one identity.
  const shared = resolveLocalIdentityPublic(PUBLIC_TEST_MNEMONIC)

  // The Pylon consumer resolves through the shared service (deriveNip06Identity
  // → resolveLocalIdentityPublic). The path is irrelevant to identity.
  const pylon = deriveNip06Identity(PUBLIC_TEST_MNEMONIC, "/dev/null/identity.mnemonic")

  // The Desktop consumer projects the SAME Pylon identity for its boot host.
  const desktop = projectDesktopIdentity(pylon, "rehydrated")

  test("one identityRef resolves across the shared service, Pylon, and Desktop", () => {
    expect(shared.identityRef).toBe(deriveIdentityRef(PUBLIC_TEST_MNEMONIC))
    expect(pylon.identityRef).toBe(shared.identityRef)
    expect(desktop.identityRef).toBe(shared.identityRef)
    // There is exactly one distinct identityRef across every surface.
    expect(new Set([shared.identityRef, pylon.identityRef, desktop.identityRef]).size).toBe(1)
  })

  test("one npub resolves across the shared service, Pylon, and Desktop", () => {
    expect(shared.npub).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.npub)
    expect(pylon.npub).toBe(shared.npub)
    expect(desktop.npub).toBe(shared.npub)
    expect(new Set([shared.npub, pylon.npub, desktop.npub]).size).toBe(1)
    // The canonical identityRef equals the npub under the frozen profile.
    expect(shared.identityRef).toBe(shared.npub)
  })

  test("the surfaces agree on the public Spark fingerprint and profile", () => {
    expect(pylon.sparkFingerprint).toBe(shared.sparkFingerprint)
    expect(desktop.walletFingerprint).toBe(shared.sparkFingerprint)
    expect(pylon.profileId).toBe(shared.profileId)
    expect(desktop.profileId).toBe(shared.profileId)
  })

  test("the web seam carries only public identity + signer ops — no raw key", () => {
    const identity = Schema.decodeUnknownSync(WebPublicIdentity)({
      identityRef: shared.identityRef,
      npub: shared.npub,
      pubkey: shared.publicKey,
      profileId: shared.profileId,
    })
    const bridge = webSignerBridgeFromSigner(identity, shared.signer)
    // The browser-facing bridge exposes public identity + signer operations only.
    expect(Object.keys(bridge).sort()).toEqual([
      "getPublicKey",
      "identity",
      "nip44Decrypt",
      "nip44Encrypt",
      "signEvent",
    ])
    // No raw-key-shaped field crosses to the browser.
    const serialized = JSON.stringify({ identity: bridge.identity })
    expect(serialized.includes(PUBLIC_TEST_MNEMONIC)).toBe(false)
    expect(serialized.includes("nsec")).toBe(false)
    expect(serialized.includes("mnemonic")).toBe(false)
    expect(serialized.includes("privateKey")).toBe(false)
    expect(serialized.includes("seed")).toBe(false)
    // The bridge still resolves the one identity.
    expect(bridge.identity.identityRef).toBe(shared.identityRef)
  })
})
