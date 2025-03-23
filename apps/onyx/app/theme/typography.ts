import { useFonts } from "expo-font"

export const customFontsToLoad = {
  "Berkeley Mono": require("../../assets/fonts/BerkeleyMonoVariable-Regular.ttf"),
}

const fonts = {
  berkeleyMono: {
    normal: "Berkeley Mono",
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
