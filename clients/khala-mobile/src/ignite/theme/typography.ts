// Ported from Ignite's `theme/typography.ts`. The `@expo-google-fonts/space-grotesk`
// import and `customFontsToLoad` are dropped: khala-mobile's host app already
// loads Space Grotesk (see `src/theme/typography.ts` /
// `khalaMobileFontsToLoad`) under these exact native family names, so the
// ported components render in the real Space Grotesk face by referencing those
// names directly.

import { Platform } from "react-native"

const fonts = {
  spaceGrotesk: {
    // Cross-platform Google font, loaded by the host app.
    light: "SpaceGrotesk_300Light",
    normal: "SpaceGrotesk_400Regular",
    medium: "SpaceGrotesk_500Medium",
    semiBold: "SpaceGrotesk_600SemiBold",
    bold: "SpaceGrotesk_700Bold",
  },
  helveticaNeue: {
    // iOS only font.
    thin: "HelveticaNeue-Thin",
    light: "HelveticaNeue-Light",
    normal: "Helvetica Neue",
    medium: "HelveticaNeue-Medium",
  },
  courier: {
    // iOS only font.
    normal: "Courier",
  },
  sansSerif: {
    // Android only font.
    thin: "sans-serif-thin",
    light: "sans-serif-light",
    normal: "sans-serif",
    medium: "sans-serif-medium",
  },
  monospace: {
    // Android only font.
    normal: "monospace",
  },
}

export const typography = {
  /**
   * The fonts are available to use, but prefer using the semantic name.
   */
  fonts,
  /**
   * The primary font. Used in most places.
   */
  primary: fonts.spaceGrotesk,
  /**
   * An alternate font used for perhaps titles and stuff.
   */
  secondary: Platform.select({ ios: fonts.helveticaNeue, android: fonts.sansSerif }),
  /**
   * Lets get fancy with a monospace font!
   */
  code: Platform.select({ ios: fonts.courier, android: fonts.monospace }),
}
