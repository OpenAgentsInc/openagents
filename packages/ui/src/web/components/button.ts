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
  const baseStyle = "cursor-pointer inline-flex items-center justify-center whitespace-nowrap text-sm font-medium font-mono transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 rounded-none"

  const variantStyles = {
    default: "bg-primary text-primary-foreground hover:bg-primary/90",
    destructive: "bg-destructive text-destructive-foreground hover:bg-destructive/90",
    outline: "border border-input bg-background hover:bg-accent hover:text-accent-foreground",
    secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/80",
    ghost: "hover:bg-accent hover:text-accent-foreground",
    link: "text-primary underline-offset-4 hover:underline"
  }

  const sizeStyles = {
    default: "h-9 px-4 py-2",
    sm: "h-8 px-3 text-xs",
    lg: "h-10 px-8",
    icon: "h-9 w-9"
  }

  const variant = props.variant || "default"
  const size = props.size || "default"
  
  const className = [
    baseStyle,
    variantStyles[variant],
    sizeStyles[size],
    props.className
  ].filter(Boolean).join(" ")

  return button(
    {
      className,
      disabled: props.disabled ?? false,
      onClick: props.onClick
    },
    props.children
  )
}