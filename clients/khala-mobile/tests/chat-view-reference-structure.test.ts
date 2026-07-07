import { describe, expect, test } from "bun:test"

const mobileRoot = new URL("../", import.meta.url)
const read = (path: string) => Bun.file(new URL(path, mobileRoot)).text()

describe("Khala mobile chat view reference structure", () => {
  // MM-H1 follow-up (Ignite port): the thread view was rebuilt on the ported
  // Infinite Red Ignite kit (`../ignite`). The floating header + scroll
  // affordance stay (behavior), but the presentation is now Ignite `Text`/
  // `EmptyState` + theme tokens rather than NativeWind classNames.
  test("thread screen uses floating header, scroll affordance, and Ignite-composed transcript", async () => {
    const source = await read("src/screens/thread-messages-screen.tsx")

    expect(source).toContain("KhalaThreadHeader")
    expect(source).toContain("KhalaScrollToLatestButton")
    expect(source).toContain('from "../ignite"')
    expect(source).toContain("EmptyState")
    expect(source).toContain("$transcriptContent")
    expect(source).not.toContain("AppHeader showBack")
    expect(source).not.toContain("KhalaText")
    expect(source).not.toContain("KhalaEmptyState")
  })

  test("transcript rows default to one-line tool summaries and plain prose", async () => {
    const source = await read("src/components/transcript-part-row.tsx")

    expect(source).toContain("summarizeToolPart")
    expect(source).toContain("numberOfLines={1}")
    expect(source).toContain("›")
    // Prose is now the ported Ignite `Text` primitive + explicit style objects.
    expect(source).toContain('from "../ignite"')
    expect(source).toContain("fontSize: 22")
    expect(source).not.toContain("KhalaText")
    expect(source).not.toContain("BackgroundGradient")
  })

  test("composer is a floating pill instead of the old full-width Arcade rail", async () => {
    const source = await read("src/components/chat-composer.tsx")

    // Floating pill layout is now themed Ignite tokens (borderRadius 999 pill)
    // instead of NativeWind classNames.
    expect(source).toContain("borderRadius: 999")
    expect(source).toContain('from "../ignite"')
    expect(source).toContain("Follow up")
    expect(source).toContain("Show composer options")
    expect(source).not.toContain("ArwesButton")
    expect(source).not.toContain("KhalaText")
  })
})
