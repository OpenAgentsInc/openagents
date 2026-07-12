import type { DesktopVoiceState } from "../voice-host.ts"

export type VoiceModeState = Readonly<{
  host: DesktopVoiceState
  sessionRef: string | null
  disclosureAccepted: boolean
  errorText: string | null
}>

export const idleVoiceModeState = (): VoiceModeState => ({
  host: { protocolVersion: 1, phase: "idle", generation: 0, nextSequence: 0, acknowledgedSequence: 0, capture: false, egress: false, playback: false, retainedAudio: false, activity: "stopped" },
  sessionRef: null,
  disclosureAccepted: false,
  errorText: null,
})

export const withVoiceHostState = (current: VoiceModeState, host: DesktopVoiceState): VoiceModeState => ({
  ...current,
  host,
  errorText: host.phase === "denied" ? "Microphone permission was denied. Text remains available."
    : host.phase === "offline" ? "Voice connection was lost. Reconnecting without capture."
    : host.phase === "backpressured" ? "Audio delivery is paused. Text remains available."
    : host.phase === "device_changed" ? "The microphone changed. Voice is paused until you resume."
    : host.phase === "revoked" ? "Voice access was revoked. No audio is leaving this device."
    : host.phase === "failed" ? "Voice media failed. Continue with text or restart voice."
    : null,
})

export const voiceActive = (state: VoiceModeState): boolean =>
  !["idle", "denied", "revoked", "failed"].includes(state.host.phase)

export const voiceIndicatorText = (state: VoiceModeState): Readonly<{
  capture: string; egress: string; retention: string; playback: string; status: string
}> => ({
  capture: state.host.capture ? "Mic capturing" : "Mic off",
  egress: state.host.egress ? "Audio sending" : "Not sending",
  retention: state.host.retainedAudio ? "Audio retained" : "Not retained",
  playback: state.host.playback ? "Reply audio on" : "Playback off",
  status: ({
    stopped: "Voice stopped", permission: "Requesting microphone permission", connecting: "Connecting voice", listening: "Listening", speech_detected: "Speech detected", transcribing: "Transcribing", awaiting_confirmation: "Awaiting confirmation", executing: "Executing confirmed action", speaking: "Speaking", muted: "Muted", reconnecting: "Reconnecting", degraded: "Voice degraded", revoked: "Voice revoked",
  } as const)[state.host.activity],
})

export const voiceBehaviorContractIds = [
  "openagents_voice.capture_egress_retention_truth.v1",
  "openagents_voice.mute_and_stop_fail_closed.v1",
  "openagents_voice.visible_text_fallback_and_media_failure.v1",
  "openagents_voice.explicit_command_confirmation.v1",
] as const
