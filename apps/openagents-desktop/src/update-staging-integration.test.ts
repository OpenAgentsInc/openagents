import { readFile } from "node:fs/promises"
import { describe, expect, test } from "vite-plus/test"

const read = (file: string) => readFile(new URL(file, import.meta.url), "utf8")

describe("Desktop signed update staging integration", () => {
  test("main and preload both decode one fixed update channel", async () => {
    const [main, preload, boot, updateHost] = await Promise.all([
      read("main.ts"), read("preload.cts"), read("renderer/boot.ts"), read("update-staging-host.ts"),
    ])
    expect(main).toContain("decodeDesktopUpdateStagingAction(raw)")
    expect(main).toContain("openDesktopUpdateStagingHost")
    expect(main).toContain("openMacOSUpdateApplier")
    // REL-FEED-01 (#8993): the feed host/pin come only from the typed resolver,
    // and a rejected configuration disables checks instead of falling back.
    expect(main).toContain("resolveDesktopUpdateFeedConfig(process.env, desktopUpdateChannel)")
    expect(main).toContain("{ baseUrl: desktopUpdateFeed.baseUrl, pin: desktopUpdateFeed.pin }")
    expect(main).toContain('new Response(null, { status: 503 })')
    expect(main).toContain('case "apply": return desktopUpdateHost.apply()')
    expect(main).toContain('case "rollback": return desktopUpdateHost.rollback()')
    expect(main).toContain("desktopUpdateHost.reconcile()")
    expect(main).toContain("desktopUpdateHost.recordHealthyLaunch({ rendererReadyAt, providerReadyAt })")
    expect(main.indexOf("runSmoke(window)")).toBeLessThan(main.indexOf("const rendererReady ="))
    expect(main).toContain("desktopUpdateHost.recordCleanShutdown(drain)")
    expect(main.indexOf("desktopUpdateHost.recordCleanShutdown(drain)")).toBeGreaterThan(main.indexOf("await drainDesktopUpdateRuntimes()"))
    expect(main.indexOf("app.relaunch()", main.indexOf("const launchHealth"))).toBeGreaterThan(main.indexOf("desktopUpdateHost.recordCleanShutdown(drain)"))
    expect(main).not.toContain('app.on("before-quit", () => {\n  desktopUpdateHost.recordCleanShutdown')
    expect(main).toContain("updateRecoveryRequiresStartupExit(updateRecovery)")
    expect(main).toContain("drainChildRuntimes")
    const completion = updateHost.slice(updateHost.indexOf("const completeAutomaticRollback"), updateHost.indexOf("// Power loss can occur"))
    expect(updateHost).toContain("fsyncSync(temporaryDescriptor)")
    expect(updateHost).toContain("fsyncSync(parentDescriptor)")
    expect(completion.indexOf("writeDocument(documentFile, document)")).toBeLessThan(completion.indexOf('rmSync(path.join(input.root, "apply-transaction.json")'))
    expect(completion.indexOf('rmSync(path.join(input.root, "apply-transaction.json")')).toBeLessThan(completion.indexOf('rmSync(path.join(input.root, "rollback")'))
    expect(preload).toContain("decodeDesktopUpdateStagingAction(value)")
    expect(preload).toContain("decodeDesktopUpdateProjection")
    expect(preload).toContain("ipcRenderer.invoke(DesktopUpdateStagingChannel, request)")
    expect(boot).toContain('updateRendererHost.run("snapshot")')
    expect(preload).not.toContain("artifactUrl:")
    expect(preload).not.toContain(`ipcRenderer.on(DesktopUpdateStagingChannel`)
  })
})
