import { forwardRef, type ReactNode } from "react"
import { Text as RNText, type TextProps as RNTextProps, type TextStyle } from "react-native"

import { khalaMobileTypography } from "../theme/typography"

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

/** Color + exact size/line-height, as Tailwind classes (arcade's own
 * `$sizeStyles` pixel values, `theme/typography.ts`'s `khalaMobileTextSizes`,
 * via arbitrary-value `text-[Npx]`/`leading-[Npx]` utilities rather than
 * Tailwind's built-in `text-*` scale, which has slightly different
 * line-heights than arcade's at several sizes). Kept as `className` — not
 * inline `style` — specifically so call sites that already override size
 * with their own `className` (there are ~20 of these across the app, e.g.
 * a screen title that needs 36px regardless of variant) keep working:
 * NativeWind's explicit `style` prop always wins over `className` for a
 * conflicting property, so a variant default size in `style` would have
 * silently overridden every one of those call-site overrides. Color and
 * size stay OpenAgents' own (see the arcade-fidelity audit,
 * docs/khala-code/2026-07-06-khala-mobile-arcade-ignite-fidelity-audit.md
 * §0). */
const variantClassName: Record<KhalaTextVariant, string> = {
  body: "text-[16px] leading-[24px] text-text",
  caption: "text-[14px] leading-[21px] text-textMuted",
  danger: "text-[14px] leading-[21px] text-danger",
  faint: "text-[12px] leading-[18px] text-textFaint",
  heading: "text-[36px] leading-[44px] text-text",
  label: "text-[12px] leading-[18px] uppercase tracking-wide text-textFaint",
  mono: "text-[14px] leading-[21px] text-text",
  muted: "text-[14px] leading-[21px] text-textMuted",
  success: "text-[14px] leading-[21px] text-success",
  warning: "text-[14px] leading-[21px] text-warning",
}

/** Font FAMILY only, as an inline style — no call site overrides font
 * family via className today, so there is no conflict here, and a real
 * loaded native font name (Space Grotesk / Protomolecule / JetBrains Mono)
 * can only be set via `style`, never reliably via a NativeWind class in
 * this app's current token setup (see `theme/typography.ts`). */
const variantFontStyle: Record<KhalaTextVariant, TextStyle> = {
  body: { fontFamily: khalaMobileTypography.primary.normal },
  caption: { fontFamily: khalaMobileTypography.primary.normal },
  danger: { fontFamily: khalaMobileTypography.primary.normal },
  faint: { fontFamily: khalaMobileTypography.code.normal },
  // Arcade's `heading` preset layers `{ fontFamily: protomolecule }` LAST
  // over its bold base, so the display font itself carries the whole
  // "heavy title" look — no separate fontWeight is set alongside a custom
  // OTF (React Native doesn't reliably combine a numeric fontWeight with a
  // non-system font file).
  heading: { fontFamily: khalaMobileTypography.display },
  label: { fontFamily: khalaMobileTypography.code.normal },
  mono: { fontFamily: khalaMobileTypography.code.normal },
  muted: { fontFamily: khalaMobileTypography.primary.normal },
  success: { fontFamily: khalaMobileTypography.primary.normal },
  warning: { fontFamily: khalaMobileTypography.primary.normal },
}

export const KhalaText = forwardRef<RNText, KhalaTextProps>(
  ({ children, className = "", style, text, variant = "body", ...props }, ref) => (
    <RNText
      {...props}
      className={`${variantClassName[variant]} ${className}`.trim()}
      ref={ref}
      style={[variantFontStyle[variant], style]}
    >
      {text ?? children}
    </RNText>
  ),
)

KhalaText.displayName = "KhalaText"
