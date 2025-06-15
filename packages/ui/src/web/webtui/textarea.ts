import { textarea } from "@typed/ui/hyperscript"

export type WebTUITextareaSize = "small" | "default" | "large"

export type WebTUITextareaProps = {
  placeholder?: string | undefined
  value?: string | undefined
  size?: WebTUITextareaSize | undefined
  disabled?: boolean | undefined
  rows?: number | undefined
  cols?: number | undefined
  onChange?: any
  onInput?: any
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const WebTUITextarea = (props: WebTUITextareaProps): any => {
  const { className, cols, disabled, onChange, onInput, placeholder, rows, size, style, value, ...otherProps } = props

  const attributes: Record<string, any> = {
    ...otherProps
  }

  if (placeholder) {
    attributes.placeholder = placeholder
  }

  if (value !== undefined) {
    attributes.value = value
  }

  if (size && size !== "default") {
    attributes[`size-`] = size
  }

  if (disabled) {
    attributes.disabled = true
  }

  if (rows) {
    attributes.rows = rows
  }

  if (cols) {
    attributes.cols = cols
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

  return textarea(attributes, value || "")
}
