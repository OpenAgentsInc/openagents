import {
  JetBrainsMono_300Light as jetBrainsMonoLight,
  JetBrainsMono_400Regular as jetBrainsMonoRegular,
  JetBrainsMono_500Medium as jetBrainsMonoMedium,
  JetBrainsMono_600SemiBold as jetBrainsMonoSemiBold,
  JetBrainsMono_700Bold as jetBrainsMonoBold
} from "@expo-google-fonts/jetbrains-mono"

export const customFontsToLoad = {
  jetBrainsMonoLight,
  jetBrainsMonoRegular,
  jetBrainsMonoMedium,
  jetBrainsMonoSemiBold,
  jetBrainsMonoBold,
}

const fonts = {
  jetBrainsMono: {
    light: "jetBrainsMonoLight",
    normal: "jetBrainsMonoRegular",
    medium: "jetBrainsMonoMedium",
    semiBold: "jetBrainsMonoSemiBold",
    bold: "jetBrainsMonoBold",
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
  primary: fonts.jetBrainsMono,
  secondary: fonts.jetBrainsMono,
}
