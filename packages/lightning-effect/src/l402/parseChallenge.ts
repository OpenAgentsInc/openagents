import { Effect } from "effect"

import { decodeL402Challenge } from "../contracts/l402.js"
import { ChallengeParseError } from "../errors/lightningErrors.js"

const challengePrefix = /^L402(?:\s+|$)/i
const integerPattern = /^(0|[1-9][0-9]*)$/

const toParseError = (header: string, reason: string): ChallengeParseError =>
  ChallengeParseError.make({ header, reason })

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

export const parseChallengeHeader = Effect.fn("l402.parseChallengeHeader")(function* (header: string) {
  const trimmed = header.trim()
  if (!trimmed || !challengePrefix.test(trimmed)) {
    return yield* toParseError(header, "Expected an L402 challenge header")
  }

  const attributesPart = trimmed.replace(challengePrefix, "").trim()
  const entries = yield* splitAttributeEntries(header, attributesPart)
  const attrs = yield* parseAttributeEntries(header, entries)

  const invoice = attrs.invoice
  const macaroon = attrs.macaroon
  if (!invoice || !macaroon) {
    return yield* toParseError(header, "Challenge must include invoice and macaroon attributes")
  }

  const amountMsats = yield* parseAmountMsats(header, attrs)

  return yield* decodeL402Challenge({
    invoice,
    macaroon,
    ...(amountMsats !== undefined ? { amountMsats } : {}),
    ...(attrs.issuer ? { issuer: attrs.issuer } : {}),
  }).pipe(
    Effect.mapError(() =>
      toParseError(
        header,
        "L402 challenge failed schema validation",
      )),
  )
})
