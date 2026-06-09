import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { kitFamily } from './primitives'

export type V4ButtonVariant = 'primary' | 'secondary' | 'danger' | 'ghost'
export type V4ButtonSize = 'lg' | 'md'
export type V4ButtonStatus = 'default' | 'hover' | 'disabled'
export type V4ControlStatus =
  | 'default'
  | 'hover'
  | 'active'
  | 'error'
  | 'disabled'
export type V4Tone = 'neutral' | 'primary' | 'success' | 'warning' | 'danger'
export type V4Size = 'sm' | 'md' | 'lg'

export const v4ButtonClass = (input?: {
  variant?: V4ButtonVariant
  size?: V4ButtonSize
  status?: V4ButtonStatus
  block?: boolean
}): string => {
  const variant = input?.variant ?? 'primary'
  const size = input?.size ?? 'lg'
  const status = input?.status ?? 'default'

  return clsx(
    'inline-flex items-center justify-center overflow-hidden rounded-[4px] border px-3 text-center font-mono font-bold leading-none tracking-normal no-underline transition-colors',
    'disabled:cursor-not-allowed disabled:opacity-45',
    {
      'h-12 min-h-12 gap-3 text-xl': size === 'lg',
      'h-9 min-h-9 gap-2 text-base': size === 'md',
      'w-full': input?.block === true,
      'border-[var(--primary,#fff)] bg-[var(--primary,#fff)] text-[var(--handle,#000)] hover:border-[var(--primary,#fff)] hover:bg-[var(--text,#d7d8e5)]':
        variant === 'primary',
      'border-[var(--outline,#525458)] bg-transparent text-[var(--primary,#fff)] hover:bg-[var(--highlight,rgba(255,255,255,0.08))]':
        variant === 'secondary',
      'border-[#d32f2f] bg-[#d32f2f] text-white hover:border-[#ff6f00]':
        variant === 'danger',
      'border-transparent bg-transparent text-[var(--text,#d7d8e5)] hover:border-[var(--outline,#525458)] hover:bg-[var(--highlight,rgba(255,255,255,0.08))] hover:text-[var(--primary,#fff)]':
        variant === 'ghost',
      'bg-[var(--highlight,rgba(255,255,255,0.08))]':
        status === 'hover' && variant !== 'primary',
      'bg-[var(--text,#d7d8e5)]': status === 'hover' && variant === 'primary',
      'pointer-events-none opacity-45': status === 'disabled',
    },
  )
}

