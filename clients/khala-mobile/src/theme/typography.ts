import {
  SpaceGrotesk_300Light,
  SpaceGrotesk_400Regular,
  SpaceGrotesk_500Medium,
  SpaceGrotesk_600SemiBold,
  SpaceGrotesk_700Bold,
} from "@expo-google-fonts/space-grotesk"
import { JetBrainsMono_400Regular, JetBrainsMono_700Bold } from "@expo-google-fonts/jetbrains-mono"

/** Mirrors the owned `OpenAgentsInc/arcade` app's `theme/typography.ts`
 * structure (font-loading map + a primary/code/display route) — the
 * owner's mandate is arcade's font, size, and structure everywhere except
 * its literal StarCraft color palette (kept as OpenAgents' own blue/dark
 * tokens, see `theme/tokens.ts`). Space Grotesk is arcade's real primary
 * font (not a fallback CSS stack — `@openagentsinc/ui`'s shared NativeWind
 * `font-sans` token is a web font-stack string that silently falls back to
 * the OS default on native, which is why this app's typography previously
 * didn't actually match arcade at all despite looking like it should).
 * Protomolecule is arcade's display font, used only for the `heading`
 * preset, exactly as arcade uses it. One deliberate deviation from arcade's
 * own `code` route (arcade uses bare system `Courier`/`monospace`): the
 * owner asked to keep JetBrains Mono for code/mono content specifically,
 * since it reads better than the bare system monospace. */
export const khalaMobileFontsToLoad = {
  SpaceGrotesk_300Light,
  SpaceGrotesk_400Regular,
  SpaceGrotesk_500Medium,
  SpaceGrotesk_600SemiBold,
  SpaceGrotesk_700Bold,
  JetBrainsMono_400Regular,
  JetBrainsMono_700Bold,
  Protomolecule: require("../../assets/fonts/Protomolecule.otf") as number,
}

const spaceGrotesk = {
  light: "SpaceGrotesk_300Light",
  normal: "SpaceGrotesk_400Regular",
  medium: "SpaceGrotesk_500Medium",
  semiBold: "SpaceGrotesk_600SemiBold",
  bold: "SpaceGrotesk_700Bold",
} as const

export type KhalaTypographyWeight = keyof typeof spaceGrotesk

const jetBrainsMono = {
  normal: "JetBrainsMono_400Regular",
  bold: "JetBrainsMono_700Bold",
} as const

export type KhalaMonoWeight = keyof typeof jetBrainsMono

export const khalaMobileTypography = {
  /** The primary font — used in most places, matching arcade's `primary`. */
  primary: spaceGrotesk,
  /** Code/mono content — JetBrains Mono (owner call, 2026-07-06) rather
   * than arcade's bare system `Courier`/`monospace`. */
  code: jetBrainsMono,
  /** The one display font, reserved for the `heading` preset only —
   * matches arcade's `typography.fonts.protomolecule`. */
  display: "Protomolecule",
}

/** Mirrors arcade's `$sizeStyles` exactly (`app/components/Text.tsx`):
 * `{ fontSize, lineHeight }` pairs, not Tailwind's `text-*` scale (which
 * has slightly different line-heights at several sizes) — the owner asked
 * for arcade's EXACT sizes. */
export const khalaMobileTextSizes = {
  xxs: { fontSize: 12, lineHeight: 18 },
  xs: { fontSize: 14, lineHeight: 21 },
  sm: { fontSize: 16, lineHeight: 24 },
  md: { fontSize: 18, lineHeight: 26 },
  lg: { fontSize: 20, lineHeight: 32 },
  xl: { fontSize: 24, lineHeight: 34 },
  xxl: { fontSize: 36, lineHeight: 44 },
} as const

export type KhalaTextSize = keyof typeof khalaMobileTextSizes
