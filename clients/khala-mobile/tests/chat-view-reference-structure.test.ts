import { describe, expect, test } from "bun:test"

const mobileRoot = new URL("../", import.meta.url)
const read = (path: string) => Bun.file(new URL(path, mobileRoot)).text()

describe("Khala mobile chat view reference structure", () => {
  test("thread screen uses floating header, scroll affordance, and plain transcript spacing", async () => {
    const source = await read("src/screens/thread-messages-screen.tsx")

    expect(source).toContain("KhalaThreadHeader")
    expect(source).toContain("KhalaScrollToLatestButton")
    expect(source).toContain("contentContainerClassName=\"gap-4 px-8")
    expect(source).not.toContain("AppHeader showBack")
    expect(source).not.toContain("rounded-xl border border-border bg-surfaceRaised px-3 py-2")
  })

  test("transcript rows default to one-line tool summaries and plain prose", async () => {
    const source = await read("src/components/transcript-part-row.tsx")

    expect(source).toContain("summarizeToolPart")
    expect(source).toContain("numberOfLines={1}")
    expect(source).toContain("›")
    expect(source).toContain("text-[22px] leading-8 text-text")
    expect(source).not.toContain("BackgroundGradient")
    expect(source).not.toContain("bg-surfaceRaised px-3 py-2")
  })

  test("composer is a floating pill instead of the old full-width Arcade rail", async () => {
    const source = await read("src/components/chat-composer.tsx")

    expect(source).toContain("rounded-full border border-borderMuted bg-surfaceRaised")
    expect(source).toContain("Follow up")
    expect(source).toContain("Show composer options")
    expect(source).not.toContain("ArwesButton")
    expect(source).not.toContain("border-t border-borderMuted bg-bg")
  })
})
