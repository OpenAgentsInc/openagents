import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { aiElementBaseTag, aiElementBase } from './base'

const MODULE_ID = 'shimmer'

export const shimmerClass = 'oa-ai-shimmer'
export const shimmerBaseTag = aiElementBaseTag(MODULE_ID, 'Shimmer')

export const ShimmerElement = Schema.Literals(['span', 'p', 'div'])
export type ShimmerElement = typeof ShimmerElement.Type

export const ShimmerProps = Schema.Struct({
  children: Schema.String,
  as: Schema.optional(ShimmerElement),
  duration: Schema.optional(Schema.Number),
  spread: Schema.optional(Schema.Number),
  ariaLabel: Schema.optional(Schema.String),
})
export type ShimmerProps = typeof ShimmerProps.Type

const shimmerStyle = (input: {
  children: string
  duration?: number
  spread?: number
}): Record<string, string> => {
  const style: Record<string, string> = {}
  if (input.duration !== undefined && Number.isFinite(input.duration)) {
    style['--oa-ai-shimmer-duration'] = `${Math.max(0.4, input.duration)}s`
  }
  if (input.spread !== undefined && Number.isFinite(input.spread)) {
    const spread = Math.max(4, input.children.length * input.spread)
    style['--oa-ai-shimmer-spread'] = `${spread.toFixed(2)}ch`
  }
  return style
}

export const shimmer = <Message>(input: {
  props: ShimmerProps
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(ShimmerProps)(input.props)
  const attrs = [
    ...(input.attrs ?? []),
    aiElementBase<Message>(MODULE_ID, 'Shimmer'),
    h.Class(shimmerClass),
    h.DataAttribute('oa-ai-shimmer', ''),
    h.Role('status'),
    h.AriaLive('polite'),
    h.AriaLabel(props.ariaLabel ?? props.children),
    h.Style(shimmerStyle({
      children: props.children,
      ...(props.duration === undefined ? {} : { duration: props.duration }),
      ...(props.spread === undefined ? {} : { spread: props.spread }),
    })),
  ]
  const children = [props.children]

  if (props.as === 'div') return h.div(attrs, children)
  if (props.as === 'p') return h.p(attrs, children)
  return h.span(attrs, children)
}
