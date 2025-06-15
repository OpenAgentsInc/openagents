import { div } from "@typed/ui/hyperscript"

export type PopoverPosition = 
  | "top" | "top-start" | "top-end"
  | "bottom" | "bottom-start" | "bottom-end"
  | "left" | "left-start" | "left-end"
  | "right" | "right-start" | "right-end"

export type PopoverProps = {
  children: any
  position?: PopoverPosition | undefined
  id?: string | undefined
  popover?: boolean | undefined
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const Popover = (props: PopoverProps): any => {
  const { children, position, id, popover, className, style, ...otherProps } = props
  
  const attributes: Record<string, any> = {
    ...otherProps
  }
  
  if (id) {
    attributes.id = id
  }
  
  if (position) {
    attributes[`position-`] = position
  }
  
  if (popover) {
    attributes.popover = true
  }
  
  if (className) {
    attributes.className = className
  }
  
  if (style) {
    attributes.style = style
  }

  return div(attributes, children)
}