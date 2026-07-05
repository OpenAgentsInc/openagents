import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import type { KhalaCodeDesktopUpdaterStatus } from "../src/shared/rpc"
import { mountKhalaCodeUpdaterSettingsSection } from "../src/ui/khala-code-updater-settings-section"

const installDom = (): { readonly container: HTMLElement; readonly cleanup: () => void } => {
  const window = new Window()
  const previousWindow = globalThis.window
  const previousDocument = globalThis.document
  const previousHTMLElement = globalThis.HTMLElement

  Object.defineProperty(globalThis, "window", { configurable: true, value: window })
  Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
  Object.defineProperty(globalThis, "HTMLElement", { configurable: true, value: window.HTMLElement })

  const container = window.document.createElement("section")
  window.document.body.append(container)

  return {
    container: container as unknown as HTMLElement,
    cleanup: () => {
      Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
      Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
      Object.defineProperty(globalThis, "HTMLElement", { configurable: true, value: previousHTMLElement })
      window.close()
    },
  }
}

const status = (overrides: Partial<KhalaCodeDesktopUpdaterStatus> = {}): KhalaCodeDesktopUpdaterStatus => ({
  app: "Khala Code Desktop",
  capability: "in_app_updater",
  channel: "stable",
  currentVersion: "0.1.0",
  enabled: true,
  observedAt: "2026-07-05T00:00:00.000Z",
  ok: true,
  releaseNotesUrl: "https://github.com/OpenAgentsInc/openagents/releases/tag/khala-code-desktop-v0.1.0",
  state: { status: "idle" },
  ...overrides,
})

const values = (container: HTMLElement): string[] =>
  Array.from(container.querySelectorAll(".khala-settings-metric-value")).map(node => node.textContent ?? "")

