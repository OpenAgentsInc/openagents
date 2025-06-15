import { button } from "@typed/ui/hyperscript"

export type ButtonProps = {
  children: string
  variant?: "default" | "destructive" | "outline" | "secondary" | "ghost" | "link" | undefined
  size?: "default" | "sm" | "lg" | "icon" | undefined
  disabled?: boolean | undefined
  onClick?: any
  className?: string | undefined
}

export const Button = (props: ButtonProps): any => {
  const baseStyle =
    "display: inline-flex; align-items: center; justify-content: center; white-space: nowrap; border-radius: 0.375rem; font-size: 0.875rem; line-height: 1.25rem; font-weight: 500; font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace; transition-property: color, background-color, border-color, text-decoration-color, fill, stroke; transition-timing-function: cubic-bezier(0.4, 0, 0.2, 1); transition-duration: 150ms; border: 1px solid transparent; cursor: pointer; outline: 2px solid transparent; outline-offset: 2px;"

  const variantStyles = {
    default: "background-color: rgb(15 23 42); color: rgb(248 250 252); box-shadow: 0 1px 2px 0 rgb(0 0 0 / 0.05);",
    destructive:
      "background-color: rgb(239 68 68); color: rgb(248 250 252); box-shadow: 0 1px 2px 0 rgb(0 0 0 / 0.05);",
    outline:
      "border-color: rgb(226 232 240); background-color: rgb(255 255 255); color: rgb(15 23 42); box-shadow: 0 1px 2px 0 rgb(0 0 0 / 0.05);",
    secondary: "background-color: rgb(241 245 249); color: rgb(15 23 42); box-shadow: 0 1px 2px 0 rgb(0 0 0 / 0.05);",
    ghost: "color: rgb(15 23 42); background-color: transparent;",
    link:
      "color: rgb(15 23 42); text-decoration-line: underline; text-underline-offset: 4px; background-color: transparent;"
  }

  const sizeStyles = {
    default: "height: 2.25rem; padding-left: 1rem; padding-right: 1rem; padding-top: 0.5rem; padding-bottom: 0.5rem;",
    sm:
      "height: 2rem; border-radius: 0.375rem; padding-left: 0.75rem; padding-right: 0.75rem; font-size: 0.75rem; line-height: 1rem;",
    lg: "height: 2.75rem; border-radius: 0.375rem; padding-left: 2rem; padding-right: 2rem;",
    icon: "height: 2.25rem; width: 2.25rem;"
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
