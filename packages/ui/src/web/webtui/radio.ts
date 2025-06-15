import { input, label } from "@typed/ui/hyperscript"

export type RadioProps = {
  id?: string | undefined
  name?: string | undefined
  value?: string | undefined
  checked?: boolean | undefined
  disabled?: boolean | undefined
  onChange?: any
  children?: string | undefined
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const Radio = (props: RadioProps): any => {
  const { checked, children, className, disabled, id, name, onChange, style, value, ...otherProps } = props

  const inputAttributes: Record<string, any> = {
    type: "radio",
    ...otherProps
  }

  if (id) {
    inputAttributes.id = id
  }

  if (name) {
    inputAttributes.name = name
  }

  if (value) {
    inputAttributes.value = value
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
