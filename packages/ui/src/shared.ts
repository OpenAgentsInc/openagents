import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  buttonClass,
  eyebrowClass,
  kitFamily,
  metaClass,
  textLinkClass,
} from './primitives'
import type {
  ButtonSize,
  ButtonVariant,
  MediaRowItem,
  NavItem,
} from './primitives'

export const className = <Message>(value: string): Attribute<Message> =>
  html<Message>().Class(value)

const mergeAttrs = <Message>(
  attrs: ReadonlyArray<Attribute<Message>> | undefined,
  className: string,
): ReadonlyArray<Attribute<Message>> => [
  ...(attrs ?? []),
  html<Message>().Class(className),
]

export const headingBlock = <Message>(input: {
  eyebrow?: string
  title: string
  body?: string
  level?: 1 | 2 | 3
  className?: string
}): Html => {
  const h = html<Message>()
  const titleAttrs = [
    h.Class(
      clsx('m-0 font-medium leading-[1.1] tracking-normal text-[#f1efe8]', {
        'text-4xl sm:text-5xl': input.level === 1,
        'text-xl': input.level === 2 || input.level === undefined,
        'text-sm': input.level === 3,
      }),
    ),
  ]
  const title =
    input.level === 1
      ? h.h1(titleAttrs, [input.title])
      : input.level === 3
        ? h.h3(titleAttrs, [input.title])
        : h.h2(titleAttrs, [input.title])

  return h.div(
    [h.Class(clsx('min-w-0', input.className))],
    [
      input.eyebrow === undefined
        ? null
        : h.p([h.Class(clsx(eyebrowClass, 'mb-2'))], [input.eyebrow]),
      title,
      input.body === undefined
        ? null
        : h.p(
            [h.Class('m-0 mt-3 max-w-[58ch] text-sm leading-6 text-white/55')],
            [input.body],
          ),
    ],
  )
}

export const button = <Message>(input: {
  label: string
  variant?: ButtonVariant
  size?: ButtonSize
  block?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...(input.attrs ?? []),
      h.Class(
        clsx(
          buttonClass(input.variant ?? 'primary', input.size ?? 'md'),
          input.block === true && 'w-full',
        ),
      ),
    ],
    [input.label],
  )
}

export const linkButton = <Message>(input: {
  href: string
  label: string
  variant?: ButtonVariant
  size?: ButtonSize
  block?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.a(
    [
      ...(input.attrs ?? []),
      h.Href(input.href),
      h.Class(
        clsx(
          buttonClass(input.variant ?? 'primary', input.size ?? 'md'),
          input.block === true && 'w-full',
        ),
      ),
    ],
    [input.label],
  )
}

export const textLink = <Message>(input: {
  href: string
  label: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.a(
    [
      ...(input.attrs ?? []),
      h.Href(input.href),
      h.Class(textLinkClass),
      kitFamily<Message>('elements/buttons'),
    ],
    [input.label],
  )
}

export const avatar = <Message>(input: {
  name: string
  imageUrl?: string
  size?: 'sm' | 'md' | 'lg'
}): Html => {
  const h = html<Message>()
  const sizeClass = clsx({
    'h-8 w-8 text-[0.6875rem]': input.size === 'sm' || input.size === undefined,
    'h-10 w-10 text-xs': input.size === 'md',
    'h-14 w-14 text-sm': input.size === 'lg',
  })
  const initials = input.name
    .split(/\s+/)
    .filter(Boolean)
    .map(part => part[0])
    .join('')
    .slice(0, 2)
    .toUpperCase()

  if (input.imageUrl !== undefined && input.imageUrl !== '') {
    return h.img([
      kitFamily<Message>('elements/avatars'),
      h.Src(input.imageUrl),
      h.Alt(''),
      h.Class(clsx(sizeClass, 'border border-[#222] object-cover')),
    ])
  }

  return h.div(
    [
      kitFamily<Message>('elements/avatars'),
      h.Class(
        clsx(
          sizeClass,
          'grid place-items-center border border-[#222] bg-[#080808] text-white/45',
        ),
      ),
      h.AriaHidden(true),
    ],
    [initials === '' ? 'OA' : initials],
  )
}

export const avatarGroup = <Message>(
  people: ReadonlyArray<Pick<MediaRowItem, 'title' | 'avatarUrl'>>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('elements/avatars'),
      h.Class('flex -space-x-2 overflow-hidden'),
    ],
    people.slice(0, 6).map(person =>
      avatar<Message>({
        name: person.title,
        ...(person.avatarUrl === undefined
          ? {}
          : { imageUrl: person.avatarUrl }),
        size: 'sm',
      }),
    ),
  )
}

export const buttonGroup = <Message>(
  actions: ReadonlyArray<Html>,
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...attrs,
      kitFamily<Message>('elements/button-groups'),
      h.Class('flex flex-wrap items-center gap-2'),
    ],
    actions,
  )
}

export const dropdownMenu = <Message>(input: {
  label: string
  items: ReadonlyArray<NavItem>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.div(
    mergeAttrs<Message>(
      input.attrs,
      'grid gap-2 border border-[#222] bg-[#010102] p-2',
    ),
    [
      h.button(
        [
          kitFamily<Message>('elements/dropdowns'),
          h.Type('button'),
          h.Class(
            'flex min-h-9 items-center justify-between gap-3 border border-[#333] bg-[#080808] px-3 text-left font-[inherit] text-sm text-[#f1efe8]',
          ),
        ],
        [h.span([], [input.label]), h.span([h.AriaHidden(true)], ['v'])],
      ),
      h.ul(
        [h.Role('list'), h.Class('m-0 grid list-none gap-1 p-0')],
        input.items.map(item =>
          h.li(
            [],
            [
              h.a(
                [
                  h.Href(item.href),
                  h.Class(
                    clsx(
                      'grid gap-0.5 border border-transparent px-2.5 py-2 text-sm text-white/60 no-underline hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8]',
                      {
                        'border-[#333] bg-[#141414] text-[#f1efe8]':
                          item.active === true,
                      },
                    ),
                  ),
                ],
                [
                  h.span([h.Class('truncate')], [item.label]),
                  item.meta === undefined
                    ? null
                    : h.span([h.Class(metaClass)], [item.meta]),
                ],
              ),
            ],
          ),
        ),
      ),
    ],
  )
}
