import { describe, expect, test } from "bun:test"
import type {
  ConfirmedChatMessage,
  ConfirmedChatThread,
  KhalaSyncConversation,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

import {
  selectMobileConversation,
  type MobileConversationSelection,
} from "../src/conversation/mobile-conversation"
import { openMobileExperienceReconciler } from "../src/conversation/mobile-experience-reconciler"
import {
  chromeProps,
  initialHomeStateForConversation,
  type MobileSyncPhase,
} from "../src/screens/home-core"

const now = "2026-07-11T20:15:00.000Z"

/**
 * A confirmed personal-scope conversation whose reported phase is caller-
 * controllable, so a test can reproduce the real device race: the pre-live
 * read returns local, then the scope reaches live.
 */
const makePhaseControlledConversation = (): Readonly<{
  conversation: KhalaSyncConversation
  phase: () => "catching_up" | "live"
  setPhase: (phase: "catching_up" | "live") => void
}> => {
  let phase: "catching_up" | "live" = "catching_up"
  const status = () => ({ phase, cursor: 5, pendingMutationCount: 0 })
  const thread: ConfirmedChatThread = {
    threadRef: "thread.synced.1",
    title: "Synced",
    messageCount: 1,
    lastMessageAt: now,
    updatedAt: now,
    version: 3,
  }
  const message: ConfirmedChatMessage = {
    messageRef: "message.synced.1",
    threadRef: "thread.synced.1",
    body: "Confirmed",
    createdAt: now,
    updatedAt: now,
    version: 5,
  }
  const conversation: KhalaSyncConversation = {
    personalStatus: status,
    threadStatus: status,
    listConfirmedThreads: () => Effect.succeed([thread]),
    listConfirmedMessages: () => Effect.succeed([message]),
    openThread: () => Effect.succeed(undefined),
    closeThread: () => Effect.succeed(undefined),
    subscribeThread: () => () => undefined,
    createThread: () => Effect.succeed(1 as never),
    appendMessage: () => Effect.succeed(2 as never),
  }
  return { conversation, phase: () => phase, setPhase: next => { phase = next } }
}

const flush = async (): Promise<void> => {
  for (let attempt = 0; attempt < 5; attempt += 1) {
    await new Promise(resolve => setTimeout(resolve, 0))
  }
}

describe("contract openagents_mobile.chat.post_auth_live_upgrade.v1", () => {
  test("pre-live connect stays local, then a live scope upgrades to sync once", async () => {
    const fixture = makePhaseControlledConversation()
    const state = { selection: { mode: "local" } as MobileConversationSelection }
    const upgrades: MobileConversationSelection[] = []
    const reconciler = openMobileExperienceReconciler({
      currentMode: () => state.selection.mode,
      isAuthenticatedLive: () => fixture.phase() === "live",
      selectExperience: async () => ({
        conversation: await selectMobileConversation({ conversation: () => fixture.conversation }),
      }),
      onUpgrade: experience => {
        state.selection = experience.conversation
        upgrades.push(experience.conversation)
      },
    })

    // The single-shot read taken right after connect is necessarily pre-live.
    const preLive = await selectMobileConversation({ conversation: () => fixture.conversation })
    expect(preLive).toEqual({ mode: "local" })

    // Bootstrapping/catching-up ticks must NOT upgrade (scope not yet live).
    reconciler.observePhase("bootstrapping")
    reconciler.observePhase("catching_up")
    await flush()
    expect(upgrades).toHaveLength(0)
    expect(state.selection.mode).toBe("local")

    // The scope reaches live: the surface upgrades to the confirmed sync
    // selection exactly once.
    fixture.setPhase("live")
    reconciler.observePhase("live")
    await flush()
    expect(upgrades).toHaveLength(1)
    const upgraded = upgrades[0]!
    if (upgraded.mode !== "sync") throw new Error("expected sync selection")
    expect(upgraded.threads[0]).toMatchObject({ threadRef: "thread.synced.1", version: 3 })
    expect(upgraded.activeThread?.messages[0]).toMatchObject({
      messageRef: "message.synced.1",
      body: "Confirmed",
    })

    // Authority + owner-facing title/placeholder flip.
    const homeState = initialHomeStateForConversation(upgraded)
    expect(homeState.conversationAuthority).toBe("sync")
    expect(chromeProps(homeState).pillLabel).toBe("OpenAgents")
    expect(chromeProps(homeState).composerPlaceholder).toBe("Continue conversation")

    // A later live tick never re-runs the selection: no duplicate conversation.
    reconciler.observePhase("live")
    await flush()
    expect(upgrades).toHaveLength(1)
  })

  test("stays local when the scope never becomes live", async () => {
    const fixture = makePhaseControlledConversation()
    const state = { selection: { mode: "local" } as MobileConversationSelection }
    const upgrades: MobileConversationSelection[] = []
    const reconciler = openMobileExperienceReconciler({
      currentMode: () => state.selection.mode,
      // The scope stays pre-live: the callable host never exposes a live scope.
      isAuthenticatedLive: () => fixture.phase() === "live",
      selectExperience: async () => ({
        conversation: await selectMobileConversation({ conversation: () => fixture.conversation }),
      }),
      onUpgrade: experience => {
        state.selection = experience.conversation
        upgrades.push(experience.conversation)
      },
    })

    for (const phase of ["bootstrapping", "catching_up", "live", "catching_up"] as MobileSyncPhase[]) {
      reconciler.observePhase(phase)
      await flush()
    }

    // isAuthenticatedLive gated the "live" tick out because the fixture scope
    // never actually became live, so the genuine local fallback is preserved.
    expect(upgrades).toHaveLength(0)
    expect(state.selection).toEqual({ mode: "local" })
    const homeState = initialHomeStateForConversation(undefined)
    expect(homeState.conversationAuthority).toBe("local")
    expect(chromeProps(homeState).composerPlaceholder).toBe("Message Khala")
  })

  test("does not upgrade after close even if a live tick arrives", async () => {
    const fixture = makePhaseControlledConversation()
    fixture.setPhase("live")
    const state = { selection: { mode: "local" } as MobileConversationSelection }
    const upgrades: MobileConversationSelection[] = []
    const reconciler = openMobileExperienceReconciler({
      currentMode: () => state.selection.mode,
      isAuthenticatedLive: () => true,
      selectExperience: async () => ({
        conversation: await selectMobileConversation({ conversation: () => fixture.conversation }),
      }),
      onUpgrade: experience => {
        state.selection = experience.conversation
        upgrades.push(experience.conversation)
      },
    })
    reconciler.close()
    reconciler.observePhase("live")
    await flush()
    expect(upgrades).toHaveLength(0)
    expect(state.selection).toEqual({ mode: "local" })
  })
})
