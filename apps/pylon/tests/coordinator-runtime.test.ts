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
})