describe("Khala Code updater settings section (#8440)", () => {
  test("renders channel/version metrics and offers a Check for Updates action when idle", async () => {
    const { cleanup, container } = installDom()
    try {
      const section = mountKhalaCodeUpdaterSettingsSection(container, {
        check: async () => ({ ok: true, status: status() }),
        download: async () => ({ ok: true, status: status() }),
        install: async () => ({ ok: true, status: status() }),
        openReleaseNotes: () => undefined,
        status: async () => status(),
      })
      await section.refresh()

      expect(values(container)).toEqual(["stable", "0.1.0", "Not checked yet"])
      const button = container.querySelector("button.khala-settings-refresh") as HTMLButtonElement
      expect(button.textContent).toBe("Check for Updates")
      expect(button.disabled).toBe(false)
    } finally {
      cleanup()
    }
  })

  test("clicking the action button triggers check() and re-renders the returned status", async () => {
    const { cleanup, container } = installDom()
    try {
      let checkCalls = 0
      const section = mountKhalaCodeUpdaterSettingsSection(container, {
        check: async () => {
          checkCalls += 1
          return { ok: true, status: status({ state: { checkedAt: "2026-07-05T00:00:00.000Z", status: "available", version: "0.2.0" } }) }
        },
        download: async () => ({ ok: true, status: status() }),
        install: async () => ({ ok: true, status: status() }),
        openReleaseNotes: () => undefined,
        status: async () => status(),
      })
      await section.refresh()
      const button = container.querySelector("button.khala-settings-refresh") as HTMLButtonElement
      button.click()
      await Promise.resolve()
      await Promise.resolve()

      expect(checkCalls).toBe(1)
      const updated = container.querySelector("button.khala-settings-refresh") as HTMLButtonElement
      expect(updated.textContent).toBe("Download Update")
    } finally {
      cleanup()
    }
  })

  // Oracle for khala_code.desktop.updater_never_silently_installs.v1
  test("the ready state offers Restart to Install and never auto-invokes install()", async () => {
    const { cleanup, container } = installDom()
    try {
      let installCalls = 0
      const section = mountKhalaCodeUpdaterSettingsSection(container, {
        check: async () => ({ ok: true, status: status() }),
        download: async () => ({ ok: true, status: status() }),
        install: async () => {
          installCalls += 1
          return { ok: true, status: status() }
        },
        openReleaseNotes: () => undefined,
        status: async () => status({ state: { status: "ready", version: "0.2.0" } }),
      })
      await section.refresh()

      expect(installCalls).toBe(0)
      const button = container.querySelector("button.khala-settings-refresh") as HTMLButtonElement
      expect(button.textContent).toBe("Restart to Install")
      button.click()
      await Promise.resolve()
      await Promise.resolve()
      expect(installCalls).toBe(1)
    } finally {
      cleanup()
    }
  })

  // Oracle for khala_code.desktop.updater_never_silently_installs.v1
  test("a downloading state disables the action button so downloading/installing can never overlap", async () => {
    const { cleanup, container } = installDom()
    try {
      const section = mountKhalaCodeUpdaterSettingsSection(container, {
        check: async () => ({ ok: true, status: status() }),
        download: async () => ({ ok: true, status: status() }),
        install: async () => ({ ok: true, status: status() }),
        openReleaseNotes: () => undefined,
        status: async () =>
          status({ state: { progressPercent: 42, status: "downloading", version: "0.2.0" } }),
      })
      await section.refresh()

      const button = container.querySelector("button.khala-settings-refresh") as HTMLButtonElement
      expect(button.disabled).toBe(true)
      expect(values(container)).toContain("Downloading 0.2.0 (42%)")
    } finally {
      cleanup()
    }
  })

  // Oracle for khala_code.desktop.updater_error_states_legible_and_retryable.v1
  test("an error state is legible and offers Retry when retryable", async () => {
    const { cleanup, container } = installDom()
    try {
      const section = mountKhalaCodeUpdaterSettingsSection(container, {
        check: async () => ({ ok: true, status: status() }),
        download: async () => ({ ok: true, status: status() }),
        install: async () => ({ ok: true, status: status() }),
        openReleaseNotes: () => undefined,
        status: async () => status({ state: { message: "network unreachable", retryable: true, status: "error" } }),
      })
      await section.refresh()

      expect(values(container)).toContain("network unreachable")
      const button = container.querySelector("button.khala-settings-refresh") as HTMLButtonElement
      expect(button.textContent).toBe("Retry")
      expect(button.disabled).toBe(false)
    } finally {
      cleanup()
    }
  })

  test("a disabled build shows an honest disabled message instead of update controls", async () => {
    const { cleanup, container } = installDom()
    try {
      const section = mountKhalaCodeUpdaterSettingsSection(container, {
        check: async () => ({ ok: true, status: status() }),
        download: async () => ({ ok: true, status: status() }),
        install: async () => ({ ok: true, status: status() }),
        openReleaseNotes: () => undefined,
        status: async () => status({ enabled: false }),
      })
      await section.refresh()

      expect(container.textContent).toContain("In-app updates are disabled for this build.")
      expect(container.querySelector("button.khala-settings-refresh")).toBeNull()
    } finally {
      cleanup()
    }
  })

  test("clicking Release Notes opens the current build's release notes URL", async () => {
    const { cleanup, container } = installDom()
    try {
      const opened: string[] = []
      const section = mountKhalaCodeUpdaterSettingsSection(container, {
        check: async () => ({ ok: true, status: status() }),
        download: async () => ({ ok: true, status: status() }),
        install: async () => ({ ok: true, status: status() }),
        openReleaseNotes: url => opened.push(url),
        status: async () => status(),
      })
      await section.refresh()

      const releaseNotesButton = container.querySelector(
        "button.khala-settings-updater-release-notes",
      ) as HTMLButtonElement
      releaseNotesButton.click()
      expect(opened).toEqual([
        "https://github.com/OpenAgentsInc/openagents/releases/tag/khala-code-desktop-v0.1.0",
      ])
    } finally {
      cleanup()
    }
  })
})
