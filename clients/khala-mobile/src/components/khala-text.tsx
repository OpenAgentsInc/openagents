import { forwardRef, type ReactNode } from "react"
import { Text as RNText, type TextProps as RNTextProps } from "react-native"

export type KhalaTextVariant =
  | "body"
  | "caption"
  | "danger"
  | "faint"
  | "heading"
  | "label"
  | "mono"
  | "muted"
  | "success"
  | "warning"

export type KhalaTextProps = RNTextProps &
  Readonly<{
    children?: ReactNode
    className?: string
    text?: string
    variant?: KhalaTextVariant
  }>

const variantClassName: Record<KhalaTextVariant, string> = {
  body: "font-sans text-base text-text",
  caption: "font-sans text-sm text-textMuted",
  danger: "font-sans text-sm text-danger",
  faint: "font-mono text-xs text-textFaint",
  heading: "font-sans text-2xl font-semibold text-text",
  label: "font-mono text-xs uppercase tracking-wide text-textFaint",
  mono: "font-mono text-sm text-text",
  muted: "font-sans text-sm text-textMuted",
  success: "font-sans text-sm text-success",
  warning: "font-sans text-sm text-warning",
}

export const KhalaText = forwardRef<RNText, KhalaTextProps>(
  ({ children, className = "", text, variant = "body", ...props }, ref) => (
    <RNText
      {...props}
      className={`${variantClassName[variant]} ${className}`.trim()}
      ref={ref}
    >
      {text ?? children}
    </RNText>
  ),
)

KhalaText.displayName = "KhalaText"
