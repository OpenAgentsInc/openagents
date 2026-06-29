// CL / #4995: ambient-auth browser automation for genuinely-external actions
// (desktop, Bun main) — e.g. social publish, Canva export.
//
// COORDINATOR WIRING:
//   - Expose `runApprovedBrowserAction` from `src/bun/index.ts` and surface it
//     over the Electrobun RPC bridge as a webview-callable handler (e.g. a
//     `browser.runApprovedAction` verb). The webview only ever sees the typed
//     `BrowserActionOutcome` result, never the raw client or any session
//     material.
//   - The cloud approval/lifecycle decision seam is the SAME seam Pylon already
//     uses for sensitive actions (see pylon-control.ts `resolveApproval` /
//     `approvals.list`). Wire `requireCloudApproval` to forward the action over
//     loopback /command so the cloud (not the desktop) decides. This module
//     introduces NO new authority on the desktop: it cannot act without an
//     approved decision from that seam.
//   - The LIVE `BrowserAutomationClient` (driving the user's already-signed-in
//     local browser session via ambient auth — NO stored third-party OAuth, no
//     credential capture) is owner/runtime-gated: constructed in the Bun
//     process behind an explicit enable, and INJECTED here. This module never
//     drives a real browser itself so it stays fully unit-testable with fakes.

// A genuinely-external action to perform through the ambient browser session.
// `target` names the surface (e.g. "x", "canva"); `kind` the operation (e.g.
// "publish_post", "export_design"). `payload` is the action-specific, already
// redaction-reviewed parameters.
export type BrowserActionSpec = {
  readonly target: string
  readonly kind: string
  readonly payload: Readonly<Record<string, unknown>>
}

export type BrowserActionOutcome = {
  readonly performed: boolean
  // Approval decision that gated this action.
  readonly decision: "approved" | "denied"
  // Reason the cloud seam supplied for the decision, if any.
  readonly approvalReason: string | null
  // Client-reported result reference (e.g. a post URL) when performed, else null.
  readonly resultRef: string | null
  // A short error message when the client failed mid-action, else null.
  readonly error: string | null
}

// The injected browser-automation client. Uses the user's already-authenticated
// local browser session (ambient auth). No real browser at this layer.
export interface BrowserAutomationClient {
  performAction(action: BrowserActionSpec): Promise<{ resultRef?: string | null | undefined }>
}

// The cloud approval seam decision. `approved: false` MUST block the action.
export type CloudApprovalDecision = {
  readonly approved: boolean
  readonly reason?: string | null | undefined
}

function errMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}

// Run an externally-visible browser action ONLY after the cloud approval seam
// approves it. The seam is injected (`requireCloudApproval`) and is always
// consulted with the exact action before the client is touched.
//
// - denied => the client is never invoked; `{ performed: false, decision:
//   "denied" }`.
// - the approval seam itself rejecting => treated as denied (fail-closed); the
//   client is never invoked.
// - approved + client succeeds => `{ performed: true, decision: "approved" }`.
// - approved + client throws => `{ performed: false, decision: "approved",
//   error }` (the cloud said yes, the external action failed).
export async function runApprovedBrowserAction(input: {
  readonly action: BrowserActionSpec
  readonly client: BrowserAutomationClient
  readonly requireCloudApproval: (action: BrowserActionSpec) => Promise<CloudApprovalDecision>
}): Promise<BrowserActionOutcome> {
  let decision: CloudApprovalDecision
  try {
    decision = await input.requireCloudApproval(input.action)
  } catch (e) {
    // Fail closed: a broken/unreachable approval seam is NOT an approval.
    return {
      performed: false,
      decision: "denied",
      approvalReason: errMessage(e),
      resultRef: null,
      error: null,
    }
  }

  const approvalReason = decision.reason ?? null

  if (!decision.approved) {
    return {
      performed: false,
      decision: "denied",
      approvalReason,
      resultRef: null,
      error: null,
    }
  }

  try {
    const res = await input.client.performAction(input.action)
    return {
      performed: true,
      decision: "approved",
      approvalReason,
      resultRef: typeof res.resultRef === "string" ? res.resultRef : null,
      error: null,
    }
  } catch (e) {
    return {
      performed: false,
      decision: "approved",
      approvalReason,
      resultRef: null,
      error: errMessage(e),
    }
  }
}
