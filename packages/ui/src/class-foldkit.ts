import type { Attribute } from 'foldkit/html'
import { html } from 'foldkit/html'

/** A component style is a stable CSS class name, or a falsy value to skip. */
export type ComponentClass = string
export type MaybeComponentClass = ComponentClass | false | null | undefined

export const componentClass = (className: string): ComponentClass => className

const classNamesFrom = (
  styles: ReadonlyArray<MaybeComponentClass>,
): string =>
  styles.filter((value): value is ComponentClass => typeof value === 'string' && value !== '').join(' ')

export const classAttrs = <Message>(
  ...styles: ReadonlyArray<MaybeComponentClass>
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()
  const className = classNamesFrom(styles)
  return className === '' ? [] : [h.Class(className)]
}

export const classAttrsWithClass = <Message>(
  className: string,
  ...styles: ReadonlyArray<MaybeComponentClass>
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()
  const classes = [className, classNamesFrom(styles)]
    .filter(value => value !== '')
    .join(' ')
  return classes === '' ? [] : [h.Class(classes)]
}
