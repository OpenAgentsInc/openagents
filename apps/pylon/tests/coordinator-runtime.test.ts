import { describe, expect, test } from "bun:test"
import { createIntentQueue } from "../src/node/intent-intake"
import { createCoordinatorRuntime } from "../src/coordinator/coordinator-runtime"

describe("coordinator runtime (CL-36)", () => {
  test("fans a checklist intent into sessions and advances status to shipped", async () => {
    const q = createIntentQueue()
    q.enqueue({
      intentId: "intent.1",
      title: "Improve the docs",
      body: "- fix the README\n- add a quickstart\n- document the API",
      submittedByClientRef: "mobile",
      createdAt: "2026-06-13T12:00:00.000Z",
    })

    const spawned: string[] = []
    const states = new Map<string, string>()
    const rt = createCoordinatorRuntime({
      intentQueue: q,
      spawnSession: async (input) => {
        const ref = `session.${spawned.length}`
        spawned.push(input.objective)
        states.set(ref, "running")
        return { sessionRef: ref }
      },
      sessionState: async (ref) => states.get(ref) ?? null,
      createWorktree: async (id, i) => `/tmp/wt/${id}-${i}`,
    })

    // First tick: plan + fan out (3 checklist parts -> 3 sessions), status fanning_out.
    await rt.tick()
    expect(spawned.length).toBe(3)
    expect(q.get("intent.1")?.status).toBe("fanning_out")
    expect(rt.view()[0].sessionRefs.length).toBe(3)

    // Sessions still running -> no advance.
    await rt.tick()
    expect(q.get("intent.1")?.status).toBe("fanning_out")

    // All sessions complete -> reconcile -> shipped.
    for (const ref of rt.view()[0].sessionRefs) states.set(ref, "completed")
    await rt.tick()
    expect(q.get("intent.1")?.status).toBe("shipped")
  })

  test("a single-line intent spawns one session; a failed session -> failed", async () => {
    const q = createIntentQueue()
    q.enqueue({ intentId: "i2", title: "One thing", body: "just do it", submittedByClientRef: "m", createdAt: "2026-06-13T12:00:00.000Z" })
    const states = new Map<string, string>()
    let n = 0
    const rt = createCoordinatorRuntime({
      intentQueue: q,
      spawnSession: async () => { const ref = `s${n++}`; states.set(ref, "running"); return { sessionRef: ref } },
      sessionState: async (ref) => states.get(ref) ?? null,
      createWorktree: async () => "/tmp/wt/x",
    })
    await rt.tick()
    expect(rt.view()[0].sessionRefs.length).toBe(1)
    for (const ref of rt.view()[0].sessionRefs) states.set(ref, "failed")
    await rt.tick()
    expect(q.get("i2")?.status).toBe("failed")
  })

  test("ship step classifies mode + gates spend + records a receipt (CL-37/CL-41)", async () => {
    const q = createIntentQueue()
    q.enqueue({ intentId: "i3", title: "ship it", body: "do the thing", submittedByClientRef: "m", createdAt: "2026-06-13T12:00:00.000Z" })
    const states = new Map<string, string>()
    let n = 0
    const recorded: any[] = []
    const rt = createCoordinatorRuntime({
      intentQueue: q,
      spawnSession: async () => { const ref = `s${n++}`; states.set(ref, "running"); return { sessionRef: ref } },
      sessionState: async (ref) => states.get(ref) ?? null,
      createWorktree: async () => "/tmp/wt/x",
      // JS-only change (fingerprint unchanged) + spend allowed -> ota, eligible.
      shipContext: async () => ({
        previousRuntimeFingerprint: "fp1",
        nextRuntimeFingerprint: "fp1",
        changedPaths: ["app/nodes.tsx"],
        spendGate: { decision: "allow" },
      }),
      recordShip: (intentId, decision) => recorded.push({ intentId, ...decision }),
    })
    await rt.tick()
    for (const ref of rt.view()[0].sessionRefs) states.set(ref, "completed")
    await rt.tick()
    expect(q.get("i3")?.status).toBe("shipped")
    expect(recorded.length).toBe(1)
    expect(recorded[0].intentId).toBe("i3")
    expect(recorded[0].shipMode).toBe("ota")
    expect(recorded[0].eligible).toBe(true)
    expect(recorded[0].decision).toBe("auto")
  })

  test("pause holds new fan-out; resume dispatches again (CL-17)", async () => {
    const q = createIntentQueue()
    q.enqueue({ intentId: "p1", title: "One thing", body: "do it", submittedByClientRef: "m", createdAt: "2026-06-13T12:00:00.000Z" })
    const states = new Map<string, string>()
    let n = 0
    const rt = createCoordinatorRuntime({
      intentQueue: q,
      spawnSession: async () => { const ref = `s${n++}`; states.set(ref, "running"); return { sessionRef: ref } },
      sessionState: async (ref) => states.get(ref) ?? null,
      createWorktree: async () => "/tmp/wt/x",
    })
    rt.pause()
    expect(rt.isPaused()).toBe(true)
    await rt.tick()
    // Paused: the received intent was NOT dispatched.
    expect(q.get("p1")?.status).toBe("received")
    expect(rt.view().length).toBe(0)

    rt.resume()
    expect(rt.isPaused()).toBe(false)
    await rt.tick()
    // Resumed: now it fans out.
    expect(q.get("p1")?.status).toBe("fanning_out")
    expect(rt.view()[0].sessionRefs.length).toBe(1)
  })

  test("ship step escalates when spend is denied (CL-41)", async () => {
    const q = createIntentQueue()
    q.enqueue({ intentId: "i4", title: "ship it", body: "do the thing", submittedByClientRef: "m", createdAt: "2026-06-13T12:00:00.000Z" })
    const states = new Map<string, string>()
    let n = 0
    const recorded: any[] = []
    const rt = createCoordinatorRuntime({
      intentQueue: q,
      spawnSession: async () => { const ref = `s${n++}`; states.set(ref, "running"); return { sessionRef: ref } },
      sessionState: async (ref) => states.get(ref) ?? null,
      createWorktree: async () => "/tmp/wt/x",
      // native fingerprint change + spend denied -> rebuild, not eligible, escalate.
      shipContext: async () => ({
        previousRuntimeFingerprint: "fp1",
        nextRuntimeFingerprint: "fp2",
        changedPaths: ["ios/Podfile"],
        spendGate: { decision: "deny" },
      }),
      recordShip: (intentId, decision) => recorded.push({ intentId, ...decision }),
    })
    await rt.tick()
    for (const ref of rt.view()[0].sessionRefs) states.set(ref, "completed")
    await rt.tick()
    expect(q.get("i4")?.status).toBe("shipped")
    expect(recorded[0].shipMode).toBe("rebuild")
    expect(recorded[0].eligible).toBe(false)
    expect(recorded[0].decision).toBe("escalate")
  })
})
