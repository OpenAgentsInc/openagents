import { Image, StyleSheet, View, type TextStyle, type ViewStyle } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { Button, Text, useAppTheme } from "../ignite"
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
      <View pointerEvents="none" style={styles.scrim} />

      <SafeAreaView style={styles.safe} edges={["top", "bottom"]}>
        <View style={themed($content)}>
          <View />

          <Text preset="heading" style={[styles.title, themed($titleGlow)]} text={tx("app.title")} />

          <View style={themed($ctaColumn)}>
            <Button
              preset="reversed"
              disabled={!githubSignInReady || signingIn}
              onPress={signInWithGitHub}
              // Explicit solid CTA styling: the hero is a busy dark image, so
              // the button needs an unmistakable filled surface + high-contrast
              // label rather than relying on the theme preset (which rendered
              // as near-invisible dark text over the artwork).
              style={styles.loginButton}
              pressedStyle={styles.loginButtonPressed}
              textStyle={styles.loginButtonText}
              text={tx("signIn.github.primary")}
            />

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
  loginButtonPressed: { backgroundColor: "#3bb8e6" },
  loginButtonText: {
    color: "#02060d",
    fontSize: 17,
    fontWeight: "700",
  },
  safe: { flex: 1 },
  scrim: {
    ...StyleSheet.absoluteFill,
    backgroundColor: "rgba(0, 0, 0, 0.75)",
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
