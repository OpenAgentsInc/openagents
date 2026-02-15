import { Effect } from "effect"

import { decodeL402Challenge } from "../contracts/l402.js"
import { ChallengeParseError } from "../errors/lightningErrors.js"

const challengePrefix = /^(?:L402|LSAT)(?:\s+|$)/i
const integerPattern = /^(0|[1-9][0-9]*)$/

const toParseError = (header: string, reason: string): ChallengeParseError =>
  ChallengeParseError.make({ header, reason })

type ChallengeSegment = Readonly<{
  scheme: "L402" | "LSAT"
  delimiterIndex: number
  start: number
}>

const extractSupportedChallengeSegment = (
  header: string,
): Effect.Effect<string, ChallengeParseError> =>
  Effect.gen(function* () {
    const trimmed = header.trim()
    if (trimmed.length === 0) {
      return yield* toParseError(header, "Expected an L402 challenge header")
    }

    const matcher = /(^|,\s*)(L402|LSAT)\s+/gi
    const segments: Array<ChallengeSegment> = []
    let match: RegExpExecArray | null = matcher.exec(trimmed)
    while (match) {
      const delimiter = match[1] ?? ""
      const schemeRaw = (match[2] ?? "").toUpperCase()
      const scheme = schemeRaw === "L402" ? "L402" : "LSAT"
      segments.push({
        scheme,
        delimiterIndex: match.index,
        start: match.index + delimiter.length,
      })
      match = matcher.exec(trimmed)
    }

    if (segments.length === 0) {
      return yield* toParseError(header, "Expected an L402 challenge header")
    }

    const preferred =
      segments.find((segment) => segment.scheme === "L402") ??
      segments[0]
    if (!preferred) {
      return yield* toParseError(header, "Expected an L402 challenge header")
    }

    const preferredIndex = segments.findIndex(
      (segment) => segment.start === preferred.start,
    )
    const nextSegment = segments[preferredIndex + 1]
    const end = nextSegment ? nextSegment.delimiterIndex : trimmed.length
    const challengeSegment = trimmed.slice(preferred.start, end).trim()
    if (challengeSegment.length === 0 || !challengePrefix.test(challengeSegment)) {
      return yield* toParseError(header, "Expected an L402 challenge header")
    }

    return challengeSegment
  })

const splitAttributeEntries = (header: string, rawAttributes: string): Effect.Effect<Array<string>, ChallengeParseError> =>
  Effect.gen(function* () {
    const trimmed = rawAttributes.trim()
    if (trimmed.length === 0) return []

    const entries: Array<string> = []
    let current = ""
    let inQuotes = false
    let escaping = false

    for (let index = 0; index < rawAttributes.length; index += 1) {
      const char = rawAttributes[index] ?? ""

      if (inQuotes && escaping) {
        current += char
        escaping = false
        continue
      }

      if (inQuotes && char === "\\") {
        current += char
        escaping = true
        continue
      }

      if (char === "\"") {
        current += char
        inQuotes = !inQuotes
        continue
      }

      if (!inQuotes && char === ",") {
        const entry = current.trim()
        if (entry.length === 0) {
          return yield* toParseError(header, "Challenge contains an empty attribute entry")
        }
        entries.push(entry)
        current = ""
        continue
      }

      current += char
    }

    if (inQuotes) {
      return yield* toParseError(header, "Challenge contains an unterminated quoted attribute")
    }

    const finalEntry = current.trim()
    if (finalEntry.length > 0) {
      entries.push(finalEntry)
    }

    return entries
  })

