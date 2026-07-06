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
    expect(text).toContain("font-sans text-base text-text")
    expect(text).toContain("font-mono text-xs text-textFaint")

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

  test("migrates Settings onto the new plain UI primitives", async () => {
    const settings = await readSource("src/screens/settings-screen.tsx")

    expect(settings).toContain("<KhalaScreen")
    expect(settings).toContain("<KhalaText")
    expect(settings).toContain("<KhalaButton")
    expect(settings).toContain('text="Sign out"')
    expect(settings).not.toContain("<Pressable")
  })

  test("uses restrained Arcade primitives on the signed-in thread list", async () => {
    const threadList = await readSource("src/screens/thread-list-screen.tsx")

    expect(threadList).toContain("ActivityIndicator")
    expect(threadList).toContain("BackgroundGradient")
    expect(threadList).toContain("KhalaListItem")
    expect(threadList).toContain("KhalaEmptyState")
    expect(threadList).not.toContain("Frame")
    expect(threadList).not.toContain("usePowerOnVisible")
    expect(threadList).not.toContain("FadeIn.delay")
    expect(threadList).not.toContain("rowFrame")
    expect(threadList).not.toContain('className="border-b border-borderMuted px-4 py-4"')
  })

  test("uses button primitives on the GitHub-only sign-in fallback", async () => {
    const signIn = await readSource("src/components/sign-in-screen.tsx")

    expect(signIn).toContain("KhalaButton")
    expect(signIn).toContain("signInWithGitHub")
    expect(signIn).toContain("signIn.github.primary")
    expect(signIn).not.toContain("KhalaTextField")
    expect(signIn).not.toContain("KhalaEmptyState")
    expect(signIn).not.toContain("<TextInput")
    expect(signIn).not.toContain("<Pressable")
    expect(signIn).not.toContain("retryDiscovery")
  })
})
