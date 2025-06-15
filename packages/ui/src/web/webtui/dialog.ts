import { dialog } from "@typed/ui/hyperscript"

export type DialogPosition = 
  | "start" | "center" | "end"
  | "start start" | "center start" | "end start"
  | "start center" | "center center" | "end center" 
  | "start end" | "center end" | "end end"

export type DialogContainer = "auto" | "fill"
export type DialogSize = "small" | "default" | "full"

export type WebTUIDialogProps = {
  children: any
  position?: DialogPosition | undefined
  container?: DialogContainer | undefined
  size?: DialogSize | undefined
  popover?: boolean | undefined
  id?: string | undefined
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const WebTUIDialog = (props: WebTUIDialogProps): any => {
  const { children, position, container, size, popover, id, className, style, ...otherProps } = props
  
  const attributes: Record<string, any> = {
    ...otherProps
  }
  
  if (id) {
    attributes.id = id
  }
  
  if (position) {
    attributes[`position-`] = position
  }
  
  if (container) {
    attributes[`container-`] = container
  }
  
  if (size && size !== "default") {
    attributes[`size-`] = size
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

  return dialog(attributes, children)
}