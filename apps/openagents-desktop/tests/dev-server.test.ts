import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import path from "node:path"

import config, { desktopDevServerHost, desktopDevServerPort, desktopPreviewHmrPlugin } from "../vite.config.ts"
import { classifyDesktopPreviewChange, decodeDesktopPreviewChangeEvent } from "../src/dev-preview-contract.ts"
import { desktopPreviewReloadRisk } from "../src/renderer/dev-preview.ts"
import { initialDesktopShellState } from "../src/renderer/shell.ts"

const root = path.resolve(import.meta.dirname, "..")
const resolveConfig = async (mode: string) => typeof config === "function"
  ? await config({ command: "serve", mode, isSsrBuild: false, isPreview: false })
  : config

describe("OpenAgents Desktop renderer dev loop", () => {
  test("uses one strict loopback Vite server and HMR endpoint", async () => {
    const resolved = await resolveConfig("development")
    expect(desktopDevServerHost).toBe("127.0.0.1")
    expect(desktopDevServerPort).toBe(5734)
    expect(resolved.server).toMatchObject({
      host: "127.0.0.1",
      port: 5734,
      strictPort: true,
      hmr: { host: "127.0.0.1", port: 5734, clientPort: 5734 },
    })
  })

  test("allocates a loopback preview port dynamically without weakening stable mode", async () => {
    const preview = await resolveConfig("openagents-preview")
    expect(preview.server).toMatchObject({
      host: "127.0.0.1",
      port: 0,
      strictPort: false,
      hmr: { host: "127.0.0.1" },
    })
    expect(preview.plugins).toHaveLength(3)
  })

  test("classifies preview changes conservatively and blocks unsafe Vite updates", () => {
    expect(classifyDesktopPreviewChange("apps/openagents-desktop/src/renderer/app.css")).toBe("css_hmr")
    expect(classifyDesktopPreviewChange("apps/openagents-desktop/src/renderer/react-composer.tsx")).toBe("react_fast_refresh")
    expect(classifyDesktopPreviewChange("apps/openagents-desktop/src/renderer/boot.ts")).toBe("renderer_reload_required")
    expect(classifyDesktopPreviewChange("apps/openagents-desktop/src/main.ts")).toBe("host_restart_required")
    expect(classifyDesktopPreviewChange("pnpm-lock.yaml")).toBe("dependency_sync_required")

    const sent: unknown[] = []
    const plugin = desktopPreviewHmrPlugin(path.resolve(root, "../.."))
    const hook = typeof plugin.handleHotUpdate === "function"
      ? plugin.handleHotUpdate
      : plugin.handleHotUpdate?.handler
    const result = hook?.call({} as never, {
      file: path.join(root, "src", "main.ts"),
      server: { ws: { send: (event: unknown) => sent.push(event) } },
    } as never)
    expect(result).toEqual([])
    expect(sent).toEqual([{
      type: "custom",
      event: "openagents:preview-change",
      data: { kind: "host_restart_required", pathRef: "apps/openagents-desktop/src/main.ts" },
    }])
  })

  test("accepts only bounded public-safe preview events and protects unsent renderer state", () => {
    expect(decodeDesktopPreviewChangeEvent({ kind: "host_restart_required", pathRef: "apps/openagents-desktop/src/main.ts" })).not.toBeNull()
    expect(decodeDesktopPreviewChangeEvent({ kind: "host_restart_required", pathRef: "/private/worktree/src/main.ts" })).toBeNull()
    expect(desktopPreviewReloadRisk(initialDesktopShellState("electron/darwin"))).toBe(false)
    expect(desktopPreviewReloadRisk({ ...initialDesktopShellState("electron/darwin"), input: "unsent" })).toBe(true)
    expect(desktopPreviewReloadRisk({
      ...initialDesktopShellState("electron/darwin"),
      composerDraftsByThread: { background: "unsent in another chat" },
    })).toBe(true)
    expect(desktopPreviewReloadRisk({
      ...initialDesktopShellState("electron/darwin"),
      questionCards: { pending: { selections: [["Approve"]], answered: false, answers: null } },
    })).toBe(true)
  })

  test("keeps the production renderer origin while proxying only development assets", () => {
    const main = readFileSync(path.join(root, "src", "main.ts"), "utf8")
    expect(main).toContain("OPENAGENTS_DESKTOP_DEV_SERVER_URL")
    expect(main).toContain('url.hostname !== "127.0.0.1"')
    expect(main).toContain('asset === "index.html" ? "/index.dev.html"')
    expect(main).toContain("bypassCustomProtocolHandlers: true")
    expect(main).toContain("if (app.isPackaged) return null")
    expect(main).toContain('path.join(app.getPath("appData"), "OpenAgents Dev")')
    expect(main).toContain("desktopDevServerUrl !== null")
    expect(main).toContain("desktopPreviewMode && !isolatedAppProofMode")
    expect(main).toContain("title: desktopApplicationName")
  })

  test("launches Electron only after Vite listens and retains the typed React entry", () => {
    const runner = readFileSync(path.join(root, "scripts", "dev.ts"), "utf8")
    const html = readFileSync(path.join(root, "index.dev.html"), "utf8")
    expect(runner.indexOf("await server.listen()")).toBeLessThan(runner.indexOf("Runtime.spawn"))
    expect(runner).toContain('OPENAGENTS_DESKTOP_DEV_SERVER_URL: devServerUrl')
    expect(runner).toContain('mode: preview ? "openagents-preview" : "development"')
    const boot = readFileSync(path.join(root, "src", "renderer", "boot.ts"), "utf8")
    expect(boot).toContain('import.meta.hot.on("vite:beforeFullReload"')
    expect(boot).toContain("An unsent draft, attachment, or pending owner interaction may be discarded")
    expect(html).toContain('src="/src/renderer/boot.ts"')
    expect(html).toContain("ws://127.0.0.1:5734")
  })

  test("the worktree launcher validates ownership and isolates lifecycle/profile state", () => {
    const launcher = readFileSync(path.join(root, "scripts", "oa-dev-preview"), "utf8")
    expect(launcher).toContain("--worktree must be an absolute path")
    expect(launcher).toContain("selected_common")
    expect(launcher).toContain("openagents-desktop-preview.XXXXXX")
    expect(launcher).toContain("OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF=1")
    expect(launcher).toContain("OPENAGENTS_DESKTOP_PREVIEW=1")
    expect(launcher).toContain("trap cleanup EXIT")
    expect(launcher).toContain('kill -TERM "$child_pid"')
  })

  test("the stable launcher reports freshness before focusing an existing app", () => {
    const launcher = readFileSync(path.join(root, "scripts", "oa-dev-launch"), "utf8")
    expect(launcher.indexOf("running stable preview is")).toBeLessThan(launcher.indexOf("osascript"))
    expect(launcher).toContain("restart OpenAgents Dev to update")
  })

  test("the stable launcher reclaims its exact Electron process after a leader crash", () => {
    const launcher = readFileSync(path.join(root, "scripts", "oa-dev-launch"), "utf8")
    expect(launcher).toContain('/usr/sbin/lsof -a -p "$candidate" -d txt -Fn')
    expect(launcher).toContain('"$launch_repo/"*"/Electron.app/Contents/MacOS/Electron"')
    expect(launcher).toContain("recovered launcher ownership for OpenAgents Dev process")
    expect(launcher).toContain("renderer port 5734 is not ready; stopping the stale launcher-owned process group")
    expect(launcher).toContain("openagents-desktop-(smoke-|preview\\.)")
    expect(launcher).toContain('blocking_unmanaged_pid="$candidate"')
    expect(launcher).toContain("an unmanaged OpenAgents Dev process is running")
    // On an explicit --restart a DEV build (Electron under node_modules — e.g.
    // a leftover agent worktree) is replaced instead of dead-ending the owner;
    // an installed release (not under node_modules) still fails closed.
    expect(launcher).toContain("--restart is replacing an unmanaged dev build")
    expect(launcher).toContain('*"/node_modules/"*"/electron/dist/Electron.app/Contents/MacOS/Electron"')
  })
})
