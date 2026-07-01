import { describe, expect, test } from "bun:test"

import {
  buildCompanionAuthenticated,
  buildCompanionAuth,
  buildCompanionHello,
  buildCompanionPairingOffer,
  buildCompanionReady,
  companionAllowedMethods,
  decryptCompanionFrame,
  deriveCompanionSessionKey,
  encryptCompanionFrame,
  generateCompanionKeyPair,
} from "./companion-e2ee.js"

describe("companion E2EE protocol", () => {
  test("projects a public-safe pairing offer without the one-time secret", async () => {
    const server = await generateCompanionKeyPair()
    const offer = buildCompanionPairingOffer({
      relayUrl: "https://openagents.com/pylon/relay",
      bootstrapId: "bootstrap.fixture.0001",
      serverPublicKey: server.publicKey,
      projectionLevel: "public_safe",
      capabilities: ["observe_public", "read_artifact"],
      expiresAt: "2026-06-30T12:00:00.000Z",
    })

    expect(offer.protocol).toBe("openagents.companion.e2ee.v1")
    expect(offer.allowedMethods).toContain("session.subscribe")
    expect(offer.allowedMethods).toContain("artifact.read")
    expect(offer.allowedMethods).not.toContain("turn.steer")
    expect(JSON.stringify(offer)).not.toContain("pairing-secret.fixture")
  })

  test("derives the same AES session key and decrypts an encrypted relay frame", async () => {
    const server = await generateCompanionKeyPair()
    const client = await generateCompanionKeyPair()
    const pairingRef = "pairing.fixture.0001"
    const clientId = "ios.fixture"
    const serverKey = await deriveCompanionSessionKey({
      privateKey: server.privateKey,
      peerPublicKey: client.publicKey,
      pairingRef,
      clientId,
    })
    const clientKey = await deriveCompanionSessionKey({
      privateKey: client.privateKey,
      peerPublicKey: server.publicKey,
      pairingRef,
      clientId,
    })

    const frame = await encryptCompanionFrame({
      key: clientKey,
      pairingRef,
      sequence: 7,
      message: {
        kind: "rpc",
        method: "turn.steer",
        clientRequestId: "client.req.7",
        payload: { sessionRef: "session.1", instruction: "continue with tests" },
      },
    })
    const decrypted = await decryptCompanionFrame({ key: serverKey, frame })

    expect(frame.type).toBe("e2ee_frame")
    expect(frame.ciphertext).not.toContain("continue with tests")
    expect(decrypted).toEqual({
      kind: "rpc",
      method: "turn.steer",
      clientRequestId: "client.req.7",
      payload: { sessionRef: "session.1", instruction: "continue with tests" },
    })
  })

  test("binds ciphertext to pairingRef and sequence through authenticated data", async () => {
    const server = await generateCompanionKeyPair()
    const client = await generateCompanionKeyPair()
    const key = await deriveCompanionSessionKey({
      privateKey: server.privateKey,
      peerPublicKey: client.publicKey,
      pairingRef: "pairing.fixture.0001",
      clientId: "ios.fixture",
    })
    const frame = await encryptCompanionFrame({
      key,
      pairingRef: "pairing.fixture.0001",
      sequence: 1,
      message: { kind: "event", event: "turn.completed", payload: { sessionRef: "session.1" } },
    })

    await expect(
      decryptCompanionFrame({
        key,
        frame: { ...frame, sequence: 2 },
      }),
    ).rejects.toThrow()
  })

  test("builds handshake frames with capability-derived method allowlists", () => {
    const hello = buildCompanionHello({
      bootstrapId: "bootstrap.fixture.0001",
      clientId: "ios.fixture",
      deviceClass: "mobile",
      clientPublicKey: "client-public-key",
    })
    const ready = buildCompanionReady({
      pairingRef: "pairing.fixture.0001",
      serverPublicKey: "server-public-key",
      capabilities: ["observe_public", "send_instruction"],
    })
    const auth = buildCompanionAuth({
      pairingRef: "pairing.fixture.0001",
      frame: {
        type: "e2ee_frame",
        protocol: "openagents.companion.e2ee.v1",
        pairingRef: "pairing.fixture.0001",
        sequence: 1,
        nonce: "nonce",
        ciphertext: "ciphertext",
      },
    })
    const authenticated = buildCompanionAuthenticated({
      pairingRef: "pairing.fixture.0001",
      capabilities: ["observe_public", "send_instruction"],
      expiresAt: "2026-06-30T12:00:00.000Z",
    })

    expect(hello.type).toBe("e2ee_hello")
    expect(ready.acceptedMethods).toContain("session.subscribe")
    expect(ready.acceptedMethods).toContain("turn.steer")
    expect(ready.acceptedMethods).not.toContain("deploy.cloud")
    expect(auth.type).toBe("e2ee_auth")
    expect(authenticated.acceptedMethods).toEqual(ready.acceptedMethods)
  })

  test("keeps read-only and steer-capable clients on separate method surfaces", () => {
    expect(companionAllowedMethods(["observe_public", "read_artifact"])).toEqual([
      "bridge.clients.list",
      "session.list",
      "session.subscribe",
      "session.snapshot",
      "session.history",
      "artifact.read",
      "capability.list",
    ])
    expect(companionAllowedMethods(["observe_public", "spawn_session", "send_instruction"])).toEqual([
      "bridge.clients.list",
      "session.list",
      "session.subscribe",
      "session.snapshot",
      "session.history",
      "capability.list",
      "turn.steer",
      "session.spawn",
      "intent.submit",
    ])
  })
})