const parseAttributeEntries = (
  header: string,
  entries: ReadonlyArray<string>,
): Effect.Effect<Record<string, string>, ChallengeParseError> =>
  Effect.gen(function* () {
    const attributes: Record<string, string> = {}

    for (const entry of entries) {
      const separatorIndex = entry.indexOf("=")
      if (separatorIndex <= 0) {
        return yield* toParseError(header, `Malformed challenge attribute: ${entry}`)
      }

      const rawKey = entry.slice(0, separatorIndex).trim().toLowerCase()
      const rawValue = entry.slice(separatorIndex + 1).trim()

      if (!/^[a-z0-9_-]+$/.test(rawKey)) {
        return yield* toParseError(header, `Invalid challenge attribute key: ${rawKey}`)
      }

      if (!rawValue) {
        return yield* toParseError(header, `Missing value for challenge attribute: ${rawKey}`)
      }

      if (Object.prototype.hasOwnProperty.call(attributes, rawKey)) {
        return yield* toParseError(header, `Duplicate challenge attribute: ${rawKey}`)
      }

      const parsedValue = rawValue.startsWith("\"")
        ? (() => {
            if (rawValue.length < 2 || !rawValue.endsWith("\"")) {
              return null
            }
            const unquoted = rawValue.slice(1, -1)
            return unquoted.replaceAll('\\"', "\"").replaceAll("\\\\", "\\")
          })()
        : (() => {
            if (/[\s,"]/.test(rawValue)) return null
            return rawValue
          })()

      if (parsedValue == null || parsedValue.length === 0) {
        return yield* toParseError(header, `Malformed value for challenge attribute: ${rawKey}`)
      }

      attributes[rawKey] = parsedValue
    }

    return attributes
  })

const parseAmountMsats = (header: string, attrs: Record<string, string>): Effect.Effect<number | undefined, ChallengeParseError> =>
  Effect.gen(function* () {
    const amountUnderscore = attrs.amount_msats
    const amountNoUnderscore = attrs.amountmsats

    if (
      amountUnderscore !== undefined &&
      amountNoUnderscore !== undefined &&
      amountUnderscore !== amountNoUnderscore
    ) {
      return yield* toParseError(
        header,
        "amount_msats and amountmsats conflict in challenge attributes",
      )
    }

    const raw = amountUnderscore ?? amountNoUnderscore
    if (raw === undefined) return undefined

    if (!integerPattern.test(raw)) {
      return yield* toParseError(header, "amount_msats must be a non-negative integer")
    }

    const parsed = Number(raw)
    if (!Number.isSafeInteger(parsed)) {
      return yield* toParseError(header, "amount_msats is outside safe integer bounds")
    }

    return parsed
  })

const inferAmountMsatsFromBolt11 = (invoice: string): number | undefined => {
  // We only need the HRP amount portion; we don't decode the full invoice.
  // BOLT11 format: ln<network><amount><multiplier?>1<data+checksum>
  // Amount is in BTC units; multiplier scales by 10^-3 (m), 10^-6 (u), 10^-9 (n), 10^-12 (p).
  const trimmed = invoice.trim().toLowerCase()
  const sepIndex = trimmed.indexOf("1")
  if (sepIndex <= 0) return undefined

  const hrp = trimmed.slice(0, sepIndex)
  const match = /^ln(?:bc|tb|bcrt)([0-9]+)([munp]?)$/.exec(hrp)
  if (!match) return undefined

  const digits = match[1] ?? ""
  const unit = match[2] ?? ""
  if (!digits) return undefined

  let divisor = 1n
  switch (unit) {
    case "":
      divisor = 1n
      break
    case "m":
      divisor = 1_000n
      break
    case "u":
      divisor = 1_000_000n
      break
    case "n":
      divisor = 1_000_000_000n
      break
    case "p":
      divisor = 1_000_000_000_000n
      break
    default:
      return undefined
  }

  const msatsPerBtc = 100_000_000_000n
  const numerator = BigInt(digits) * msatsPerBtc
  if (numerator % divisor !== 0n) return undefined

  const msats = numerator / divisor
  if (msats > BigInt(Number.MAX_SAFE_INTEGER)) return undefined
  return Number(msats)
}

export const parseChallengeHeader = Effect.fn("l402.parseChallengeHeader")(function* (header: string) {
  const challengeHeader = yield* extractSupportedChallengeSegment(header)

  const attributesPart = challengeHeader.replace(challengePrefix, "").trim()
  const entries = yield* splitAttributeEntries(header, attributesPart)
  const attrs = yield* parseAttributeEntries(header, entries)

  const invoice = attrs.invoice
  const macaroon = attrs.macaroon
  if (!invoice || !macaroon) {
    return yield* toParseError(header, "Challenge must include invoice and macaroon attributes")
  }

  const amountMsats = yield* parseAmountMsats(header, attrs)
  const amountMsatsResolved = amountMsats ?? inferAmountMsatsFromBolt11(invoice)

  return yield* decodeL402Challenge({
    invoice,
    macaroon,
    ...(amountMsatsResolved !== undefined ? { amountMsats: amountMsatsResolved } : {}),
    ...(attrs.issuer ? { issuer: attrs.issuer } : {}),
  }).pipe(
    Effect.mapError(() =>
      toParseError(
        header,
        "L402 challenge failed schema validation",
      )),
  )
})
