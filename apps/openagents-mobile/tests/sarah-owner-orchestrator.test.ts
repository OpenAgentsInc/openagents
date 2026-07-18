import { ComponentValueBinding, IntentRef } from "@effect-native/core";
import { Effect, Stream } from "@effect-native/core/effect";
import type { SarahPrincipalProjection } from "@openagentsinc/sarah";
import { describe, expect, test } from "vite-plus/test";

import type {
  MobileConversationHost,
  MobileConversationSelection,
  MobileConversationThread,
} from "../src/conversation/mobile-conversation";
import {
  buildHomeProgram,
  chromeProps,
  mobileHeaderProps,
  renderDrawerView,
} from "../src/screens/home-core";

const contractId = "openagents_mobile.sarah_owner_orchestrator.v1";
const now = "2026-07-18T15:00:00.000Z";
const thread: MobileConversationThread = {
  threadRef: "thread.sarah.0123456789abcdef01234567",
  title: "Sarah",
  status: "active",
  messageCount: 0,
  lastMessageAt: null,
  updatedAt: now,
  version: 1,
  messages: [],
};
const principal: SarahPrincipalProjection = {
  schema: "openagents.sarah.principal.v1",
  principalRef: "principal.sarah",
  displayName: "Sarah",
  role: "Owner orchestrator",
  threadRef: thread.threadRef,
  authorityProfileRef: "openagents.sarah-owner-orchestrator",
  authorityRevision: 1,
  rootAuthorityProfileRef: "openagents.owner-delegated-autonomy",
  rootAuthorityRevision: 3,
  memory: "durable_cited",
  capabilities: [],
};

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), (option) => {
    if (option._tag !== "Some") throw new Error("expected state");
    return option.value;
  });

describe(`contract ${contractId}`, () => {
  test("pins Sarah in the existing drawer and identifies the authority-bound thread", () => {
    const host: MobileConversationHost = {
      listThreads: async () => [thread],
      newThread: async () => ({ ok: true, thread }),
      openThread: async () => thread,
      sendMessage: async () => ({ ok: true, thread }),
    };
    const conversation: Extract<MobileConversationSelection, { mode: "sync" }> = {
      mode: "sync",
      host,
      threads: [thread],
      archivedThreads: [],
      activeThread: thread,
    };
    const program = buildHomeProgram({ conversation, sarah: principal });
    expect(mobileHeaderProps(program.initialState)).toEqual({
      title: "Sarah",
      subtitle: "Owner orchestrator · Authority v3",
    });
    expect(chromeProps(program.initialState).composerPlaceholder).toBe("Message Sarah");
    expect(JSON.stringify(renderDrawerView(program.initialState))).toContain(
      "Sarah · Owner orchestrator",
    );
  });

  test("forces ordinary Sarah messages through the hosted Khala lane", async () => {
    let runtimeTarget: unknown;
    const host: MobileConversationHost = {
      listThreads: async () => [thread],
      newThread: async () => ({ ok: true, thread }),
      openThread: async () => thread,
      sendMessage: async (input) => {
        runtimeTarget = input.runtimeTarget;
        return { ok: true, thread };
      },
    };
    const program = buildHomeProgram({
      sarah: principal,
      conversation: {
        mode: "sync",
        host,
        threads: [thread],
        archivedThreads: [],
        activeThread: thread,
      },
    });
    await Effect.runPromise(
      program.report(
        IntentRef("KhalaTurnSubmitted", ComponentValueBinding()),
        "What is the release status?",
      ) as Effect.Effect<unknown>,
    );
    await Effect.runPromise(Effect.yieldNow);
    expect(runtimeTarget).toEqual({ lane: "hosted_khala" });
    expect((await Effect.runPromise(lastState(program))).activeThreadRef).toBe(thread.threadRef);
  });
});
