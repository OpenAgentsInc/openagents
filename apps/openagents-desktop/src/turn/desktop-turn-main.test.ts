import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Effect, Schema as S, Stream } from "effect"
import { afterEach, beforeEach, describe, expect, test } from "vite-plus/test"

import {
  CANDIDATE_SCHEMA_LITERAL,
  CandidateRef,
  InferenceProviderDescriptor,
  PROVIDER_SCHEMA_LITERAL,
  ProviderTurnRef,
  SafeMessageChainEntry,
  TurnCandidate,
} from "@openagentsinc/agent-runtime-schema"
import {
  ProviderStartError,
  ProviderStreamEvent,
  TurnServiceTesting,
  type ProviderRegistryInterface,
} from "@openagentsinc/agent-turn-runtime"

import { makeThreadStore } from "../thread-store.ts"
import {
  DesktopTurnCancelChannel,
  DesktopTurnEventChannel,
  DesktopTurnStartChannel,
  DesktopTurnStatusChannel,
  DesktopTurnSubmitChannel,
  decodeDesktopTurnSubmitResult,
} from "./desktop-turn-ipc.ts"
import { installDesktopTurnKernel } from "./desktop-turn-main.ts"

const decodeDescriptor = S.decodeUnknownSync(InferenceProviderDescriptor)
const decodeCandidate = S.decodeUnknownSync(TurnCandidate)
const decodeChainEntry = S.decodeUnknownSync(SafeMessageChainEntry)
const providerTurnRef = S.decodeUnknownSync(ProviderTurnRef)("providerturn.fake.1")

const answerCandidateWith = (candidateRef: string, text: string): TurnCandidate =>
  decodeCandidate({
    schema: CANDIDATE_SCHEMA_LITERAL,
    kind: "answer",
    candidateRef: S.decodeUnknownSync(CandidateRef)(candidateRef),
    provenance: {
      providerRef: "provider.apple_fm.local",
      candidate: "apple_fm",
      model: "apple-fm",
      taskClass: "local_answer",
      usageTruth: "estimated",
      dataDestination: "on_device_local",
      stale: false,
    },
    text,
  })

const appleDescriptor = decodeDescriptor({
  schema: PROVIDER_SCHEMA_LITERAL,
  providerRef: "provider.apple_fm.local",
  candidate: "apple_fm",
  model: "apple-fm",
  placement: "owner_local",
  supportedIntents: ["Ask"],
  supportedCandidateKinds: ["answer"],
  dataDestination: "on_device_local",
  usageTruth: "estimated",
  costClass: "local_resource_only",
  maxContextChars: 4000,
  maxOutputChars: 8192,
  supportsStreaming: false,
  supportsCancellation: true,
  supportsExternalTools: false,
  supportsExternalActions: false,
  readiness: { state: "ready" },
})

const codexDescriptor = (ready: boolean) =>
  decodeDescriptor({
    schema: PROVIDER_SCHEMA_LITERAL,
    providerRef: "provider.codex.local",
    candidate: "codex",
    model: "openai/codex",
    placement: "owner_local",
    supportedIntents: ["Ask"],
    supportedCandidateKinds: ["answer"],
    dataDestination: "remote_provider",
    usageTruth: "exact",
    costClass: "metered_provider_tokens",
    maxContextChars: 4000,
    maxOutputChars: 8192,
    supportsStreaming: true,
    supportsCancellation: true,
    supportsExternalTools: true,
    supportsExternalActions: true,
    readiness: ready ? { state: "ready" } : { state: "unavailable", reason: "account_missing" },
  })

/** A fake Apple FM router registry whose one answer is the given text. */
const appleRouterRegistry = (answerText: string): ProviderRegistryInterface => ({
  describe: Effect.succeed([appleDescriptor]),
  start: () =>
    Effect.succeed({
      providerTurnRef,
      events: Stream.fromIterable([
        ProviderStreamEvent.Progress(),
        ProviderStreamEvent.Completed({ candidate: answerCandidateWith("candidate.apple.1", answerText) }),
      ]),
    }),
})

/** A fake codex delegate registry. */
const codexRegistry = (ready: boolean): ProviderRegistryInterface => ({
  describe: Effect.succeed([codexDescriptor(ready)]),
  start: () =>
    ready
      ? Effect.succeed({
          providerTurnRef: S.decodeUnknownSync(ProviderTurnRef)("providerturn.codex.1"),
          events: Stream.fromIterable([
            ProviderStreamEvent.Progress(),
            ProviderStreamEvent.Chain({
              entries: [decodeChainEntry({ entryRef: "e.0", role: "assistant", text: "delegated work summary" })],
            }),
            ProviderStreamEvent.Completed({ candidate: answerCandidateWith("candidate.codex.1", "codex done") }),
          ]),
        })
      : Effect.fail(new ProviderStartError({ reason: "unauthorized" })),
})

