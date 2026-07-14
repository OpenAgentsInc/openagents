import { describe, expect, test } from "vite-plus/test"
import type { DesktopVoiceState } from "../voice-host.ts"
import { idleVoiceModeState, voiceActive, voiceBehaviorContractIds, voiceIndicatorText, withVoiceHostState } from "./voice-mode.ts"

const projected = (overrides: Partial<DesktopVoiceState>): DesktopVoiceState => ({
  protocolVersion: 1, phase: "live", generation: 1, nextSequence: 2,
  acknowledgedSequence: 1, capture: true, egress: true, playback: false,
  retainedAudio: false, activity: "listening", ...overrides,
})

describe("persistent voice renderer truth", () => {
  test("enforces the registered capture, mute, failure, and confirmation contracts", () => {
    expect(voiceBehaviorContractIds).toEqual([
      "openagents_voice.capture_egress_retention_truth.v1",
      "openagents_voice.mute_and_stop_fail_closed.v1",
      "openagents_voice.visible_text_fallback_and_media_failure.v1",
      "openagents_voice.explicit_command_confirmation.v1",
    ])
  })

  test("projects every activity as visible, stable status copy", () => {
    const activities: DesktopVoiceState["activity"][] = [
      "stopped", "permission", "connecting", "listening", "speech_detected",
      "transcribing", "awaiting_confirmation", "executing", "speaking", "muted",
      "reconnecting", "degraded", "revoked",
    ]
    for (const activity of activities) {
      expect(voiceIndicatorText(withVoiceHostState(idleVoiceModeState(), projected({ activity }))).status.length).toBeGreaterThan(3)
    }
  })

  test("never conflates capture, egress, retention, or playback", () => {
    const indicators = voiceIndicatorText(withVoiceHostState(idleVoiceModeState(), projected({
      capture: false, egress: false, retainedAudio: true, playback: true, phase: "muted", activity: "muted",
    })))
    expect(indicators).toMatchObject({ capture: "Mic off", egress: "Not sending", retention: "Audio retained", playback: "Reply audio on" })
  })

  test("keeps provisional/final and proposal/applied truth explicit", () => {
    const interim = withVoiceHostState(idleVoiceModeState(), projected({ transcript: { utteranceRef: "u1", text: "open", final: false } }))
    const final = withVoiceHostState(interim, projected({
      transcript: { utteranceRef: "u1", text: "Open files", final: true },
      proposal: { proposalRef: "p1", utteranceRef: "u1", turnRef: "turn.1", targetRef: "workspace.files", commandId: "workspace.files", expiresAtMs: 2_000_000_000_000, state: "proposed" },
      activity: "awaiting_confirmation",
    }))
    expect(interim.host.transcript?.final).toBe(false)
    expect(final.host.transcript?.final).toBe(true)
    expect(final.host.proposal?.state).toBe("proposed")
    expect(voiceIndicatorText(final).status).toBe("Awaiting confirmation")
  })

  test("failure states are inactive or degraded and always provide text fallback copy", () => {
    for (const phase of ["denied", "offline", "backpressured", "device_changed", "revoked", "failed"] as const) {
      const state = withVoiceHostState(idleVoiceModeState(), projected({ phase, capture: false, egress: false }))
      expect(state.errorText).not.toBeNull()
      if (["denied", "revoked", "failed"].includes(phase)) expect(voiceActive(state)).toBe(false)
    }
  })
})
