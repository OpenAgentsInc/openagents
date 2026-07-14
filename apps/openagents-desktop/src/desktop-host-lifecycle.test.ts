import { describe, expect, test } from "vite-plus/test"

import { makeDesktopHostLifecycle } from "./desktop-host-lifecycle.ts"

const tracked = <Method extends "dispose" | "close">(
  method: Method,
): Readonly<{ service: Record<Method, () => void>; count: () => number }> => {
  let calls = 0
  return {
    service: { [method]: () => { calls++ } } as Record<Method, () => void>,
    count: () => calls,
  }
}

describe("Desktop host lifecycle", () => {
  test("replaces representative services and closes each old scope exactly once", () => {
    const runtimeA = tracked("dispose")
    const runtimeB = tracked("dispose")
    const accountA = tracked("dispose")
    const accountB = tracked("dispose")
    const historyA = tracked("dispose")
    const historyB = tracked("dispose")
    const workspaceA = tracked("dispose")
    const workspaceB = tracked("dispose")
    const syncA = tracked("close")
    const syncB = tracked("close")
    const voiceA = tracked("dispose")
    const voiceB = tracked("dispose")
    const lifecycle = makeDesktopHostLifecycle({
      runtime: runtimeA.service as never,
      account: accountA.service as never,
      history: historyA.service as never,
    })

    lifecycle.replaceWorkspace(workspaceA.service as never)
    lifecycle.replaceSync(syncA.service as never)
    lifecycle.replaceRuntime(runtimeB.service as never)
    lifecycle.replaceWorkspace(workspaceB.service as never)
    lifecycle.replaceSync(syncB.service as never)
    lifecycle.replaceAccount(accountB.service as never)
    lifecycle.replaceHistory(historyB.service as never)
    lifecycle.replaceVoice(voiceA.service as never)
    lifecycle.replaceVoice(voiceB.service as never)

    expect([runtimeA.count(), workspaceA.count(), syncA.count(), accountA.count(), historyA.count(), voiceA.count()]).toEqual([1, 1, 1, 1, 1, 1])
    lifecycle.dispose()
    lifecycle.dispose()
    expect([runtimeB.count(), workspaceB.count(), syncB.count(), accountB.count(), historyB.count(), voiceB.count()]).toEqual([1, 1, 1, 1, 1, 1])
    expect(lifecycle.snapshot()).toEqual({
      disposed: true,
      runtime: false,
      workspace: false,
      sync: false,
      account: false,
      history: false,
      voice: false,
      windowCount: 0,
    })
  })

  test("owns window teardown once and immediately closes late resources", () => {
    const runtime = tracked("dispose")
    const account = tracked("dispose")
    const history = tracked("dispose")
    const lateWorkspace = tracked("dispose")
    let windowCloses = 0
    const lifecycle = makeDesktopHostLifecycle({
      runtime: runtime.service as never,
      account: account.service as never,
      history: history.service as never,
    })
    const closeWindow = lifecycle.registerWindow("window.1", () => { windowCloses++ })
    closeWindow()
    closeWindow()
    expect(windowCloses).toBe(1)

    lifecycle.dispose()
    expect(lifecycle.replaceWorkspace(lateWorkspace.service as never)).toBeNull()
    expect(lateWorkspace.count()).toBe(1)
  })

  test("closes windows and gateway before their host dependencies", () => {
    const order: string[] = []
    const lifecycle = makeDesktopHostLifecycle({
      runtime: { dispose: () => order.push("runtime") } as never,
      account: { dispose: () => order.push("account") } as never,
      history: { dispose: () => order.push("history") } as never,
    })
    lifecycle.replaceWorkspace({ dispose: () => order.push("workspace") } as never)
    lifecycle.replaceSync({ close: () => order.push("sync") } as never)
    lifecycle.registerWindow("window.ordered", () => order.push("window"))

    lifecycle.dispose()
    expect(order).toEqual(["window", "runtime", "workspace", "account", "history", "sync"])
  })
})
