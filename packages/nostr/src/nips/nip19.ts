/**
 * NIP-19: bech32-encoded entities
 * Implements encoding and decoding of Nostr entities using bech32 format
 *
 * Supported entity types:
 * - npub (public keys)
 * - nsec (private keys)
 * - note (event IDs)
 * - nprofile (public key + relay hints)
 * - nevent (event ID + relay hints + author)
 * - naddr (parameterized replaceable event coordinates)
 * - nrelay (relay URLs)
 */

import { bech32 } from "bech32"
import { Brand, Data, Effect, Schema } from "effect"

// --- Branded Types ---
export type Npub = string & Brand.Brand<"Npub">
export const Npub = Brand.nominal<Npub>()

export type Nsec = string & Brand.Brand<"Nsec">
export const Nsec = Brand.nominal<Nsec>()

export type Note = string & Brand.Brand<"Note">
export const Note = Brand.nominal<Note>()

export type Nprofile = string & Brand.Brand<"Nprofile">
export const Nprofile = Brand.nominal<Nprofile>()

export type Nevent = string & Brand.Brand<"Nevent">
export const Nevent = Brand.nominal<Nevent>()

export type Naddr = string & Brand.Brand<"Naddr">
export const Naddr = Brand.nominal<Naddr>()

export type Nrelay = string & Brand.Brand<"Nrelay">
export const Nrelay = Brand.nominal<Nrelay>()

// --- TLV (Type-Length-Value) Encoding ---
export const TlvType = {
  SPECIAL: 0,
  RELAY: 1,
  AUTHOR: 2,
  KIND: 3
} as const

// --- Errors ---
export class Nip19Error extends Data.TaggedError("Nip19Error")<{
  reason: "invalid_prefix" | "decode_failed" | "encode_failed" | "invalid_hex" | "invalid_tlv"
  message: string
  cause?: unknown
}> {}

// --- Schemas ---
export const ProfilePointer = Schema.Struct({
  pubkey: Schema.String,
  relays: Schema.optional(Schema.Array(Schema.String))
})
export type ProfilePointer = Schema.Schema.Type<typeof ProfilePointer>

export const EventPointer = Schema.Struct({
  id: Schema.String,
  relays: Schema.optional(Schema.Array(Schema.String)),
  author: Schema.optional(Schema.String),
  kind: Schema.optional(Schema.Number)
})
export type EventPointer = Schema.Schema.Type<typeof EventPointer>

export const AddressPointer = Schema.Struct({
  identifier: Schema.String,
  pubkey: Schema.String,
  kind: Schema.Number,
  relays: Schema.optional(Schema.Array(Schema.String))
})
export type AddressPointer = Schema.Schema.Type<typeof AddressPointer>

// --- Utility Functions ---
const hexToBytes = (hex: string): Uint8Array => {
  if (hex.length % 2 !== 0) {
    throw new Error("Invalid hex string length")
  }
  const bytes = new Uint8Array(hex.length / 2)
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16)
  }
  return bytes
}

const bytesToHex = (bytes: Uint8Array): string => {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("")
}

// --- TLV Encoding/Decoding ---
const encodeTlv = (data: Array<{ type: number; value: string | number }>): Uint8Array => {
  const buffers: Array<Uint8Array> = []

  for (const { type, value } of data) {
    const typeBuffer = new Uint8Array([type])

    let valueBuffer: Uint8Array
    if (typeof value === "string") {
      valueBuffer = hexToBytes(value)
    } else {
      // For numbers (kind), encode as 32-bit big-endian
      valueBuffer = new Uint8Array(4)
      new DataView(valueBuffer.buffer).setUint32(0, value, false)
    }

    const lengthBuffer = new Uint8Array([valueBuffer.length])
    buffers.push(typeBuffer, lengthBuffer, valueBuffer)
  }

  // Concatenate all buffers
  const totalLength = buffers.reduce((sum, buf) => sum + buf.length, 0)
  const result = new Uint8Array(totalLength)
  let offset = 0
  for (const buf of buffers) {
    result.set(buf, offset)
    offset += buf.length
  }

  return result
}

const decodeTlv = (data: Uint8Array): Record<number, Array<string | number>> => {
  const result: Record<number, Array<string | number>> = {}
  let offset = 0

  while (offset < data.length) {
    const type = data[offset]
    offset++

    if (offset >= data.length) break

    const length = data[offset]
    offset++

    if (offset + length > data.length) break

    const value = data.slice(offset, offset + length)
    offset += length

    // Decode value based on type
    let decodedValue: string | number
    if (type === TlvType.KIND) {
      // Kind is stored as 32-bit big-endian
      decodedValue = new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(0, false)
    } else {
      decodedValue = bytesToHex(value)
    }

    if (!result[type]) {
      result[type] = []
    }
    result[type].push(decodedValue)
  }

  return result
}

