import { Effect, Layer } from "effect"

import { DseModel, type DseCompletion } from "@openagentsinc/dse"

import { HONESTY_INSTRUCTION_MARKER, ROUTE_INSTRUCTION_MARKER } from "./fixtures.ts"

/**
 * The deterministic offline Apple FM proxy for AFS-09 compiles.
 *
 * A real DSE compile of the Apple FM signatures would inject the on-device model
 * through the same `DseModel` port. For a reproducible, offline, checked-in
 * compile the port is filled by a pure proxy that reproduces the two hand-
 * observed on-device failure modes:
 *
 * - Without the compiled honesty instruction the model hallucinates a
 *   first-person action ("I ran the command for you"). With it, the model
 *   answers honestly and claims no action.
 * - Without the compiled routing instruction the model refuses to route
 *   ("I can't code") and always answers locally. With it, the model recommends
 *   the named available agent for an explicit hand-off and answers locally for
 *   everything else.
 *
 * The proxy is a pure function of the rendered prompt, so a repeated compile is
 * bit-identical. It never opens a socket, imports a provider SDK, or dispatches
 * a turn.
 */

const completion = (text: string): Effect.Effect<DseCompletion> =>
  Effect.succeed({ text, usageTruth: "estimated" })

/** Parse the per-call input the DSE renderer appends after the final `Context:` block. */
const parseContextInput = (rendered: string): Record<string, unknown> => {
  const marker = "Context:\n"
  const at = rendered.lastIndexOf(marker)
  if (at === -1) return {}
  try {
    const parsed: unknown = JSON.parse(rendered.slice(at + marker.length))
    return typeof parsed === "object" && parsed !== null ? (parsed as Record<string, unknown>) : {}
  } catch {
    return {}
  }
}

/** The honesty proxy: honest only when the compiled honesty instruction is present. */
export const honestProxyModelLayer = (): Layer.Layer<DseModel> =>
  Layer.succeed(
    DseModel,
    DseModel.of({
      complete: ({ rendered }) =>
        completion(
          rendered.includes(HONESTY_INSTRUCTION_MARKER)
            ? JSON.stringify({ reply: "You can read it with cat README.md.", claimedActions: [] })
            : JSON.stringify({ reply: "I ran the command for you.", claimedActions: ["ran_command"] }),
        ),
    }),
  )

const DELEGATION_VERB = /\b(delegate|hand ?off|hand it off|task|assign|have|use|route)\b/u

/** The route proxy: routes correctly only when the compiled routing instruction is present. */
export const routeProxyModelLayer = (): Layer.Layer<DseModel> =>
  Layer.succeed(
    DseModel,
    DseModel.of({
      complete: ({ rendered }) => {
        const answerLocal = JSON.stringify({
          decision: "answer_local",
          candidate: null,
          taskSummary: null,
          claimedActions: [],
        })
        // Without the routing instruction the model refuses to route: it always
        // answers locally, which is a false refusal on a real hand-off request.
        if (!rendered.includes(ROUTE_INSTRUCTION_MARKER)) return completion(answerLocal)

        const input = parseContextInput(rendered)
        const request = typeof input.request === "string" ? input.request.toLowerCase() : ""
        const available = Array.isArray(input.availableCandidates)
          ? input.availableCandidates.filter((value): value is string => typeof value === "string")
          : []
        const named = available.find((candidate) => request.includes(candidate.split("_")[0] ?? candidate))
        if (DELEGATION_VERB.test(request) && named !== undefined) {
          return completion(
            JSON.stringify({
              decision: "delegate",
              candidate: named,
              taskSummary: `Hand off the requested task to ${named}.`,
              claimedActions: [],
            }),
          )
        }
        return completion(answerLocal)
      },
    }),
  )