export const v4Button = <Message>(input: {
  label: string
  variant?: V4ButtonVariant
  size?: V4ButtonSize
  status?: V4ButtonStatus
  block?: boolean
  left?: Html
  right?: Html
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.button(
    [
      kitFamily<Message>('v4/button'),
      ...(input.attrs ?? []),
      h.Class(v4ButtonClass(input)),
    ],
    [
      input.left ?? null,
      h.span([h.Class('min-w-0 whitespace-nowrap')], [input.label]),
      input.right ?? null,
    ],
  )
}

export const v4LinkButton = <Message>(input: {
  href: string
  label: string
  variant?: V4ButtonVariant
  size?: V4ButtonSize
  status?: V4ButtonStatus
  block?: boolean
  left?: Html
  right?: Html
  attrs?: ReadonlyArray<Attribute<Message>>
  labelAttrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.a(
    [
      kitFamily<Message>('v4/button'),
      h.Href(input.href),
      ...(input.attrs ?? []),
      h.Class(v4ButtonClass(input)),
    ],
    [
      input.left ?? null,
      h.span(
        [h.Class('min-w-0 whitespace-nowrap'), ...(input.labelAttrs ?? [])],
        [input.label],
      ),
      input.right ?? null,
    ],
  )
}

export const v4Badge = <Message>(input: {
  label: string
  tone?: V4Tone
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const tone = input.tone ?? 'neutral'

  return h.span(
    [
      kitFamily<Message>('v4/badge'),
      ...(input.attrs ?? []),
      h.Class(
        clsx(
          'inline-flex h-6 items-center justify-center rounded-[4px] border px-2 font-mono text-xs font-bold leading-none whitespace-nowrap',
          {
            'border-[var(--outline,#525458)] bg-transparent text-[var(--text-secondary,#8a8c93)]':
              tone === 'neutral',
            'border-[var(--primary,#fff)] bg-[var(--primary,#fff)] text-[var(--handle,#000)]':
              tone === 'primary',
            'border-[#00c853]/60 bg-[#00c853]/10 text-[#86efac]':
              tone === 'success',
            'border-[#ffb400]/60 bg-[#ffb400]/10 text-[#facc15]':
              tone === 'warning',
            'border-[#d32f2f]/70 bg-[#d32f2f]/10 text-[#fca5a5]':
              tone === 'danger',
          },
        ),
      ),
    ],
    [input.label],
  )
}

export const v4AgentIcon = <Message>(input: {
  label: string
  size?: V4Size
  tone?: V4Tone
  status?: 'none' | 'online' | 'busy' | 'offline'
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const size = input.size ?? 'md'
  const tone = input.tone ?? 'neutral'
  const status = input.status ?? 'none'
  const initials = input.label.trim().slice(0, 2).toUpperCase()

  return h.span(
    [
      kitFamily<Message>('v4/agent-icon'),
      ...(input.attrs ?? []),
      h.Class(
        clsx(
          'relative inline-flex shrink-0 items-center justify-center rounded-[4px] border font-mono font-bold leading-none',
          {
            'size-6 text-[10px]': size === 'sm',
            'size-8 text-xs': size === 'md',
            'size-10 text-sm': size === 'lg',
            'border-[var(--outline,#525458)] bg-[var(--bg-secondary,#262626)] text-[var(--primary,#fff)]':
              tone === 'neutral',
            'border-[var(--primary,#fff)] bg-[var(--primary,#fff)] text-[var(--handle,#000)]':
              tone === 'primary',
            'border-[#00c853]/60 bg-[#00c853]/10 text-[#86efac]':
              tone === 'success',
            'border-[#ffb400]/60 bg-[#ffb400]/10 text-[#facc15]':
              tone === 'warning',
            'border-[#d32f2f]/70 bg-[#d32f2f]/10 text-[#fca5a5]':
              tone === 'danger',
          },
        ),
      ),
    ],
    [
      initials,
      status === 'none'
        ? null
        : h.span(
            [
              h.AriaHidden(true),
              h.Class(
                clsx(
                  'absolute -right-0.5 -bottom-0.5 size-2 rounded-full border border-black',
                  {
                    'bg-[#00c853]': status === 'online',
                    'bg-[#ffb400]': status === 'busy',
                    'bg-[#525458]': status === 'offline',
                  },
                ),
              ),
            ],
            [],
          ),
    ],
  )
}

export const v4TextInput = <Message>(input: {
  id: string
  name: string
  type?: 'text' | 'email' | 'password' | 'search'
  value?: string
  placeholder?: string
  status?: V4ControlStatus
  left?: Html
  right?: Html
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const status = input.status ?? 'default'

  return h.div(
    [
      kitFamily<Message>('v4/text-input'),
      h.Class(
        clsx(
          'flex h-12 items-center gap-2 rounded-[4px] border bg-transparent px-3 font-mono text-sm text-[var(--primary,#fff)]',
          {
            'border-[var(--outline,#525458)]': status !== 'error',
            'border-[var(--primary,#fff)]':
              status === 'active' || status === 'hover',
            'border-[#d32f2f]': status === 'error',
            'pointer-events-none opacity-45': status === 'disabled',
          },
        ),
      ),
    ],
    [
      input.left ?? null,
      h.input([
        h.Id(input.id),
        h.Name(input.name),
        h.Type(input.type ?? 'text'),
        ...(input.value === undefined ? [] : [h.Value(input.value)]),
        ...(input.placeholder === undefined
          ? []
          : [h.Placeholder(input.placeholder)]),
        ...(input.attrs ?? []),
        h.Class(
          'min-w-0 flex-1 bg-transparent text-sm leading-6 text-[var(--primary,#fff)] outline-none placeholder:text-[var(--text-secondary,#8a8c93)] disabled:cursor-not-allowed',
        ),
      ]),
      input.right ?? null,
    ],
  )
}

export const v4ModalCard = <Message>(input: {
  title?: string
  body?: string
  eyebrow?: Html
  children?: ReadonlyArray<Html>
  footer?: Html
  size?: 'sm' | 'md'
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      kitFamily<Message>('v4/modal-card'),
      ...(input.attrs ?? []),
      h.Class(
        clsx(
          'grid gap-4 rounded-[4px] border border-[var(--outline,#525458)] bg-[var(--bg,#000)] p-5 text-center shadow-[0_18px_80px_rgba(0,0,0,0.55)]',
          {
            'w-[min(100%,24rem)]': input.size !== 'md',
            'w-[min(100%,27rem)]': input.size === 'md',
          },
        ),
      ),
    ],
    [
      input.eyebrow ?? null,
      input.title === undefined
        ? null
        : h.h1(
            [
              h.Class(
                'm-0 font-mono text-xl font-bold leading-tight text-[var(--heading,#fff)]',
              ),
            ],
            [input.title],
          ),
      input.body === undefined
        ? null
        : h.p(
            [
              h.Class(
                'm-0 text-sm leading-6 text-[var(--text-secondary,#8a8c93)]',
              ),
            ],
            [input.body],
          ),
      ...(input.children ?? []),
      input.footer ?? null,
    ],
  )
}

export const v4ButtonSelector = <Message>(input: {
  items: ReadonlyArray<
    Readonly<{
      label: string
      active?: boolean
      attrs?: ReadonlyArray<Attribute<Message>>
    }>
  >
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('v4/button-selector'),
      ...(input.attrs ?? []),
      h.Role('tablist'),
      h.Class(
        'inline-flex rounded-[4px] border border-[var(--outline,#525458)] bg-transparent p-1',
      ),
    ],
    input.items.map(item =>
      h.button(
        [
          ...(item.attrs ?? []),
          h.Role('tab'),
          h.AriaSelected(item.active === true),
          h.Class(
            clsx(
              'h-8 rounded-[3px] px-3 font-mono text-xs font-bold leading-none transition-colors',
              item.active === true
                ? 'bg-[var(--primary,#fff)] text-[var(--handle,#000)]'
                : 'text-[var(--text-secondary,#8a8c93)] hover:bg-[rgba(255,255,255,0.08)] hover:text-[var(--primary,#fff)]',
            ),
          ),
        ],
        [item.label],
      ),
    ),
  )
}

export const v4ListButton = <Message>(input: {
  label: string
  meta?: string
  active?: boolean
  disabled?: boolean
  left?: Html
  right?: Html
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.button(
    [
      kitFamily<Message>('v4/list-button'),
      ...(input.attrs ?? []),
      h.Disabled(input.disabled === true),
      h.Class(
        clsx(
          'flex min-h-11 w-full items-center gap-3 rounded-[4px] border px-3 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-45',
          input.active === true
            ? 'border-[var(--primary,#fff)] bg-[rgba(255,255,255,0.08)] text-[var(--primary,#fff)]'
            : 'border-transparent bg-transparent text-[var(--text,#d7d8e5)] hover:border-[var(--outline,#525458)] hover:bg-[rgba(255,255,255,0.08)]',
        ),
      ),
    ],
    [
      input.left ?? null,
      h.span(
        [h.Class('grid min-w-0 flex-1 gap-1')],
        [
          h.span(
            [h.Class('truncate font-mono text-sm font-bold leading-none')],
            [input.label],
          ),
          input.meta === undefined
            ? null
            : h.span(
                [
                  h.Class(
                    'truncate text-xs leading-none text-[var(--text-secondary,#8a8c93)]',
                  ),
                ],
                [input.meta],
              ),
        ],
      ),
      input.right ?? null,
    ],
  )
}

export const v4AgentButton = <Message>(input: {
  name: string
  description?: string
  badge?: string
  active?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html =>
  v4ListButton<Message>({
    label: input.name,
    ...(input.description === undefined ? {} : { meta: input.description }),
    ...(input.active === undefined ? {} : { active: input.active }),
    left: v4AgentIcon<Message>({
      label: input.name,
      tone: input.active === true ? 'primary' : 'neutral',
      status: input.active === true ? 'online' : 'none',
    }),
    ...(input.badge === undefined
      ? {}
      : { right: v4Badge<Message>({ label: input.badge, tone: 'neutral' }) }),
    ...(input.attrs === undefined ? {} : { attrs: input.attrs }),
  })

export const v4Composer = <Message>(input: {
  id: string
  name: string
  placeholder?: string
  value?: string
  mode?: Html
  action?: Html
  caption?: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('v4/composer'),
      h.Class(
        'grid gap-2 rounded-[4px] border border-[var(--outline,#525458)] bg-[var(--bg,#000)] p-3',
      ),
    ],
    [
      h.textarea(
        [
          h.Id(input.id),
          h.Name(input.name),
          ...(input.placeholder === undefined
            ? []
            : [h.Placeholder(input.placeholder)]),
          ...(input.value === undefined ? [] : [h.Value(input.value)]),
          ...(input.attrs ?? []),
          h.Class(
            'min-h-20 resize-none bg-transparent font-mono text-sm leading-6 text-[var(--primary,#fff)] outline-none placeholder:text-[var(--text-secondary,#8a8c93)]',
          ),
        ],
        [],
      ),
      h.div(
        [h.Class('flex items-center justify-between gap-3')],
        [
          input.mode ?? null,
          input.caption === undefined
            ? null
            : h.p(
                [
                  h.Class(
                    'm-0 text-xs leading-5 text-[var(--text-secondary,#8a8c93)]',
                  ),
                ],
                [input.caption],
              ),
          input.action ?? null,
        ],
      ),
    ],
  )
}

export const v4ChatMessage = <Message>(input: {
  author: string
  avatarUrl?: string
  body: string
  meta?: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.article(
    [
      kitFamily<Message>('v4/chat-message'),
      ...(input.attrs ?? []),
      h.Class('flex w-full max-w-full items-start gap-3 text-left'),
    ],
    [
      input.avatarUrl === undefined || input.avatarUrl === ''
        ? v4AgentIcon<Message>({ label: input.author })
        : h.img([
            h.Src(input.avatarUrl),
            h.Alt(''),
            h.Class(
              'size-8 shrink-0 rounded-[4px] border border-[var(--outline,#525458)] object-cover',
            ),
          ]),
      h.div(
        [h.Class('grid min-w-0 gap-1')],
        [
          h.div(
            [h.Class('flex items-baseline gap-2')],
            [
              h.h3(
                [
                  h.Class(
                    'm-0 font-mono text-sm font-bold text-[var(--primary,#fff)]',
                  ),
                ],
                [input.author],
              ),
              input.meta === undefined
                ? null
                : h.span(
                    [
                      h.Class(
                        'text-xs leading-none text-[var(--text-secondary,#8a8c93)]',
                      ),
                    ],
                    [input.meta],
                  ),
            ],
          ),
      h.p(
        [
          h.Class(
            'm-0 min-w-0 max-w-full whitespace-pre-wrap break-words text-sm leading-6 text-[var(--text,#d7d8e5)]',
          ),
        ],
        [input.body],
      ),
        ],
      ),
    ],
  )
}

export const v4Navbar = <Message>(input: {
  product: string
  left?: Html
  right?: Html
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.nav(
    [
      kitFamily<Message>('v4/navbar'),
      ...(input.attrs ?? []),
      h.Class(
        'flex h-14 items-center justify-between border-b border-[var(--outline,#525458)] px-4',
      ),
    ],
    [
      h.div(
        [h.Class('flex items-center gap-3')],
        [
          input.left ?? null,
          h.span(
            [h.Class('font-mono text-sm font-bold text-[var(--primary,#fff)]')],
            [input.product],
          ),
        ],
      ),
      input.right ?? null,
    ],
  )
}

export const v4Sidebar = <Message>(input: {
  title: string
  items: ReadonlyArray<Readonly<{ label: string; active?: boolean }>>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.aside(
    [
      kitFamily<Message>('v4/sidebar'),
      ...(input.attrs ?? []),
      h.Class(
        'grid w-64 content-start gap-3 border-r border-[var(--outline,#525458)] bg-[var(--bg,#000)] p-3',
      ),
    ],
    [
      h.h2(
        [
          h.Class(
            'm-0 px-1 font-mono text-xs font-bold text-[var(--text-secondary,#8a8c93)]',
          ),
        ],
        [input.title],
      ),
      ...input.items.map(item =>
        v4ListButton<Message>({
          label: item.label,
          ...(item.active === undefined ? {} : { active: item.active }),
        }),
      ),
    ],
  )
}
