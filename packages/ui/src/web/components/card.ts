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
  const baseStyle = "font-mono bg-card text-card-foreground border border-border flex flex-col shadow-sm"

  const sizeStyles = {
    default: "p-6",
    sm: "p-4",
    lg: "p-8"
  }

  const size = props.size || "default"
  const className = [baseStyle, sizeStyles[size], props.className].filter(Boolean).join(" ")

  return div({ className }, ...props.children)
}

export const CardHeader = (props: CardHeaderProps): any => {
  const className = ["flex flex-col space-y-1.5", props.className].filter(Boolean).join(" ")
  return div({ className }, ...props.children)
}

export const CardTitle = (props: CardTitleProps): any => {
  const className = ["font-semibold leading-none tracking-tight", props.className].filter(Boolean).join(" ")
  return div({ className }, props.children)
}

export const CardDescription = (props: CardDescriptionProps): any => {
  const className = ["text-sm text-muted-foreground", props.className].filter(Boolean).join(" ")
  return div({ className }, props.children)
}

export const CardContent = (props: CardContentProps): any => {
  const className = ["", props.className].filter(Boolean).join(" ")
  return div({ className }, props.children)
}

export const CardFooter = (props: CardFooterProps): any => {
  const className = ["flex items-center pt-6", props.className].filter(Boolean).join(" ")
  return div({ className }, props.children)
}
