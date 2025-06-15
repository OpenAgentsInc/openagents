import { input } from "@typed/ui/hyperscript"

export type WebTUIInputSize = "small" | "default" | "large"

export type WebTUIInputProps = {
  type?: string | undefined
  placeholder?: string | undefined
  value?: string | undefined
  size?: WebTUIInputSize | undefined
  disabled?: boolean | undefined
  onChange?: any
  onInput?: any
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const WebTUIInput = (props: WebTUIInputProps): any => {
  const { className, disabled, onChange, onInput, placeholder, size, style, type = "text", value, ...otherProps } =
    props

  const attributes: Record<string, any> = {
    type,
    ...otherProps
  }

  if (placeholder) {
    attributes.placeholder = placeholder
  }

  if (value !== undefined) {
    attributes.value = value
  }

  // Add size attribute if specified (WebTUI default is handled by CSS)
  if (size && size !== "default") {
    attributes[`size-`] = size
  }

  if (disabled) {
    attributes.disabled = true
  }

  if (onChange) {
    attributes.onChange = onChange
  }

  if (onInput) {
    attributes.onInput = onInput
  }

  if (className) {
    attributes.className = className
  }

  if (style) {
    attributes.style = style
  }

  return input(attributes)
}
