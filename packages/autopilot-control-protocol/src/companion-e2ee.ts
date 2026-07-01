import { Schema as S } from "effect"

import {
  BridgeRequestVerb,
  Capability,
  verbAllowedByCapabilities,
  type Capability as CapabilityType,
  type BridgeEventName as BridgeEventNameType,
  type BridgeRequestVerb as BridgeRequestVerbType,
} from "./bridge.js"
import { ProjectionLevel, type ProjectionLevel as ProjectionLevelType } from "./control.js"

export const COMPANION_E2EE_PROTOCOL = "openagents.companion.e2ee.v1" as const
export const COMPANION_E2EE_CURVE = "P-256" as const
export const COMPANION_E2EE_CIPHER = "AES-GCM-256" as const

const textEncoder = new TextEncoder()
const textDecoder = new TextDecoder("utf-8", { fatal: true })
const B64URL = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"

export type CompanionCrypto = {
  readonly subtle: SubtleCrypto
  getRandomValues<T extends Exclude<BufferSource, ArrayBuffer>>(array: T): T
}

export type CompanionKeyPair = {
  readonly publicKey: string
  readonly privateKey: CryptoKey
}

export type CompanionSessionKeyInput = {
  readonly privateKey: CryptoKey
  readonly peerPublicKey: string
  readonly pairingRef: string
  readonly clientId: string
  readonly cryptoImpl?: CompanionCrypto
}

export type CompanionPlaintextFrame = {
  readonly kind: "rpc" | "event"
  readonly method?: BridgeRequestVerbType
  readonly event?: BridgeEventNameType
  readonly clientRequestId?: string
  readonly payload: unknown
}

export const CompanionPairingOffer = S.Struct({
  protocol: S.Literal(COMPANION_E2EE_PROTOCOL),
  relayUrl: S.String,
  bootstrapId: S.String,
  serverPublicKey: S.String,
  serverKeyAlgorithm: S.Literal(COMPANION_E2EE_CURVE),
  cipher: S.Literal(COMPANION_E2EE_CIPHER),
  projectionLevel: ProjectionLevel,
  capabilities: S.Array(Capability),
  allowedMethods: S.Array(BridgeRequestVerb),
  expiresAt: S.String,
})
export type CompanionPairingOffer = typeof CompanionPairingOffer.Type

export const CompanionHelloMessage = S.Struct({
  type: S.Literal("e2ee_hello"),
  protocol: S.Literal(COMPANION_E2EE_PROTOCOL),
  bootstrapId: S.String,
  clientId: S.String,
  deviceClass: S.String,
  clientPublicKey: S.String,
})
export type CompanionHelloMessage = typeof CompanionHelloMessage.Type

export const CompanionReadyMessage = S.Struct({
  type: S.Literal("e2ee_ready"),
  protocol: S.Literal(COMPANION_E2EE_PROTOCOL),
  pairingRef: S.String,
  serverPublicKey: S.String,
  acceptedMethods: S.Array(BridgeRequestVerb),
})
export type CompanionReadyMessage = typeof CompanionReadyMessage.Type

export const CompanionEncryptedFrame = S.Struct({
  type: S.Literal("e2ee_frame"),
  protocol: S.Literal(COMPANION_E2EE_PROTOCOL),
  pairingRef: S.String,
  sequence: S.Number,
  nonce: S.String,
  ciphertext: S.String,
})
export type CompanionEncryptedFrame = typeof CompanionEncryptedFrame.Type

export const CompanionAuthMessage = S.Struct({
  type: S.Literal("e2ee_auth"),
  protocol: S.Literal(COMPANION_E2EE_PROTOCOL),
  pairingRef: S.String,
  frame: CompanionEncryptedFrame,
})
export type CompanionAuthMessage = typeof CompanionAuthMessage.Type

export const CompanionAuthenticatedMessage = S.Struct({
  type: S.Literal("e2ee_authenticated"),
  protocol: S.Literal(COMPANION_E2EE_PROTOCOL),
  pairingRef: S.String,
  acceptedMethods: S.Array(BridgeRequestVerb),
  expiresAt: S.String,
})
export type CompanionAuthenticatedMessage = typeof CompanionAuthenticatedMessage.Type

export const CompanionHandshakeMessage = S.Union([
  CompanionHelloMessage,
  CompanionReadyMessage,
  CompanionAuthMessage,
  CompanionAuthenticatedMessage,
])
export type CompanionHandshakeMessage = typeof CompanionHandshakeMessage.Type

const decodeOffer = S.decodeUnknownSync(CompanionPairingOffer)
const decodeHello = S.decodeUnknownSync(CompanionHelloMessage)
const decodeReady = S.decodeUnknownSync(CompanionReadyMessage)
const decodeAuth = S.decodeUnknownSync(CompanionAuthMessage)
const decodeAuthenticated = S.decodeUnknownSync(CompanionAuthenticatedMessage)
const decodeEncryptedFrame = S.decodeUnknownSync(CompanionEncryptedFrame)

