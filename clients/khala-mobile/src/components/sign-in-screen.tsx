import { useFonts, JetBrainsMono_700Bold } from "@expo-google-fonts/jetbrains-mono"
import { View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { tx } from "../i18n/copy"
import { KhalaScreen } from "./khala-screen"
import { KhalaText } from "./khala-text"
import { NexusBeamBackdrop, NexusSignInButton, WarpAperture } from "./nexus-beam"

/** First screen a signed-out user sees. Restyled to the "Nexus Beam" wireframe
 * direction the owner picked (`id="1d"` in
 * `~/Downloads/Khala Mobile landing wireframe/Khala Mobile Landing
 * Wireframes.dc.html`): a glowing vertical beam down the center with
 * materializing code glyphs, a rotated-diamond "warp aperture" behind the
 * hero title, and a bordered cyan CTA bar. See `./nexus-beam/` for the
 * decorative pieces. Visual-only change — auth wiring (`githubSignInReady`,
 * `signInErrorMessage`, `signInWithGitHub`, `status`) is unchanged.
 *
 * The "Khala Code" title uses JetBrains Mono (loaded via
 * @expo-google-fonts/jetbrains-mono) rather than the system sans — the
 * source wireframe's own CSS (`Khala Mobile Landing Wireframes.dc.html`)
 * imports JetBrains Mono for this exact hero across all four directions, so
 * this matches the original design intent rather than introducing a new
 * typeface. Falls back to the default heading font until the font asset
 * loads (typically within a frame or two — no splash-screen gating needed
 * for one title string). */
export const SignInScreen = () => {
  const {
    githubSignInReady,
    signInErrorMessage,
    signInWithGitHub,
    status,
  } = useKhalaAuth()
  const signingIn = status === "signing_in"
  const [monoTitleFontLoaded] = useFonts({ JetBrainsMono_700Bold })

  return (
    <KhalaScreen contentClassName="px-7">
      <View className="relative flex-1">
        <NexusBeamBackdrop />

        <View className="flex-1 items-center justify-center gap-3">
          <WarpAperture />

          <KhalaText
            className="mt-4 text-center text-4xl"
            style={monoTitleFontLoaded ? { fontFamily: "JetBrainsMono_700Bold" } : undefined}
            variant="heading"
          >
            {tx("app.title")}
          </KhalaText>
        </View>

        <View className="pb-6">
          <NexusSignInButton
            disabled={!githubSignInReady}
            loading={signingIn}
            onPress={signInWithGitHub}
            text={tx("signIn.github.primary")}
          />

          {signInErrorMessage === null ? null : (
            <KhalaText className="mt-4 text-center" variant="danger">
              {signInErrorMessage}
            </KhalaText>
          )}
        </View>
      </View>
    </KhalaScreen>
  )
}
