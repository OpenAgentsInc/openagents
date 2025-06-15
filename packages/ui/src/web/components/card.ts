import { div } from "@typed/ui/hyperscript"

export type CardProps = {
  children: Array<any>
  size?: "default" | "sm" | "lg"
  className?: string
}

export type CardHeaderProps = {
  children: Array<any>
  className?: string
}

export type CardTitleProps = {
  children: string
  className?: string
}

export type CardDescriptionProps = {
  children: string
  className?: string
}

export type CardContentProps = {
  children: string
  className?: string
}

export type CardFooterProps = {
  children: any
  className?: string
}

export const Card = (props: CardProps): any => {
  const baseStyle =
    "font-family: monospace; background-color: #fff; color: #000; border: 1px solid #ccc; display: flex; flex-direction: column; box-shadow: 0 1px 3px rgba(0,0,0,0.1);"

  const sizeStyles = {
    default: "padding: 24px;",
    sm: "padding: 16px;",
    lg: "padding: 32px;"
  }

  const size = props.size || "default"
  const style = [baseStyle, sizeStyles[size]].filter(Boolean).join(" ")

  return div({ style, className: props.className }, ...props.children)
}

export const CardHeader = (props: CardHeaderProps): any => {
  const style = "display: flex; flex-direction: column; gap: 6px;"
  return div({ style, className: props.className }, ...props.children)
}

export const CardTitle = (props: CardTitleProps): any => {
  const style = "font-weight: 600; line-height: 1; letter-spacing: -0.025em;"
  return div({ style, className: props.className }, props.children)
}

export const CardDescription = (props: CardDescriptionProps): any => {
  const style = "font-size: 14px; color: #6b7280;"
  return div({ style, className: props.className }, props.children)
}

export const CardContent = (props: CardContentProps): any => {
  return div({ className: props.className }, props.children)
}

export const CardFooter = (props: CardFooterProps): any => {
  const style = "display: flex; align-items: center; padding-top: 24px;"
  return div({ style, className: props.className }, props.children)
}
