import { clsx } from 'clsx'
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
  classAttrs,
  componentClass,
} from './class-foldkit'

const sharedStyles = {
  headingRoot: componentClass('oa-ui-heading-root'),
  headingTitle: componentClass('oa-ui-heading-title'),
  headingTitleLevel1: componentClass('oa-ui-heading-title-level-1'),
  headingTitleLevel2: componentClass('oa-ui-heading-title-level-2'),
  headingTitleLevel3: componentClass('oa-ui-heading-title-level-3'),
  headingBody: componentClass('oa-ui-heading-body'),
  button: componentClass('oa-ui-button'),
  buttonSm: componentClass('oa-ui-button-sm'),
  buttonMd: componentClass('oa-ui-button-md'),
  buttonBlock: componentClass('oa-ui-button-block'),
  buttonPrimary: componentClass('oa-ui-button-primary'),
  buttonSecondary: componentClass('oa-ui-button-secondary'),
  buttonGhost: componentClass('oa-ui-button-ghost'),
  buttonDanger: componentClass('oa-ui-button-danger'),
  textLink: componentClass('oa-ui-text-link'),
  avatarImage: componentClass('oa-ui-avatar-image'),
  avatarFallback: componentClass('oa-ui-avatar-fallback'),
  avatarSm: componentClass('oa-ui-avatar-sm'),
  avatarMd: componentClass('oa-ui-avatar-md'),
  avatarLg: componentClass('oa-ui-avatar-lg'),
  avatarGroup: componentClass('oa-ui-avatar-group'),
  buttonGroup: componentClass('oa-ui-button-group'),
  dropdown: componentClass('oa-ui-dropdown'),
  dropdownButton: componentClass('oa-ui-dropdown-button'),
  dropdownList: componentClass('oa-ui-dropdown-list'),
  dropdownItem: componentClass('oa-ui-dropdown-item'),
  dropdownItemActive: componentClass('oa-ui-dropdown-item-active'),
  dropdownLabel: componentClass('oa-ui-dropdown-label'),
}

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
  styles: ReadonlyArray<ReturnType<typeof componentClass> | null>,
): ReadonlyArray<Attribute<Message>> => [
  ...(attrs ?? []),
  ...classAttrs<Message>(...styles),
]

export const headingBlock = <Message>(input: {
  eyebrow?: string
  title: string
  body?: string
  level?: 1 | 2 | 3
  className?: string
}): Html => {
  const h = html<Message>()
  const titleAttrs = classAttrs<Message>(
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
      ...classAttrs<Message>(sharedStyles.headingRoot),
      ...(input.className === undefined ? [] : [h.Class(input.className)]),
    ],
    [
      input.eyebrow === undefined
        ? null
        : h.p([h.Class(clsx(eyebrowClass, 'mb-2'))], [input.eyebrow]),
      title,
      input.body === undefined
        ? null
        : h.p(classAttrs<Message>(sharedStyles.headingBody), [input.body]),
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
      ...classAttrs<Message>(
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
      ...classAttrs<Message>(
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
      ...classAttrs<Message>(sharedStyles.textLink),
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
      ...classAttrs<Message>(
        sharedStyles.avatarImage,
        avatarSizeStyle(input.size),
      ),
    ])
  }

  return h.div(
    [
      kitFamily<Message>('elements/avatars'),
      ...classAttrs<Message>(
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
      ...classAttrs<Message>(sharedStyles.avatarGroup),
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
      ...classAttrs<Message>(sharedStyles.buttonGroup),
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
          ...classAttrs<Message>(sharedStyles.dropdownButton),
        ],
        [h.span([], [input.label]), h.span([h.AriaHidden(true)], ['v'])],
      ),
      h.ul(
        [
          h.Role('list'),
          ...classAttrs<Message>(sharedStyles.dropdownList),
        ],
        input.items.map(item =>
          h.li(
            [],
            [
              h.a(
                [
                  h.Href(item.href),
                  ...classAttrs<Message>(
                    sharedStyles.dropdownItem,
                    item.active === true
                      ? sharedStyles.dropdownItemActive
                      : null,
                  ),
                ],
                [
                  h.span(
                    classAttrs<Message>(sharedStyles.dropdownLabel),
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
