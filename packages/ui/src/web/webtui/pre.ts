import { pre } from "@typed/ui/hyperscript"

export type PreProps = {
  children: string
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const Pre = (props: PreProps): any => {
  const { children, className, style, ...otherProps } = props
  
  const attributes: Record<string, any> = {
    ...otherProps
  }
  
  if (className) {
    attributes.className = className
  }
  
  if (style) {
    attributes.style = style
  }

  return pre(attributes, children)
}