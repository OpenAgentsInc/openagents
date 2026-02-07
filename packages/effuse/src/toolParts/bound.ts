import { Effect } from "effect"
import type { BlobRefLike, BoundedText } from "./types.js"

export type PutText<R, E> = (options: {
  readonly text: string
  readonly mime?: string
}) => Effect.Effect<BlobRefLike, E, R>

/**
 * Bound a string for safe rendering in the DOM.
 *
 * - If `text.length <= maxChars`, returns it inline.
 * - Otherwise, stores the full payload via `putText` and returns a truncated preview + BlobRef.
 */
export const boundText = <R, E>(input: {
  readonly text: string
  readonly maxChars: number
  readonly putText: PutText<R, E>
  readonly mime?: string
  readonly suffix?: string
}): Effect.Effect<BoundedText, E, R> => {
  const suffix = input.suffix ?? "\n… (truncated)"
  const maxChars = Math.max(0, Math.floor(input.maxChars))

  if (input.text.length <= maxChars) {
    return Effect.succeed({ preview: input.text, truncated: false })
  }

  return Effect.gen(function* () {
    const blob = yield* input.putText({ text: input.text, mime: input.mime })
    const preview = input.text.slice(0, maxChars) + suffix
    return { preview, truncated: true, blob }
  })
}

