import { Deferred, Effect, Stream } from "effect"
import { afterEach, describe, expect, test } from "vite-plus/test"

import {
  IdeCursorCandidateRefSchema,
  IdeCursorDecisionRefSchema,
  IdeCursorDecisionSchema,
  type IdeCursorProviderInput,
  type IdeCursorStreamEvent,
} from "./cursor-contract.ts"
import {
  ideCursorFixtureCandidate,
  ideCursorFixtureCapabilities,
  ideCursorFixtureDecision,
  ideCursorFixtureDigest,
  ideCursorFixtureDisclosure,
  ideCursorFixtureInput,
  ideCursorFixtureRequest,
} from "./cursor-fixture.ts"
import { openIdeCursorHost, type IdeCursorHost } from "./cursor-host.ts"
import type { IdeCursorProviderShape } from "./cursor-provider.ts"
import {
  IdeCursorAuthorityFailure,
  type IdeCursorDocumentAuthorityShape,
} from "./cursor-service.ts"
import { IdeTimestampSchema } from "./project-contract.ts"

const hosts: IdeCursorHost[] = []

afterEach(async () => {
  while (hosts.length > 0) {
    const host = hosts.pop()
    if (host !== undefined) await host.dispose()
  }
})

const streamEvents = (input: IdeCursorProviderInput): ReadonlyArray<IdeCursorStreamEvent> => {
  const candidate = ideCursorFixtureCandidate(input.request)
  return [
    {
      _tag: "Identity",
      requestRef: input.request.requestRef,
      attemptRef: input.request.attemptRef,
      identity: input.request.identity,
    },
    { _tag: "Candidate", candidate },
    {
      _tag: "Finished",
      requestRef: input.request.requestRef,
      attemptRef: input.request.attemptRef,
      disclosure: ideCursorFixtureDisclosure(),
    },
  ]
}

const setup = async (
  validate: IdeCursorDocumentAuthorityShape["validate"] = () => Effect.void,
) => {
  const consumed = await Effect.runPromise(Deferred.make<void>())
  const provider = {
    capabilities: ideCursorFixtureCapabilities(),
    generate: (input: IdeCursorProviderInput) => Stream.fromIterable(streamEvents(input)).pipe(
      Stream.ensuring(Deferred.succeed(consumed, undefined)),
    ),
  } satisfies IdeCursorProviderShape
  const authority = {
    validate,
    accept: () => Effect.succeed({
      previousContentDigest: ideCursorFixtureDigest("a"),
      resultContentDigest: ideCursorFixtureDigest("d"),
    }),
    undo: () => Effect.succeed({
      previousContentDigest: ideCursorFixtureDigest("d"),
      resultContentDigest: ideCursorFixtureDigest("a"),
    }),
  } satisfies IdeCursorDocumentAuthorityShape
  const host = await openIdeCursorHost(provider, authority, {
    now: () => "2026-07-19T12:00:10.000Z",
  })
  hosts.push(host)
  return { host, consumed }
}

const startAndWait = async (fixture: Awaited<ReturnType<typeof setup>>) => {
  const result = await fixture.host.command({ _tag: "Start", input: ideCursorFixtureInput() })
  expect(result._tag).toBe("Succeeded")
  await Effect.runPromise(Deferred.await(fixture.consumed))
  return fixture.host.snapshot()
}

