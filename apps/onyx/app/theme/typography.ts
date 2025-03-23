export const customFontsToLoad = {
  "Berkeley Mono": require("../../assets/fonts/BerkeleyMonoVariable-Regular.ttf"),
}

const fonts = {
  berkeleyMono: {
    // Using the same font family name but will control weight via fontWeight in styles
    light: "Berkeley Mono",
    normal: "Berkeley Mono",
    medium: "Berkeley Mono",
    semiBold: "Berkeley Mono",
    bold: "Berkeley Mono",
  }
}

export const typography = {
  /**
   * The fonts are available to use, but prefer using the semantic name.
   */
  fonts,
  /**
   * The primary font. Used in most places.
   */
  primary: fonts.berkeleyMono,
  secondary: fonts.berkeleyMono,
}

// Font weight values that correspond to the font weights
export const fontWeights = {
  light: "300" as const,
  normal: "400" as const,
  medium: "500" as const,
  semiBold: "600" as const,
  bold: "700" as const,
}