const CODEX_ROUTE_JSON = JSON.stringify({
  candidate: "codex",
  taskClass: "delegate",
  reasonCode: "needs_delegation",
  confidence: 0.9,
})

const descriptor = decodeDescriptor({
  schema: PROVIDER_SCHEMA_LITERAL,
  providerRef: "provider.codex.1",
  candidate: "codex",
  model: "codex",
  placement: "owner_local",
  supportedIntents: ["Ask"],
  supportedCandidateKinds: ["answer"],
  dataDestination: "remote_provider",
  usageTruth: "exact",
  costClass: "metered_provider_tokens",
  maxContextChars: 4000,
  maxOutputChars: 8192,
  supportsStreaming: true,
  supportsCancellation: true,
  supportsExternalTools: false,
  supportsExternalActions: false,
  readiness: { state: "ready" },
})

const completingEvents: ReadonlyArray<ProviderStreamEvent> = [
  ProviderStreamEvent.Progress(),
  ProviderStreamEvent.Completed({ candidate: TurnServiceTesting.fixtureAnswerCandidate }),
]

const completingRegistry: ProviderRegistryInterface = {
  describe: Effect.succeed([descriptor]),
  start: () => Effect.succeed({ providerTurnRef, events: Stream.fromIterable(completingEvents) }),
}

interface RecordedSend {
  readonly channel: string
  readonly payload: unknown
}

let dir: string

beforeEach(() => {
  dir = mkdtempSync(path.join(tmpdir(), "afs-desktop-main-"))
})
afterEach(() => {
  rmSync(dir, { recursive: true, force: true })
})

const install = () => {
  const handlers = new Map<string, (event: unknown, value: unknown) => unknown>()
  const sent: RecordedSend[] = []
  const store = makeThreadStore(path.join(dir, "threads.json"))
  const thread = store.newThread("Kernel turn")
  const kernel = installDesktopTurnKernel({
    ipcMain: {
      handle: (channel, handler) => handlers.set(channel, handler),
      removeHandler: (channel) => handlers.delete(channel),
    },
    sender: () => ({ isDestroyed: () => false, send: (channel, payload) => sent.push({ channel, payload }) }),
    threadStore: store,
    journalFilePath: path.join(dir, "agent-turns", "journal.json"),
    providerRegistry: completingRegistry,
  })
  return { handlers, sent, thread, kernel }
}

const startPayload = (threadId: string) => ({
  requestRef: "request.main.1",
  threadRef: threadId,
  intent: { _tag: "Ask", text: "hi" },
  candidateSet: TurnServiceTesting.fixtureCandidateSet,
})

describe("Desktop turn main composition", () => {
  test("start dispatches through the kernel and forwards a terminal frame", async () => {
    const { handlers, sent, thread, kernel } = install()
    try {
      const start = handlers.get(DesktopTurnStartChannel)!
      const ack = (await start(null, startPayload(thread.id))) as { accepted: boolean }
      expect(ack.accepted).toBe(true)

      const terminal = sent.find(
        (record) =>
          record.channel === DesktopTurnEventChannel &&
          typeof record.payload === "object" &&
          record.payload !== null &&
          (record.payload as { kind?: string }).kind === "terminal",
      )
      expect(terminal).toBeDefined()
      const payload = terminal?.payload as { projection: { cardState: string } }
      expect(payload.projection.cardState).toBe("done")

      const status = handlers.get(DesktopTurnStatusChannel)!
      const projection = (await status(null, { requestRef: "request.main.1" })) as { cardState: string } | null
      expect(projection?.cardState).toBe("done")
    } finally {
      await kernel.dispose()
    }
  })

  test("cancel and status reject an unknown request cleanly", async () => {
    const { handlers, kernel } = install()
    try {
      const cancel = handlers.get(DesktopTurnCancelChannel)!
      const result = (await cancel(null, { requestRef: "request.unknown.1" })) as { ok: boolean }
      expect(result.ok).toBe(true)

      const status = handlers.get(DesktopTurnStatusChannel)!
      const projection = await status(null, { requestRef: "request.unknown.1" })
      expect(projection).toBeNull()
    } finally {
      await kernel.dispose()
    }
  })

  test("an invalid start request is rejected without dispatching", async () => {
    const { handlers, kernel } = install()
    try {
      const start = handlers.get(DesktopTurnStartChannel)!
      const ack = (await start(null, { nonsense: true })) as { accepted: boolean; error?: string }
      expect(ack.accepted).toBe(false)
    } finally {
      await kernel.dispose()
    }
  })
})

