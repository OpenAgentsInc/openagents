import { describe, expect, test } from "bun:test"
import { createDesktopVoiceHost, type VoiceNativeMedia, type VoiceNativeMediaSession } from "./voice-host.ts"

const identity = { ownerRef: "owner", deviceRef: "device", threadRef: "thread", sessionRef: "session", generation: 1 }
const harness = () => {
  const events: string[] = []; let callbacks: Parameters<VoiceNativeMedia["open"]>[0] | null = null
  const media: VoiceNativeMedia = { open: input => { callbacks = input; events.push("open"); return { setCaptureEnabled: enabled => events.push(enabled ? "capture:on" : "capture:off"), close: reason => events.push(`close:${reason}`) } satisfies VoiceNativeMediaSession } }
  const host = createDesktopVoiceHost({ resolveIdentity: ({ generation }) => ({ ...identity, generation }), permission: () => "granted", requestPermission: () => "granted", media })
  return { host, events, callbacks: () => callbacks! }
}

describe("Desktop host-owned persistent voice lifecycle", () => {
  test("start, ordered packets, mute/unmute and stop preserve one generation and dispose", async () => {
    const h = harness(); await h.host.command({ protocolVersion: 1, id: "voice.start", threadRef: "thread", sessionRef: "session", disclosureRef: "disclosure.v1" }); h.callbacks().onState("live")
    h.callbacks().onPacket({ generation: 1, sequence: 0, payloadLength: 320, sha256: "a".repeat(64) })
    h.callbacks().onPacket({ generation: 1, sequence: 1, payloadLength: 320, sha256: "b".repeat(64) })
    expect(h.host.state()).toMatchObject({ phase: "live", generation: 1, nextSequence: 2, capture: true, egress: true, playback: true })
    await h.host.command({ protocolVersion: 1, id: "voice.mute" }); expect(h.events.at(-1)).toBe("capture:off"); expect(h.host.state()).toMatchObject({ capture: false, egress: false, playback: true })
    h.callbacks().onPacket({ generation: 1, sequence: 2, payloadLength: 320, sha256: "c".repeat(64) }); expect(h.host.state().nextSequence).toBe(2)
    await h.host.command({ protocolVersion: 1, id: "voice.unmute" }); expect(h.host.state()).toMatchObject({ phase: "live", generation: 1 })
    await h.host.command({ protocolVersion: 1, id: "voice.stop" }); expect(h.events.at(-1)).toBe("close:stop"); expect(h.host.state().phase).toBe("idle")
  })
  test("permission denial never opens capture", async () => {
    let opened = false
    const host = createDesktopVoiceHost({ resolveIdentity: ({ generation }) => ({ ...identity, generation }), permission: () => "denied", requestPermission: () => "denied", media: { open: () => { opened = true; throw new Error() } } })
    expect(await host.command({ protocolVersion: 1, id: "voice.start", threadRef: "thread", sessionRef: "session", disclosureRef: "d" })).toMatchObject({ phase: "denied", reason: "permission_denied" }); expect(opened).toBe(false)
  })
  test("network, backpressure, revocation, stale ACK, suspend and crash are typed", async () => {
    const h = harness(); await h.host.command({ protocolVersion: 1, id: "voice.start", threadRef: "thread", sessionRef: "session", disclosureRef: "disclosure.v1" }); h.callbacks().onState("live"); h.callbacks().onState("offline"); expect(h.host.state()).toMatchObject({ reason: "network_lost", capture: false, egress: false, playback: false })
    h.callbacks().onState("backpressured"); expect(h.host.state().reason).toBe("backpressure")
    h.callbacks().onState("device_changed"); expect(h.host.state()).toMatchObject({ phase: "device_changed", reason: "device_changed", capture: false })
    await h.host.command({ protocolVersion: 1, id: "voice.suspend" }); expect(h.host.state().phase).toBe("suspended")
    await h.host.command({ protocolVersion: 1, id: "voice.resume" }); h.callbacks().onAck(2, 99); expect(h.host.state().reason).toBe("stale_generation")
    h.callbacks().onState("revoked"); expect(h.host.state()).toMatchObject({ phase: "revoked", reason: "gateway_revoked" }); expect(h.events).toContain("close:revoke")
  })
  test("replacement and disposal finalize exactly once", async () => {
    const h = harness(); await h.host.command({ protocolVersion: 1, id: "voice.start", threadRef: "thread", sessionRef: "session", disclosureRef: "disclosure.v1" }); await h.host.command({ protocolVersion: 1, id: "voice.start", threadRef: "thread", sessionRef: "session-2", disclosureRef: "d" }); expect(h.events.filter(x => x === "close:replace")).toHaveLength(1)
    h.host.dispose(); h.host.dispose(); expect(h.events.filter(x => x === "close:shutdown")).toHaveLength(1)
  })
})
