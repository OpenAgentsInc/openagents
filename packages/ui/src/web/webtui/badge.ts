import { span } from "@typed/ui/hyperscript"

export type BadgeVariant = 
  | "foreground0" 
  | "foreground1" 
  | "foreground2" 
  | "background0" 
  | "background1" 
  | "background2" 
  | "background3"

export type BadgeCap = 
  | "round" 
  | "triangle" 
  | "slant-top" 
  | "slant-bottom" 
  | "ribbon"

export type BadgeProps = {
  children: string
  variant?: BadgeVariant | undefined
  capStart?: BadgeCap | undefined
  capEnd?: BadgeCap | undefined
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const Badge = (props: BadgeProps): any => {
  const { children, variant, capStart, capEnd, className, style, ...otherProps } = props
  
  const attributes: Record<string, any> = {
    [`is-`]: "badge",
    ...otherProps
  }
  
  // Add variant attribute if specified
  if (variant) {
    attributes[`variant-`] = variant
  }
  
  // Add cap attributes if specified
  if (capStart) {
    attributes[`cap-^`] = capStart
  }
  
  if (capEnd) {
    attributes[`cap-$`] = capEnd
  }
  
  // Add className if provided
  if (className) {
    attributes.className = className
  }
  
  // Add style if provided
  if (style) {
    attributes.style = style
  }

  return span(attributes, children)
}