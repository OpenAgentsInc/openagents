import { describe, expect, test } from "bun:test"
import {
  initCampaign,
  isComplete,
  markDispatched,
  markVerified,
  markMerged,
  markNeedsAttention,
  readyToDispatch,
  type CampaignTask,
} from "../scripts/multi-session-campaign"

describe("multi-session campaign scheduling core", () => {
  test("dispatches a diamond DAG continuously without unblocking a join early", () => {
    const tasks: CampaignTask[] = [
      { id: "A", dependsOn: [] },
      { id: "B", dependsOn: ["A"] },
      { id: "C", dependsOn: ["A"] },
      { id: "D", dependsOn: ["B", "C"] },
    ]

    let state = initCampaign(tasks)

    expect(readyToDispatch(state, 2)).toEqual(["A"])
    state = markDispatched(state, "A")
    expect(readyToDispatch(state, 2)).toEqual([])

    state = markMerged(state, "A")
    expect(state.tasks.B?.state).toBe("ready")
    expect(state.tasks.C?.state).toBe("ready")
    expect(state.tasks.D?.state).toBe("blocked")
    expect(readyToDispatch(state, 2)).toEqual(["B", "C"])

    state = markDispatched(state, "B")
    state = markDispatched(state, "C")
    state = markMerged(state, "B")
    expect(state.tasks.D?.state).toBe("blocked")
    expect(readyToDispatch(state, 2)).toEqual([])

    state = markMerged(state, "C")
    expect(state.tasks.D?.state).toBe("ready")
    expect(readyToDispatch(state, 2)).toEqual(["D"])
  })

  test("serializes ready tasks in the same conflict group", () => {
    const tasks: CampaignTask[] = [
      { id: "same-a", dependsOn: [], conflictGroup: "shared-worktree" },
      { id: "same-b", dependsOn: [], conflictGroup: "shared-worktree" },
      { id: "other", dependsOn: [] },
    ]

    let state = initCampaign(tasks)
    expect(readyToDispatch(state, 3)).toEqual(["same-a", "other"])

    state = markDispatched(state, "same-a")
    expect(readyToDispatch(state, 3)).toEqual(["other"])

    state = markMerged(state, "same-a")
    expect(readyToDispatch(state, 3)).toEqual(["same-b", "other"])
  })

  test("caps dispatch by remaining maxConcurrency", () => {
    const tasks: CampaignTask[] = [
      { id: "one", dependsOn: [] },
      { id: "two", dependsOn: [] },
      { id: "three", dependsOn: [] },
    ]

    let state = initCampaign(tasks)
    expect(readyToDispatch(state, 2)).toEqual(["one", "two"])

    state = markDispatched(state, "one")
    expect(readyToDispatch(state, 2)).toEqual(["two"])
  })

  test("needs_attention frees the slot without unblocking dependents", () => {
    const tasks: CampaignTask[] = [
      { id: "root", dependsOn: [] },
      { id: "dependent", dependsOn: ["root"] },
      { id: "independent", dependsOn: [] },
    ]

    let state = initCampaign(tasks)
    state = markDispatched(state, "root")
    state = markNeedsAttention(state, "root")

    expect(state.inFlight).toEqual([])
    expect(state.tasks.root?.state).toBe("needs_attention")
    expect(state.tasks.dependent?.state).toBe("blocked")
    expect(readyToDispatch(state, 1)).toEqual(["independent"])
  })

  test("verified frees the slot but dependents wait for merged", () => {
    const tasks: CampaignTask[] = [
      { id: "root", dependsOn: [] },
      { id: "dependent", dependsOn: ["root"] },
      { id: "independent", dependsOn: [] },
    ]

    let state = initCampaign(tasks)
    state = markDispatched(state, "root")
    state = markVerified(state, "root")

    expect(state.inFlight).toEqual([])
    expect(state.tasks.root?.state).toBe("verified")
    expect(state.tasks.dependent?.state).toBe("blocked")
    expect(readyToDispatch(state, 1)).toEqual(["independent"])

    state = markMerged(state, "root")
    expect(readyToDispatch(state, 2)).toEqual(["dependent", "independent"])
  })

  test("isComplete is true only when all tasks are merged", () => {
    const tasks: CampaignTask[] = [
      { id: "one", dependsOn: [] },
      { id: "two", dependsOn: [] },
    ]

    let state = initCampaign(tasks)
    expect(isComplete(state)).toBe(false)

    state = markMerged(state, "one")
    expect(isComplete(state)).toBe(false)

    state = markMerged(state, "two")
    expect(isComplete(state)).toBe(true)
  })
})