// --- Core Encoding Functions ---

/**
 * Encode a hex public key to npub format
 */
export const npubEncode = (pubkey: string): Effect.Effect<Npub, Nip19Error> =>
  Effect.try({
    try: () => {
      const words = bech32.toWords(hexToBytes(pubkey))
      const encoded = bech32.encode("npub", words, 1000)
      return Npub(encoded)
    },
    catch: (error) =>
      new Nip19Error({
        reason: "encode_failed",
        message: `Failed to encode npub: ${error}`,
        cause: error
      })
  })

/**
 * Decode an npub to hex public key
 */
export const npubDecode = (npub: Npub): Effect.Effect<string, Nip19Error> =>
  Effect.try({
    try: () => {
      const decoded = bech32.decode(npub, 1000)
      if (decoded.prefix !== "npub") {
        throw new Error("Invalid prefix")
      }
      const bytes = new Uint8Array(bech32.fromWords(decoded.words))
      return bytesToHex(bytes)
    },
    catch: (error) =>
      new Nip19Error({
        reason: "decode_failed",
        message: `Failed to decode npub: ${error}`,
        cause: error
      })
  })

/**
 * Encode a hex private key to nsec format
 */
export const nsecEncode = (privkey: string): Effect.Effect<Nsec, Nip19Error> =>
  Effect.try({
    try: () => {
      const words = bech32.toWords(hexToBytes(privkey))
      const encoded = bech32.encode("nsec", words, 1000)
      return Nsec(encoded)
    },
    catch: (error) =>
      new Nip19Error({
        reason: "encode_failed",
        message: `Failed to encode nsec: ${error}`,
        cause: error
      })
  })

/**
 * Decode an nsec to hex private key
 */
export const nsecDecode = (nsec: Nsec): Effect.Effect<string, Nip19Error> =>
  Effect.try({
    try: () => {
      const decoded = bech32.decode(nsec, 1000)
      if (decoded.prefix !== "nsec") {
        throw new Error("Invalid prefix")
      }
      const bytes = new Uint8Array(bech32.fromWords(decoded.words))
      return bytesToHex(bytes)
    },
    catch: (error) =>
      new Nip19Error({
        reason: "decode_failed",
        message: `Failed to decode nsec: ${error}`,
        cause: error
      })
  })

/**
 * Encode an event ID to note format
 */
export const noteEncode = (eventId: string): Effect.Effect<Note, Nip19Error> =>
  Effect.try({
    try: () => {
      const words = bech32.toWords(hexToBytes(eventId))
      const encoded = bech32.encode("note", words, 1000)
      return Note(encoded)
    },
    catch: (error) =>
      new Nip19Error({
        reason: "encode_failed",
        message: `Failed to encode note: ${error}`,
        cause: error
      })
  })

/**
 * Decode a note to event ID
 */
export const noteDecode = (note: Note): Effect.Effect<string, Nip19Error> =>
  Effect.try({
    try: () => {
      const decoded = bech32.decode(note, 1000)
      if (decoded.prefix !== "note") {
        throw new Error("Invalid prefix")
      }
      const bytes = new Uint8Array(bech32.fromWords(decoded.words))
      return bytesToHex(bytes)
    },
    catch: (error) =>
      new Nip19Error({
        reason: "decode_failed",
        message: `Failed to decode note: ${error}`,
        cause: error
      })
  })

/**
 * Encode a profile pointer to nprofile format
 */
export const nprofileEncode = (profile: ProfilePointer): Effect.Effect<Nprofile, Nip19Error> =>
  Effect.try({
    try: () => {
      const tlvData: Array<{ type: number; value: string }> = [
        { type: TlvType.SPECIAL, value: profile.pubkey }
      ]

      if (profile.relays) {
        for (const relay of profile.relays) {
          // Convert relay URL to hex
          const relayHex = bytesToHex(new TextEncoder().encode(relay))
          tlvData.push({ type: TlvType.RELAY, value: relayHex })
        }
      }

      const tlvBytes = encodeTlv(tlvData)
      const words = bech32.toWords(tlvBytes)
      const encoded = bech32.encode("nprofile", words, 1000)
      return Nprofile(encoded)
    },
    catch: (error) =>
      new Nip19Error({
        reason: "encode_failed",
        message: `Failed to encode nprofile: ${error}`,
        cause: error
      })
  })

/**
 * Decode an nprofile to profile pointer
 */
