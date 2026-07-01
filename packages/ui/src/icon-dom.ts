import { iconSvg, type IconName } from './icon'

export type IconElementOptions = Readonly<{
  ariaHidden?: boolean
  className?: string
  dataIcon?: string
  title?: string
}>

export const iconElement = (
  name: IconName,
  options: IconElementOptions = {},
): HTMLSpanElement => {
  const element = document.createElement('span')
  const classes = ['oa-ui-icon', options.className].filter(
    (value): value is string => value !== undefined && value.length > 0,
  )

  element.className = classes.join(' ')
  element.dataset.oaUiIcon = options.dataIcon ?? name
  if (options.ariaHidden !== false) {
    element.setAttribute('aria-hidden', 'true')
  }
  if (options.title !== undefined) {
    element.title = options.title
  }
  element.innerHTML = iconSvg(name)

  return element
}
