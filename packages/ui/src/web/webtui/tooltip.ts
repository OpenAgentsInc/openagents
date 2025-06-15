import { span } from "@typed/ui/hyperscript"

export type TooltipProps = {
  children: any
  tooltip?: string | undefined
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const Tooltip = (props: TooltipProps): any => {
  const { children, tooltip, className, style, ...otherProps } = props
  
  const attributes: Record<string, any> = {
    ...otherProps
  }
  
  if (tooltip) {
    attributes.title = tooltip
  }
  
  if (className) {
    attributes.className = className
  }
  
  if (style) {
    attributes.style = style
  }

  return span(attributes, children)
}