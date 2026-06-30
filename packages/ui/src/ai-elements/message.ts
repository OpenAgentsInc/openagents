import { clsx } from 'clsx'
import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { aiElementBase } from './base'
import { response } from './response'

const MODULE_ID = 'message'

// Ported from the autopilot3 message markup + Maud `AI_MESSAGE_*` contracts,
// re-expressed in the kit's dark-only / pure-black palette. The user/assistant
// split mirrors the upstream `is-user` / `is-assistant` group contract.
export const messageBaseClass = 'group flex w-full max-w-[95%] flex-col gap-2'
export const messageUserClass = 'is-user ml-auto items-end justify-end'
export const messageAssistantClass = 'is-assistant items-start'
export const messageContentClass =
  'flex w-fit min-w-0 max-w-full flex-col gap-2 overflow-hidden border border-[#222] bg-[#010102] px-3 py-2.5 text-[0.8125rem] leading-[1.45] text-[#f1efe8]'
export const messageContentUserClass = 'border-[#333] bg-[#141414]'
export const messageActionsClass = 'flex items-center gap-1'
export const messageEyebrowClass =
  'text-[0.6875rem] font-semibold uppercase leading-[1.2] tracking-[0.08em] text-white/35'
export const messageMetaClass = clsx(
  'm-0 overflow-hidden text-ellipsis whitespace-nowrap text-[0.75rem] text-white/35',
  'flex items-center gap-2',
)

export const MessageRole = Schema.Literals(['user', 'assistant', 'system'])
export type MessageRole = typeof MessageRole.Type

export const MessageProps = Schema.Struct({
  role: MessageRole,
  body: Schema.optional(Schema.String),
  author: Schema.optional(Schema.String),
  time: Schema.optional(Schema.String),
})
export type MessageProps = typeof MessageProps.Type

export const messageActions = <Message>(
  actions: ReadonlyArray<Html>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'MessageActions'),
      h.Class(messageActionsClass),
    ],
    actions,
  )
}

export const messageMeta = <Message>(input: {
  author?: string
  time?: string
}): Html => {
  const h = html<Message>()

  return h.div(
    [aiElementBase<Message>(MODULE_ID, 'MessageMeta'), h.Class(messageMetaClass)],
    [
      input.author === undefined
        ? null
        : h.span([h.Class(messageEyebrowClass)], [input.author]),
      input.time === undefined ? null : h.span([], [input.time]),
    ],
  )
}

export const messageContent = <Message>(input: {
  role: MessageRole
  children: ReadonlyArray<Html | string>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'MessageContent'),
      h.Class(
        clsx(messageContentClass, {
          [messageContentUserClass]: input.role === 'user',
        }),
      ),
    ],
    input.children,
  )
}

// A single chat turn. Body paths, in precedence order:
//   - `markdown`: render the body through the centralized `response` Markdown
//     element (bold/italics/headings/lists/code/links), streaming-tolerant.
//     Pass `streaming: true` to append a live typing cursor while a reply lands.
//   - `props.body`: the plain-text path (no markdown parsing).
// `extra` (e.g. a code block, sources, or an actions row) appends after the body.
export const message = <Message>(input: {
  props: MessageProps
  markdown?: string
  streaming?: boolean
  extra?: ReadonlyArray<Html>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(MessageProps)(input.props)

  const bodyChildren: ReadonlyArray<Html | string> =
    input.markdown !== undefined
      ? [
          response<Message>({
            markdown: input.markdown,
            ...(input.streaming === undefined
              ? {}
              : { streaming: input.streaming }),
          }),
        ]
      : props.body === undefined
        ? []
        : [props.body]

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'Message'),
      h.Class(
        clsx(messageBaseClass, {
          [messageUserClass]: props.role === 'user',
          [messageAssistantClass]: props.role !== 'user',
        }),
      ),
    ],
    [
      props.author === undefined && props.time === undefined
        ? null
        : messageMeta<Message>({
            ...(props.author === undefined ? {} : { author: props.author }),
            ...(props.time === undefined ? {} : { time: props.time }),
          }),
      messageContent<Message>({
        role: props.role,
        children: [...bodyChildren, ...(input.extra ?? [])],
      }),
    ],
  )
}
