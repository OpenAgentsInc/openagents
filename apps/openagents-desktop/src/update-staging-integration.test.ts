import { readFile } from "node:fs/promises"
import { describe, expect, test } from "vite-plus/test"

const read = (file: string) => readFile(new URL(file, import.meta.url), "utf8")

describe("Desktop signed update staging integration", () => {
  test("main and preload both decode one fixed update channel", async () => {
    const [main, preload, boot] = await Promise.all([
      read("main.ts"), read("preload.cts"), read("renderer/boot.ts"),
    ])
    expect(main).toContain("decodeDesktopUpdateStagingAction(raw)")
    expect(main).toContain("openDesktopUpdateStagingHost")
    expect(main).toContain("openMacOSUpdateApplier")
    expect(main).toContain('case "apply": return desktopUpdateHost.apply()')
    expect(main).toContain('case "rollback": return desktopUpdateHost.rollback()')
    expect(preload).toContain("decodeDesktopUpdateStagingAction(value)")
    expect(preload).toContain("decodeDesktopUpdateProjection")
    expect(preload).toContain("ipcRenderer.invoke(DesktopUpdateStagingChannel, request)")
    expect(boot).toContain('updateRendererHost.run("snapshot")')
    expect(preload).not.toContain("artifactUrl:")
  })
})
