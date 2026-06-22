import * as stylex from '@stylexjs/stylex'
import type { StyleXStyles } from '@stylexjs/stylex'
import type { Attribute } from 'foldkit/html'
import { html } from 'foldkit/html'

export type StylexStyle = StyleXStyles

type StylexAttrs = Readonly<{
  class?: string
  style?: string
  'data-style-src'?: string
}>

export const stylexAttrs = <Message>(
  ...styles: ReadonlyArray<StylexStyle>
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()
  const attrs = (
    stylex.attrs as (...compiledStyles: ReadonlyArray<StylexStyle>) => StylexAttrs
  )(...styles)
  const result: Array<Attribute<Message>> = []

  if (attrs.class !== undefined) {
    result.push(h.Class(attrs.class))
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
  }) as unknown as StylexStyle
