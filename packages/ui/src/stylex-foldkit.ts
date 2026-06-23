import type { Attribute } from 'foldkit/html'
import { html } from 'foldkit/html'

/**
 * #6046: StyleX removed.
 *
 * This module used to bridge `@stylexjs/stylex` compiled styles into Foldkit
 * attributes. StyleX's `stylex.create` runs `window`-dependent code at module
 * load, which threw `window is not defined` whenever the renderer's import
 * graph was mounted headless (the desktop app-replica harness) and forced the
 * `OA_STYLEX_RUNTIME_FALLBACK` shim hack.
 *
 * The bridge is now a thin class-name carrier with NO StyleX dependency: a
 * "style" is just a CSS class name (a {@link StylexStyle}). Components keep
 * emitting the same stable `oa-*` class names; the matching CSS lives in plain
 * stylesheets (co-located `*.css` files) instead of StyleX-generated output.
 *
 * The original API surface is preserved so call sites do not have to change.
 */

/** A style is now simply a CSS class name (or a falsy value to skip). */
export type StylexStyle = string
export type StylexMaybeStyle = StylexStyle | false | null | undefined

/** Back-compat: returns the class name directly (no StyleX wrapping). */
export const stylexFallback = (className: string): StylexStyle => className

const classNamesFrom = (
  styles: ReadonlyArray<StylexMaybeStyle>,
): string =>
  styles.filter((value): value is StylexStyle => typeof value === 'string' && value !== '').join(' ')

export const stylexAttrs = <Message>(
  ...styles: ReadonlyArray<StylexMaybeStyle>
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()
  const className = classNamesFrom(styles)
  return className === '' ? [] : [h.Class(className)]
}

export const stylexAttrsWithClass = <Message>(
  className: string,
  ...styles: ReadonlyArray<StylexMaybeStyle>
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()
  const classes = [className, classNamesFrom(styles)]
    .filter(value => value !== '')
    .join(' ')
  return classes === '' ? [] : [h.Class(classes)]
}

/**
 * Back-compat shim. The StyleX runtime fallback no longer exists, so this is
 * always effectively "on" (named classes are the only path). Kept so any
 * remaining callers compile; it always returns `true`.
 */
export const stylexRuntimeFallbackEnabled = (): boolean => true
