export type DesktopMediaPermission = "media" | "microphone" | "camera" | "display-capture" | string

/** Electron main calls this only for a trusted OpenAgents window after an explicit voice.start. */
export const decideDesktopMediaPermission = (input: Readonly<{
  permission: DesktopMediaPermission
  requestingOrigin: string
  trustedOrigin: string
  explicitVoiceStartPending: boolean
}>): boolean => input.permission === "microphone" &&
  input.requestingOrigin === input.trustedOrigin &&
  input.explicitVoiceStartPending
