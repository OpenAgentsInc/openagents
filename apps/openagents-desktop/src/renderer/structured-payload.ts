/**
 * Structured-payload detection for conversation message bodies (renderer-only).
 *
 * Owner-reported problem: when a Full Auto run starts, the first conversation
 * message is the host-authoritative mission packet, and it rendered as a raw
 * inline JSON blob (`{ "schema": "openagents.desktop.full_auto_mission.v1", …}`)
 * with a "Show full message" truncation. The owner wants: no raw JSON inline —
 * if a message body is (or embeds) a JSON payload, present it as a clean,
 * collapsible structured card, and render the mission packet as a purpose-built
 * mission card.
 *
 * This module is PURE detection: it never mutates how the packet is produced
 * (see `../full-auto-mission.ts`, the producer, which wraps the packet in a
 * prompt). It reads the already-rendered message body and classifies it. The
 * React presentation lives in `./react-structured-payload.tsx`.
 */
import { Exit, Schema } from "@effect-native/core/effect"

/**
 * The Full Auto mission-packet discriminator. This MUST equal the producer's
 * `FULL_AUTO_MISSION_SCHEMA` (`../full-auto-mission.ts`). The renderer keeps
 * its own copy so the renderer bundle never imports the main-process producer
 * module; `structured-payload.test.ts` asserts the two literals stay in sync.
 */
export const FULL_AUTO_MISSION_SCHEMA_ID = "openagents.desktop.full_auto_mission.v1"

/**
 * The lenient view the mission card renders. Extra packet keys are ignored on
 * decode (Effect Schema strips excess by default), and every field beyond the
 * discriminator plus objective/done-condition is optional, so a future packet
 * revision that adds or drops a peripheral field still renders a mission card
 * instead of degrading to the generic JSON fallback.
 */
const NullableString = Schema.NullOr(Schema.String)

export const MissionCardViewSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_MISSION_SCHEMA_ID),
  objective: Schema.String,
  doneCondition: Schema.String,
  currentLane: Schema.optional(Schema.String),
  objectiveSource: Schema.optional(Schema.String),
  turnCap: Schema.optional(Schema.Number),
  remainingTurnsIncludingThisOne: Schema.optional(Schema.Number),
  continuationOrdinal: Schema.optional(Schema.Number),
  runRef: Schema.optional(NullableString),
  threadRef: Schema.optional(NullableString),
  workspaceRef: Schema.optional(NullableString),
  accountRef: Schema.optional(NullableString),
  planBrief: Schema.optional(
    Schema.Struct({
      text: Schema.String,
      done: Schema.optional(Schema.Number),
      total: Schema.optional(Schema.Number),
      currentStepTitle: Schema.optional(NullableString),
    }),
  ),
})

export type MissionCardView = typeof MissionCardViewSchema.Type

export type StructuredPayloadDetection =
  | Readonly<{ kind: "mission"; mission: MissionCardView; value: unknown; json: string }>
  | Readonly<{ kind: "json"; value: unknown; json: string; schemaLabel: string | null }>

const isPlainObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const decodeMission = (value: unknown): MissionCardView | null => {
  const decoded = Schema.decodeUnknownExit(MissionCardViewSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/**
 * Canonical pretty-print used by BOTH cards' "copy raw" and raw-packet views,
 * so what the owner copies is stable, valid JSON regardless of how the source
 * body was whitespaced.
 */
const prettyJson = (value: unknown): string => JSON.stringify(value, null, 2)

/**
 * The generic JSON card's small header chip label: a payload's own `schema`
 * string (trimmed), else a discriminator-like `type`/`kind`, else null.
 */
const schemaLabelOf = (value: unknown): string | null => {
  if (!isPlainObject(value)) return null
  for (const key of ["schema", "type", "kind"]) {
    const candidate = value[key]
    if (typeof candidate === "string" && candidate.trim() !== "" && candidate.length <= 120) {
      return candidate.trim()
    }
  }
  return null
}

/**
 * Scan from the first `{` and return the first BALANCED, string-aware object
 * substring that `JSON.parse`s to a plain object. String/escape awareness is
 * what lets the mission packet be found even though its `objective` /
 * `doneCondition` string values can themselves contain `{` and `}`.
 */
const extractFirstJsonObject = (
  body: string,
): Readonly<{ json: string; value: Record<string, unknown> }> | null => {
  const start = body.indexOf("{")
  if (start === -1) return null
  let depth = 0
  let inString = false
  let escaped = false
  for (let index = start; index < body.length; index += 1) {
    const char = body[index]!
    if (inString) {
      if (escaped) escaped = false
      else if (char === "\\") escaped = true
      else if (char === '"') inString = false
      continue
    }
    if (char === '"') {
      inString = true
      continue
    }
    if (char === "{") depth += 1
    else if (char === "}") {
      depth -= 1
      if (depth === 0) {
        const candidate = body.slice(start, index + 1)
        try {
          const value: unknown = JSON.parse(candidate)
          return isPlainObject(value) ? { json: candidate, value } : null
        } catch {
          return null
        }
      }
    }
  }
  return null
}

const parseWholeBody = (trimmed: string): unknown | undefined => {
  const first = trimmed[0]
  const last = trimmed[trimmed.length - 1]
  const looksLikeJson =
    (first === "{" && last === "}") || (first === "[" && last === "]")
  if (!looksLikeJson) return undefined
  try {
    return JSON.parse(trimmed) as unknown
  } catch {
    return undefined
  }
}

/**
 * Classify a message body.
 *
 * - The whole trimmed body parses as a JSON object or array → structured card
 *   ("if the content is JSON, render it as a structured component"). A mission
 *   packet delivered on its own is recognized here too.
 * - Otherwise, if the body EMBEDS a Full Auto mission packet (the producer
 *   wraps the packet in prompt prose), render the mission card. Non-mission
 *   embedded JSON in prose is deliberately left to normal markdown so prose is
 *   never silently replaced by a card.
 * - Otherwise `null`: render as ordinary text.
 */
export const detectStructuredPayload = (
  body: string,
): StructuredPayloadDetection | null => {
  const trimmed = body.trim()
  if (trimmed === "") return null

  const whole = parseWholeBody(trimmed)
  if (whole !== undefined) {
    const mission = decodeMission(whole)
    if (mission !== null) {
      return { kind: "mission", mission, value: whole, json: prettyJson(whole) }
    }
    return { kind: "json", value: whole, json: prettyJson(whole), schemaLabel: schemaLabelOf(whole) }
  }

  const embedded = extractFirstJsonObject(body)
  if (embedded !== null) {
    const mission = decodeMission(embedded.value)
    if (mission !== null) {
      return {
        kind: "mission",
        mission,
        value: embedded.value,
        json: prettyJson(embedded.value),
      }
    }
  }
  return null
}
