import { div } from "@typed/ui/hyperscript"

export type CardProps = {
  children: Array<any>
  size?: "default" | "sm" | "lg" | undefined
  className?: string | undefined
}

export type CardHeaderProps = {
  children: Array<any>
  className?: string | undefined
}

export type CardTitleProps = {
  children: string
  className?: string | undefined
}

export type CardDescriptionProps = {
  children: string
  className?: string | undefined
}

export type CardContentProps = {
  children: string
  className?: string | undefined
}

export type CardFooterProps = {
  children: any
  className?: string | undefined
}

export const Card = (props: CardProps): any => {
  const baseStyle =
    "font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace; background-color: hsl(0 0% 100%); color: hsl(222.2 84% 4.9%); border-width: 1px; border-color: hsl(214.3 31.8% 91.4%); display: flex; flex-direction: column; border-radius: 0.5rem; box-shadow: 0 1px 3px 0 rgb(0 0 0 / 0.1), 0 1px 2px -1px rgb(0 0 0 / 0.1);"

  const sizeStyles = {
    default: "padding: 1.5rem;",
    sm: "padding: 1rem;",
    lg: "padding: 2rem;"
  }

  const size = props.size || "default"
  const style = [baseStyle, sizeStyles[size]].filter(Boolean).join(" ")

  return div({ style, className: props.className }, ...props.children)
}

export const CardHeader = (props: CardHeaderProps): any => {
  const style = "display: flex; flex-direction: column; gap: 0.375rem;"
  return div({ style, className: props.className }, ...props.children)
}

export const CardTitle = (props: CardTitleProps): any => {
  const style = "font-size: 1.5rem; line-height: 2rem; font-weight: 600; line-height: 1; letter-spacing: -0.025em;"
  return div({ style, className: props.className }, props.children)
}

export const CardDescription = (props: CardDescriptionProps): any => {
  const style = "font-size: 0.875rem; line-height: 1.25rem; color: hsl(215.4 16.3% 46.9%);"
  return div({ style, className: props.className }, props.children)
}

export const CardContent = (props: CardContentProps): any => {
  return div({ className: props.className }, props.children)
}

export const CardFooter = (props: CardFooterProps): any => {
  const style = "display: flex; align-items: center; padding-top: 1.5rem;"
  return div({ style, className: props.className }, props.children)
}
