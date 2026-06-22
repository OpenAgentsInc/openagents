import { clsx } from 'clsx'
import * as stylex from '@stylexjs/stylex'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  eyebrowClass,
  kitFamily,
  metaClass,
} from './primitives'
import type {
  ButtonSize,
  ButtonVariant,
  MediaRowItem,
  NavItem,
} from './primitives'
import {
  stylexAttrs,
  stylexFallback,
  stylexRuntimeFallbackEnabled,
} from './stylex-foldkit'

const sharedStyles = stylexRuntimeFallbackEnabled()
  ? {
      headingRoot: stylexFallback('oa-ui-heading-root'),
      headingTitle: stylexFallback('oa-ui-heading-title'),
      headingTitleLevel1: stylexFallback('oa-ui-heading-title-level-1'),
      headingTitleLevel2: stylexFallback('oa-ui-heading-title-level-2'),
      headingTitleLevel3: stylexFallback('oa-ui-heading-title-level-3'),
      headingBody: stylexFallback('oa-ui-heading-body'),
      button: stylexFallback('oa-ui-button'),
      buttonSm: stylexFallback('oa-ui-button-sm'),
      buttonMd: stylexFallback('oa-ui-button-md'),
      buttonBlock: stylexFallback('oa-ui-button-block'),
      buttonPrimary: stylexFallback('oa-ui-button-primary'),
      buttonSecondary: stylexFallback('oa-ui-button-secondary'),
      buttonGhost: stylexFallback('oa-ui-button-ghost'),
      buttonDanger: stylexFallback('oa-ui-button-danger'),
      textLink: stylexFallback('oa-ui-text-link'),
      avatarImage: stylexFallback('oa-ui-avatar-image'),
      avatarFallback: stylexFallback('oa-ui-avatar-fallback'),
      avatarSm: stylexFallback('oa-ui-avatar-sm'),
      avatarMd: stylexFallback('oa-ui-avatar-md'),
      avatarLg: stylexFallback('oa-ui-avatar-lg'),
      avatarGroup: stylexFallback('oa-ui-avatar-group'),
      buttonGroup: stylexFallback('oa-ui-button-group'),
      dropdown: stylexFallback('oa-ui-dropdown'),
      dropdownButton: stylexFallback('oa-ui-dropdown-button'),
      dropdownList: stylexFallback('oa-ui-dropdown-list'),
      dropdownItem: stylexFallback('oa-ui-dropdown-item'),
      dropdownItemActive: stylexFallback('oa-ui-dropdown-item-active'),
      dropdownLabel: stylexFallback('oa-ui-dropdown-label'),
    }
  : stylex.create({
      headingRoot: {
        minWidth: 0,
      },
      headingTitle: {
        margin: 0,
        fontWeight: 500,
        lineHeight: 1.1,
        letterSpacing: 0,
        color: '#f1efe8',
      },
      headingTitleLevel1: {
        fontSize: {
          default: '2.25rem',
          '@media (min-width: 640px)': '3rem',
        },
      },
      headingTitleLevel2: {
        fontSize: '1.25rem',
      },
      headingTitleLevel3: {
        fontSize: '0.875rem',
      },
      headingBody: {
        margin: 0,
        marginTop: 12,
        maxWidth: '58ch',
        fontSize: '0.875rem',
        lineHeight: 1.5,
        color: 'rgba(255,255,255,0.55)',
      },
      button: {
        display: 'inline-grid',
        placeItems: 'center',
        borderWidth: 1,
        borderStyle: 'solid',
        textAlign: 'center',
        fontWeight: 500,
        textDecorationLine: 'none',
        transitionProperty: 'color, border-color, background-color',
        transitionDuration: '150ms',
        transitionTimingFunction: 'ease',
        ':disabled': {
          cursor: 'not-allowed',
          opacity: 0.45,
        },
      },
      buttonMd: {
        minHeight: 44,
        paddingInline: 16,
        fontSize: '0.875rem',
      },
      buttonSm: {
        minHeight: 36,
        paddingInline: 12,
        fontSize: '0.75rem',
      },
      buttonBlock: {
        width: '100%',
      },
      buttonPrimary: {
        borderColor: '#f1efe8',
        backgroundColor: '#f1efe8',
        color: '#000',
        ':hover': {
          borderColor: '#ffb400',
        },
      },
      buttonSecondary: {
        borderColor: '#222',
        backgroundColor: 'transparent',
        color: '#f1efe8',
        ':hover': {
          borderColor: '#ffb400',
        },
      },
      buttonGhost: {
        borderColor: 'transparent',
        backgroundColor: 'transparent',
        color: 'rgba(255,255,255,0.6)',
        ':hover': {
          borderColor: '#333',
          backgroundColor: '#080808',
          color: '#f1efe8',
        },
      },
      buttonDanger: {
        borderColor: '#d32f2f',
        backgroundColor: '#d32f2f',
        color: '#fff',
        ':hover': {
          borderColor: '#ff6f00',
        },
      },
      textLink: {
        color: '#f1efe8',
        textDecorationLine: 'underline',
        textUnderlineOffset: 3,
        ':hover': {
          color: '#ffb400',
        },
      },
      avatarImage: {
        borderWidth: 1,
        borderStyle: 'solid',
        borderColor: '#222',
        objectFit: 'cover',
      },
      avatarFallback: {
        display: 'grid',
        placeItems: 'center',
        borderWidth: 1,
        borderStyle: 'solid',
        borderColor: '#222',
        backgroundColor: '#080808',
        color: 'rgba(255,255,255,0.45)',
      },
      avatarSm: {
        width: 32,
        height: 32,
        fontSize: '0.6875rem',
      },
      avatarMd: {
        width: 40,
        height: 40,
        fontSize: '0.75rem',
      },
      avatarLg: {
        width: 56,
        height: 56,
        fontSize: '0.875rem',
      },
      avatarGroup: {
        display: 'flex',
        marginLeft: -8,
        overflow: 'hidden',
      },
      buttonGroup: {
        display: 'flex',
        flexWrap: 'wrap',
        alignItems: 'center',
        gap: 8,
      },
      dropdown: {
        display: 'grid',
        gap: 8,
        borderWidth: 1,
        borderStyle: 'solid',
        borderColor: '#222',
        backgroundColor: '#010102',
        padding: 8,
      },
      dropdownButton: {
        display: 'flex',
        minHeight: 36,
        alignItems: 'center',
        justifyContent: 'space-between',
        gap: 12,
        borderWidth: 1,
        borderStyle: 'solid',
        borderColor: '#333',
        backgroundColor: '#080808',
        paddingInline: 12,
        fontFamily: 'inherit',
        fontSize: '0.875rem',
        color: '#f1efe8',
        textAlign: 'left',
      },
      dropdownList: {
        margin: 0,
        display: 'grid',
        listStyle: 'none',
        gap: 4,
        padding: 0,
      },
      dropdownItem: {
        display: 'grid',
        gap: 2,
        borderWidth: 1,
        borderStyle: 'solid',
        borderColor: 'transparent',
        paddingInline: 10,
        paddingBlock: 8,
        fontSize: '0.875rem',
        color: 'rgba(255,255,255,0.6)',
        textDecorationLine: 'none',
        ':hover': {
          borderColor: '#333',
          backgroundColor: '#080808',
          color: '#f1efe8',
        },
      },
      dropdownItemActive: {
        borderColor: '#333',
        backgroundColor: '#141414',
        color: '#f1efe8',
      },
      dropdownLabel: {
        overflow: 'hidden',
        textOverflow: 'ellipsis',
        whiteSpace: 'nowrap',
      },
    })

