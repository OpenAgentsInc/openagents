import { expect, test } from "bun:test"
import { createDesktopRuntimeGateway } from "../src/runtime-gateway.ts"
import { createDesktopVoiceHost, type VoiceNativeMedia } from "../src/voice-host.ts"

test("Runtime Gateway admits only bounded voice commands and public-safe projections", async () => {
  let callbacks: Parameters<VoiceNativeMedia["open"]>[0] | null = null
  const host = createDesktopVoiceHost({ resolveIdentity: ({ threadRef, sessionRef, generation }) => ({ ownerRef: "owner", deviceRef: "device", threadRef, sessionRef, generation }), permission: () => "granted", requestPermission: () => "granted", media: { open: input => { callbacks = input; return { setCaptureEnabled: () => undefined, close: () => undefined } } } })
  const gateway = createDesktopRuntimeGateway(undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, () => host)
  const events: unknown[] = []; gateway.subscribe(event => events.push(event)); gateway.start()
  const started = await gateway.request({ kind: "command", commandId: "voice-1", command: { id: "voice.start", protocolVersion: 1, threadRef: "thread", sessionRef: "session", disclosureRef: "disclosure.v1" } })
  expect(started).toMatchObject({ kind: "voice_state", state: { phase: "connecting", generation: 1 } })
  callbacks!.onState("live")
  expect(events.at(-1)).toMatchObject({ kind: "voice.lifecycle", state: { phase: "live" } })
  expect(await gateway.request({ kind: "query", requestId: "voice-state", query: { id: "voice.state" } })).toMatchObject({ kind: "voice_state", state: { phase: "live" } })
  gateway.dispose(); expect(host.state().phase).toBe("idle")
})
