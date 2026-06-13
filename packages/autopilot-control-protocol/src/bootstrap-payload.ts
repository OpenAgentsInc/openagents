import { Schema as S } from "effect"

import { Capability } from "./bridge"
import { ProjectionLevel } from "./control"

// RN-safe: this module is bundled by Metro (mobile) as well as Bun/Node, so it
// must avoid `node:` builtins and `Buffer`. Use globals + a pure base64url codec.
const textEncoder = new TextEncoder()
const textDecoder = new TextDecoder("utf-8", { fatal: true })

const B64URL = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"

function base64urlEncode(bytes: Uint8Array): string {
  let out = ""
  for (let i = 0; i < bytes.length; i += 3) {
    const b0 = bytes[i]
    const has1 = i + 1 < bytes.length
    const has2 = i + 2 < bytes.length
    const b1 = has1 ? bytes[i + 1] : 0
    const b2 = has2 ? bytes[i + 2] : 0
    out += B64URL[b0 >> 2] + B64URL[((b0 & 3) << 4) | (b1 >> 4)]
    if (has1) out += B64URL[((b1 & 15) << 2) | (b2 >> 6)]
    if (has2) out += B64URL[b2 & 63]
  }
  return out
}

function base64urlDecode(str: string): Uint8Array {
  const lookup = new Map<string, number>()
  for (let i = 0; i < B64URL.length; i++) lookup.set(B64URL[i], i)
  const bytes: number[] = []
  for (let i = 0; i < str.length; i += 4) {
    const c0 = lookup.get(str[i]) ?? 0
    const c1 = lookup.get(str[i + 1]) ?? 0
    const c2 = str[i + 2] === undefined ? undefined : lookup.get(str[i + 2])
    const c3 = str[i + 3] === undefined ? undefined : lookup.get(str[i + 3])
    bytes.push((c0 << 2) | (c1 >> 4))
    if (c2 !== undefined) bytes.push(((c1 & 15) << 4) | (c2 >> 2))
    if (c3 !== undefined) bytes.push(((c2 & 3) << 6) | c3)
  }
  return new Uint8Array(bytes)
}

export const BootstrapPayload = S.Struct({
  version: S.Literal(1),
  addresses: S.Struct({
    loopback: S.optional(S.String),
    lan: S.optional(S.String),
    tailnet: S.optional(S.String),
  }).pipe(
    S.refine((addresses) =>
      typeof addresses.loopback === "string"
        || typeof addresses.lan === "string"
        || typeof addresses.tailnet === "string", {
      message: () => "at least one bootstrap address is required",
    }),
  ),
  bootstrapId: S.String,
  secret: S.String,
  projectionLevel: ProjectionLevel,
  capabilities: S.Array(Capability),
})
export type BootstrapPayload = typeof BootstrapPayload.Type

const decodePayload = S.decodeUnknownSync(BootstrapPayload)

export function encodeBootstrapPayload(payload: BootstrapPayload): string {
  const decoded = decodePayload(payload)
  const json = JSON.stringify(decoded)
  return base64urlEncode(textEncoder.encode(json))
}

export function decodeBootstrapPayload(encoded: string): BootstrapPayload {
  if (!/^[A-Za-z0-9_-]+$/.test(encoded)) {
    throw new Error("Malformed bootstrap payload")
  }

  try {
    const bytes = base64urlDecode(encoded)
    const json = textDecoder.decode(bytes)
    const payload = decodePayload(JSON.parse(json))
    return redactSecretForInspection(payload)
  } catch {
    throw new Error("Malformed bootstrap payload")
  }
}

function redactSecretForInspection(payload: BootstrapPayload): BootstrapPayload {
  Object.defineProperty(payload, "secret", {
    value: payload.secret,
    enumerable: false,
    configurable: true,
    writable: true,
  })

  // Node's console.log redaction without importing node:util (RN-safe): the
  // well-known inspect-custom symbol. Harmless/unused under RN/Hermes.
  Object.defineProperty(payload, Symbol.for("nodejs.util.inspect.custom"), {
    value: () => ({ ...payload, secret: "[redacted]" }),
    enumerable: false,
    configurable: true,
  })

  return payload
}
