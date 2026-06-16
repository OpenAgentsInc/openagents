import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { metaClass, textLinkClass, titleClass } from '../primitives'
import { aiElementBase } from './base'

const MODULE_ID = 'sources'

// Ported from the autopilot3 sources markup + Maud `AI_SOURCES` contract, in
// the kit's dark-only palette. Ties to receipts/provenance: each source is a
// trusted external link.
export const sourcesClass = 'grid gap-2 text-[0.75rem]'
export const sourcesTriggerClass =
  'flex w-full items-center gap-2 text-[0.8125rem] text-white/60 transition-colors hover:text-[#f1efe8]'
export const sourcesContentClass = 'grid w-fit gap-2 pl-1'
export const sourceClass = 'flex items-center gap-2 text-[0.8125rem]'

export const SourceProps = Schema.Struct({
  title: Schema.String,
  href: Schema.String,
})
export type SourceProps = typeof SourceProps.Type

export const SourcesProps = Schema.Struct({
  open: Schema.optional(Schema.Boolean),
  label: Schema.optional(Schema.String),
  sources: Schema.Array(SourceProps),
})
export type SourcesProps = typeof SourcesProps.Type

export const source = <Message>(props: SourceProps): Html => {
  const h = html<Message>()
  const decoded = Schema.decodeUnknownSync(SourceProps)(props)

  return h.a(
    [
      aiElementBase<Message>(MODULE_ID, 'Source'),
      h.Href(decoded.href),
      h.Rel('noreferrer'),
      h.Target('_blank'),
      h.Class(`${sourceClass} ${textLinkClass}`),
    ],
    [h.span([h.Class('block font-medium')], [decoded.title])],
  )
}

export const sourcesTrigger = <Message>(input: {
  count: number
  label?: string
  open?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const label = input.label ?? `Used ${input.count} sources`

  return h.button(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'SourcesTrigger'),
      h.Type('button'),
      h.AriaExpanded(input.open ?? false),
      h.Class(sourcesTriggerClass),
    ],
    [h.span([h.Class(titleClass)], [label])],
  )
}

export const sourcesContent = <Message>(items: ReadonlyArray<Html>): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'SourcesContent'),
      h.Class(sourcesContentClass),
    ],
    items,
  )
}

// A collapsible list of citation/provenance links used by an answer.
export const sources = <Message>(input: {
  props: SourcesProps
  triggerAttrs?: ReadonlyArray<Attribute<Message>>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(SourcesProps)(input.props)

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'Sources'),
      h.Class(sourcesClass),
    ],
    [
      sourcesTrigger<Message>({
        count: props.sources.length,
        ...(props.label === undefined ? {} : { label: props.label }),
        ...(props.open === undefined ? {} : { open: props.open }),
        ...(input.triggerAttrs === undefined
          ? {}
          : { attrs: input.triggerAttrs }),
      }),
      props.sources.length === 0
        ? h.p([h.Class(metaClass)], ['No sources'])
        : sourcesContent<Message>(props.sources.map(s => source<Message>(s))),
    ],
  )
}