const COMPANION_METHOD_ALLOWLIST = [
  "bridge.clients.list",
  "session.list",
  "session.subscribe",
  "session.snapshot",
  "session.history",
  "artifact.read",
  "capability.list",
  "decision.resolve",
  "turn.steer",
  "turn.interrupt",
  "session.cancel",
  "session.pause",
  "session.resume",
  "session.spawn",
  "intent.submit",
  "coordinator.pause",
  "coordinator.resume",
  "deploy.cloud",
] as const satisfies readonly BridgeRequestVerbType[]

function cryptoOrGlobal(cryptoImpl?: CompanionCrypto): CompanionCrypto {
  const c = cryptoImpl ?? globalThis.crypto
  if (!c?.subtle || typeof c.getRandomValues !== "function") {
    throw new Error("WebCrypto is required for companion E2EE")
  }
  return c
}

function alpha(index: number): string {
  const c = B64URL[index]
  if (c === undefined) throw new Error("base64url alphabet index out of range")
  return c
}

function base64urlEncode(bytes: Uint8Array): string {
  let out = ""
  for (let i = 0; i < bytes.length; i += 3) {
    const b0 = bytes[i] ?? 0
    const has1 = i + 1 < bytes.length
    const has2 = i + 2 < bytes.length
    const b1 = has1 ? bytes[i + 1] ?? 0 : 0
    const b2 = has2 ? bytes[i + 2] ?? 0 : 0
    out += alpha(b0 >> 2) + alpha(((b0 & 3) << 4) | (b1 >> 4))
    if (has1) out += alpha(((b1 & 15) << 2) | (b2 >> 6))
    if (has2) out += alpha(b2 & 63)
  }
  return out
}

function base64urlDecode(str: string): Uint8Array {
  if (!/^[A-Za-z0-9_-]*$/.test(str)) {
    throw new Error("malformed base64url")
  }
  const lookup = new Map<string, number>()
  for (let i = 0; i < B64URL.length; i++) {
    const ch = B64URL[i]
    if (ch !== undefined) lookup.set(ch, i)
  }
  const bytes: number[] = []
  for (let i = 0; i < str.length; i += 4) {
    const s0 = str[i]
    const s1 = str[i + 1]
    const s2 = str[i + 2]
    const s3 = str[i + 3]
    const c0 = s0 === undefined ? 0 : lookup.get(s0) ?? 0
    const c1 = s1 === undefined ? 0 : lookup.get(s1) ?? 0
    const c2 = s2 === undefined ? undefined : lookup.get(s2)
    const c3 = s3 === undefined ? undefined : lookup.get(s3)
    bytes.push((c0 << 2) | (c1 >> 4))
    if (c2 !== undefined) bytes.push(((c1 & 15) << 4) | (c2 >> 2))
    if (c3 !== undefined && c2 !== undefined) bytes.push(((c2 & 3) << 6) | c3)
  }
  return new Uint8Array(bytes)
}

function aad(pairingRef: string, sequence: number): Uint8Array {
  return textEncoder.encode(`${COMPANION_E2EE_PROTOCOL}:${pairingRef}:${sequence}`)
}

function arrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(bytes.byteLength)
  copy.set(bytes)
  return copy.buffer
}

function encodedBuffer(text: string): ArrayBuffer {
  return arrayBuffer(textEncoder.encode(text))
}

export function companionAllowedMethods(capabilities: ReadonlyArray<CapabilityType>): BridgeRequestVerbType[] {
  return COMPANION_METHOD_ALLOWLIST.filter((method) => companionMethodAllowed(method, capabilities))
}

export function companionMethodAllowed(
  method: BridgeRequestVerbType,
  capabilities: ReadonlyArray<CapabilityType>,
): boolean {
  return verbAllowedByCapabilities(method, capabilities)
}

export function buildCompanionPairingOffer(input: {
  readonly relayUrl: string
  readonly bootstrapId: string
  readonly serverPublicKey: string
  readonly projectionLevel: ProjectionLevelType
  readonly capabilities: ReadonlyArray<CapabilityType>
  readonly expiresAt: string
}): CompanionPairingOffer {
  return decodeOffer({
    protocol: COMPANION_E2EE_PROTOCOL,
    relayUrl: input.relayUrl,
    bootstrapId: input.bootstrapId,
    serverPublicKey: input.serverPublicKey,
    serverKeyAlgorithm: COMPANION_E2EE_CURVE,
    cipher: COMPANION_E2EE_CIPHER,
    projectionLevel: input.projectionLevel,
    capabilities: [...input.capabilities],
    allowedMethods: companionAllowedMethods(input.capabilities),
    expiresAt: input.expiresAt,
  })
}

export function buildCompanionHello(input: {
  readonly bootstrapId: string
  readonly clientId: string
  readonly deviceClass: string
  readonly clientPublicKey: string
}): CompanionHelloMessage {
  return decodeHello({
    type: "e2ee_hello",
    protocol: COMPANION_E2EE_PROTOCOL,
    ...input,
  })
}

