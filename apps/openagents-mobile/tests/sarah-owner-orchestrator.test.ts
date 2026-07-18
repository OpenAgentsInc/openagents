import { ComponentValueBinding, IntentRef } from "@effect-native/core";
import { Effect, Stream } from "@effect-native/core/effect";
import {
  composerBlockId,
  decodeCodingComposerDraftSnapshot,
  emptyComposerState,
} from "@openagentsinc/khala-sync-client";
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
  test("keeps Sarah visible and starts owner authentication when access is locked", async () => {
    const calls: Array<string> = [];
    const program = buildHomeProgram({
      sessionActions: {
        signIn: async () => { calls.push("sign-in"); },
        signOut: async () => { calls.push("sign-out"); },
      },
    });
    const drawer = JSON.stringify(renderDrawerView(program.initialState));
    expect(drawer).toContain("Sarah · Sign in as owner");
    expect(drawer).toContain("OpenAgentsSignInPressed");
    program.session.signIn();
    await Effect.runPromise(Effect.yieldNow);
    expect(calls).toEqual(["sign-in"]);
  });

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
    const drawer = JSON.stringify(renderDrawerView(program.initialState));
    expect(drawer).toContain('"label":"Sarah"');
    expect(drawer).not.toContain("workspace-search");
    expect(drawer).not.toContain("workspace-status-filter");
    expect(drawer).not.toContain("workspace-fleet-summary");
    expect(drawer).not.toContain("drawer-bundle");
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

  test("clears an old coding composer before Sarah can send", async () => {
    const oldThread: MobileConversationThread = {
      ...thread,
      threadRef: "thread.coding.0123456789abcdef01234567",
      title: "Old coding chat",
    };
    let cleared = 0;
    let runtimeTarget: unknown;
    const host: MobileConversationHost = {
      listThreads: async () => [oldThread, thread],
      newThread: async () => ({ ok: true, thread: oldThread }),
      openThread: async (threadRef) => threadRef === thread.threadRef ? thread : oldThread,
      sendMessage: async (input) => {
        runtimeTarget = input.runtimeTarget;
        return { ok: true, thread };
      },
    };
    const composerState = emptyComposerState();
    const program = buildHomeProgram({
      sarah: principal,
      conversation: {
        mode: "sync",
        host,
        threads: [oldThread, thread],
        archivedThreads: [],
        activeThread: oldThread,
      },
      coding: {
        activeComposer: () => ({
          repositoryLabel: "openagents",
          worktreeLabel: "main",
          targetLabel: "Old coding target",
          draft: decodeCodingComposerDraftSnapshot({
            schema: "openagents.coding_composer_draft.v1",
            draftRef: "draft.old-coding-chat",
            ownerRef: "local_mobile_home",
            sessionRef: "session.old-coding-chat",
            threadRef: oldThread.threadRef,
            revision: 1,
            doc: {
              ...composerState.doc,
              blocks: [{
                id: composerBlockId("block-old-coding-chat"),
                kind: "paragraph",
                text: "Old coding draft",
                marks: [],
              }],
            },
            selection: composerState.selection,
            view: composerState.view,
            context: [],
            target: {
              laneRef: "lane.codex_app_server",
              providerRef: "provider.openai.codex",
              readiness: "ready",
              executionTargetRef: "target.old-coding-chat",
            },
            submission: { status: "editing" },
            updatedAt: now,
          }),
        }),
        directory: {
          authority: "confirmed",
          phase: "live",
          cacheState: "current",
          repositories: [],
          sessions: [],
        },
        clearSelection: async () => { cleared += 1; },
      } as never,
    });

    expect(program.initialState.codingComposer).not.toBeNull();
    await Effect.runPromise(program.report(
      IntentRef("ConversationThreadSelected", ComponentValueBinding()),
      { threadRef: thread.threadRef },
    ) as Effect.Effect<unknown>);
    const selected = await Effect.runPromise(lastState(program));
    expect(cleared).toBe(1);
    expect(selected.activeThreadRef).toBe(thread.threadRef);
    expect(selected.codingComposer).toBeNull();

    await Effect.runPromise(program.report(
      IntentRef("KhalaTurnSubmitted", ComponentValueBinding()),
      "What is the release status?",
    ) as Effect.Effect<unknown>);
    expect(runtimeTarget).toEqual({ lane: "hosted_khala" });
  });
});
