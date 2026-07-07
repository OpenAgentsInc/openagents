import { describe, expect, test } from "bun:test"

const mobileRoot = new URL("../", import.meta.url)

const readSource = (path: string) => Bun.file(new URL(path, mobileRoot)).text()

describe("Khala mobile provider spine and primitives", () => {
  test("wraps the app in the Ignite-style provider spine", async () => {
    const app = await readSource("src/app.tsx")

    expect(app).toContain("SafeAreaProvider")
    expect(app).toContain("initialWindowMetrics")
    expect(app).toContain("KhalaThemeProvider")
    expect(app).toContain("KhalaAuthProvider")
    expect(app).toContain("KhalaErrorBoundary")
    expect(app).toContain("BlurredPopupProvider")
  })

  test("defines token-backed ordinary UI primitives", async () => {
    const [screen, text, button, textField, listItem, emptyState] = await Promise.all([
      readSource("src/components/khala-screen.tsx"),
      readSource("src/components/khala-text.tsx"),
      readSource("src/components/khala-button.tsx"),
      readSource("src/components/khala-text-field.tsx"),
      readSource("src/components/khala-list-item.tsx"),
      readSource("src/components/khala-empty-state.tsx"),
    ])

    expect(screen).toContain('export type KhalaScreenPreset = "fixed" | "keyboardAware" | "scroll"')
    expect(screen).toContain("SafeAreaView")
    expect(screen).toContain("KeyboardAvoidingView")
    expect(screen).toContain("ScrollView")

    expect(text).toContain("export type KhalaTextVariant")
    // Font family/size moved to explicit style objects sourced from
    // theme/typography.ts (arcade's own Text.tsx structure), not Tailwind
    // text-* classes — see docs/khala-code/2026-07-06-khala-mobile-arcade-ignite-fidelity-audit.md.
    expect(text).toContain("khalaMobileTypography.primary.normal")
    expect(text).toContain("text-textFaint")

    expect(button).toContain("khalaMobileTheme")
    expect(button).toContain('accessibilityRole="button"')
    expect(button).toContain("accessibilityState")
    expect(button).toContain("busy: loading")
    expect(button).toContain("disabled: unavailable")
    expect(button).toContain("leftAccessory")
    expect(button).toContain("rightAccessory")

    expect(textField).toContain("export type KhalaTextFieldProps")
    expect(textField).toContain("aria-invalid")
    expect(textField).toContain("placeholderTextColor")

    expect(listItem).toContain("export type KhalaListItemProps")
    expect(listItem).toContain("TouchableFeedback")
    expect(listItem).toContain('accessibilityRole="button"')

    expect(emptyState).toContain("export type KhalaEmptyStateProps")
    expect(emptyState).toContain('accessibilityRole={loading ? "progressbar" : "summary"}')
  })

  // MM-H1 follow-up (Ignite port, #8487): Settings was deliberately rebuilt on
  // the ported Infinite Red Ignite component kit (`../ignite`) so the app shows
  // the real Ignite look on a live screen — replacing the earlier
  // KhalaScreen/KhalaText/KhalaButton composition. This oracle now pins that
  // owner-directed migration: Settings is composed ENTIRELY from Ignite
  // primitives and imports none of the bespoke Khala UI wrappers.
  test("rebuilds Settings entirely on the ported Ignite component kit", async () => {
    const settings = await readSource("src/screens/settings-screen.tsx")

    // Composed from the ported Ignite kit: Screen/Header/Card/Button remain the
    // structural spine (Button still drives Credits/Models/Notifications and the
    // delete-confirmation modal).
    expect(settings).toContain('from "../ignite"')
    expect(settings).toContain("<Screen")
    expect(settings).toContain("<Header")
    expect(settings).toContain("<Card")
    expect(settings).toContain("<Button")

    // No bespoke Khala UI wrappers.
    expect(settings).not.toContain("KhalaScreen")
    expect(settings).not.toContain("KhalaText")
    expect(settings).not.toContain("KhalaButton")
    expect(settings).not.toContain("AppHeader")

    // Sign out and Delete account are the ONLY deliberate exception to the
    // Ignite-Button rule: under the New Architecture (Fabric) an Ignite
    // `Button`/`Pressable` with a function `style` does not paint its own
    // background, so these two visible-fill controls use the same Fabric-safe
    // `Pressable` + inner plain `View` fill pattern as the login button and the
    // onboarding "Get started" CTA (see sign-in-screen.tsx / onboarding-flow.tsx,
    // and contract khala_mobile.settings.sign_out_button_fill_on_inner_view.v1).
    // That is why a raw `<Pressable` is expected here — not a regression to a
    // bespoke wrapper. The old invisible `Button preset="reversed" text="Sign
    // out"` must be gone.
    expect(settings).not.toContain('text="Sign out"')
    expect(settings).toContain("<Pressable")
    expect(settings).toContain("styles.signOutButton")
    expect(settings).toContain("styles.deleteButton")
  })

  // MM-H1 follow-up (Ignite port): the signed-in thread list was rebuilt on
  // the ported Infinite Red Ignite kit (`../ignite`) — Screen/Header/Card/
  // ListItem/EmptyState/Text + theme tokens — replacing the KhalaListItem/
  // KhalaEmptyState/BackgroundGradient composition. The staggered entrance
  // (arcade-fidelity §4 behavior) stays.
  test("uses the Ignite component kit on the signed-in thread list", async () => {
    const threadList = await readSource("src/screens/thread-list-screen.tsx")

    expect(threadList).toContain('from "../ignite"')
    expect(threadList).toContain("<Screen")
    expect(threadList).toContain("<Header")
    expect(threadList).toContain("ListItem")
    expect(threadList).toContain("EmptyState")
    expect(threadList).toContain("FadeIn.delay")
    expect(threadList).not.toContain("KhalaListItem")
    expect(threadList).not.toContain("KhalaEmptyState")
    expect(threadList).not.toContain("BackgroundGradient")
    expect(threadList).not.toContain("Frame")
    expect(threadList).not.toContain("usePowerOnVisible")
  })

  test("uses a raw filled Pressable CTA on the GitHub-only sign-in fallback", async () => {
    const signIn = await readSource("src/components/sign-in-screen.tsx")

    // Owner report (2026-07-07): the ported Ignite `Button` ignored the `style`
    // background override and rendered as bare dark text with no fill over the
    // hero art. The CTA is now a raw `Pressable` + `RNText` with an explicit
    // `backgroundColor`, guaranteeing a real filled cyan button. `Text` is
    // still the Ignite primitive (title + error copy).
    expect(signIn).toContain("<Pressable")
    expect(signIn).toContain("backgroundColor")
    expect(signIn).toContain('from "../ignite"')
    expect(signIn).toContain("signInWithGitHub")
    expect(signIn).toContain("signIn.github.primary")
    expect(signIn).not.toContain("NexusSignInButton")
    expect(signIn).not.toContain("KhalaButton")
    expect(signIn).not.toContain("KhalaTextField")
    expect(signIn).not.toContain("KhalaEmptyState")
    expect(signIn).not.toContain("<TextInput")
    expect(signIn).not.toContain("retryDiscovery")
  })

  test("the signed-out screen matches the owned arcade app's plain look, not a composed dashboard widget", async () => {
    const signIn = await readSource("src/components/sign-in-screen.tsx")

    // Two composed-widget redesigns landed here before this one — a "Nexus
    // Beam" Skia backdrop, then a glass "LIVE TASK DECK" console with
    // hardcoded "$10 START CREDIT"/queued/ready/proofs numbers on a screen
    // the user hasn't even signed into yet. The owner asked for the simple,
    // owned `OpenAgentsInc/arcade` `HomeScreen` look instead: one full-bleed
    // background image, one glowing title, one CTA — never fabricated
    // metrics as decoration. The full-bleed art is now the baked Protoss-blue
    // duotone hero (`home-hero.jpg`), replacing the old `city-cyan.png`.
    expect(signIn).toContain("home-hero.jpg")
    expect(signIn).not.toContain("LandingGlassConsole")
    expect(signIn).not.toContain("NexusBeamBackdrop")
    expect(signIn).not.toContain("WarpAperture")
    expect(signIn).not.toContain("BackgroundGradient")
    expect(signIn).not.toContain("<Frame")
    expect(signIn).not.toContain("Ready for first run")
    expect(signIn).not.toContain("START CREDIT")
    expect(signIn).not.toContain("LIVE TASK DECK")
  })
})
