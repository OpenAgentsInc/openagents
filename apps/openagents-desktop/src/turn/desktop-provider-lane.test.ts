import { Effect, Schema as S, Stream } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  CONTEXT_ENVELOPE_SCHEMA_LITERAL,
  InferenceProviderDescriptor,
  PROVIDER_SCHEMA_LITERAL,
  TurnIntent,
  WorkContextEnvelope,
} from "@openagentsinc/agent-runtime-schema"
import {
  ProviderStartError,
  turnRequestRef,
  turnThreadRef,
  type ProviderStartInput,
} from "@openagentsinc/agent-turn-runtime"

import type { ClaudeLocalEvent, ClaudeLocalStartRequest } from "../claude-local-contract.ts"
import type { ProviderLane, ProviderLaneCapabilityReport } from "../provider-lane.ts"
import { makeDesktopLaneProvider } from "./desktop-provider-lane.ts"

const decodeDescriptor = S.decodeUnknownSync(InferenceProviderDescriptor)
const decodeContext = S.decodeUnknownSync(WorkContextEnvelope)
const decodeIntent = S.decodeUnknownSync(TurnIntent)

const capabilities = (): ProviderLaneCapabilityReport => ({
  laneRef: "fixture",
  provider: "fixture",
  models: ["fixture-model"],
  features: {
    skills: false,
    planOnly: false,
    reasoningEffort: false,
    images: false,
    fullAuto: false,
    interrupt: true,
    queueFollowup: false,
    steerTurn: false,
    steerChild: false,
    answerQuestion: false,
  },
  composer: { displayName: "Fixture", reasoningEfforts: [], permissionModes: ["owner_full"], approvals: "none", extensions: [] },
  policy: {
    source: "native-static-declaration",
    profileRef: "native:fixture:v1",
    evidence: "conformant",
    allowedModels: ["fixture-model"],
    allowedFeatures: ["interrupt"],
    allowedExtensions: [],
  },
  recovery: "interrupt_on_restart",
})

const makeFakeLane = (options: { readonly admit: boolean }): ProviderLane<null> => ({
  laneRef: "fixture",
  graphLaneRef: "fixture",
  eventChannel: "openagents:fixture",
  usageProvider: "fixture",
  capabilities,
  admit: () => (options.admit ? { ok: true, model: "fixture-model", context: null } : { ok: false, error: "refused" }),
  streamMeta: (ctx) => ({ lane: "fixture", turnRef: ctx.request.turnRef }),
  modelNoteText: (model) => `Fixture · ${model}`,
  runTurn: async ({ emit }) => {
    const events: ReadonlyArray<ClaudeLocalEvent> = [
      { kind: "text_delta", text: "Hello " },
      { kind: "text_delta", text: "world." },
    ]
    for (const event of events) emit(event)
    return { ok: true, text: "Hello world.", totalTokens: 3 }
  },
  interrupt: () => true,
  finalMeta: (ctx) => ({ lane: "fixture", turnRef: ctx.request.turnRef, model: "fixture-model", totalTokens: 3 }),
  failureMessage: (_reason, detail) => detail,
})

const descriptor = decodeDescriptor({
  schema: PROVIDER_SCHEMA_LITERAL,
  providerRef: "provider.codex.1",
  candidate: "codex",
  model: "fixture-model",
  placement: "owner_local",
  supportedIntents: ["Ask"],
  supportedCandidateKinds: ["answer"],
  dataDestination: "remote_provider",
  usageTruth: "estimated",
  costClass: "metered_provider_tokens",
  maxContextChars: 4000,
  maxOutputChars: 8192,
  supportsStreaming: true,
  supportsCancellation: true,
  supportsExternalTools: false,
  supportsExternalActions: false,
  readiness: { state: "ready" },
})

const buildRequest = (input: ProviderStartInput): ClaudeLocalStartRequest => ({
  turnRef: input.requestRef,
  threadRef: input.threadRef,
  message: "hi",
})

const requestRef = turnRequestRef("request.pl.1")
const threadRef = turnThreadRef("thread.pl.1")

const makeStartInput = (): ProviderStartInput => ({
  providerRef: descriptor.providerRef,
  requestRef,
  threadRef,
  intent: decodeIntent({ _tag: "Ask", text: "hi" }),
  context: decodeContext({
    schema: CONTEXT_ENVELOPE_SCHEMA_LITERAL,
    manifestRef: "context.fixture.1",
    threadRef,
    generation: { state: "unknown", reason: "not_observed" },
    createdAt: "2026-07-20T08:00:00.000Z",
    items: [],
    totalByteLength: 0,
    byteLimit: 0,
    truncated: false,
    redacted: false,
  }),
})

describe("Desktop provider-lane transition adapter", () => {
  test("a completing lane yields bounded progress then an answer candidate", async () => {
    const provider = makeDesktopLaneProvider({ lane: makeFakeLane({ admit: true }), descriptor, buildRequest })
    const events = await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const run = yield* provider.start(makeStartInput())
          const collected = yield* run.events.pipe(Stream.runCollect)
          return [...collected]
        }),
      ),
    )
    const terminal = events[events.length - 1]
    expect(terminal?._tag).toBe("Completed")
    if (terminal?._tag === "Completed") {
      expect(terminal.candidate.kind).toBe("answer")
      if (terminal.candidate.kind === "answer") expect(terminal.candidate.text).toBe("Hello world.")
    }
  })

  test("a refused admission surfaces a typed provider start error", async () => {
    const provider = makeDesktopLaneProvider({ lane: makeFakeLane({ admit: false }), descriptor, buildRequest })
    const error = await Effect.runPromise(
      Effect.scoped(provider.start(makeStartInput())).pipe(Effect.flip),
    )
    expect(error).toBeInstanceOf(ProviderStartError)
  })
})
