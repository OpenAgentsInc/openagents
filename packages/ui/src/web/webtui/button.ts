import { button } from "@typed/ui/hyperscript"

export type WebTUIButtonVariant =
  | "foreground0"
  | "foreground1"
  | "foreground2"
  | "background0"
  | "background1"
  | "background2"
  | "background3"

export type WebTUIButtonSize = "small" | "default" | "large"

export type WebTUIButtonBox = "square" | "round" | "double"

export type WebTUIButtonShear = "top" | "bottom" | "both"

export type WebTUIButtonProps = {
  children: string
  variant?: WebTUIButtonVariant | undefined
  size?: WebTUIButtonSize | undefined
  box?: WebTUIButtonBox | undefined
  shear?: WebTUIButtonShear | undefined
  disabled?: boolean | undefined
  onClick?: any
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const WebTUIButton = (props: WebTUIButtonProps): any => {
  const { box, children, className, disabled, onClick, shear, size, style, variant, ...otherProps } = props

  const attributes: Record<string, any> = {
    ...otherProps
  }

  // Add variant attribute if specified
  if (variant) {
    attributes[`variant-`] = variant
  }

  // Add size attribute if specified (WebTUI default is handled by CSS)
  if (size && size !== "default") {
    attributes[`size-`] = size
  }

  // Add box border attribute if specified
  if (box) {
    attributes[`box-`] = box
  }

  // Add shear attribute if specified
  if (shear) {
    attributes[`shear-`] = shear
  }

  // Add disabled state
  if (disabled) {
    attributes.disabled = true
  }

  // Add click handler
  if (onClick) {
    attributes.onClick = onClick
  }

  // Add className if provided
  if (className) {
    attributes.className = className
  }

  // Add style if provided
  if (style) {
    attributes.style = style
  }

  return button(attributes, children)
}
