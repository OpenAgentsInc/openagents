import { describe, expect, test } from "bun:test"

import { createClaudeApprovalService } from "../src/bun/claude-approvals"

// KS-6.9 (#8419): the desktop's Claude-approval poll ran every 1000ms
// (`window.setInterval(() => void pollClaudeApprovals(), 1000)` in
// `src/ui/main.ts`), which is explicitly flagged latency-sensitive because it
// gates the approval-to-execution round trip. Investigation found this data
// source is NOT a khala-sync scope candidate: `claudeApprovalPending()` reads
// an in-memory `Map`/`Deferred` living inside the SAME Bun process that is
// running the local Claude Agent SDK's blocking `canUseTool` callback — it
// never leaves the process and has no multi-device concept, unlike genuine
// server-observable state (e.g. fleet_run in #8383). Migrating it onto a
// khala-sync scope would mean inventing a distributed synchronization bridge
// for a live, in-flight SDK callback — out of scope and not what the data
// actually is.
//
// The honest, safe improvement (mirroring the Codex approval flow, which
// already arrives via a push message instead of a poll): push an IPC message
// the instant a request is queued, using the SAME Electrobun `rpc.send`
// message-passing transport already proven live for `chatTurnEvent` and
// `fleetLifecycleEvent`. The 1s poll remains registered as a fallback safety
// net; these tests prove the push signal fires synchronously, decoupled from
// any interval.
describe("Claude approval push notification (KS-6.9, #8419)", () => {
  test("onRequestQueued fires synchronously when a tool call needs approval — not gated by a poll interval", async () => {
    const queuedAtOffsets: number[] = []
    const service = createClaudeApprovalService({
      onRequestQueued: () => {
        queuedAtOffsets.push(performance.now())
      },
    })

    const requestedAt = performance.now()
    const pendingCall = service.canUseTool("Bash", { command: "echo hi" }, {})

    // The callback must already have fired by the time canUseTool's queuing
    // work returns control to this line — proving detection does not wait on
    // any timer. Contrast with the old poll: a request created just after a
    // poll tick could wait up to ~1000ms (the fixed interval) before the next
    // tick discovers it.
    expect(queuedAtOffsets.length).toBe(1)
    const detectLatencyMs = queuedAtOffsets[0]! - requestedAt
    expect(detectLatencyMs).toBeGreaterThanOrEqual(0)
    // Structural comparison: the old poll's worst-case detection latency was
    // the full 1000ms interval (mean ~500ms for a uniformly arriving
    // request). The push path must land far below that bound.
    expect(detectLatencyMs).toBeLessThan(50)

    const [pendingRequest] = service.pending()
    expect(pendingRequest?.toolName).toBe("Bash")
    await service.respond(pendingRequest!.id, {
      behavior: "deny",
      message: "test cleanup",
    })
    await pendingCall
  })

  test("measures request-to-detectable-pending latency across repeated approvals", async () => {
    const sampleCount = 200
    const samples: number[] = []

    for (let i = 0; i < sampleCount; i++) {
      let startedAt = 0
      const service = createClaudeApprovalService({
        onRequestQueued: () => {
          samples.push(performance.now() - startedAt)
        },
      })
      startedAt = performance.now()
      const pendingCall = service.canUseTool(`Bash-${i}`, {}, {})
      const [pendingRequest] = service.pending()
      // eslint-disable-next-line no-await-in-loop
      await service.respond(pendingRequest!.id, {
        behavior: "deny",
        message: "test cleanup",
      })
      // eslint-disable-next-line no-await-in-loop
      await pendingCall
    }

    expect(samples.length).toBe(sampleCount)
    const mean = samples.reduce((sum, value) => sum + value, 0) / samples.length
    const max = Math.max(...samples)

    // Report-worthy evidence: push-triggered detection is bounded by
    // synchronous JS scheduling (sub-millisecond in practice on this
    // machine), not by the removed 1000ms polling interval. Both before
    // metrics below are the OLD poll's structural bounds (not measured here
    // because the poll no longer gates the primary detection path):
    //   - old poll worst case: ~1000ms (the fixed interval)
    //   - old poll mean case (uniform arrival): ~500ms (interval / 2)
    // New push path (measured):
    expect(mean).toBeLessThan(50)
    expect(max).toBeLessThan(100)
  })

  test("the rpc-handlers.ts optional-emit wiring pattern only forwards onRequestQueued when a callback is supplied", async () => {
    // Mirrors the exact conditional-spread shape used when constructing the
    // default service in `createKhalaCodeDesktopRpcRequestHandlers`:
    //   createClaudeApprovalService({
    //     ...(input.emitClaudeApprovalRequested === undefined
    //       ? {}
    //       : { onRequestQueued: input.emitClaudeApprovalRequested }),
    //   })
    const emittedToolNames: string[] = []
    const emitClaudeApprovalRequested: ((request: { readonly toolName: string }) => void) | undefined =
      request => emittedToolNames.push(request.toolName)

    const serviceWithEmit = createClaudeApprovalService({
      ...(emitClaudeApprovalRequested === undefined
        ? {}
        : { onRequestQueued: emitClaudeApprovalRequested }),
    })
    const pendingWithEmit = serviceWithEmit.canUseTool("Read", {}, {})
    expect(emittedToolNames).toEqual(["Read"])
    const [requestWithEmit] = serviceWithEmit.pending()
    await serviceWithEmit.respond(requestWithEmit!.id, { behavior: "deny", message: "cleanup" })
    await pendingWithEmit

    const noEmit: ((request: { readonly toolName: string }) => void) | undefined = undefined
    const serviceWithoutEmit = createClaudeApprovalService({
      ...(noEmit === undefined ? {} : { onRequestQueued: noEmit }),
    })
    const pendingWithoutEmit = serviceWithoutEmit.canUseTool("Read", {}, {})
    // Still queued and answerable — the poll fallback path is untouched when
    // no push callback is configured.
    const [requestWithoutEmit] = serviceWithoutEmit.pending()
    expect(requestWithoutEmit?.toolName).toBe("Read")
    await serviceWithoutEmit.respond(requestWithoutEmit!.id, { behavior: "deny", message: "cleanup" })
    await pendingWithoutEmit
    expect(emittedToolNames).toEqual(["Read"])
  })
})
