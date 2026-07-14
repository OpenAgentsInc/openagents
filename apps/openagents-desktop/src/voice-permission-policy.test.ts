import { expect, test } from "vite-plus/test"
import { decideDesktopMediaPermission } from "./voice-permission-policy.ts"

test("microphone permission is explicit, origin-bound, and deny-by-default", () => {
  const base = { requestingOrigin: "file://openagents", trustedOrigin: "file://openagents", explicitVoiceStartPending: true }
  expect(decideDesktopMediaPermission({ ...base, permission: "microphone" })).toBe(true)
  for (const permission of ["camera", "media", "display-capture"]) expect(decideDesktopMediaPermission({ ...base, permission })).toBe(false)
  expect(decideDesktopMediaPermission({ ...base, permission: "microphone", explicitVoiceStartPending: false })).toBe(false)
  expect(decideDesktopMediaPermission({ ...base, permission: "microphone", requestingOrigin: "https://evil.invalid" })).toBe(false)
})
