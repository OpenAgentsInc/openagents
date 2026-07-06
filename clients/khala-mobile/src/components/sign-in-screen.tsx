import { Image, StyleSheet, View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { tx } from "../i18n/copy"
import { khalaMobileTheme } from "../theme/tokens"
import { khalaMobileTypography } from "../theme/typography"
import { KhalaButton } from "./khala-button"
import { KhalaScreen } from "./khala-screen"
import { KhalaText } from "./khala-text"

/** First screen a signed-out user sees — deliberately plain, matching the
 * owned `OpenAgentsInc/arcade` app's `HomeScreen`/`CityBackground` (a
 * full-bleed moody background image behind a dark scrim, one huge
 * neon-glow title, one CTA) rather than a bespoke composed dashboard.
 * Two composed-widget redesigns landed here before this one (a Skia beam
 * backdrop, then a glass console card with hardcoded placeholder credit/
 * queue/proof numbers) — the second shipped fabricated metrics on a screen
 * the user hasn't even signed into yet, and the owner asked for the simple
 * arcade look instead of another composed widget. Auth wiring
 * (`githubSignInReady`, `signInErrorMessage`, `signInWithGitHub`, `status`)
 * is unchanged; only the visual shell is. The CTA is the shared
 * `KhalaButton` (arcade-fidelity audit, 2026-07-06) instead of a fourth
 * bespoke button component — arcade's own `HomeScreen` title uses its
 * `heading` preset (Protomolecule), matched here via `khalaMobileTypography.display`. */
export const SignInScreen = () => {
  const {
    githubSignInReady,
    signInErrorMessage,
    signInWithGitHub,
    status,
  } = useKhalaAuth()
  const signingIn = status === "signing_in"

  return (
    <KhalaScreen edges={["top", "bottom"]} preset="fixed">
      <View className="flex-1" style={styles.root}>
        <Image
          resizeMode="cover"
          source={require("../../assets/images/city-cyan.png")}
          style={StyleSheet.absoluteFill}
        />
        <View pointerEvents="none" style={styles.scrim} />

        <View className="flex-1 justify-between px-6 pb-6">
          <View />

          <KhalaText className="text-center" style={styles.title} variant="heading">
            {tx("app.title")}
          </KhalaText>

          <View className="gap-4">
            <KhalaButton
              disabled={!githubSignInReady}
              loading={signingIn}
              onPress={signInWithGitHub}
              text={tx("signIn.github.primary")}
              variant="primary"
            />

            {signInErrorMessage === null ? null : (
              <KhalaText className="text-center" variant="danger">
                {signInErrorMessage}
              </KhalaText>
            )}

            {/* TEMP-DIAG-8467: visible build marker so we both know the debug
             * build is actually running before testing sign-in. Remove with
             * the sign-in diagnostics. */}
            <KhalaText className="text-center" variant="caption">
              build dbg3
            </KhalaText>
          </View>
        </View>
      </View>
    </KhalaScreen>
  )
}

const styles = StyleSheet.create({
  root: {
    backgroundColor: khalaMobileTheme.background,
  },
  scrim: {
    ...StyleSheet.absoluteFill,
    backgroundColor: "rgba(0, 0, 0, 0.75)",
  },
  title: {
    color: "white",
    fontFamily: khalaMobileTypography.display,
    fontSize: 56,
    letterSpacing: 4,
    lineHeight: 68,
    textShadowColor: khalaMobileTheme.accent,
    textShadowOffset: { height: 0, width: 0 },
    textShadowRadius: 12,
  },
})