export const nprofileDecode = (nprofile: Nprofile): Effect.Effect<ProfilePointer, Nip19Error> =>
  Effect.try({
    try: () => {
      const decoded = bech32.decode(nprofile, 1000)
      if (decoded.prefix !== "nprofile") {
        throw new Error("Invalid prefix")
      }

      const bytes = new Uint8Array(bech32.fromWords(decoded.words))
      const tlvData = decodeTlv(bytes)

      const pubkey = tlvData[TlvType.SPECIAL]?.[0]
      if (!pubkey || typeof pubkey !== "string") {
        throw new Error("Missing or invalid pubkey")
      }

      const relays: Array<string> = []
      if (tlvData[TlvType.RELAY]) {
        for (const relayHex of tlvData[TlvType.RELAY]) {
          if (typeof relayHex === "string") {
            const relayBytes = hexToBytes(relayHex)
            relays.push(new TextDecoder().decode(relayBytes))
          }
        }
      }

      return {
        pubkey,
        relays: relays.length > 0 ? relays : undefined
      }
    },
    catch: (error) =>
      new Nip19Error({
        reason: "decode_failed",
        message: `Failed to decode nprofile: ${error}`,
        cause: error
      })
  })

/**
 * Encode an event pointer to nevent format
 */
export const neventEncode = (event: EventPointer): Effect.Effect<Nevent, Nip19Error> =>
  Effect.try({
    try: () => {
      const tlvData: Array<{ type: number; value: string | number }> = [
        { type: TlvType.SPECIAL, value: event.id }
      ]

      if (event.relays) {
        for (const relay of event.relays) {
          const relayHex = bytesToHex(new TextEncoder().encode(relay))
          tlvData.push({ type: TlvType.RELAY, value: relayHex })
        }
      }

      if (event.author) {
        tlvData.push({ type: TlvType.AUTHOR, value: event.author })
      }

      if (event.kind !== undefined) {
        tlvData.push({ type: TlvType.KIND, value: event.kind })
      }

      const tlvBytes = encodeTlv(tlvData)
      const words = bech32.toWords(tlvBytes)
      const encoded = bech32.encode("nevent", words, 1000)
      return Nevent(encoded)
    },
    catch: (error) =>
      new Nip19Error({
        reason: "encode_failed",
        message: `Failed to encode nevent: ${error}`,
        cause: error
      })
  })

/**
 * Decode an nevent to event pointer
 */
export const neventDecode = (nevent: Nevent): Effect.Effect<EventPointer, Nip19Error> =>
  Effect.try({
    try: () => {
      const decoded = bech32.decode(nevent, 1000)
      if (decoded.prefix !== "nevent") {
        throw new Error("Invalid prefix")
      }

      const bytes = new Uint8Array(bech32.fromWords(decoded.words))
      const tlvData = decodeTlv(bytes)

      const id = tlvData[TlvType.SPECIAL]?.[0]
      if (!id || typeof id !== "string") {
        throw new Error("Missing or invalid event ID")
      }

      const relays: Array<string> = []
      if (tlvData[TlvType.RELAY]) {
        for (const relayHex of tlvData[TlvType.RELAY]) {
          if (typeof relayHex === "string") {
            const relayBytes = hexToBytes(relayHex)
            relays.push(new TextDecoder().decode(relayBytes))
          }
        }
      }

      const author = tlvData[TlvType.AUTHOR]?.[0]
      const kind = tlvData[TlvType.KIND]?.[0]

      return {
        id,
        relays: relays.length > 0 ? relays : undefined,
        author: typeof author === "string" ? author : undefined,
        kind: typeof kind === "number" ? kind : undefined
      }
    },
    catch: (error) =>
      new Nip19Error({
        reason: "decode_failed",
        message: `Failed to decode nevent: ${error}`,
        cause: error
      })
  })

/**
 * Encode an address pointer to naddr format
 */
export const naddrEncode = (addr: AddressPointer): Effect.Effect<Naddr, Nip19Error> =>
  Effect.try({
    try: () => {
      const identifierHex = bytesToHex(new TextEncoder().encode(addr.identifier))

      const tlvData: Array<{ type: number; value: string | number }> = [
        { type: TlvType.SPECIAL, value: identifierHex },
        { type: TlvType.AUTHOR, value: addr.pubkey },
        { type: TlvType.KIND, value: addr.kind }
      ]

      if (addr.relays) {
        for (const relay of addr.relays) {
          const relayHex = bytesToHex(new TextEncoder().encode(relay))
          tlvData.push({ type: TlvType.RELAY, value: relayHex })
        }
      }

      const tlvBytes = encodeTlv(tlvData)
      const words = bech32.toWords(tlvBytes)
      const encoded = bech32.encode("naddr", words, 1000)
      return Naddr(encoded)
    },
    catch: (error) =>
      new Nip19Error({
        reason: "encode_failed",
        message: `Failed to encode naddr: ${error}`,
        cause: error
      })
  })

