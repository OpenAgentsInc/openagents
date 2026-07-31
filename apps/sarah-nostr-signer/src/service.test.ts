import {
  generateSecretKeyBytes,
  generateSarahNostrSigner,
  publicKeyFromSecret,
  verifySignedEvent,
  type SarahNostrSignedEvent,
} from "@openagentsinc/sarah/nostr-identity"
import { describe, expect, it } from "vite-plus/test"

import {
  SARAH_GROUP_TEXT_KIND,
  SARAH_PRESENCE_KIND,
  SARAH_SIGNING_REQUEST_SCHEMA,
  buildSigningTemplate,
  createStableSarahSigner,
  makeSarahNostrSignerHandler,
  parseCommunityAllowlist,
  type SarahSignerTemplate,
} from "./service.ts"

const now = 1_800_000_000
const allowlist = parseCommunityAllowlist(
  JSON.stringify([
    { groupRef: "openagents-public", channelRefs: ["agent-chat"] },
  ]),
)

const request = (template: SarahSignerTemplate, overrides: object = {}) =>
  new Request("https://signer.internal/v1/sign", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      schemaVersion: SARAH_SIGNING_REQUEST_SCHEMA,
      template,
      ...overrides,
    }),
  })

const presence = (): Extract<SarahSignerTemplate, { type: "presence" }> => ({
  type: "presence",
  createdAt: now,
  expiresAt: now + 120,
  groupRef: "openagents-public",
  channelRef: "agent-chat",
  presenceLeaseRef: "presence:one",
  roomEpochDigest: "a".repeat(64),
  sessionDigest: "b".repeat(64),
  generation: 1,
  membershipRevision: "c".repeat(64),
  e2eeKeyRevision: "d".repeat(64),
  admissionDigest: "e".repeat(64),
  authorityDigest: "f".repeat(64),
  status: "active",
})

const signedEvent = async (
  response: Response,
  template: SarahSignerTemplate,
): Promise<SarahNostrSignedEvent> => {
  const body = (await response.json()) as {
    eventId: string
    pubkey: string
    signature: string
  }
  const unsigned = buildSigningTemplate(template)
  return {
    id: body.eventId,
    pubkey: body.pubkey,
    sig: body.signature,
    ...unsigned,
  }
}

