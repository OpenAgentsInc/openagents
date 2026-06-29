import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { titleClass } from '../primitives'
import { aiElementBase } from './base'

const MODULE_ID = 'reasoning'

// Ported from the autopilot3 reasoning markup + Maud reasoning contract, in the
// kit's dark-only palette. The trigger/content split mirrors the upstream
// collapsible thinking summary (server emits both; JS only toggles).
export const reasoningClass = 'grid gap-2'
export const reasoningTriggerClass =
  'flex w-full items-center gap-2 text-[0.8125rem] text-white/60 transition-colors hover:text-[#f1efe8]'
export const reasoningContentClass =
  'grid gap-2 border-l border-[#222] pl-4 text-[0.8125rem] leading-[1.45] text-white/50'

export const ReasoningProps = Schema.Struct({
  text: Schema.String,
  open: Schema.optional(Schema.Boolean),
  streaming: Schema.optional(Schema.Boolean),
  duration: Schema.optional(Schema.Number),
})
export type ReasoningProps = typeof ReasoningProps.Type

const thinkingLabel = (input: {
  streaming?: boolean
  duration?: number
}): string => {
  if (input.streaming === true) {
    return 'Thinking...'
  }

  if (input.duration === undefined) {
    return 'Thought for a few seconds'
  }

  return `Thought for ${input.duration} seconds`
}

export const reasoningTrigger = <Message>(input: {
  label: string
  open?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'ReasoningTrigger'),
      h.Type('button'),
      h.AriaExpanded(input.open ?? false),
      h.Class(reasoningTriggerClass),
    ],
    [h.span([h.Class(titleClass)], [input.label])],
  )
}

export const reasoningContent = <Message>(text: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'ReasoningContent'),
      h.Class(reasoningContentClass),
    ],
    [h.p([], [text])],
  )
}

// A collapsible reasoning/thinking summary. The trigger shows a duration- or
// streaming-aware label; the content holds the bounded reasoning text.
export const reasoning = <Message>(input: {
  props: ReasoningProps
  triggerAttrs?: ReadonlyArray<Attribute<Message>>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(ReasoningProps)(input.props)

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'Reasoning'),
      h.Class(reasoningClass),
    ],
    [
      reasoningTrigger<Message>({
        label: thinkingLabel({
          ...(props.streaming === undefined ? {} : { streaming: props.streaming }),
          ...(props.duration === undefined ? {} : { duration: props.duration }),
        }),
        open: props.open ?? props.streaming ?? false,
        ...(input.triggerAttrs === undefined
          ? {}
          : { attrs: input.triggerAttrs }),
      }),
      reasoningContent<Message>(props.text),
    ],
  )
}