/**
 * Decode an naddr to address pointer
 */
export const naddrDecode = (naddr: Naddr): Effect.Effect<AddressPointer, Nip19Error> =>
  Effect.try({
    try: () => {
      const decoded = bech32.decode(naddr, 1000)
      if (decoded.prefix !== "naddr") {
        throw new Error("Invalid prefix")
      }

      const bytes = new Uint8Array(bech32.fromWords(decoded.words))
      const tlvData = decodeTlv(bytes)

      const identifierHex = tlvData[TlvType.SPECIAL]?.[0]
      if (!identifierHex || typeof identifierHex !== "string") {
        throw new Error("Missing or invalid identifier")
      }
      const identifier = new TextDecoder().decode(hexToBytes(identifierHex))

      const pubkey = tlvData[TlvType.AUTHOR]?.[0]
      if (!pubkey || typeof pubkey !== "string") {
        throw new Error("Missing or invalid pubkey")
      }

      const kind = tlvData[TlvType.KIND]?.[0]
      if (kind === undefined || typeof kind !== "number") {
        throw new Error("Missing or invalid kind")
      }

      const relays: Array<string> = []
      if (tlvData[TlvType.RELAY]) {
        for (const relayHex of tlvData[TlvType.RELAY]) {
          if (typeof relayHex === "string") {
            const relayBytes = hexToBytes(relayHex)
            relays.push(new TextDecoder().decode(relayBytes))
          }
        }
      }

      return {
        identifier,
        pubkey,
        kind,
        relays: relays.length > 0 ? relays : undefined
      }
    },
    catch: (error) =>
      new Nip19Error({
        reason: "decode_failed",
        message: `Failed to decode naddr: ${error}`,
        cause: error
      })
  })

/**
 * Encode a relay URL to nrelay format
 */
export const nrelayEncode = (url: string): Effect.Effect<Nrelay, Nip19Error> =>
  Effect.try({
    try: () => {
      const urlBytes = new TextEncoder().encode(url)
      const words = bech32.toWords(urlBytes)
      const encoded = bech32.encode("nrelay", words, 1000)
      return Nrelay(encoded)
    },
    catch: (error) =>
      new Nip19Error({
        reason: "encode_failed",
        message: `Failed to encode nrelay: ${error}`,
        cause: error
      })
  })

/**
 * Decode an nrelay to URL
 */
export const nrelayDecode = (nrelay: Nrelay): Effect.Effect<string, Nip19Error> =>
  Effect.try({
    try: () => {
      const decoded = bech32.decode(nrelay, 1000)
      if (decoded.prefix !== "nrelay") {
        throw new Error("Invalid prefix")
      }
      const bytes = new Uint8Array(bech32.fromWords(decoded.words))
      return new TextDecoder().decode(bytes)
    },
    catch: (error) =>
      new Nip19Error({
        reason: "decode_failed",
        message: `Failed to decode nrelay: ${error}`,
        cause: error
      })
  })

// --- Generic decode function ---
export const decode = (encoded: string): Effect.Effect<
  | { type: "npub"; data: string }
  | { type: "nsec"; data: string }
  | { type: "note"; data: string }
  | { type: "nprofile"; data: ProfilePointer }
  | { type: "nevent"; data: EventPointer }
  | { type: "naddr"; data: AddressPointer }
  | { type: "nrelay"; data: string },
  Nip19Error
> =>
  Effect.gen(function*() {
    try {
      const decoded = bech32.decode(encoded, 1000)

      switch (decoded.prefix) {
        case "npub":
          return { type: "npub", data: yield* npubDecode(Npub(encoded)) }
        case "nsec":
          return { type: "nsec", data: yield* nsecDecode(Nsec(encoded)) }
        case "note":
          return { type: "note", data: yield* noteDecode(Note(encoded)) }
        case "nprofile":
          return { type: "nprofile", data: yield* nprofileDecode(Nprofile(encoded)) }
        case "nevent":
          return { type: "nevent", data: yield* neventDecode(Nevent(encoded)) }
        case "naddr":
          return { type: "naddr", data: yield* naddrDecode(Naddr(encoded)) }
        case "nrelay":
          return { type: "nrelay", data: yield* nrelayDecode(Nrelay(encoded)) }
        default:
          return yield* Effect.fail(
            new Nip19Error({
              reason: "invalid_prefix",
              message: `Unknown prefix: ${decoded.prefix}`
            })
          )
      }
    } catch (error) {
      return yield* Effect.fail(
        new Nip19Error({
          reason: "decode_failed",
          message: `Failed to decode: ${error}`,
          cause: error
        })
      )
    }
  })