export function buildCompanionReady(input: {
  readonly pairingRef: string
  readonly serverPublicKey: string
  readonly capabilities: ReadonlyArray<CapabilityType>
}): CompanionReadyMessage {
  return decodeReady({
    type: "e2ee_ready",
    protocol: COMPANION_E2EE_PROTOCOL,
    pairingRef: input.pairingRef,
    serverPublicKey: input.serverPublicKey,
    acceptedMethods: companionAllowedMethods(input.capabilities),
  })
}

export function buildCompanionAuth(input: {
  readonly pairingRef: string
  readonly frame: CompanionEncryptedFrame
}): CompanionAuthMessage {
  return decodeAuth({
    type: "e2ee_auth",
    protocol: COMPANION_E2EE_PROTOCOL,
    pairingRef: input.pairingRef,
    frame: input.frame,
  })
}

export function buildCompanionAuthenticated(input: {
  readonly pairingRef: string
  readonly capabilities: ReadonlyArray<CapabilityType>
  readonly expiresAt: string
}): CompanionAuthenticatedMessage {
  return decodeAuthenticated({
    type: "e2ee_authenticated",
    protocol: COMPANION_E2EE_PROTOCOL,
    pairingRef: input.pairingRef,
    acceptedMethods: companionAllowedMethods(input.capabilities),
    expiresAt: input.expiresAt,
  })
}

export async function generateCompanionKeyPair(options: {
  readonly cryptoImpl?: CompanionCrypto
} = {}): Promise<CompanionKeyPair> {
  const c = cryptoOrGlobal(options.cryptoImpl)
  const keyPair = await c.subtle.generateKey(
    { name: "ECDH", namedCurve: COMPANION_E2EE_CURVE },
    true,
    ["deriveBits"],
  )
  const publicBytes = new Uint8Array(await c.subtle.exportKey("raw", keyPair.publicKey))
  return {
    publicKey: base64urlEncode(publicBytes),
    privateKey: keyPair.privateKey,
  }
}

export async function deriveCompanionSessionKey(input: CompanionSessionKeyInput): Promise<CryptoKey> {
  const c = cryptoOrGlobal(input.cryptoImpl)
  const peer = await c.subtle.importKey(
    "raw",
    arrayBuffer(base64urlDecode(input.peerPublicKey)),
    { name: "ECDH", namedCurve: COMPANION_E2EE_CURVE },
    false,
    [],
  )
  const sharedBits = await c.subtle.deriveBits(
    { name: "ECDH", public: peer },
    input.privateKey,
    256,
  )
  const hkdfMaterial = await c.subtle.importKey("raw", sharedBits, "HKDF", false, ["deriveKey"])
  const salt = await c.subtle.digest(
    "SHA-256",
    encodedBuffer(`${COMPANION_E2EE_PROTOCOL}:${input.pairingRef}:${input.clientId}`),
  )
  return c.subtle.deriveKey(
    {
      name: "HKDF",
      hash: "SHA-256",
      salt,
      info: textEncoder.encode(COMPANION_E2EE_CIPHER),
    },
    hkdfMaterial,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  )
}

export async function encryptCompanionFrame(input: {
  readonly key: CryptoKey
  readonly pairingRef: string
  readonly sequence: number
  readonly message: CompanionPlaintextFrame
  readonly cryptoImpl?: CompanionCrypto
}): Promise<CompanionEncryptedFrame> {
  const c = cryptoOrGlobal(input.cryptoImpl)
  const nonce = c.getRandomValues(new Uint8Array(12))
  const plaintext = encodedBuffer(JSON.stringify(input.message))
  const ciphertext = await c.subtle.encrypt(
    {
      name: "AES-GCM",
      iv: arrayBuffer(nonce),
      additionalData: arrayBuffer(aad(input.pairingRef, input.sequence)),
    },
    input.key,
    plaintext,
  )
  return decodeEncryptedFrame({
    type: "e2ee_frame",
    protocol: COMPANION_E2EE_PROTOCOL,
    pairingRef: input.pairingRef,
    sequence: input.sequence,
    nonce: base64urlEncode(nonce),
    ciphertext: base64urlEncode(new Uint8Array(ciphertext)),
  })
}

export async function decryptCompanionFrame(input: {
  readonly key: CryptoKey
  readonly frame: CompanionEncryptedFrame
  readonly cryptoImpl?: CompanionCrypto
}): Promise<CompanionPlaintextFrame> {
  const c = cryptoOrGlobal(input.cryptoImpl)
  const frame = decodeEncryptedFrame(input.frame)
  const plaintext = await c.subtle.decrypt(
    {
      name: "AES-GCM",
      iv: arrayBuffer(base64urlDecode(frame.nonce)),
      additionalData: arrayBuffer(aad(frame.pairingRef, frame.sequence)),
    },
    input.key,
    arrayBuffer(base64urlDecode(frame.ciphertext)),
  )
  const decoded = JSON.parse(textDecoder.decode(new Uint8Array(plaintext))) as CompanionPlaintextFrame
  if (decoded.kind !== "rpc" && decoded.kind !== "event") {
    throw new Error("malformed companion plaintext frame")
  }
  return decoded
}
