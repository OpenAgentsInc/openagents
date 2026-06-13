import { inspect } from "node:util"

import { Schema as S } from "effect"

import { Capability } from "./bridge"
import { ProjectionLevel } from "./control"

const textEncoder = new TextEncoder()
const textDecoder = new TextDecoder("utf-8", { fatal: true })

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
  return Buffer.from(textEncoder.encode(json)).toString("base64url")
}

export function decodeBootstrapPayload(encoded: string): BootstrapPayload {
  if (!/^[A-Za-z0-9_-]+$/.test(encoded)) {
    throw new Error("Malformed bootstrap payload")
  }

  try {
    const bytes = Buffer.from(encoded, "base64url")
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

  Object.defineProperty(payload, inspect.custom, {
    value: () => ({ ...payload, secret: "[redacted]" }),
    enumerable: false,
    configurable: true,
  })

  return payload
}
