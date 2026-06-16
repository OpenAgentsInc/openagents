import { clsx } from 'clsx'
import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { eyebrowClass, statusDotClass, titleClass } from '../primitives'
import { aiElementBase } from './base'

const MODULE_ID = 'tool'

// Ported from the autopilot3 tool markup + Maud tool contracts, in the kit's
// dark-only palette. State labels mirror the upstream tool-call status set.
export const toolClass =
  'grid w-full gap-0 border border-[#222] bg-[#010102] font-mono text-[0.75rem]'
export const toolHeaderClass =
  'flex w-full items-center justify-between gap-2 border-b border-[#222] px-2.5 py-2'
export const toolContentClass = 'grid gap-2 px-2.5 py-2'
export const toolSectionLabelClass = clsx(eyebrowClass, 'text-white/35')
export const toolPreClass =
  'm-0 overflow-x-auto border border-[#222] bg-[#030303] px-2.5 py-2 text-[0.75rem] leading-[1.45] text-white/60'
export const toolOutputErrorClass = 'border-[#d32f2f] text-[#d32f2f]'

export const ToolState = Schema.Literals([
  'pending',
  'running',
  'awaiting-approval',
  'completed',
  'denied',
  'error',
])
export type ToolState = typeof ToolState.Type

export const ToolProps = Schema.Struct({
  name: Schema.String,
  state: ToolState,
  input: Schema.optional(Schema.String),
  output: Schema.optional(Schema.String),
  errorText: Schema.optional(Schema.String),
  open: Schema.optional(Schema.Boolean),
})
export type ToolProps = typeof ToolProps.Type

const stateTone = (state: ToolState) => {
  switch (state) {
    case 'completed':
      return 'positive' as const
    case 'error':
      return 'negative' as const
    case 'denied':
      return 'warning' as const
    case 'running':
      return 'info' as const
    case 'awaiting-approval':
      return 'accent' as const
    case 'pending':
      return 'neutral' as const
  }
}

const stateLabel = (state: ToolState): string => {
  switch (state) {
    case 'pending':
      return 'Pending'
    case 'running':
      return 'Running'
    case 'awaiting-approval':
      return 'Awaiting approval'
    case 'completed':
      return 'Completed'
    case 'denied':
      return 'Denied'
    case 'error':
      return 'Error'
  }
}

export const toolHeader = <Message>(input: {
  name: string
  state: ToolState
  open?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'ToolHeader'),
      h.Type('button'),
      h.AriaExpanded(input.open ?? false),
      h.Class(toolHeaderClass),
    ],
    [
      h.span(
        [h.Class('flex min-w-0 items-center gap-1.5')],
        [
          h.span([h.Class(statusDotClass(stateTone(input.state)))], []),
          h.span([h.Class(clsx(titleClass, 'font-mono'))], [input.name]),
        ],
      ),
      h.span([h.Class(eyebrowClass)], [stateLabel(input.state)]),
    ],
  )
}

export const toolInput = <Message>(input: string): Html => {
  const h = html<Message>()

  return h.div(
    [aiElementBase<Message>(MODULE_ID, 'ToolInput'), h.Class('grid gap-1')],
    [
      h.span([h.Class(toolSectionLabelClass)], ['Parameters']),
      h.pre([h.Class(toolPreClass)], [h.code([], [input])]),
    ],
  )
}

export const toolOutput = <Message>(input: {
  output?: string
  errorText?: string
}): Html | null => {
  const h = html<Message>()

  if (input.output === undefined && input.errorText === undefined) {
    return null
  }

  const isError = input.errorText !== undefined

  return h.div(
    [aiElementBase<Message>(MODULE_ID, 'ToolOutput'), h.Class('grid gap-1')],
    [
      h.span([h.Class(toolSectionLabelClass)], [isError ? 'Error' : 'Result']),
      h.pre(
        [h.Class(clsx(toolPreClass, { [toolOutputErrorClass]: isError }))],
        [h.code([], [input.errorText ?? input.output ?? ''])],
      ),
    ],
  )
}

export const toolContent = <Message>(children: ReadonlyArray<Html>): Html => {
  const h = html<Message>()

  return h.div(
    [aiElementBase<Message>(MODULE_ID, 'ToolContent'), h.Class(toolContentClass)],
    children,
  )
}

// A tool/agent call card: a header with the tool name + status, and a content
// section with the input parameters and the output (or error).
export const tool = <Message>(input: {
  props: ToolProps
  headerAttrs?: ReadonlyArray<Attribute<Message>>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(ToolProps)(input.props)
  const output = toolOutput<Message>({
    ...(props.output === undefined ? {} : { output: props.output }),
    ...(props.errorText === undefined ? {} : { errorText: props.errorText }),
  })
  const contentChildren = [
    props.input === undefined ? null : toolInput<Message>(props.input),
    output,
  ].filter((child): child is Html => child !== null)

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'Tool'),
      h.Class(toolClass),
    ],
    [
      toolHeader<Message>({
        name: props.name,
        state: props.state,
        ...(props.open === undefined ? {} : { open: props.open }),
        ...(input.headerAttrs === undefined
          ? {}
          : { attrs: input.headerAttrs }),
      }),
      contentChildren.length === 0
        ? null
        : toolContent<Message>(contentChildren),
    ],
  )
}
