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
  TurnCandidate,
  type WorkContextEnvelope,
} from "@openagentsinc/agent-runtime-schema"
import {
  ProviderStreamEvent,
  type ProviderRegistryInterface,
  type ProviderStartInput,
} from "@openagentsinc/agent-turn-runtime"

import { ContextSource } from "@openagentsinc/agent-turn-runtime"

import { makeThreadStore } from "../thread-store.ts"
import { DesktopTurnSubmitChannel, decodeDesktopTurnSubmitResult } from "./desktop-turn-ipc.ts"
import { installDesktopTurnKernel } from "./desktop-turn-main.ts"
import {
  decodeEditorContextBinding,
  EDITOR_CONTEXT_BINDING_SCHEMA_LITERAL,
  makeEditorContextRegistry,
} from "./editor-context-binding.ts"
import { desktopContextSourceLayer } from "./desktop-turn-policy.ts"

const decodeDescriptor = S.decodeUnknownSync(InferenceProviderDescriptor)
const decodeCandidate = S.decodeUnknownSync(TurnCandidate)
const providerTurnRef = S.decodeUnknownSync(ProviderTurnRef)("providerturn.apple.1")

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

const answer = decodeCandidate({
  schema: CANDIDATE_SCHEMA_LITERAL,
  kind: "answer",
  candidateRef: S.decodeUnknownSync(CandidateRef)("candidate.apple.1"),
  provenance: {
    providerRef: "provider.apple_fm.local",
    candidate: "apple_fm",
    model: "apple-fm",
    taskClass: "local_answer",
    usageTruth: "estimated",
    dataDestination: "on_device_local",
    stale: false,
  },
  text: "here is the local answer",
})

/** A fake Apple FM registry that captures the exact context envelope the kernel resolved. */
const capturingRegistry = (): {
  readonly registry: ProviderRegistryInterface
  readonly captured: () => WorkContextEnvelope | null
} => {
  let captured: WorkContextEnvelope | null = null
  return {
    captured: () => captured,
    registry: {
      describe: Effect.succeed([appleDescriptor]),
      start: (input: ProviderStartInput) =>
        Effect.sync(() => {
          captured = input.context
          return {
            providerTurnRef,
            events: Stream.fromIterable([
              ProviderStreamEvent.Progress(),
              ProviderStreamEvent.Completed({ candidate: answer }),
            ]),
          }
        }),
    },
  }
}

const editorBinding = (threadRef: string) => ({
  schema: EDITOR_CONTEXT_BINDING_SCHEMA_LITERAL,
  threadRef,
  identity: { projectRef: "project.demo", rootRef: "root.demo", worktreeRef: "worktree.demo", generation: 3 },
  byteLimit: 8_000,
  items: [
    { kind: "active_file", itemRef: "item.active", derived: false, byteLength: 420, truncated: false, redacted: false },
    { kind: "local_symbol", itemRef: "item.symbol", derived: true, byteLength: 55, truncated: false, redacted: false },
  ],
})

let dir: string
beforeEach(() => {
  dir = mkdtempSync(path.join(tmpdir(), "afs-editor-join-"))
})
afterEach(() => {
  rmSync(dir, { recursive: true, force: true })
})

const submitResultOf = (raw: unknown) => {
  const decoded = decodeDesktopTurnSubmitResult(raw)
  if (decoded._tag === "None") throw new Error("submit result did not decode")
  return decoded.value
}

const install = (capture: ProviderRegistryInterface, withRegistry: boolean) => {
  const handlers = new Map<string, (event: unknown, value: unknown) => unknown>()
  const store = makeThreadStore(path.join(dir, "threads.json"))
  const thread = store.newThread("Editor join")
  const kernel = installDesktopTurnKernel({
    ipcMain: {
      handle: (channel, handler) => handlers.set(channel, handler),
      removeHandler: (channel) => handlers.delete(channel),
    },
    sender: () => ({ isDestroyed: () => false, send: () => {} }),
    threadStore: store,
    journalFilePath: path.join(dir, "agent-turns", "journal.json"),
    providerRegistry: capture,
    ...(withRegistry ? { editorContextRegistry: makeEditorContextRegistry() } : {}),
  })
  return { handlers, thread, kernel }
}

describe("Editor and chat share one turn service (AFS-05)", () => {
  test("an Editor turn feeds its IDE-08 context into the same shared kernel", async () => {
    const capture = capturingRegistry()
    const { handlers, thread, kernel } = install(capture.registry, true)
    try {
      const submit = handlers.get(DesktopTurnSubmitChannel)!
      const raw = await submit(null, {
        threadRef: thread.id,
        message: "explain this function",
        editorContext: editorBinding(thread.id),
      })
      const result = submitResultOf(raw)
      expect(result.outcome).toBe("answered")
      // The SAME kernel carried the editor's IDE-08 context: the provider saw the
      // effective manifest built from the bound active-file and symbol items.
      const context = capture.captured()
      expect(context).not.toBeNull()
      expect(context!.projectRef).toBe("project.demo")
      expect(context!.generation).toEqual({ state: "known", value: 3 })
      expect(context!.items.map((item) => item.kind)).toEqual(["active_file", "local_symbol"])
    } finally {
      await kernel.dispose()
    }
  })

  test("the shared context source withholds a binding from another editor generation", async () => {
    const registry = makeEditorContextRegistry()
    const binding = decodeEditorContextBinding(editorBinding("thread.demo"))
    registry.set(binding)
    // The authoritative editor moved to a newer generation; the stale binding is refused.
    registry.setExpectation({ ...binding.identity, generation: 4 })
    const manifest = await Effect.runPromise(
      Effect.gen(function* () {
        const source = yield* ContextSource
        return yield* source.manifest({
          threadRef: binding.threadRef,
          intent: { _tag: "Ask", text: "explain this" },
        })
      }).pipe(Effect.provide(desktopContextSourceLayer(registry))),
    )
    expect(manifest.items).toEqual([])
    expect(manifest.generation).toEqual({ state: "unknown", reason: "not_observed" })
  })

  test("the shared context source admits a matching binding", async () => {
    const registry = makeEditorContextRegistry()
    const binding = decodeEditorContextBinding(editorBinding("thread.demo"))
    registry.set(binding)
    registry.setExpectation(binding.identity)
    const manifest = await Effect.runPromise(
      Effect.gen(function* () {
        const source = yield* ContextSource
        return yield* source.manifest({
          threadRef: binding.threadRef,
          intent: { _tag: "Ask", text: "explain this" },
        })
      }).pipe(Effect.provide(desktopContextSourceLayer(registry))),
    )
    expect(manifest.items.map((item) => item.kind)).toEqual(["active_file", "local_symbol"])
  })

  test("a plain chat turn carries no editor context (AFS-03 path unchanged)", async () => {
    const capture = capturingRegistry()
    const { handlers, thread, kernel } = install(capture.registry, true)
    try {
      const submit = handlers.get(DesktopTurnSubmitChannel)!
      const raw = await submit(null, { threadRef: thread.id, message: "what is 2+2?" })
      const result = submitResultOf(raw)
      expect(result.outcome).toBe("answered")
      const context = capture.captured()
      expect(context).not.toBeNull()
      expect(context!.items).toEqual([])
    } finally {
      await kernel.dispose()
    }
  })
})
