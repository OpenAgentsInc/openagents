import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Effect, Schema as S, Stream } from "effect"
import { afterEach, beforeEach, describe, expect, test } from "vite-plus/test"

import {
  InferenceProviderDescriptor,
  PROVIDER_SCHEMA_LITERAL,
  ProviderTurnRef,
} from "@openagentsinc/agent-runtime-schema"
import {
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
} from "./desktop-turn-ipc.ts"
import { installDesktopTurnKernel } from "./desktop-turn-main.ts"

const decodeDescriptor = S.decodeUnknownSync(InferenceProviderDescriptor)
const providerTurnRef = S.decodeUnknownSync(ProviderTurnRef)("providerturn.fake.1")

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
