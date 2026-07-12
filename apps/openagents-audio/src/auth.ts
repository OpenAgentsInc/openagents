import { createHmac, timingSafeEqual } from "node:crypto"
import type { VoiceIdentity } from "@openagentsinc/audio-contract"

export type AudioGrant = Readonly<{ identity: VoiceIdentity; expiresAtMs: number }>
const encode = (value: unknown) => Buffer.from(JSON.stringify(value)).toString("base64url")
const sign = (body: string, secret: string) => createHmac("sha256", secret).update(body).digest("base64url")
export const mintAudioGrant = (grant: AudioGrant, secret: string): string => {
  const body = encode(grant); return `${body}.${sign(body, secret)}`
}
export const verifyAudioGrant = (token: string, secret: string, nowMs = Date.now()): AudioGrant | undefined => {
  const [body, signature, extra] = token.split(".")
  if (!body || !signature || extra !== undefined) return undefined
  const expected = Buffer.from(sign(body, secret)); const actual = Buffer.from(signature)
  if (expected.length !== actual.length || !timingSafeEqual(expected, actual)) return undefined
  try {
    const value = JSON.parse(Buffer.from(body, "base64url").toString("utf8")) as AudioGrant
    const i = value.identity
    if (!i || !Number.isSafeInteger(value.expiresAtMs) || value.expiresAtMs <= nowMs || value.expiresAtMs > nowMs + 15 * 60_000) return undefined
    if (![i.ownerRef, i.deviceRef, i.threadRef, i.sessionRef].every((ref) => typeof ref === "string" && ref.length > 0 && ref.length <= 256) || !Number.isSafeInteger(i.generation) || i.generation < 1) return undefined
    return value
  } catch { return undefined }
}