describe("IDE-09 main-owned cursor host", () => {
  test("decodes unknown commands and returns a stable invalid-input refusal", async () => {
    const { host } = await setup()
    const result = await host.command({ _tag: "Launch", input: null })
    expect(result).toMatchObject({
      _tag: "Refused",
      reason: "invalid_input",
      snapshot: { state: "idle", latestSequence: 0 },
    })
  })

  test("starts through the injected provider and exposes the canonical snapshot", async () => {
    const fixture = await setup()
    const snapshot = await startAndWait(fixture)
    expect(snapshot).toMatchObject({ state: "complete", latestSequence: 1 })
    expect(snapshot.candidates).toHaveLength(1)
    expect(snapshot.candidates[0]?.identity.effective.provider.value).toBe("provider.fixture")
  })

  test("routes accept, reject, and cancel through typed decisions", async () => {
    const fixture = await setup()
    const snapshot = await startAndWait(fixture)
    const candidate = snapshot.candidates[0]
    if (candidate === undefined) return

    expect((await fixture.host.command({
      _tag: "Decide",
      decision: ideCursorFixtureDecision(candidate, "accept"),
    }))._tag).toBe("Succeeded")

    const rejected = IdeCursorDecisionSchema.make({
      _tag: "Reject",
      decisionRef: IdeCursorDecisionRefSchema.make("ide.cursor-decision.fixture.reject"),
      candidateRef: candidate.candidateRef,
      requestRef: candidate.requestRef,
      sequence: candidate.sequence,
      decidedAt: IdeTimestampSchema.make("2026-07-19T12:00:11.000Z"),
      reason: "fixture rejection",
    })
    expect((await fixture.host.command({ _tag: "Decide", decision: rejected }))._tag).toBe("Succeeded")

    const cancelled = IdeCursorDecisionSchema.make({
      _tag: "Cancel",
      decisionRef: IdeCursorDecisionRefSchema.make("ide.cursor-decision.fixture.cancel"),
      candidateRef: candidate.candidateRef,
      requestRef: candidate.requestRef,
      sequence: candidate.sequence,
      decidedAt: IdeTimestampSchema.make("2026-07-19T12:00:12.000Z"),
      reason: "fixture cancellation",
    })
    const cancelledResult = await fixture.host.command({ _tag: "Decide", decision: cancelled })
    expect(cancelledResult._tag).toBe("Succeeded")
    if (cancelledResult._tag !== "Succeeded") return
    expect(cancelledResult.snapshot.decisions.map(decision => decision._tag)).toEqual(["Accept", "Reject", "Cancel"])
    expect(cancelledResult.snapshot.receipts.map(receipt => receipt.applied)).toEqual([true, false, false])
    expect(cancelledResult.snapshot.state).toBe("idle")
  })

  test("stops explicitly and refuses later starts with a stable reason", async () => {
    const { host } = await setup()
    const stopped = await host.command({ _tag: "Stop", reason: "fixture stop" })
    expect(stopped).toMatchObject({ _tag: "Succeeded", snapshot: { state: "stopped" } })
    const restarted = await host.command({ _tag: "Start", input: ideCursorFixtureInput() })
    expect(restarted).toMatchObject({ _tag: "Refused", reason: "stopped", snapshot: { state: "stopped" } })
  })

  test("owns disposal and keeps post-dispose commands fenced", async () => {
    const { host } = await setup()
    await host.dispose()
    const snapshot = await host.snapshot()
    expect(snapshot).toMatchObject({ state: "stopped", activeRequestRef: null, activeAttemptRef: null })
    expect(await host.command({ _tag: "Stop", reason: "again" })).toMatchObject({
      _tag: "Refused",
      reason: "stopped",
      snapshot: { state: "stopped" },
    })
  })

  test("maps typed authority and sequencing failures to stable transport reasons", async () => {
    const conflictFixture = await setup(() => Effect.fail(new IdeCursorAuthorityFailure({
      operation: "IdeCursor.validate",
      reason: "conflict",
      detail: "fixture conflict",
    })))
    expect(await conflictFixture.host.command({ _tag: "Start", input: ideCursorFixtureInput() })).toMatchObject({
      _tag: "Refused",
      reason: "conflict",
    })

    const sequenceFixture = await setup()
    await startAndWait(sequenceFixture)
    const skipped = ideCursorFixtureInput(ideCursorFixtureRequest("skipped", 3))
    expect(await sequenceFixture.host.command({ _tag: "Start", input: skipped })).toMatchObject({
      _tag: "Refused",
      reason: "stale_sequence",
      snapshot: { latestSequence: 1 },
    })

    const missingCandidate = IdeCursorDecisionSchema.make({
      _tag: "Reject",
      decisionRef: IdeCursorDecisionRefSchema.make("ide.cursor-decision.fixture.missing"),
      candidateRef: IdeCursorCandidateRefSchema.make("ide.cursor-candidate.fixture.missing"),
      requestRef: ideCursorFixtureRequest().requestRef,
      sequence: ideCursorFixtureRequest().sequence,
      decidedAt: IdeTimestampSchema.make("2026-07-19T12:00:13.000Z"),
      reason: "missing candidate",
    })
    expect(await sequenceFixture.host.command({ _tag: "Decide", decision: missingCandidate })).toMatchObject({
      _tag: "Refused",
      reason: "candidate_missing",
    })
  })
})