const installRouter = (routerAnswer: string, codexReady: boolean) => {
  const handlers = new Map<string, (event: unknown, value: unknown) => unknown>()
  const sent: RecordedSend[] = []
  const store = makeThreadStore(path.join(dir, "threads.json"))
  const thread = store.newThread("Delegation turn")
  const kernel = installDesktopTurnKernel({
    ipcMain: {
      handle: (channel, handler) => handlers.set(channel, handler),
      removeHandler: (channel) => handlers.delete(channel),
    },
    sender: () => ({ isDestroyed: () => false, send: (channel, payload) => sent.push({ channel, payload }) }),
    threadStore: store,
    journalFilePath: path.join(dir, "agent-turns", "journal.json"),
    providerRegistry: appleRouterRegistry(routerAnswer),
    codexProvider: codexRegistry(codexReady),
  })
  return { handlers, sent, thread, kernel }
}

const submitResultOf = (payload: unknown) => {
  const decoded = decodeDesktopTurnSubmitResult(payload)
  if (decoded._tag === "None") throw new Error("submit result did not decode")
  return decoded.value
}

const waitFor = async (predicate: () => boolean, attempts = 50): Promise<void> => {
  for (let index = 0; index < attempts; index += 1) {
    if (predicate()) return
    await new Promise((resolve) => setTimeout(resolve, 5))
  }
}

describe("AFS-04 codex delegation router", () => {
  test("an admitted codex recommendation starts ONE real codex turn and returns delegated", async () => {
    const { handlers, sent, thread, kernel } = installRouter(CODEX_ROUTE_JSON, true)
    try {
      const submit = handlers.get(DesktopTurnSubmitChannel)!
      const raw = await submit(null, { threadRef: thread.id, message: "please implement issue #9082" })
      const result = submitResultOf(raw)
      expect(result.outcome).toBe("delegated")
      expect(result.provider).toBe("codex")
      expect(result.delegationRequestRef).not.toBeNull()
      expect(result.objective).toBe("please implement issue #9082")

      const delegationRef = result.delegationRequestRef
      // The forked codex turn streams its lifecycle and reaches a terminal frame.
      await waitFor(() =>
        sent.some(
          (record) =>
            record.channel === DesktopTurnEventChannel &&
            (record.payload as { kind?: string; requestRef?: string }).kind === "terminal" &&
            (record.payload as { requestRef?: string }).requestRef === delegationRef,
        ),
      )
      const terminal = sent.find(
        (record) =>
          record.channel === DesktopTurnEventChannel &&
          (record.payload as { kind?: string; requestRef?: string }).kind === "terminal" &&
          (record.payload as { requestRef?: string }).requestRef === delegationRef,
      )
      expect(terminal).toBeDefined()
      const projection = (terminal!.payload as { projection: { cardState: string; messageChain: unknown[] } }).projection
      expect(projection.cardState).toBe("done")
      // The delegation card carries a redacted message chain.
      expect(projection.messageChain.length).toBeGreaterThanOrEqual(1)
    } finally {
      await kernel.dispose()
    }
  })

  test("an unavailable codex lane produces NO start and an honest refusal", async () => {
    const { handlers, sent, thread, kernel } = installRouter(CODEX_ROUTE_JSON, false)
    try {
      const submit = handlers.get(DesktopTurnSubmitChannel)!
      const raw = await submit(null, { threadRef: thread.id, message: "delegate this" })
      const result = submitResultOf(raw)
      expect(result.outcome).toBe("refused")
      expect(result.delegationRequestRef).toBeNull()
      // No codex terminal frame was ever forwarded — nothing started.
      await waitFor(() => false, 6)
      const codexTerminal = sent.find(
        (record) =>
          record.channel === DesktopTurnEventChannel &&
          typeof (record.payload as { requestRef?: string }).requestRef === "string" &&
          ((record.payload as { requestRef?: string }).requestRef ?? "").startsWith("request.codex."),
      )
      expect(codexTerminal).toBeUndefined()
    } finally {
      await kernel.dispose()
    }
  })

  test("a plain Apple FM answer does not delegate", async () => {
    const { handlers, thread, kernel } = installRouter("Sure, here is a plain answer.", true)
    try {
      const submit = handlers.get(DesktopTurnSubmitChannel)!
      const raw = await submit(null, { threadRef: thread.id, message: "what is 2+2?" })
      const result = submitResultOf(raw)
      expect(result.outcome).toBe("answered")
      expect(result.text).toBe("Sure, here is a plain answer.")
      expect(result.delegationRequestRef).toBeNull()
    } finally {
      await kernel.dispose()
    }
  })
})
