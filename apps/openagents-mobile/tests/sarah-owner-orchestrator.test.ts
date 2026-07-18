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
  renderContentView,
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
const recentThread: MobileConversationThread = {
  ...thread,
  messageCount: 2,
  lastMessageAt: now,
  messages: [{
    messageRef: "message.sarah.1",
    threadRef: thread.threadRef,
    body: "Earlier message",
    createdAt: "2026-07-18T14:58:00.000Z",
    updatedAt: "2026-07-18T14:58:00.000Z",
    version: 1,
  }, {
    messageRef: "message.sarah.2",
    threadRef: thread.threadRef,
    body: "Most recent message",
    createdAt: now,
    updatedAt: now,
    version: 2,
  }],
};
const principal: SarahPrincipalProjection = {
  schema: "openagents.sarah.principal.v1",
  principalRef: "principal.sarah",
  displayName: "Sarah",
  role: "Owner orchestrator",
  threadRef: thread.threadRef,
  authorityProfileRef: "openagents.sarah-owner-orchestrator",
  authorityRevision: 3,
  rootAuthorityProfileRef: "openagents.owner-delegated-autonomy",
  rootAuthorityRevision: 5,
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
      subtitle: null,
    });
    const home = JSON.stringify(renderContentView(program.initialState));
    expect(home).toContain('"key":"khala-surface"');
    expect(home).toContain('"paddingTop":"2"');
    expect(home).toContain('"autoCorrect":false');
    expect(home).not.toContain("Owner orchestrator");
    expect(chromeProps(program.initialState).composerPlaceholder).toBe("Message Sarah");
    const drawer = JSON.stringify(renderDrawerView(program.initialState));
    expect(drawer).toContain('"label":"Sarah"');
    expect(drawer).not.toContain("workspace-search");
    expect(drawer).not.toContain("workspace-status-filter");
    expect(drawer).not.toContain("workspace-fleet-summary");
    expect(drawer).not.toContain("drawer-bundle");
  });

  test("positions an initially selected Sarah conversation at its newest message", () => {
    const host: MobileConversationHost = {
      listThreads: async () => [recentThread],
      newThread: async () => ({ ok: true, thread: recentThread }),
      openThread: async () => recentThread,
      sendMessage: async () => ({ ok: true, thread: recentThread }),
    };
    const program = buildHomeProgram({
      sarah: principal,
      conversation: {
        mode: "sync",
        host,
        threads: [recentThread],
        archivedThreads: [],
        activeThread: recentThread,
      },
    });

    expect(program.initialState.khala.transcriptPinned).toBe(true);
    expect(program.initialState.khala.transcriptScrollToKey).toBe("message.sarah.2");
    expect(JSON.stringify(renderContentView(program.initialState)))
      .toContain('"scrollToKey":"message.sarah.2"');
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

  test("keeps a submitted Sarah turn pinned to the newest message while the keyboard contracts", async () => {
    let finish: ((value: Awaited<ReturnType<MobileConversationHost["sendMessage"]>>) => void) | undefined;
    const host: MobileConversationHost = {
      listThreads: async () => [recentThread],
      newThread: async () => ({ ok: true, thread: recentThread }),
      openThread: async () => recentThread,
      sendMessage: () => new Promise(resolve => { finish = resolve; }),
    };
    const program = buildHomeProgram({
      sarah: principal,
      conversation: {
        mode: "sync",
        host,
        threads: [recentThread],
        archivedThreads: [],
        activeThread: recentThread,
      },
    });

    program.khala.submitTurn("Keep this visible");
    await Effect.runPromise(Effect.yieldNow);
    const pending = await Effect.runPromise(lastState(program));
    expect(pending.khala.transcriptPinned).toBe(true);
    expect(pending.khala.transcriptScrollToKey).toBe("pending-mobile-1");

    await Effect.runPromise(program.report(
      IntentRef("TranscriptPinnedChanged", ComponentValueBinding()),
      false,
    ) as Effect.Effect<unknown>);
    const contracted = await Effect.runPromise(lastState(program));
    expect(contracted.khala.transcriptPinned).toBe(true);
    expect(contracted.khala.transcriptScrollToKey).toBe("pending-mobile-1");

    finish?.({ ok: true, thread: recentThread });
    await Effect.runPromise(Effect.yieldNow);
  });

  test("keeps Sarah conversational by withholding completed runtime plumbing", () => {
    const host: MobileConversationHost = {
      listThreads: async () => [thread],
      newThread: async () => ({ ok: true, thread }),
      openThread: async () => thread,
      sendMessage: async () => ({ ok: true, thread }),
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
    const view = JSON.stringify(renderContentView({
      ...program.initialState,
      khala: {
        ...program.initialState.khala,
        runtimeTurn: {
          runRef: "run.internal-runtime-detail",
          status: "failed",
        },
        runtimeControlActionsAvailable: true,
        entries: [{
          key: "runtime-detail",
          role: "tool",
          text: "Internal runtime detail",
          status: "done",
          work: {
            groupRef: "work.runtime-detail",
            runRef: "run.runtime-detail",
            summary: "Internal runtime detail",
            status: "success",
            identityLabel: "OpenAgents · hosted runtime",
            elapsedLabel: "3s",
            createdAt: now,
            totalItemCount: 1,
            omittedItemCount: 0,
            items: [],
          },
        }, {
          key: "assistant-reply",
          role: "assistant",
          text: "Visible Sarah reply [source.internal.private-ref]",
          status: "done",
          createdAt: now,
          provenanceLabel: "Gemma 4 31B · Google AI Studio",
        }],
        transcriptVisibleCount: 2,
      },
    }));

    expect(view).toContain("Visible Sarah reply");
    expect(view).not.toContain("Internal runtime detail");
    expect(view).not.toContain("source.internal.private-ref");
    expect(view).toContain("Gemma 4 31B · Google AI Studio");
    expect(view).not.toContain("Retry");
    expect(view).not.toContain("Close turn");
  });

  test("explains the stop affordance with compact live runtime truth", () => {
    const activeThread: MobileConversationThread = {
      ...thread,
      messageCount: 1,
      lastMessageAt: now,
      messages: [{
        messageRef: "message.sarah.runtime-question",
        threadRef: thread.threadRef,
        body: "What powers your responses?",
        createdAt: now,
        updatedAt: now,
        version: 1,
      }],
      timeline: {
        status: { phase: "live", cursor: 2, pendingMutationCount: 0 },
        run: {
          runRef: "run.sarah.gemma",
          routeRef: thread.threadRef,
          runtime: "openagents_native",
          backend: "hosted",
          status: "running",
          createdAt: now,
          updatedAt: now,
          startedAt: now,
          completedAt: null,
          failedAt: null,
          canceledAt: null,
          version: 1,
        },
        events: [{
          eventRef: "event.sarah.gemma.started",
          runRef: "run.sarah.gemma",
          sequence: 0,
          eventType: "turn.started",
          summary: "Turn started",
          status: "running",
          artifactRefs: [],
          item: { kind: "connected", turnRef: "run.sarah.gemma", lane: "hosted_khala" },
          source: {
            lane: "hosted_khala",
            adapterKind: "openagents_native",
            surface: "server",
            providerRef: "google-ai-studio",
            modelRef: "gemma-4-31b-it",
          },
          createdAt: now,
          version: 2,
        }],
      },
    };
    const host: MobileConversationHost = {
      listThreads: async () => [activeThread],
      newThread: async () => ({ ok: true, thread: activeThread }),
      openThread: async () => activeThread,
      sendMessage: async () => ({ ok: true, thread: activeThread }),
    };
    const program = buildHomeProgram({
      sarah: principal,
      conversation: {
        mode: "sync",
        host,
        threads: [activeThread],
        archivedThreads: [],
        activeThread,
      },
    });
    const view = JSON.stringify(renderContentView({
      ...program.initialState,
      khala: {
        ...program.initialState.khala,
        runtimeControlActionsAvailable: true,
      },
    }));

    expect(view).toContain("Sarah is thinking…");
    expect(view).toContain("Generating with Gemma 4 31B · Google AI Studio");
    expect(view).not.toContain("OpenAgents · hosted runtime · 1 activity");
    expect(view).not.toContain('"scrollToKey":"work:run.sarah.gemma"');
    expect(view).toContain('"onStop"');
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
      listThreads: async () => [oldThread, recentThread],
      newThread: async () => ({ ok: true, thread: oldThread }),
      openThread: async (threadRef) => threadRef === recentThread.threadRef ? recentThread : oldThread,
      sendMessage: async (input) => {
        runtimeTarget = input.runtimeTarget;
        return { ok: true, thread: recentThread };
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
    expect(selected.khala.transcriptPinned).toBe(true);
    expect(selected.khala.transcriptScrollToKey).toBe("message.sarah.2");

    await Effect.runPromise(program.report(
      IntentRef("KhalaTurnSubmitted", ComponentValueBinding()),
      "What is the release status?",
    ) as Effect.Effect<unknown>);
    expect(runtimeTarget).toEqual({ lane: "hosted_khala" });
  });
});
