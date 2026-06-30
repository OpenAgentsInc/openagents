import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type BreadcrumbProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  label?: string
  ariaLabel?: string
  dir?: 'ltr' | 'rtl' | 'auto'
}>

export type BreadcrumbListProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type BreadcrumbItemProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type BreadcrumbLinkProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  href: string
  rel?: string
  target?: string
}>

export type BreadcrumbCurrent =
  | 'page'
  | 'step'
  | 'location'
  | 'date'
  | 'time'
  | 'true'

export type BreadcrumbPageProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  current?: BreadcrumbCurrent
}>

export type BreadcrumbSeparatorProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children?: BasecoatChildren
    rtlFlip?: boolean
  }>

export type BreadcrumbEllipsisProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children?: BasecoatChildren
    label?: string
  }>

const breadcrumbRoot = basecoatClass('breadcrumb')

const chevronRight = <Message>(rtlFlip: boolean): Html => {
  const h = html<Message>()

  return h.svg(
    [
      ...(rtlFlip ? [h.DataAttribute('rtl-flip', '')] : []),
      h.Class('lucide lucide-chevron-right'),
      h.Attribute('xmlns', 'http://www.w3.org/2000/svg'),
      h.Attribute('width', '24'),
      h.Attribute('height', '24'),
      h.ViewBox('0 0 24 24'),
      h.Fill('none'),
      h.Attribute('stroke', 'currentColor'),
      h.Attribute('stroke-width', '2'),
      h.Attribute('stroke-linecap', 'round'),
      h.Attribute('stroke-linejoin', 'round'),
    ],
    [h.path([h.D('m9 18 6-6-6-6')], [])],
  )
}

const ellipsisIcon = <Message>(): Html => {
  const h = html<Message>()

  return h.svg(
    [
      h.Class('lucide lucide-ellipsis'),
      h.Attribute('xmlns', 'http://www.w3.org/2000/svg'),
      h.Attribute('width', '24'),
      h.Attribute('height', '24'),
      h.ViewBox('0 0 24 24'),
      h.Fill('none'),
      h.Attribute('stroke', 'currentColor'),
      h.Attribute('stroke-width', '2'),
      h.Attribute('stroke-linecap', 'round'),
      h.Attribute('stroke-linejoin', 'round'),
    ],
    [
      h.circle([
        h.Attribute('cx', '12'),
        h.Attribute('cy', '12'),
        h.Attribute('r', '1'),
      ], []),
      h.circle([
        h.Attribute('cx', '19'),
        h.Attribute('cy', '12'),
        h.Attribute('r', '1'),
      ], []),
      h.circle([
        h.Attribute('cx', '5'),
        h.Attribute('cy', '12'),
        h.Attribute('r', '1'),
      ], []),
    ],
  )
}

export const breadcrumb = <Message>(
  input: BreadcrumbProps<Message>,
): Html => {
  const h = html<Message>()

  return h.nav(
    [
      ...basecoatAttrs<Message>(input, breadcrumbRoot),
      h.AriaLabel(input.ariaLabel ?? input.label ?? 'Breadcrumb'),
      ...(input.dir === undefined ? [] : [h.Attribute('dir', input.dir)]),
    ],
    input.children,
  )
}

export const breadcrumbList = <Message>(
  input: BreadcrumbListProps<Message>,
): Html => {
  const h = html<Message>()

  return h.ol(basecoatAttrs<Message>(input), input.children)
}

export const breadcrumbItem = <Message>(
  input: BreadcrumbItemProps<Message>,
): Html => {
  const h = html<Message>()

  return h.li(basecoatAttrs<Message>(input), input.children)
}

export const breadcrumbLink = <Message>(
  input: BreadcrumbLinkProps<Message>,
): Html => {
  const h = html<Message>()

  return h.a(
    [
      ...basecoatAttrs<Message>(input),
      h.Href(input.href),
      ...(input.rel === undefined ? [] : [h.Rel(input.rel)]),
      ...(input.target === undefined ? [] : [h.Target(input.target)]),
    ],
    input.children,
  )
}

export const breadcrumbPage = <Message>(
  input: BreadcrumbPageProps<Message>,
): Html => {
  const h = html<Message>()

  return h.span(
    [
      ...basecoatAttrs<Message>(input),
      h.AriaCurrent(input.current ?? 'page'),
    ],
    input.children,
  )
}

export const breadcrumbSeparator = <Message>(
  input: BreadcrumbSeparatorProps<Message> = {},
): Html => {
  const h = html<Message>()

  return h.li(
    [
      ...basecoatAttrs<Message>(input),
      h.AriaHidden(true),
    ],
    input.children ?? [chevronRight<Message>(input.rtlFlip !== false)],
  )
}

export const breadcrumbEllipsis = <Message>(
  input: BreadcrumbEllipsisProps<Message> = {},
): Html => {
  const h = html<Message>()

  return h.li(
    basecoatAttrs<Message>(input),
    [
      h.span(
        [h.AriaHidden(true)],
        input.children ?? [ellipsisIcon<Message>()],
      ),
      h.span([h.Class('sr-only')], [input.label ?? 'More pages']),
    ],
  )
}
