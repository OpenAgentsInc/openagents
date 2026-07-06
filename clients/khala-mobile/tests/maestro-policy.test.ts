import { describe, expect, test } from "bun:test"

const mobileRoot = new URL("../", import.meta.url)

const read = (path: string) => Bun.file(new URL(path, mobileRoot)).text()

describe("Khala mobile Maestro flows", () => {
  test("define local-only startup and smoke flows", async () => {
    const [startup, fallback, signedIn] = await Promise.all([
      read(".maestro/shared/_OnFlowStart.yaml"),
      read(".maestro/flows/LaunchFallback.yaml"),
      read(".maestro/flows/SignedInThreadSmoke.yaml"),
    ])

    expect(startup).toContain("appId: ${MAESTRO_APP_ID}")
    expect(startup).toContain("clearState: true")
    expect(startup).toContain("clearKeychain: true")

    expect(fallback).toContain("Sign in with GitHub")
    expect(fallback).toContain("No desktop, Tailnet, or manual token is required.")

    expect(signedIn).toContain("${KHALA_MAESTRO_OWNER_USER_ID}")
    expect(signedIn).toContain("${KHALA_MAESTRO_TOKEN}")
    expect(signedIn).toContain("${KHALA_MAESTRO_THREAD_TITLE}")
    expect(signedIn).toContain("Khala mobile public Maestro smoke")
  })

  test("do not commit real credentials or hosted CI hooks", async () => {
    const files = await Promise.all([
      read(".maestro/shared/_OnFlowStart.yaml"),
      read(".maestro/flows/LaunchFallback.yaml"),
      read(".maestro/flows/SignedInThreadSmoke.yaml"),
    ])
    const allFlowText = files.join("\n")

    expect(allFlowText).not.toMatch(/oa_agent_[A-Za-z0-9_-]{8,}/)
    expect(allFlowText).not.toMatch(/Bearer\s+[A-Za-z0-9._-]+/)
    expect(allFlowText).not.toContain("eas ")
  })
})
