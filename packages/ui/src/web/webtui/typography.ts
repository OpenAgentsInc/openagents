import { code, em, h1, h2, h3, h4, h5, h6, p, span, strong } from "@typed/ui/hyperscript"

export type TypographyVariant = "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "span" | "strong" | "em" | "code"

export type TypographyProps = {
  children: any
  variant?: TypographyVariant | undefined
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const Typography = (props: TypographyProps): any => {
  const { children, className, style, variant = "p", ...otherProps } = props

  const attributes: Record<string, any> = {
    ...otherProps
  }

  if (className) {
    attributes.className = className
  }

  if (style) {
    attributes.style = style
  }

  // Use specific elements to avoid union type issues
  switch (variant) {
    case "h1":
      return h1(attributes, children)
    case "h2":
      return h2(attributes, children)
    case "h3":
      return h3(attributes, children)
    case "h4":
      return h4(attributes, children)
    case "h5":
      return h5(attributes, children)
    case "h6":
      return h6(attributes, children)
    case "span":
      return span(attributes, children)
    case "strong":
      return strong(attributes, children)
    case "em":
      return em(attributes, children)
    case "code":
      return code(attributes, children)
    case "p":
    default:
      return p(attributes, children)
  }
}

// Convenience components
export const Heading1 = (props: Omit<TypographyProps, "variant">): any =>
  Typography({ variant: "h1", ...props } as TypographyProps)

export const Heading2 = (props: Omit<TypographyProps, "variant">): any =>
  Typography({ variant: "h2", ...props } as TypographyProps)

export const Heading3 = (props: Omit<TypographyProps, "variant">): any =>
  Typography({ variant: "h3", ...props } as TypographyProps)

export const Heading4 = (props: Omit<TypographyProps, "variant">): any =>
  Typography({ variant: "h4", ...props } as TypographyProps)

export const Heading5 = (props: Omit<TypographyProps, "variant">): any =>
  Typography({ variant: "h5", ...props } as TypographyProps)

export const Heading6 = (props: Omit<TypographyProps, "variant">): any =>
  Typography({ variant: "h6", ...props } as TypographyProps)

export const Paragraph = (props: Omit<TypographyProps, "variant">): any =>
  Typography({ variant: "p", ...props } as TypographyProps)

export const Strong = (props: Omit<TypographyProps, "variant">): any =>
  Typography({ variant: "strong", ...props } as TypographyProps)

export const Emphasis = (props: Omit<TypographyProps, "variant">): any =>
  Typography({ variant: "em", ...props } as TypographyProps)

export const InlineCode = (props: Omit<TypographyProps, "variant">): any =>
  Typography({ variant: "code", ...props } as TypographyProps)
