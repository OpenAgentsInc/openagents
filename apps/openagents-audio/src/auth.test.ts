import { expect, test } from "vite-plus/test"
import { mintAudioGrant, verifyAudioGrant } from "./auth"
import { identity } from "./test-support"
const secret = "s".repeat(32)
test("short-lived audio grants bind exact identity", () => {
  const token = mintAudioGrant({ identity, expiresAtMs: 2_000 }, secret)
  expect(verifyAudioGrant(token, secret, 1_000)?.identity).toEqual(identity)
  expect(verifyAudioGrant(`${token}x`, secret, 1_000)).toBeUndefined()
  expect(verifyAudioGrant(token, secret, 2_001)).toBeUndefined()
})
