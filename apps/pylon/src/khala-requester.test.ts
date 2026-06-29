import { describe, expect, test } from "bun:test"

import { issuePylonKhalaRequest } from "./khala-requester.js"

describe("Pylon Khala requester", () => {
  test("surfaces public-safe dispatch gate evidence on failed coding requests", async () => {
    const fetchMock = (async () =>
      new Response(
        JSON.stringify({
          dispatchGate: {
            blockerRefs: [
              "blocker.public.pylon_dispatch.duplicate_active_assignment",
            ],
          },
          error: "target_pylon_unavailable",
          evidenceRefs: [
            "evidence.khala_coding.target_pylon_ref.dispatch_gate_blocked",
          ],
          reason:
            "The requested linked Pylon is available but the controlled assignment dispatch gate refused the coding lease.",
          requestedPylonRef: "pylon.33afd48282a649047e3a",
        }),
        { status: 409 },
      )) as unknown as typeof fetch

    const result = issuePylonKhalaRequest(
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.example",
        fetch: fetchMock,
      },
      {
        prompt: "Run the public-safe fixture.",
        targetPylonRef: "pylon.33afd48282a649047e3a",
        workflow: "codex_agent_task",
      },
    )

    await expect(result).rejects.toThrow(
      "requestedPylonRef=pylon.33afd48282a649047e3a",
    )
    await expect(result).rejects.toThrow(
      "evidence.khala_coding.target_pylon_ref.dispatch_gate_blocked",
    )
    await expect(result).rejects.toThrow(
      "blocker.public.pylon_dispatch.duplicate_active_assignment",
    )
  })
})