const buttonStyles = (
  variant: ButtonVariant = 'primary',
  size: ButtonSize = 'md',
  block = false,
) => [
  sharedStyles.button,
  size === 'sm' ? sharedStyles.buttonSm : sharedStyles.buttonMd,
  variant === 'secondary'
    ? sharedStyles.buttonSecondary
    : variant === 'ghost'
      ? sharedStyles.buttonGhost
      : variant === 'danger'
        ? sharedStyles.buttonDanger
        : sharedStyles.buttonPrimary,
  block ? sharedStyles.buttonBlock : null,
]

const avatarSizeStyle = (size: 'sm' | 'md' | 'lg' | undefined) =>
  size === 'md'
    ? sharedStyles.avatarMd
    : size === 'lg'
      ? sharedStyles.avatarLg
      : sharedStyles.avatarSm

export const className = <Message>(value: string): Attribute<Message> =>
  html<Message>().Class(value)

const mergeAttrs = <Message>(
  attrs: ReadonlyArray<Attribute<Message>> | undefined,
  styles: ReadonlyArray<ReturnType<typeof stylexFallback> | null>,
): ReadonlyArray<Attribute<Message>> => [
  ...(attrs ?? []),
  ...stylexAttrs<Message>(...styles),
]

export const headingBlock = <Message>(input: {
  eyebrow?: string
  title: string
  body?: string
  level?: 1 | 2 | 3
  className?: string
}): Html => {
  const h = html<Message>()
  const titleAttrs = stylexAttrs<Message>(
    sharedStyles.headingTitle,
    input.level === 1
      ? sharedStyles.headingTitleLevel1
      : input.level === 3
        ? sharedStyles.headingTitleLevel3
        : sharedStyles.headingTitleLevel2,
  )
  const title =
    input.level === 1
      ? h.h1(titleAttrs, [input.title])
      : input.level === 3
        ? h.h3(titleAttrs, [input.title])
        : h.h2(titleAttrs, [input.title])

  return h.div(
    [
      ...stylexAttrs<Message>(sharedStyles.headingRoot),
      ...(input.className === undefined ? [] : [h.Class(input.className)]),
    ],
    [
      input.eyebrow === undefined
        ? null
        : h.p([h.Class(clsx(eyebrowClass, 'mb-2'))], [input.eyebrow]),
      title,
      input.body === undefined
        ? null
        : h.p(stylexAttrs<Message>(sharedStyles.headingBody), [input.body]),
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
      ...stylexAttrs<Message>(
        ...buttonStyles(
          input.variant ?? 'primary',
          input.size ?? 'md',
          input.block === true,
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
      ...stylexAttrs<Message>(
        ...buttonStyles(
          input.variant ?? 'primary',
          input.size ?? 'md',
          input.block === true,
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
      ...stylexAttrs<Message>(sharedStyles.textLink),
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
      ...stylexAttrs<Message>(
        sharedStyles.avatarImage,
        avatarSizeStyle(input.size),
      ),
    ])
  }

  return h.div(
    [
      kitFamily<Message>('elements/avatars'),
      ...stylexAttrs<Message>(
        sharedStyles.avatarFallback,
        avatarSizeStyle(input.size),
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
      ...stylexAttrs<Message>(sharedStyles.avatarGroup),
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
      ...stylexAttrs<Message>(sharedStyles.buttonGroup),
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
      [sharedStyles.dropdown],
    ),
    [
      h.button(
        [
          kitFamily<Message>('elements/dropdowns'),
          h.Type('button'),
          ...stylexAttrs<Message>(sharedStyles.dropdownButton),
        ],
        [h.span([], [input.label]), h.span([h.AriaHidden(true)], ['v'])],
      ),
      h.ul(
        [
          h.Role('list'),
          ...stylexAttrs<Message>(sharedStyles.dropdownList),
        ],
        input.items.map(item =>
          h.li(
            [],
            [
              h.a(
                [
                  h.Href(item.href),
                  ...stylexAttrs<Message>(
                    sharedStyles.dropdownItem,
                    item.active === true
                      ? sharedStyles.dropdownItemActive
                      : null,
                  ),
                ],
                [
                  h.span(
                    stylexAttrs<Message>(sharedStyles.dropdownLabel),
                    [item.label],
                  ),
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
