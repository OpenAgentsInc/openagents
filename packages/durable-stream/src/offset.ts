/**
 * Offsets — PROTOCOL.md §8.
 *
 * Offsets are opaque tokens identifying a position within a stream. Per spec
 * they MUST be:
 *  - opaque (clients never interpret structure)
 *  - lexicographically sortable
 *  - persistent, unique, strictly increasing
 *  - free of `,` `&` `=` `?` `/` (URL-query safe)
 *  - never equal to the reserved sentinels `-1` (stream beginning) or `now`
 *    (current tail), which the server MUST NOT mint as real offsets.
 *
 * We mint offsets as a zero-padded decimal byte position: the count of bytes
 * (for byte streams) / messages-as-bytes preceding the position. A read at
 * offset N returns the exact suffix starting at byte N. Because the token is a
 * fixed-width zero-padded integer, byte-wise lexicographic comparison agrees
 * with numeric order, satisfying the "lexicographically sortable + strictly
 * increasing" requirement.
 */
import { Schema as S } from "effect"

/** Reserved sentinel: read from the very beginning of the stream. */
export const OFFSET_BEGINNING = "-1" as const
/** Reserved sentinel: skip existing data, read only future data from tail. */
export const OFFSET_NOW = "now" as const

/**
 * Width of the zero-padded position token. 18 decimal digits comfortably holds
 * any byte position we will ever store in a single DO (well under the 256-char
 * cap the spec recommends) while keeping lexicographic == numeric ordering.
 */
export const OFFSET_WIDTH = 18

export const StreamOffset = S.String.pipe(S.brand("StreamOffset"))
export type StreamOffset = typeof StreamOffset.Type

const makeOffset = (n: number): StreamOffset => {
  if (!Number.isInteger(n) || n < 0) {
    throw new Error(`offset position must be a non-negative integer, got ${n}`)
  }
  return String(n).padStart(OFFSET_WIDTH, "0") as StreamOffset
}

/** The canonical offset for byte position `n` (0 == start of stream). */
export const offsetForPosition = (n: number): StreamOffset => makeOffset(n)

/** The tail offset for a stream of `byteLength` bytes (== next write position). */
export const tailOffset = (byteLength: number): StreamOffset => makeOffset(byteLength)

const SENTINELS = new Set<string>([OFFSET_BEGINNING, OFFSET_NOW])

/** Is this a reserved sentinel offset value? */
export const isSentinel = (raw: string): raw is typeof OFFSET_BEGINNING | typeof OFFSET_NOW =>
  SENTINELS.has(raw)

/** Forbidden characters per §8 (would collide with URL-query syntax). */
const FORBIDDEN = /[,&=?/]/

export type ParsedOffset =
  | { readonly kind: "beginning" }
  | { readonly kind: "now" }
  | { readonly kind: "position"; readonly position: number; readonly offset: StreamOffset }

/**
 * Parse a client-supplied offset token (from `?offset=`).
 *
 * Returns `null` for a malformed offset (caller maps to 400). An omitted offset
 * is treated as the beginning by callers (spec §5.6: default is offset -1).
 */
export const parseOffset = (raw: string | null | undefined): ParsedOffset | null => {
  if (raw === null || raw === undefined || raw === OFFSET_BEGINNING) {
    return { kind: "beginning" }
  }
  if (raw === OFFSET_NOW) {
    return { kind: "now" }
  }
  if (raw.length === 0 || FORBIDDEN.test(raw)) {
    return null
  }
  // Real offsets we mint are all-digits zero-padded; tolerate non-padded digit
  // strings a client may have stored, but reject anything non-numeric.
  if (!/^\d+$/.test(raw)) {
    return null
  }
  const position = Number(raw)
  if (!Number.isSafeInteger(position) || position < 0) {
    return null
  }
  return { kind: "position", position, offset: makeOffset(position) }
}
