import { Schema as S } from "effect"
import { describe, expect, it } from "vitest"

import {
  PUBLIC_CHAT_ACCEPTED_KINDS,
  PUBLIC_CHAT_PROFILE,
  PUBLIC_CHAT_RICH_CONTENT_PROFILE,
  PublicNostrChatManifest,
  publicNostrChatManifest,
} from "./index.js"

describe("public Nostr chat manifest", () => {
  it("fails closed when the relay does not publish its self key", () => {
    const manifest = publicNostrChatManifest()
    expect(S.is(PublicNostrChatManifest)(manifest)).toBe(true)
    expect(manifest.readiness).toBe("relay-self-required")
    expect(manifest.group.naddr).toBeNull()
    expect(manifest.relay.selfPubkey).toBeNull()
  })

  it("publishes one consistent ready profile when relay self is configured", () => {
    const manifest = publicNostrChatManifest("c".repeat(64))
    expect(manifest.readiness).toBe("ready")
    expect(manifest.group.naddr).toMatch(/^naddr1/)
    expect(manifest.group.nostrUri).toBe(`nostr:${manifest.group.naddr}`)
    expect(manifest.acceptedKinds).toEqual([...PUBLIC_CHAT_ACCEPTED_KINDS])
    expect(manifest.profileVersion).toBe(PUBLIC_CHAT_PROFILE)
    expect(manifest.richContentProfileVersion).toBe(
      PUBLIC_CHAT_RICH_CONTENT_PROFILE,
    )
    expect(manifest.auth.applicationSession).toBe("none")
    expect(manifest.auth.defaultSigner).toBe("local-nak-key")
    expect(manifest.auth.externalSignerRequired).toBe(false)
    expect(manifest.auth.signers[0]).toBe("local-nak-key")
    expect(manifest.agentPolicy.sharedBotSecret).toBe(false)
  })
})
