import { useFonts, JetBrainsMono_700Bold } from "@expo-google-fonts/jetbrains-mono"
import { StyleSheet, View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { tx } from "../i18n/copy"
import { khalaMobileTheme } from "../theme/tokens"
import { BackgroundGradient } from "./background-gradient"
import { Frame } from "./frame"
import { KhalaScreen } from "./khala-screen"
import { KhalaText } from "./khala-text"
import { NexusBeamBackdrop, NexusSignInButton, WarpAperture } from "./nexus-beam"

/** First screen a signed-out user sees. Restyled to the "Nexus Beam" wireframe
 * direction the owner picked (`id="1d"` in
 * `~/Downloads/Khala Mobile landing wireframe/Khala Mobile Landing
 * Wireframes.dc.html`), then revised with the Smart Home Figma glass-control
 * treatment: a glowing vertical beam, a compact branded title stack, and a
 * glass capacity card composed from the already-ported Arcade/Ignite Skia
 * primitives (`Frame`, `BackgroundGradient`). Visual-only change — auth
 * wiring (`githubSignInReady`, `signInErrorMessage`, `signInWithGitHub`,
 * `status`) is unchanged.
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
    <KhalaScreen
      contentClassName="grow px-6 py-6"
      preset="scroll"
      scrollViewProps={{ showsVerticalScrollIndicator: false }}
    >
      <View className="relative min-h-[760px] flex-1">
        <NexusBeamBackdrop />

        <View className="flex-1 justify-between gap-6">
          <View className="items-center gap-4 pt-8">
            <WarpAperture />

            <View className="items-center gap-2">
              <KhalaText className="text-center tracking-[4px]" variant="label">
                MOBILE AGENT COMPUTER
              </KhalaText>
              <KhalaText
                className="text-center text-4xl"
                style={monoTitleFontLoaded ? { fontFamily: "JetBrainsMono_700Bold" } : undefined}
                variant="heading"
              >
                {tx("app.title")}
              </KhalaText>
              <KhalaText className="max-w-[300px] text-center leading-6" variant="muted">
                Start coding work from your phone with a glass cockpit for runs,
                credits, and verified progress.
              </KhalaText>
            </View>
          </View>

          <LandingGlassConsole />

          <View className="gap-4 pb-1">
            <NexusSignInButton
              disabled={!githubSignInReady}
              loading={signingIn}
              onPress={signInWithGitHub}
              text={tx("signIn.github.primary")}
            />

            {signInErrorMessage === null ? null : (
              <KhalaText className="text-center" variant="danger">
                {signInErrorMessage}
              </KhalaText>
            )}
          </View>
        </View>
      </View>
    </KhalaScreen>
  )
}

const LandingGlassConsole = () => (
  <Frame
    alwaysShowBackground
    alwaysShowBorder
    backgroundColor="rgba(6, 16, 35, 0.72)"
    borderColor="rgba(23, 185, 255, 0.62)"
    color={khalaMobileTheme.accent}
    internalSquareSize={28}
    style={styles.consoleFrame}
  >
    <View className="overflow-hidden rounded-[28px] border border-accent/30 bg-surfaceRaised/70 p-5" style={styles.console}>
      <View className="absolute -left-16 -top-16 h-44 w-44 rounded-full bg-accent/20" style={styles.glow} />
      <View className="absolute -right-12 top-20 h-36 w-36 rounded-full bg-borderStrong/35" style={styles.glow} />

      <View className="flex-row items-center justify-between">
        <View>
          <KhalaText className="tracking-[3px]" style={{ color: khalaMobileTheme.accent }} variant="label">
            LIVE TASK DECK
          </KhalaText>
          <KhalaText className="mt-1 text-xl" variant="heading">
            Ready for first run
          </KhalaText>
        </View>
        <View className="rounded-full border border-text/20 bg-surfaceMuted/80 px-3 py-1">
          <KhalaText className="text-[10px]" variant="faint">
            GITHUB
          </KhalaText>
        </View>
      </View>

      <View className="mt-5 items-center">
        <View className="h-48 w-48 items-center justify-center rounded-full border border-borderStrong/60 bg-surface/70">
          <View className="absolute h-44 w-44 rounded-full border-[18px] border-accent/80" />
          <View
            className="absolute right-7 top-8 h-7 w-7 rounded-full border border-text/40 bg-accent"
            style={styles.dialKnob}
          />
          <View className="h-28 w-28 items-center justify-center rounded-full border border-accent/30 bg-surfaceRaised/90">
            <KhalaText className="text-center text-4xl" variant="heading">
              $10
            </KhalaText>
            <KhalaText className="text-center tracking-[3px]" variant="faint">
              START CREDIT
            </KhalaText>
          </View>
          <KhalaText className="absolute left-3 top-[88px]" variant="faint">
            plan
          </KhalaText>
          <KhalaText className="absolute right-2 top-[88px]" variant="faint">
            ship
          </KhalaText>
        </View>
      </View>

      <View className="mt-5 overflow-hidden rounded-2xl border border-accent/25">
        <View className="flex-row border-b border-accent/15">
          <ConsoleMetric label="queued" value="0" />
          <ConsoleMetric label="ready" value="1" />
        </View>
        <View className="flex-row">
          <ConsoleMetric label="proofs" value="refs" />
          <ConsoleMetric label="private" value="yes" />
        </View>
      </View>

      <BackgroundGradient
        colors={[
          "rgba(79, 208, 255, 0.32)",
          "rgba(143, 212, 255, 0.22)",
          "rgba(2, 109, 255, 0.18)",
          "rgba(79, 208, 255, 0.32)",
        ]}
        cornerRadius={18}
        maxBlur={8}
        style={styles.ctaPreview}
      >
        <KhalaText className="text-center font-semibold" style={{ color: khalaMobileTheme.accentText }} variant="body">
          Sign in, pick a repo, launch a task
        </KhalaText>
      </BackgroundGradient>
    </View>
  </Frame>
)

const ConsoleMetric = ({ label, value }: Readonly<{ label: string; value: string }>) => (
  <View className="min-h-16 flex-1 justify-center gap-1 border-accent/10 px-4 py-3">
    <KhalaText className="text-xl" variant="heading">
      {value}
    </KhalaText>
    <KhalaText variant="faint">{label}</KhalaText>
  </View>
)

const styles = StyleSheet.create({
  console: {
    shadowColor: khalaMobileTheme.accent,
    shadowOffset: { height: 18, width: 0 },
    shadowOpacity: 0.16,
    shadowRadius: 34,
  },
  consoleFrame: {
    width: "100%",
  },
  ctaPreview: {
    alignItems: "center",
    borderColor: "rgba(255,255,255,0.24)",
    borderRadius: 18,
    borderWidth: 1,
    justifyContent: "center",
    marginTop: 18,
    minHeight: 50,
    overflow: "hidden",
  },
  dialKnob: {
    shadowColor: khalaMobileTheme.accent,
    shadowOffset: { height: 0, width: 0 },
    shadowOpacity: 0.8,
    shadowRadius: 14,
  },
  glow: {
    opacity: 0.7,
    transform: [{ scale: 1.05 }],
  },
})
