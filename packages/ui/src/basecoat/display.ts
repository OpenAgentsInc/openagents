import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type AlertVariant = 'default' | 'destructive'

export type AlertProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  variant?: AlertVariant
}>

export type AlertTitleProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  level?: 2 | 3 | 4 | 5 | 6
}>

export type AlertDescriptionProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
  }>

export type AlertFooterProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type AvatarSize = 'default' | 'sm' | 'lg'

export type AvatarProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  size?: AvatarSize
}>

export type AvatarImageProps<Message> = BasecoatAttrs<Message> & Readonly<{
  alt: string
  src: string
}>

export type AvatarFallbackProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
  }>

export type AvatarBadgeProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children?: BasecoatChildren
}>

export type AvatarGroupProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: ReadonlyArray<Html>
}>

export type AvatarGroupCountProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
  }>

export type SkeletonProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children?: BasecoatChildren
}>

const alertRoot = basecoatClass('alert')
const avatarRoot = basecoatClass('avatar')
const avatarBadgeClass = basecoatClass('avatar-badge')
const avatarGroupRoot = basecoatClass('avatar-group')
const skeletonRoot = basecoatClass('skeleton')

export const alert = <Message>(input: AlertProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, alertRoot),
      h.Role('alert'),
      ...dataAttr<Message>(
        'variant',
        input.variant === 'default' ? undefined : input.variant,
      ),
    ],
    input.children,
  )
}

export const alertTitle = <Message>(
  input: AlertTitleProps<Message>,
): Html => {
  const h = html<Message>()
  const attrs = basecoatAttrs<Message>(input)

  switch (input.level) {
    case 3:
      return h.h3(attrs, input.children)
    case 4:
      return h.h4(attrs, input.children)
    case 5:
      return h.h5(attrs, input.children)
    case 6:
      return h.h6(attrs, input.children)
    case 2:
    default:
      return h.h2(attrs, input.children)
  }
}

export const alertDescription = <Message>(
  input: AlertDescriptionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.section(basecoatAttrs<Message>(input), input.children)
}

export const alertFooter = <Message>(
  input: AlertFooterProps<Message>,
): Html => {
  const h = html<Message>()

  return h.footer(basecoatAttrs<Message>(input), input.children)
}

export const avatar = <Message>(input: AvatarProps<Message>): Html => {
  const h = html<Message>()

  return h.span(
    [
      ...basecoatAttrs<Message>(input, avatarRoot),
      ...dataAttr<Message>(
        'size',
        input.size === 'default' ? undefined : input.size,
      ),
    ],
    input.children,
  )
}

export const avatarImage = <Message>(
  input: AvatarImageProps<Message>,
): Html => {
  const h = html<Message>()

  return h.img([
    ...basecoatAttrs<Message>(input),
    h.Src(input.src),
    h.Alt(input.alt),
  ])
}

export const avatarFallback = <Message>(
  input: AvatarFallbackProps<Message>,
): Html => {
  const h = html<Message>()

  return h.span(basecoatAttrs<Message>(input), input.children)
}

export const avatarBadge = <Message>(
  input: AvatarBadgeProps<Message>,
): Html => {
  const h = html<Message>()

  return h.span(
    basecoatAttrs<Message>(input, avatarBadgeClass),
    input.children ?? [],
  )
}

export const avatarGroup = <Message>(
  input: AvatarGroupProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    basecoatAttrs<Message>(input, avatarGroupRoot),
    input.children,
  )
}

export const avatarGroupCount = <Message>(
  input: AvatarGroupCountProps<Message>,
): Html => {
  const h = html<Message>()

  return h.span(
    [
      ...basecoatAttrs<Message>(input),
      h.DataAttribute('count', ''),
    ],
    input.children,
  )
}

export const skeleton = <Message>(input: SkeletonProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    basecoatAttrs<Message>(input, skeletonRoot),
    input.children ?? [],
  )
}
