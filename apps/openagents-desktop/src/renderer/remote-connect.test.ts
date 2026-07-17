import { describe, expect, test } from "vite-plus/test"

import { decodeRemoteConnectResponse, decodeRemoteConnectSnapshot } from "./remote-connect.ts"

const snapshot = {
  revision: 4,
  manifest: { enabled: true },
  environments: [{ environmentRef: "environment.safe", state: "connected", shell: "zsh", cwdRef: "cwd.safe", privateUrl: "wss://secret" }],
  remoteControl: {
    state: "connected",
    environmentRef: "environment.safe",
    installationRef: "installation.safe",
    pairing: { pairingRef: "pairing.safe", state: "pending", expiresAt: 1234, code: "secret" },
    clients: [{ clientRef: "client.safe", displayName: "Owner phone", platform: "ios", state: "granted", id: "secret" }],
  },
}

describe("remote connect renderer projection", () => {
  test("decodes only public refs and presentation fields", () => {
    const decoded = decodeRemoteConnectSnapshot(snapshot)
    expect(decoded).toMatchObject({ phase: "ready", revision: 4, manifestReady: true })
    expect(decoded?.remote.pairing).toEqual({ pairingRef: "pairing.safe", state: "pending", expiresAt: 1234 })
    expect(JSON.stringify(decoded)).not.toContain("secret")
    expect(JSON.stringify(decoded)).not.toContain("privateUrl")
  })

  test("decodes typed action envelopes and rejects malformed snapshots", () => {
    expect(decodeRemoteConnectResponse({ ok: true, snapshot }).snapshot?.remote.state).toBe("connected")
    expect(decodeRemoteConnectResponse({ ok: false, reason: "policy_denied" })).toEqual({ ok: false, reason: "policy_denied", snapshot: null })
    expect(decodeRemoteConnectSnapshot({ revision: 1 })).toBeNull()
  })
})
