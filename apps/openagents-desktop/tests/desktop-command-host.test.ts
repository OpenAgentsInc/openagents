import { readFile } from "node:fs/promises"
import { describe, expect, test } from "vite-plus/test"

import {
  deferredDesktopCommand,
  dispatchNativeDesktopCommand,
  desktopCommandsFromArgv,
  makeDesktopCommandHost,
  parseDesktopCommandUrl,
} from "../src/desktop-command-host"
import {
  decodeDesktopDeferredCommandOrNull,
  desktopCanonicalCommandRegistry,
} from "../src/desktop-command-contract"
import { resolveDesktopDeferredCommandIntent } from "../src/renderer/command-registry"

describe("contract openagents_desktop.commands.host_routing.v1", () => {
  test("production composition owns single-instance, native menu, ready handshake, and schema-decoded renderer dispatch", async () => {
    const [main, preload, boot] = await Promise.all([
      readFile(new URL("../src/main.ts", import.meta.url), "utf8"),
      readFile(new URL("../src/preload.cts", import.meta.url), "utf8"),
      readFile(new URL("../src/renderer/boot.ts", import.meta.url), "utf8"),
    ])
    expect(main).toContain("app.requestSingleInstanceLock()")
    expect(main).toContain('app.on("second-instance"')
    expect(main).toContain('app.on("open-url"')
    expect(main).toContain("Menu.buildFromTemplate")
    expect(main).toContain("dispatchNativeDesktopCommand(command")
    expect(main).toContain("hasOpenWindow: () => BrowserWindow.getAllWindows()")
    expect(main).toContain("openWindow: () => { createWindow() }")
    expect(main).toContain('.filter(command => command.id !== "window.fullscreen_toggle")')
    expect(main).toContain('role: "togglefullscreen"')
    expect(main).toContain("bindingForNativeMenu(fullscreenCommand)")
    expect(main).toContain('command.id === "workspace.files" ? { registerAccelerator: false }')
    expect(main).toContain("DesktopCommandReadyChannel")
    expect(preload).toContain("decodeDesktopDeferredCommandOrNull")
    expect(preload).toContain("ipcRenderer.on(DesktopCommandEventChannel")
    expect(boot).toContain("bridge?.commands?.onCommand")
    expect(boot).toContain("resolveDesktopDeferredCommandIntent")
    expect(boot).toContain("desktopCommandShortcutMatches")
    expect(boot).toContain('IntentRef("DesktopFilesModeToggled"')
    expect(boot).toContain('source: "restore"')
    // The deferred-command rejection still surfaces a visible notice (CUT-15),
    // now through the transient command-notice controller (self-dismissing
    // toast) rather than a permanent commandNotice banner set inline.
    expect(boot).toContain("setTransientNotice")
  })

  test("parses only exact closed command URLs and ignores unrelated argv", () => {
    expect(parseDesktopCommandUrl("openagents://command/workspace.files")).toMatchObject({
      commandId: "workspace.files",
      arguments: { kind: "none" },
      source: "deep_link",
    })
    for (const invalid of [
      "https://command/workspace.files",
      "openagents://command/shell.exec",
      "openagents://command/workspace.files?path=/private",
      "openagents://user:pass@command/workspace.files",
      "openagents://command/workspace.files/extra",
    ]) expect(parseDesktopCommandUrl(invalid)).toBeNull()
    expect(desktopCommandsFromArgv([
      "/Applications/OpenAgents.app",
      "openagents://command/chat.new",
    ], "second_instance")).toMatchObject([{ commandId: "chat.new", source: "second_instance" }])
  })

  test("queues until renderer readiness, suppresses duplicates, bounds backlog, and detaches", () => {
    const host = makeDesktopCommandHost(2)
    const command = desktopCanonicalCommandRegistry.find(value => value.id === "chat.new")!
    const one = deferredDesktopCommand(command, "native_menu", "command.one")
    expect(host.enqueue(one)).toBe("accepted")
    const sent: Array<Readonly<{ requestRef: string; delivery: string }>> = []
    host.attach(value => sent.push({ requestRef: value.requestRef, delivery: value.delivery }))
    expect(sent).toEqual([{ requestRef: "command.one", delivery: "dispatch" }])
    expect(host.enqueue(one)).toBe("duplicate")
    expect(sent.at(-1)).toEqual({ requestRef: "command.one", delivery: "duplicate_rejected" })
    expect(host.enqueue({ commandId: "shell.exec" })).toBe("invalid")
    host.detach()
    expect(host.enqueue({ ...one, requestRef: "command.two" })).toBe("accepted")
    expect(host.enqueue({ ...one, requestRef: "command.three" })).toBe("accepted")
    expect(host.enqueue({ ...one, requestRef: "command.four" })).toBe("accepted")
    expect(host.pendingCount()).toBe(2)
    host.attach(value => sent.push({ requestRef: value.requestRef, delivery: value.delivery }))
    expect(sent.slice(-2)).toEqual([
      { requestRef: "command.three", delivery: "dispatch" },
      { requestRef: "command.four", delivery: "dispatch" },
    ])
    host.detach()
    expect(host.enqueue({ ...one, requestRef: "command.five" })).toBe("accepted")
    expect(host.pendingCount()).toBe(1)
  })

  test("Command+N recreates a closed last window before queuing a blank chat", () => {
    const chatNew = desktopCanonicalCommandRegistry.find(value => value.id === "chat.new")!
    const events: string[] = []
    const host = makeDesktopCommandHost()

    expect(dispatchNativeDesktopCommand(chatNew, {
      hasOpenWindow: () => false,
      openWindow: () => { events.push("window-opened") },
      enqueue: value => {
        const command = decodeDesktopDeferredCommandOrNull(value)
        events.push(`command-queued:${command?.commandId ?? "invalid"}`)
        return host.enqueue(value)
      },
    })).toBe("accepted")

    expect(events).toEqual(["window-opened", "command-queued:chat.new"])
    expect(host.pendingCount()).toBe(1)
    const delivered: string[] = []
    host.attach(command => { delivered.push(command.commandId) })
    expect(delivered).toEqual(["chat.new"])
  })

  test("Command+N reuses an open window without creating another", () => {
    const chatNew = desktopCanonicalCommandRegistry.find(value => value.id === "chat.new")!
    let opened = 0
    const host = makeDesktopCommandHost()
    host.attach(() => {})

    expect(dispatchNativeDesktopCommand(chatNew, {
      hasOpenWindow: () => true,
      openWindow: () => { opened += 1 },
      enqueue: host.enqueue,
    })).toBe("accepted")
    expect(opened).toBe(0)
  })

  test("renderer resolves the same command to one typed intent after readiness and owner gates", () => {
    const review = deferredDesktopCommand(
      desktopCanonicalCommandRegistry.find(value => value.id === "workspace.review")!,
      "native_menu",
      "command.review",
    )
    expect(resolveDesktopDeferredCommandIntent(review, {
      sessionReady: false,
      verifiedOwner: false,
      workspaceReady: false,
    })).toEqual({ state: "rejected", reason: "unavailable" })
    expect(resolveDesktopDeferredCommandIntent(review, {
      sessionReady: false,
      verifiedOwner: false,
      workspaceReady: true,
    })).toEqual({ state: "ready", intentName: "DesktopWorkspaceSelected", payload: "review" })
    expect(resolveDesktopDeferredCommandIntent({ ...review, arguments: { kind: "none" } }, {
      sessionReady: true,
      verifiedOwner: true,
      workspaceReady: true,
    })).toEqual({ state: "rejected", reason: "argument_mismatch" })
    expect(resolveDesktopDeferredCommandIntent({ ...review, delivery: "duplicate_rejected" }, {
      sessionReady: true,
      verifiedOwner: true,
      workspaceReady: true,
    })).toEqual({ state: "rejected", reason: "duplicate" })
  })
})
