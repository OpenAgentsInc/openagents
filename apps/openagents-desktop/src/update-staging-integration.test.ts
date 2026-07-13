import { describe, expect, test } from "bun:test"

const read = (file: string) => Bun.file(new URL(file, import.meta.url)).text()

describe("Desktop signed update staging integration", () => {
  test("main and preload both decode one fixed update channel", async () => {
    const [main, preload, boot] = await Promise.all([
      read("main.ts"), read("preload.cts"), read("renderer/boot.ts"),
    ])
    expect(main).toContain("decodeDesktopUpdateStagingAction(raw)")
    expect(main).toContain("openDesktopUpdateStagingHost")
    expect(preload).toContain("decodeDesktopUpdateStagingAction(value)")
    expect(preload).toContain("decodeDesktopUpdateProjection")
    expect(preload).toContain("ipcRenderer.invoke(DesktopUpdateStagingChannel, request)")
    expect(boot).toContain('updateRendererHost.run("snapshot")')
    expect(preload).not.toContain("artifactUrl:")
  })
})
