import * as stylex from '@stylexjs/stylex'
import type { StyleXStyles } from '@stylexjs/stylex'
import type { Attribute } from 'foldkit/html'
import { html } from 'foldkit/html'

export type StylexStyle = StyleXStyles
type StylexFallbackStyle = StylexStyle & {
  readonly __openagentsStylexFallbackClassName: string
}
export type StylexMaybeStyle =
  | StylexStyle
  | StylexFallbackStyle
  | false
  | null
  | undefined

type StylexAttrs = Readonly<{
  class?: string
  style?: string
  'data-style-src'?: string
}>

type StylexAttrParts = StylexAttrs &
  Readonly<{
    className?: string
  }>

const stylexAttrParts = (
  styles: ReadonlyArray<StylexMaybeStyle>,
): StylexAttrParts => {
  const compiledStyles = styles.filter(Boolean) as ReadonlyArray<StylexStyle>
  const fallbackClasses = compiledStyles.flatMap(style =>
    typeof style === 'object' &&
    style !== null &&
    '__openagentsStylexFallbackClassName' in style
      ? [(style as StylexFallbackStyle).__openagentsStylexFallbackClassName]
      : [],
  )
  const attrs = (
    stylex.attrs as (...compiledStyles: ReadonlyArray<StylexStyle>) => StylexAttrs
  )(
    ...compiledStyles.filter(
      style =>
        !(
          typeof style === 'object' &&
          style !== null &&
          '__openagentsStylexFallbackClassName' in style
        ),
    ),
  )
  const className = [fallbackClasses.join(' '), attrs.class]
    .filter(value => value !== undefined && value !== '')
    .join(' ')

  return className === '' ? attrs : { ...attrs, className }
}

export const stylexAttrs = <Message>(
  ...styles: ReadonlyArray<StylexMaybeStyle>
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()
  const attrs = stylexAttrParts(styles)
  const result: Array<Attribute<Message>> = []

  if (attrs.className !== undefined) {
    result.push(h.Class(attrs.className))
  }

  if (attrs.style !== undefined) {
    result.push(h.Attribute('style', attrs.style))
  }

  if (attrs['data-style-src'] !== undefined) {
    result.push(h.DataAttribute('style-src', attrs['data-style-src']))
  }

  return result
}

export const stylexAttrsWithClass = <Message>(
  className: string,
  ...styles: ReadonlyArray<StylexMaybeStyle>
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()
  const attrs = stylexAttrParts(styles)
  const result: Array<Attribute<Message>> = []
  const classes = [className, attrs.className]
    .filter(value => value !== undefined && value !== '')
    .join(' ')

  if (classes !== '') {
    result.push(h.Class(classes))
  }

  if (attrs.style !== undefined) {
    result.push(h.Attribute('style', attrs.style))
  }

  if (attrs['data-style-src'] !== undefined) {
    result.push(h.DataAttribute('style-src', attrs['data-style-src']))
  }

  return result
}

export const stylexRuntimeFallbackEnabled = (): boolean =>
  ((globalThis as { Bun?: { env?: Record<string, string | undefined> } }).Bun
    ?.env?.OA_STYLEX_RUNTIME_FALLBACK === '1' ||
    (globalThis as { process?: { env?: Record<string, string | undefined> } })
      .process?.env?.OA_STYLEX_RUNTIME_FALLBACK === '1')

export const stylexFallback = (className: string): StylexStyle =>
  ({
    $$css: true,
    [className]: className,
    __openagentsStylexFallbackClassName: className,
  }) as unknown as StylexFallbackStyle