describe("Sarah Nostr signing service", () => {
  it("binds the sealed signer to the configured stable public key", () => {
    const secretKey = generateSecretKeyBytes()
    const secretMaterial = Buffer.from(secretKey).toString("hex")
    const expectedPubkey = publicKeyFromSecret(secretKey)
    const signer = createStableSarahSigner({
      secretMaterial,
      expectedPubkey,
    })
    expect(signer.getPublicKey()).toBe(expectedPubkey)
    expect(Object.keys(signer).sort()).toEqual([
      "getPublicIdentity",
      "getPublicKey",
      "signEvent",
    ])
    expect(() =>
      createStableSarahSigner({
        secretMaterial,
        expectedPubkey: "a".repeat(64),
      }),
    ).toThrow(/does not match/)
  })

  it("signs only the canonical public-safe presence template", async () => {
    const signer = generateSarahNostrSigner()
    const handle = makeSarahNostrSignerHandler({
      signer,
      allowlist,
      relayUrl: "wss://relay.openagents.com",
      nowSeconds: () => now,
    })
    const template = presence()
    const response = await handle(request(template))
    expect(response.status).toBe(200)
    expect(response.headers.get("cache-control")).toBe("no-store")
    const body = await response.clone().json()
    expect(Object.keys(body).sort()).toEqual([
      "eventId",
      "pubkey",
      "schemaVersion",
      "signature",
    ])
    expect(JSON.stringify(body)).not.toContain("presence:one")

    const event = await signedEvent(response, template)
    expect(event.kind).toBe(SARAH_PRESENCE_KIND)
    expect(event.tags).toContainEqual(["principal", "principal.sarah"])
    expect(event.tags).toContainEqual(["participant", "principal.sarah"])
    expect(verifySignedEvent(event)).toBe(true)
  })

  it("signs a kind-9 projection with explicit non-authority tags", async () => {
    const handle = makeSarahNostrSignerHandler({
      signer: generateSarahNostrSigner(),
      allowlist,
      relayUrl: "wss://relay.openagents.com",
      nowSeconds: () => now,
    })
    const template: SarahSignerTemplate = {
      type: "kind9_projection",
      createdAt: now,
      groupRef: "openagents-public",
      channelRef: "agent-chat",
      messageRef: "message:one",
      presenceLeaseRef: "presence:one",
      generation: 1,
      content: "A public-safe Sarah room projection.",
    }
    const response = await handle(request(template))
    expect(response.status).toBe(200)
    const event = await signedEvent(response, template)
    expect(event.kind).toBe(SARAH_GROUP_TEXT_KIND)
    expect(event.tags).toContainEqual(["h", "openagents-public"])
    expect(event.tags).toContainEqual(["authority", "projection_only"])
    expect(verifySignedEvent(event)).toBe(true)
  })

  it("signs NIP-42 authentication only for the configured relay", async () => {
    const handle = makeSarahNostrSignerHandler({
      signer: generateSarahNostrSigner(),
      allowlist,
      relayUrl: "wss://relay.openagents.com",
      nowSeconds: () => now,
    })
    const template: SarahSignerTemplate = {
      type: "relay_auth",
      createdAt: now,
      relayUrl: "wss://relay.openagents.com",
      challenge: "relay-challenge",
    }
    const response = await handle(request(template))
    expect(response.status).toBe(200)
    const event = await signedEvent(response, template)
    expect(event.kind).toBe(22_242)
    expect(event.tags).toEqual([
      ["relay", "wss://relay.openagents.com"],
      ["challenge", "relay-challenge"],
    ])
    expect(verifySignedEvent(event)).toBe(true)
    expect(
      (
        await handle(
          request({
            ...template,
            relayUrl: "wss://attacker.example",
          }),
        )
      ).status,
    ).toBe(403)
  })

  it("rejects arbitrary event fields, groups, and secret-shaped content", async () => {
    const signer = generateSarahNostrSigner()
    const signEvent = signer.signEvent
    let calls = 0
    const handle = makeSarahNostrSignerHandler({
      signer: { ...signer, signEvent: template => {
        calls += 1
        return signEvent(template)
      } },
      allowlist,
      relayUrl: "wss://relay.openagents.com",
      nowSeconds: () => now,
    })

    const arbitrary = {
      ...presence(),
      kind: 1,
      tags: [["admin", "true"]],
    }
    expect(
      (
        await handle(
          request(arbitrary as unknown as SarahSignerTemplate),
        )
      ).status,
    ).toBe(400)
    expect(
      (
        await handle(
          request({ ...presence(), groupRef: "another-group" }),
        )
      ).status,
    ).toBe(403)
    expect(
      (
        await handle(
          request({
            type: "kind9_projection",
            createdAt: now,
            groupRef: "openagents-public",
            channelRef: "agent-chat",
            messageRef: "message:one",
            presenceLeaseRef: "presence:one",
            generation: 1,
            content: "Authorization: Bearer a-secret-value-that-must-not-sign",
          }),
        )
      ).status,
    ).toBe(400)
    expect(
      (
        await handle(
          request({
            type: "kind9_projection",
            createdAt: now,
            groupRef: "openagents-public",
            channelRef: "agent-chat",
            messageRef: "message:one",
            presenceLeaseRef: "presence:one",
            generation: 1,
            content: `sk-${"a".repeat(32)}`,
          }),
        )
      ).status,
    ).toBe(400)
    expect(calls).toBe(0)
  })

  it("rejects stale, overlong, and malformed presence bindings", async () => {
    const handle = makeSarahNostrSignerHandler({
      signer: generateSarahNostrSigner(),
      allowlist,
      relayUrl: "wss://relay.openagents.com",
      nowSeconds: () => now,
    })
    expect(
      (await handle(request({ ...presence(), createdAt: now - 61 }))).status,
    ).toBe(409)
    expect(
      (await handle(request({ ...presence(), expiresAt: now + 1_801 }))).status,
    ).toBe(400)
    expect(
      (
        await handle(
          request({ ...presence(), roomEpochDigest: "not-a-digest" }),
        )
      ).status,
    ).toBe(400)
  })

  it("has no key or identity accessor route", async () => {
    const handle = makeSarahNostrSignerHandler({
      signer: generateSarahNostrSigner(),
      allowlist,
      relayUrl: "wss://relay.openagents.com",
      nowSeconds: () => now,
    })
    for (const path of ["/v1/key", "/v1/identity", "/v1/secret"]) {
      const response = await handle(
        new Request(`https://signer.internal${path}`),
      )
      expect(response.status).toBe(404)
    }
  })

  it("rejects an undeclared oversized body before signing", async () => {
    const signer = generateSarahNostrSigner()
    let calls = 0
    const handle = makeSarahNostrSignerHandler({
      signer: {
        ...signer,
        signEvent: template => {
          calls += 1
          return signer.signEvent(template)
        },
      },
      allowlist,
      relayUrl: "wss://relay.openagents.com",
      nowSeconds: () => now,
    })
    const response = await handle(
      new Request("https://signer.internal/v1/sign", {
        method: "POST",
        body: JSON.stringify({
          schemaVersion: SARAH_SIGNING_REQUEST_SCHEMA,
          template: {
            type: "kind9_projection",
            createdAt: now,
            groupRef: "openagents-public",
            channelRef: "agent-chat",
            messageRef: "message:one",
            presenceLeaseRef: "presence:one",
            generation: 1,
            content: "x".repeat(17_000),
          },
        }),
      }),
    )
    expect(response.status).toBe(400)
    expect(calls).toBe(0)
  })

  it("fails closed on ambiguous community allowlists", () => {
    expect(() => parseCommunityAllowlist("[]")).toThrow()
    expect(() =>
      parseCommunityAllowlist(
        JSON.stringify([
          {
            groupRef: "openagents-public",
            channelRefs: ["agent-chat", "agent-chat"],
          },
        ]),
      ),
    ).toThrow()
    expect(() =>
      parseCommunityAllowlist(
        JSON.stringify([
          {
            groupRef: "openagents-public",
            channelRefs: ["agent-chat"],
            wildcard: true,
          },
        ]),
      ),
    ).toThrow()
  })
})
