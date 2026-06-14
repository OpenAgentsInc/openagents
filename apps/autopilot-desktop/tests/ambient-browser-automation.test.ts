import { describe, expect, test } from "bun:test"
import {
  runApprovedBrowserAction,
  type BrowserActionSpec,
  type BrowserAutomationClient,
  type CloudApprovalDecision,
} from "../src/bun/ambient-browser-automation.ts"

const action: BrowserActionSpec = {
  target: "x",
  kind: "publish_post",
  payload: { text: "shipping today" },
}

// A recording fake client — no real browser is driven at test time.
function fakeClient(input?: { resultRef?: string | null; fail?: string }): {
  client: BrowserAutomationClient
  calls: BrowserActionSpec[]
} {
  const calls: BrowserActionSpec[] = []
  return {
    calls,
    client: {
      performAction: async (a) => {
        calls.push(a)
        if (input?.fail) throw new Error(input.fail)
        return { resultRef: input?.resultRef ?? "https://x.com/post/1" }
      },
    },
  }
}

describe("#4995 runApprovedBrowserAction", () => {
  test("approved => the client performs the action and reports the result", async () => {
    const { client, calls } = fakeClient({ resultRef: "https://x.com/post/42" })
    const seamCalls: BrowserActionSpec[] = []
    const requireCloudApproval = async (a: BrowserActionSpec): Promise<CloudApprovalDecision> => {
      seamCalls.push(a)
      return { approved: true, reason: "owner approved" }
    }

    const outcome = await runApprovedBrowserAction({ action, client, requireCloudApproval })

    expect(outcome.performed).toBe(true)
    expect(outcome.decision).toBe("approved")
    expect(outcome.approvalReason).toBe("owner approved")
    expect(outcome.resultRef).toBe("https://x.com/post/42")
    expect(outcome.error).toBeNull()
    // The approval seam was consulted with the EXACT action, before the client.
    expect(seamCalls).toEqual([action])
    expect(calls).toEqual([action])
  })

  test("denied => the client is never invoked and nothing is performed", async () => {
    const { client, calls } = fakeClient()
    const seamCalls: BrowserActionSpec[] = []
    const requireCloudApproval = async (a: BrowserActionSpec): Promise<CloudApprovalDecision> => {
      seamCalls.push(a)
      return { approved: false, reason: "policy: no external publish" }
    }

    const outcome = await runApprovedBrowserAction({ action, client, requireCloudApproval })

    expect(outcome.performed).toBe(false)
    expect(outcome.decision).toBe("denied")
    expect(outcome.approvalReason).toBe("policy: no external publish")
    expect(outcome.resultRef).toBeNull()
    // Seam was asked; client was NOT touched.
    expect(seamCalls).toEqual([action])
    expect(calls).toEqual([])
  })

  test("the approval seam is always consulted with the action before any client call", async () => {
    const order: string[] = []
    const client: BrowserAutomationClient = {
      performAction: async () => {
        order.push("client")
        return { resultRef: null }
      },
    }
    const requireCloudApproval = async (): Promise<CloudApprovalDecision> => {
      order.push("approval")
      return { approved: true }
    }

    await runApprovedBrowserAction({ action, client, requireCloudApproval })

    expect(order).toEqual(["approval", "client"])
  })

  test("a rejecting approval seam fails closed (denied, client untouched)", async () => {
    const { client, calls } = fakeClient()
    const requireCloudApproval = async (): Promise<CloudApprovalDecision> => {
      throw new Error("cloud unreachable")
    }

    const outcome = await runApprovedBrowserAction({ action, client, requireCloudApproval })

    expect(outcome.performed).toBe(false)
    expect(outcome.decision).toBe("denied")
    expect(outcome.approvalReason).toBe("cloud unreachable")
    expect(calls).toEqual([])
  })

  test("approved but the client fails mid-action => not performed, error surfaced", async () => {
    const { client } = fakeClient({ fail: "browser session expired" })
    const requireCloudApproval = async (): Promise<CloudApprovalDecision> => ({ approved: true })

    const outcome = await runApprovedBrowserAction({ action, client, requireCloudApproval })

    expect(outcome.decision).toBe("approved")
    expect(outcome.performed).toBe(false)
    expect(outcome.error).toBe("browser session expired")
    expect(outcome.resultRef).toBeNull()
  })

  test("deterministic: identical approved inputs yield identical outcomes", async () => {
    const requireCloudApproval = async (): Promise<CloudApprovalDecision> => ({
      approved: true,
      reason: "ok",
    })
    const run = () =>
      runApprovedBrowserAction({
        action,
        client: fakeClient({ resultRef: "https://x.com/post/7" }).client,
        requireCloudApproval,
      })

    expect(await run()).toEqual(await run())
  })
})
