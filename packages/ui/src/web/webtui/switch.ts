import { input, label } from "@typed/ui/hyperscript"

export type SwitchProps = {
  id?: string | undefined
  checked?: boolean | undefined
  disabled?: boolean | undefined
  onChange?: any
  children?: string | undefined
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const Switch = (props: SwitchProps): any => {
  const { id, checked, disabled, onChange, children, className, style, ...otherProps } = props
  
  const inputAttributes: Record<string, any> = {
    type: "checkbox",
    [`is-`]: "switch",
    ...otherProps
  }
  
  if (id) {
    inputAttributes.id = id
  }
  
  if (checked) {
    inputAttributes.checked = true
  }
  
  if (disabled) {
    inputAttributes.disabled = true
  }
  
  if (onChange) {
    inputAttributes.onChange = onChange
  }
  
  const labelAttributes: Record<string, any> = {}
  
  if (id) {
    labelAttributes.htmlFor = id
  }
  
  if (className) {
    labelAttributes.className = className
  }
  
  if (style) {
    labelAttributes.style = style
  }

  if (children) {
    return label(labelAttributes, [
      input(inputAttributes),
      children
    ])
  }
  
  return input(inputAttributes)
}