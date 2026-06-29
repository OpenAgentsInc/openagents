import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { eyebrowClass, metaClass } from '../primitives'
import { inputClass } from '../forms'
import { aiElementBase } from './base'

const MODULE_ID = 'web-preview'

// Ported from the autopilot3 web-preview markup, in the kit's dark-only
// palette. Frames a preview URL, navigation controls, a sandboxed iframe body,
// and an optional console log panel.
export const webPreviewClass =
  'flex size-full flex-col border border-[#222] bg-[#010102]'
export const webPreviewNavigationClass =
  'flex items-center gap-1 border-b border-[#222] p-2'
export const webPreviewUrlClass = `${inputClass} h-8 py-1`
export const webPreviewBodyClass = 'min-h-0 flex-1'
export const webPreviewIframeClass = 'size-full border-0 bg-white'
export const webPreviewConsoleClass =
  'grid gap-1 border-t border-[#222] bg-[#030303] px-3 py-2.5'
export const webPreviewConsoleLineClass =
  'm-0 font-mono text-[0.75rem] leading-[1.45] text-white/60'

export const WebPreviewProps = Schema.Struct({
  url: Schema.String,
  title: Schema.optional(Schema.String),
  console: Schema.optional(Schema.Array(Schema.String)),
})
export type WebPreviewProps = typeof WebPreviewProps.Type

export const webPreviewNavigation = <Message>(input: {
  url: string
  controls?: ReadonlyArray<Html>
  urlAttrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'WebPreviewNavigation'),
      h.Class(webPreviewNavigationClass),
    ],
    [
      ...(input.controls ?? []),
      h.input([
        ...(input.urlAttrs ?? []),
        aiElementBase<Message>(MODULE_ID, 'WebPreviewUrl'),
        h.Type('text'),
        h.Value(input.url),
        h.AriaLabel('Preview URL'),
        h.Class(webPreviewUrlClass),
      ]),
    ],
  )
}

export const webPreviewBody = <Message>(input: {
  url: string
  title?: string
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'WebPreviewBody'),
      h.Class(webPreviewBodyClass),
    ],
    [
      h.iframe(
        [
          h.Src(input.url),
          h.Title(input.title ?? 'Web preview'),
          h.Sandbox('allow-scripts allow-same-origin'),
          h.Class(webPreviewIframeClass),
        ],
        [],
      ),
    ],
  )
}

export const webPreviewConsole = <Message>(
  lines: ReadonlyArray<string>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'WebPreviewConsole'),
      h.Class(webPreviewConsoleClass),
    ],
    [
      h.span([h.Class(eyebrowClass)], ['Console']),
      lines.length === 0
        ? h.p([h.Class(metaClass)], ['No console output'])
        : h.div(
            [h.Class('grid gap-0.5')],
            lines.map(line =>
              h.pre([h.Class(webPreviewConsoleLineClass)], [line]),
            ),
          ),
    ],
  )
}

// A framed artifact preview: navigation bar with the URL, a sandboxed iframe
// body, and an optional console log panel.
export const webPreview = <Message>(input: {
  props: WebPreviewProps
  controls?: ReadonlyArray<Html>
  urlAttrs?: ReadonlyArray<Attribute<Message>>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(WebPreviewProps)(input.props)

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'WebPreview'),
      h.Class(webPreviewClass),
    ],
    [
      webPreviewNavigation<Message>({
        url: props.url,
        ...(input.controls === undefined ? {} : { controls: input.controls }),
        ...(input.urlAttrs === undefined ? {} : { urlAttrs: input.urlAttrs }),
      }),
      webPreviewBody<Message>({
        url: props.url,
        ...(props.title === undefined ? {} : { title: props.title }),
      }),
      props.console === undefined
        ? null
        : webPreviewConsole<Message>(props.console),
    ],
  )
}
