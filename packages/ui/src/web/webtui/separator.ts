import { hr } from "@typed/ui/hyperscript"

export type SeparatorProps = {
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const Separator = (props: SeparatorProps): any => {
  const { className, style, ...otherProps } = props
  
  const attributes: Record<string, any> = {
    ...otherProps
  }
  
  if (className) {
    attributes.className = className
  }
  
  if (style) {
    attributes.style = style
  }

  return hr(attributes)
}