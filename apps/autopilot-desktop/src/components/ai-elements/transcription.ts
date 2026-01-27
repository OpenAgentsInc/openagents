import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type AIChildren } from "./utils.js"

export type TranscriptionSegment = {
  text: string
  startSecond: number
  endSecond: number
}

export type TranscriptionProps = {
  readonly segments: TranscriptionSegment[]
  readonly className?: string
  readonly children?: (segment: TranscriptionSegment, index: number) => AIChildren
}

export const Transcription = ({ segments, className, children }: TranscriptionProps): TemplateResult => html`
  <div class="${cx("flex flex-wrap gap-1 text-sm leading-relaxed", className)}" data-slot="transcription">
    ${segments.filter((segment) => segment.text.trim()).map((segment, index) =>
      children ? children(segment, index) : TranscriptionSegment({ segment, index })
    )}
  </div>
`

export type TranscriptionSegmentProps = {
  readonly segment: TranscriptionSegment
  readonly index: number
  readonly className?: string
}

export const TranscriptionSegment = ({ segment, index, className }: TranscriptionSegmentProps): TemplateResult => html`
  <button
    class="${cx("inline text-left text-muted-foreground/60", className)}"
    data-index="${index}"
    data-slot="transcription-segment"
    type="button"
  >
    ${segment.text}
  </button>
`
