import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  kitFamily,
  metaClass,
  statusDotClass,
  surfaceClass,
  titleClass,
} from './primitives'
import type { FeedItem } from './primitives'
import { buttonGroup, headingBlock } from './shared'

export const signInPanel = <Message>(input: {
  eyebrow?: string
  title: string
  body?: string
  fields: ReadonlyArray<Html>
  submit: Html
  onSubmit: Attribute<Message>
  footer?: Html
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...(input.attrs ?? []),
      kitFamily<Message>('forms/sign-in-forms'),
      h.Class('mx-auto w-[min(100%,28rem)]'),
    ],
    [
      section<Message>([
        headingBlock<Message>({
          eyebrow: input.eyebrow ?? 'OpenAgents',
          title: input.title,
          ...(input.body === undefined ? {} : { body: input.body }),
          className: 'mb-6',
        }),
        h.form(
          [
            kitFamily<Message>('forms/sign-in-forms'),
            h.Class('grid gap-6'),
            input.onSubmit,
          ],
          [...input.fields, input.submit],
        ),
        input.footer === undefined
          ? null
          : h.div(
              [h.Class('mt-6 text-center text-sm text-white/45')],
              [input.footer],
            ),
      ]),
    ],
  )
}

export const pageShell = <Message>(
  children: ReadonlyArray<Html | string>,
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...attrs,
      h.Class(
        'h-dvh overflow-y-auto overscroll-contain bg-[#000] font-mono text-[#f1efe8] antialiased selection:bg-[#ffb400] selection:text-[#000]',
      ),
    ],
    children,
  )
}

export const stackedApplicationShell = <Message>(input: {
  navigation: Html
  children: ReadonlyArray<Html | string>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return pageShell<Message>(
    [
      input.navigation,
      h.main(
        [
          kitFamily<Message>('application-shells/stacked'),
          h.Class('min-h-0 py-8'),
        ],
        input.children,
      ),
    ],
    input.attrs ?? [],
  )
}

export const routeMain = <Message>(
  children: ReadonlyArray<Html | string>,
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.main(
    [
      ...attrs,
      kitFamily<Message>('application-shells/stacked'),
      h.Class('py-8'),
    ],
    children,
  )
}

export const centeredFrame = <Message>(
  children: ReadonlyArray<Html | string>,
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...attrs,
      h.Class(
        'grid h-screen h-dvh w-screen place-items-center overflow-hidden overscroll-none bg-[#000] font-mono text-[#f1efe8]',
      ),
    ],
    [
      h.main(
        [
          h.Class(
            'grid min-h-0 w-full max-w-[640px] place-items-center overflow-auto overscroll-contain border-x border-[#222] bg-[#010102] p-8',
          ),
        ],
        children,
      ),
    ],
  )
}

export const container = <Message>(
  children: ReadonlyArray<Html | string>,
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.div(
    [...attrs, h.Class('mx-auto w-[min(100%,1120px)] px-4')],
    children,
  )
}

export const section = <Message>(
  children: ReadonlyArray<Html | string>,
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.section([...attrs, h.Class(clsx(surfaceClass, 'p-4'))], children)
}

export const card = <Message>(input: {
  children: ReadonlyArray<Html | string>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.article(
    [
      ...(input.attrs ?? []),
      kitFamily<Message>('layout/cards'),
      h.Class(surfaceClass),
    ],
    input.children,
  )
}

export const divider = <Message>(label?: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('layout/dividers'),
      h.Class(
        'grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-3',
      ),
    ],
    [
      h.span([h.Class('h-px bg-white/10')], []),
      label === undefined
        ? null
        : h.span(
            [h.Class('text-xs uppercase tracking-[0.08em] text-white/35')],
            [label],
          ),
      h.span([h.Class('h-px bg-white/10')], []),
    ],
  )
}

export const modalDialog = <Message>(input: {
  title: string
  body?: string
  actions?: ReadonlyArray<Html>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('overlays/modal-dialogs'),
      h.Role('dialog'),
      h.AriaModal(true),
      h.Class('grid max-w-md gap-4 border border-[#333] bg-[#010102] p-4'),
    ],
    [
      headingBlock<Message>({
        eyebrow: 'Dialog',
        title: input.title,
        ...(input.body === undefined ? {} : { body: input.body }),
        level: 3,
      }),
      input.actions === undefined ? null : buttonGroup<Message>(input.actions),
    ],
  )
}

export const drawerPanel = <Message>(input: {
  title: string
  children: ReadonlyArray<Html | string>
}): Html => {
  const h = html<Message>()

  return h.aside(
    [
      kitFamily<Message>('overlays/drawers'),
      h.Class(
        'grid w-full max-w-sm gap-4 border-l border-[#333] bg-[#010102] p-4',
      ),
    ],
    [
      headingBlock<Message>({
        eyebrow: 'Drawer',
        title: input.title,
        level: 3,
      }),
      ...input.children,
    ],
  )
}

export const notificationStack = <Message>(
  items: ReadonlyArray<FeedItem>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('overlays/notifications'),
      h.Class('grid w-full max-w-sm gap-2'),
    ],
    items.map(item =>
      h.div(
        [
          h.Class(
            'grid grid-cols-[auto_minmax(0,1fr)] gap-3 border border-[#333] bg-[#010102] p-3',
          ),
        ],
        [
          h.span([h.Class(statusDotClass(item.tone ?? 'neutral'))], []),
          h.div(
            [h.Class('min-w-0')],
            [
              h.p([h.Class(titleClass)], [item.title]),
              item.body === undefined
                ? null
                : h.p([h.Class(metaClass)], [item.body]),
            ],
          ),
        ],
      ),
    ),
  )
}
