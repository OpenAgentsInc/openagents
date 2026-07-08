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
    enterDemoMode,
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
      {/* The hero art is a baked blue duotone; the scrim keeps the title and
        * CTA readable while preserving the full-bleed image. */}
      <View pointerEvents="none" style={styles.scrim} />

      <SafeAreaView style={styles.safe} edges={["top", "bottom"]}>
        <View style={themed($content)}>
          <View />

          <Text preset="heading" style={[styles.title, themed($titleGlow)]} text={tx("app.title")} />

          <View style={themed($ctaColumn)}>
            {/* The fill lives on an INNER plain `View`, not on the `Pressable`
              * itself: under the New Architecture (Fabric) a `Pressable` with a
              * function `style` did not paint its own `backgroundColor` (it
              * rendered as bare dark text with no cyan fill over the hero art —
              * owner report + sim-confirmed 2026-07-07). A plain `View` always
              * paints its `backgroundColor`, so the pill is guaranteed; the
              * `Pressable` only owns the touch target and press feedback. */}
            <Pressable
              accessibilityRole="button"
              accessibilityState={{ disabled: !githubSignInReady || signingIn }}
              disabled={!githubSignInReady || signingIn}
              // Normal tap = real GitHub OAuth (unchanged). A deliberate ~1s
              // long-press enters App Store reviewer demo mode with hardcoded
              // example data — discoverable only to a reviewer who was told
              // about it (documented in the App Review notes), not triggerable
              // by an accidental tap. See src/demo/demo-fixtures.ts.
              delayLongPress={1000}
              onLongPress={enterDemoMode}
              onPress={signInWithGitHub}
              style={styles.loginPressable}
            >
              {({ pressed }) => (
                <View
                  style={[
                    styles.loginButton,
                    pressed && styles.loginButtonPressed,
                    (!githubSignInReady || signingIn) && styles.loginButtonDisabled,
                  ]}
                >
                  <RNText style={styles.loginButtonText}>{tx("signIn.github.primary")}</RNText>
                </View>
              )}
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
  loginPressable: { width: "100%" },
  loginButtonText: {
    color: "#02060d",
    fontSize: 17,
    fontWeight: "700",
    textAlign: "center",
  },
  safe: { flex: 1 },
  scrim: {
    ...StyleSheet.absoluteFill,
    backgroundColor: "rgba(2, 10, 22, 0.78)",
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
