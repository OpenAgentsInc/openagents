import type { TemplateResult } from "../template/types.js"

export type BlobRefLike = {
  readonly id: string
  readonly hash: string
  readonly size: number
  readonly mime?: string | undefined
}

export type BoundedText = {
  readonly preview: string
  readonly truncated: boolean
  readonly blob?: BlobRefLike | undefined
}

export type ToolPartStatus =
  | "tool-call"
  | "tool-result"
  | "tool-error"
  | "tool-denied"
  | "tool-approval"
  | "tool-canceled"
  | (string & {})

export type ToolPartDetails = {
  readonly extra?: TemplateResult | null
  readonly input?: BoundedText
  readonly output?: BoundedText
  readonly error?: BoundedText
}

export type ToolPartModel = {
  /**
   * Required minimal tool-part schema.
   *
   * MUST be rendered visibly in the UI.
   */
  readonly status: ToolPartStatus
  readonly toolName: string
  readonly toolCallId: string
  readonly summary: string

  /** Optional details, rendered behind a disclosure affordance. */
  readonly details?: ToolPartDetails
}

