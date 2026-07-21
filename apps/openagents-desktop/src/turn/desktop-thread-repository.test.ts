import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Effect } from "effect"
import { afterEach, describe, expect, test } from "vite-plus/test"

import type { TurnThreadRef } from "@openagentsinc/agent-runtime-schema"
import type { ThreadTurnMessage } from "@openagentsinc/agent-turn-runtime"

import { makeThreadStore } from "../thread-store.ts"
import { makeDesktopThreadRepository } from "./desktop-thread-repository.ts"

/**
 * #9127 persisted-note hygiene for the kernel thread repository:
 *
 * 1. An assistant answer with provenance persists WITH bounded attribution
 *    metadata (provider/model/dataDestination/usageTruth) so a delegated
 *    subagent answer stays attributed after reload.
 * 2. The Apple FM router's guided route-recommendation control frame (the JSON
 *    that selects a delegate) never persists as a conversation note.
 * 3. A plain apple_fm text answer still persists normally.
 */

const roots: Array<string> = []

const makeFixture = () => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-thread-repo-"))
  roots.push(root)
  const store = makeThreadStore(path.join(root, "threads.json"))
  const thread = store.newThread("repo test")
  return { store, thread, repository: makeDesktopThreadRepository(store) }
}

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const threadRefOf = (id: string): TurnThreadRef => id as unknown as TurnThreadRef

describe("makeDesktopThreadRepository (#9127)", () => {
  test("an assistant answer with provenance persists with attribution meta", async () => {
    const { store, thread, repository } = makeFixture()
    const message: ThreadTurnMessage = {
      role: "assistant",
      text: "The tests pass on main.",
      provenance: {
        candidate: "claude",
        model: "anthropic/claude",
        dataDestination: "remote_provider",
        usageTruth: "exact",
      },
    }
    await Effect.runPromise(repository.appendAssistant(threadRefOf(thread.id), message))
    const notes = store.open(thread.id)?.notes ?? []
    expect(notes.length).toBe(1)
    expect(notes[0]!.role).toBe("assistant")
    expect(notes[0]!.text).toBe("The tests pass on main.")
    expect(notes[0]!.meta).toEqual({
      provider: "claude",
      model: "anthropic/claude",
      dataDestination: "remote_provider",
      usageTruth: "exact",
    })
  })

  test("the apple_fm route-recommendation control frame is never persisted as a note", async () => {
    const { store, thread, repository } = makeFixture()
    const routeFrame: ThreadTurnMessage = {
      role: "assistant",
      text: JSON.stringify({
        candidate: "claude",
        taskClass: "delegate",
        reasonCode: "needs_delegation",
        confidence: 0.9,
      }),
      provenance: {
        candidate: "apple_fm",
        model: "apple/afm-on-device",
        dataDestination: "on_device_local",
        usageTruth: "exact",
      },
    }
    await Effect.runPromise(repository.appendAssistant(threadRefOf(thread.id), routeFrame))
    expect(store.open(thread.id)?.notes ?? []).toEqual([])
  })

  test("a plain apple_fm text answer persists normally (only exact route frames are skipped)", async () => {
    const { store, thread, repository } = makeFixture()
    const answer: ThreadTurnMessage = {
      role: "assistant",
      text: "Hello. I can help with that directly.",
      provenance: {
        candidate: "apple_fm",
        model: "apple/afm-on-device",
        dataDestination: "on_device_local",
        usageTruth: "exact",
      },
    }
    await Effect.runPromise(repository.appendAssistant(threadRefOf(thread.id), answer))
    const notes = store.open(thread.id)?.notes ?? []
    expect(notes.length).toBe(1)
    expect(notes[0]!.text).toBe("Hello. I can help with that directly.")
    expect(notes[0]!.meta?.provider).toBe("apple_fm")
  })

  test("a delegate answer that happens to contain route-shaped JSON still persists (skip is apple_fm-scoped)", async () => {
    const { store, thread, repository } = makeFixture()
    const answer: ThreadTurnMessage = {
      role: "assistant",
      text: JSON.stringify({
        candidate: "claude",
        taskClass: "delegate",
        reasonCode: "needs_delegation",
        confidence: 0.9,
      }),
      provenance: {
        candidate: "claude",
        model: "anthropic/claude",
        dataDestination: "remote_provider",
        usageTruth: "exact",
      },
    }
    await Effect.runPromise(repository.appendAssistant(threadRefOf(thread.id), answer))
    expect(store.open(thread.id)?.notes.length).toBe(1)
  })

  test("a provenance-free user message persists without meta", async () => {
    const { store, thread, repository } = makeFixture()
    await Effect.runPromise(
      repository.appendUser(threadRefOf(thread.id), { role: "user", text: "summarize that" }),
    )
    const notes = store.open(thread.id)?.notes ?? []
    expect(notes.length).toBe(1)
    expect(notes[0]!.role).toBe("user")
    expect(notes[0]!.meta).toBeUndefined()
  })
})
