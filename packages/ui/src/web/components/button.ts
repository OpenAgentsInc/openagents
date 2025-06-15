import { button } from "@typed/ui/hyperscript"

export type ButtonProps = {
  children: string
  variant?: "default" | "destructive" | "outline" | "secondary" | "ghost" | "link"
  size?: "default" | "sm" | "lg" | "icon"
  disabled?: boolean
  onClick?: any
  className?: string
}

export const Button = (props: ButtonProps): any => {
  const baseStyle = "display: inline-flex; align-items: center; justify-content: center; white-space: nowrap; font-size: 14px; font-weight: 500; font-family: monospace; transition: colors 0.2s; cursor: pointer; border: none; border-radius: 0;"

  const variantStyles = {
    default: "background-color: #000; color: #fff;",
    destructive: "background-color: #dc2626; color: #fff;",
    outline: "background-color: transparent; color: #000; border: 1px solid #ccc;",
    secondary: "background-color: #6b7280; color: #fff;",
    ghost: "background-color: transparent; color: #000;",
    link: "background-color: transparent; color: #3b82f6; text-decoration: underline;"
  }

  const sizeStyles = {
    default: "height: 36px; padding: 8px 16px;",
    sm: "height: 32px; padding: 4px 12px; font-size: 12px;",
    lg: "height: 40px; padding: 8px 32px;",
    icon: "height: 36px; width: 36px; padding: 8px;"
  }

  const variant = props.variant || "default"
  const size = props.size || "default"

  const style = [
    baseStyle,
    variantStyles[variant],
    sizeStyles[size]
  ].filter(Boolean).join(" ")

  return button(
    {
      style,
      className: props.className,
      disabled: props.disabled ?? false,
      onClick: props.onClick
    },
    props.children
  )
}
