import {
  Image,
  Pressable,
  StyleSheet,
  Text as RNText,
  View,
  type TextStyle,
  type ViewStyle,
} from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { Text, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import { tx } from "../i18n/copy"

/** First screen a signed-out user sees — deliberately plain, matching the
 * owned `OpenAgentsInc/arcade` app's `HomeScreen`/`CityBackground` (a
 * full-bleed moody background image behind a dark scrim, one huge
 * neon-glow title, one CTA) rather than a bespoke composed dashboard.
 * Rebuilt on the ported Infinite Red Ignite `Button`/`Text` primitives
 * (`../ignite`) so the signed-out surface shows the real Ignite look; auth
 * wiring (`githubSignInReady`, `signInErrorMessage`, `signInWithGitHub`,
 * `status`) and product copy are unchanged. */
export const SignInScreen = () => {
  const { themed } = useAppTheme()
  const {
    githubSignInReady,
    signInErrorMessage,
    signInWithGitHub,
    status,
  } = useKhalaAuth()
  const signingIn = status === "signing_in"

  return (
    <View style={themed($root)}>
      <Image
        resizeMode="cover"
        source={require("../../assets/images/home-hero.jpg")}
        style={StyleSheet.absoluteFill}
      />
      {/* StarCraft/Protoss-blue color grade over the hero (owner request): a
        * blue tint layer so the whole image reads as uniformly blue-cast, then
        * a darkening scrim under the content for text contrast. */}
      <View pointerEvents="none" style={styles.tint} />
      <View pointerEvents="none" style={styles.scrim} />

      <SafeAreaView style={styles.safe} edges={["top", "bottom"]}>
        <View style={themed($content)}>
          <View />

          <Text preset="heading" style={[styles.title, themed($titleGlow)]} text={tx("app.title")} />

          <View style={themed($ctaColumn)}>
            {/* Raw Pressable, NOT the Ignite Button: the ported Button was not
              * applying the `style` background override (it rendered as bare
              * dark text with no fill over the hero art). A plain Pressable +
              * RN Text with inline styles guarantees a real, filled CTA. */}
            <Pressable
              accessibilityRole="button"
              accessibilityState={{ disabled: !githubSignInReady || signingIn }}
              disabled={!githubSignInReady || signingIn}
              onPress={signInWithGitHub}
              style={({ pressed }) => [
                styles.loginButton,
                pressed && styles.loginButtonPressed,
                (!githubSignInReady || signingIn) && styles.loginButtonDisabled,
              ]}
            >
              <RNText style={styles.loginButtonText}>{tx("signIn.github.primary")}</RNText>
            </Pressable>

            {signInErrorMessage === null ? null : (
              <Text style={[styles.center, themed($danger)]} text={signInErrorMessage} />
            )}
          </View>
        </View>
      </SafeAreaView>
    </View>
  )
}

const $root: ThemedStyle<ViewStyle> = ({ colors }) => ({
  flex: 1,
  backgroundColor: colors.background,
})

const $content: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flex: 1,
  justifyContent: "space-between",
  paddingHorizontal: spacing.lg,
  paddingBottom: spacing.lg,
})

const $ctaColumn: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  gap: spacing.md,
})

const $titleGlow: ThemedStyle<TextStyle> = ({ colors }) => ({
  textShadowColor: colors.tint,
  textShadowOffset: { height: 0, width: 0 },
  textShadowRadius: 12,
})

const $danger: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.error })

const styles = StyleSheet.create({
  center: { textAlign: "center" },
  loginButton: {
    alignItems: "center",
    backgroundColor: "#4fd0ff",
    borderRadius: 12,
    justifyContent: "center",
    minHeight: 54,
    paddingHorizontal: 20,
    width: "100%",
  },
  loginButtonDisabled: { opacity: 0.5 },
  loginButtonPressed: { backgroundColor: "#3bb8e6" },
  loginButtonText: {
    color: "#02060d",
    fontSize: 17,
    fontWeight: "700",
    textAlign: "center",
  },
  safe: { flex: 1 },
  scrim: {
    ...StyleSheet.absoluteFill,
    backgroundColor: "rgba(2, 10, 22, 0.5)",
  },
  tint: {
    ...StyleSheet.absoluteFill,
    // StarCraft/Protoss blue cast over the whole hero image.
    backgroundColor: "rgba(20, 92, 150, 0.45)",
  },
  title: {
    color: "white",
    // Restore the previous display font (arcade's Protomolecule), overriding
    // the Ignite `heading` preset's Space Grotesk, and smaller than the old
    // 56px. Protomolecule is loaded in app.tsx under this exact family name.
    fontFamily: "Protomolecule",
    fontSize: 40,
    letterSpacing: 2,
    lineHeight: 48,
    textAlign: "center",
  },
})
