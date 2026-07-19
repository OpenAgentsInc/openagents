import { Effect, SubscriptionRef } from "@effect-native/core/effect";
import { describe, expect, test } from "vite-plus/test";

import type { DesktopWorkspaceDocument } from "../../workspace-contract.ts";
import {
  IdeCursorDecisionReceiptSchema,
  IdeCursorSnapshotSchema,
  emptyIdeCursorSnapshot,
  type IdeCursorCommand,
  type IdeCursorCommandResult,
  type IdeCursorRequest,
} from "../../ide/cursor-contract.ts";
import { ideCursorFixtureCandidate, ideCursorFixtureDisclosure } from "../../ide/cursor-fixture.ts";
import { emptyIdeAgentCodeSnapshot } from "../../ide/agent-code-contract.ts";
import {
  IdeDocumentGeneration,
  IdeDocumentSequence,
  IdeMonacoModelVersion,
} from "../../ide/monaco-document-contract.ts";
import {
  IdeAttachmentGenerationSchema,
  IdeProjectRefSchema,
  IdeTimestampSchema,
} from "../../ide/project-contract.ts";
import { assembleActiveFileAgentManifest } from "./agent-code.ts";
import { initialDesktopShellState, type DesktopShellState } from "../shell.ts";
import {
  emptyWorkspaceEditorState,
  withWorkspaceEditorOpened,
  withWorkspaceEditorOpening,
} from "../workspace-editor.ts";
import {
  IdeCursorRendererStateSchema,
  buildIdeCursorProviderInput,
  emptyIdeCursorRendererState,
  invalidateIdeCursorRendererState,
  makeIdeCursorRendererHandlers,
  type IdeCursorRendererHost,
} from "./cursor.ts";

const document = (overrides: Partial<DesktopWorkspaceDocument> = {}): DesktopWorkspaceDocument => ({
  grantRef: "workspace.grant.cursor",
  pathRef: "src/index.ts",
  content: "const answer = 41\n",
  revisionRef: "workspace.document.cursor.1",
  languageMode: "typescript",
  encoding: "utf-8",
  lineEnding: "lf",
  sizeBytes: 18,
  ...overrides,
});

const sourceState = async (): Promise<DesktopShellState> => {
  const doc = document();
  const editor = withWorkspaceEditorOpened(
    withWorkspaceEditorOpening(emptyWorkspaceEditorState(), doc.pathRef, doc.grantRef),
    doc.pathRef,
    { state: "available", document: doc },
  );
  const base = initialDesktopShellState("fixture", "2026-07-19T12:00:00.000Z", "files");
  const withFile: DesktopShellState = {
    ...base,
    workspaceBrowser: { ...base.workspaceBrowser, grantRef: doc.grantRef },
    workspaceEditor: editor,
    composerFileContext: {
      path: doc.pathRef,
      revisionRef: doc.revisionRef,
      languageMode: doc.languageMode,
      content: doc.content,
      dirty: false,
    },
  };
  const assembled = await assembleActiveFileAgentManifest(withFile, "2026-07-19T12:00:01.000Z");
  if (assembled === null) throw new Error("fixture context did not assemble");
  return {
    ...withFile,
    agentCode: {
      ...emptyIdeAgentCodeSnapshot(),
      attachment: assembled.attachment,
      manifests: [assembled.manifest],
      lifecycle: "attached",
      revision: 2,
    },
  };
};

const readyInput = async () => {
  const state = await sourceState();
  const built = buildIdeCursorProviderInput(
    state,
    { _tag: "Complete", acceptance: "all" },
    "2026-07-19T12:00:02.000Z",
  );
  if (built._tag !== "Ready") throw new Error(built.reason);
  return { state, input: built.input };
};

describe("IDE-09 renderer cursor integration", () => {
  test("joins IDE-08 authority to distinct Monaco identity and versions", async () => {
    const { state, input } = await readyInput();
    const tab = state.workspaceEditor.tabs[0];
    if (tab === undefined) throw new Error("fixture editor tab missing");
    expect(input.request.anchor.sourceDocumentRef).toBe(tab.documentRef);
    expect(input.request.anchor.sourceDocumentGeneration).toBe(tab.generation);
    expect(input.request.anchor.documentRef).not.toBe(tab.documentRef);
    expect(input.request.anchor.documentSequence).toBe(0);
    expect(input.request.anchor.modelVersion).toBe(1);
    expect(input.request.anchor.selectionVersion).toBe(0);
    expect(input.documentText).toBe(document().content);
    expect(input.context.length).toBeGreaterThan(0);
    expect(input.request.identity.effective.provider.value).toBe(
      state.agentCode.manifests[0]?.effectiveRuntime.providerRef,
    );
  });

  test("invalidates every project, attachment, document-version, selection, and content fence", async () => {
    const { state, input } = await readyInput();
    const active = IdeCursorRendererStateSchema.make({
      ...emptyIdeCursorRendererState(),
      activeRequest: input.request,
      snapshot: IdeCursorSnapshotSchema.make({
        ...emptyIdeCursorSnapshot(),
        latestSequence: input.request.sequence,
        activeRequestRef: input.request.requestRef,
        activeAttemptRef: input.request.attemptRef,
        state: "running",
      }),
    });
    const attachment = state.agentCode.attachment;
    if (attachment === null) throw new Error("fixture attachment missing");
    const invalidate = (next: DesktopShellState) =>
      invalidateIdeCursorRendererState(active, { ...next, ideCursor: active }).invalidation?._tag;
    expect(
      invalidate({
        ...state,
        agentCode: {
          ...state.agentCode,
          attachment: {
            ...attachment,
            projectRef: IdeProjectRefSchema.make(`${attachment.projectRef}.changed`),
          },
        },
      }),
    ).toBe("Project");
    expect(
      invalidate({
        ...state,
        agentCode: {
          ...state.agentCode,
          attachment: {
            ...attachment,
            attachmentGeneration: IdeAttachmentGenerationSchema.make(
              attachment.attachmentGeneration + 1,
            ),
          },
        },
      }),
    ).toBe("Attachment");
    expect(
      invalidate({
        ...state,
        workspaceEditor: {
          ...state.workspaceEditor,
          tabs: state.workspaceEditor.tabs.map((tab) => ({
            ...tab,
            generation: IdeDocumentGeneration.make((tab.generation ?? 0) + 1),
          })),
        },
      }),
    ).toBe("Generation");
    expect(
      invalidate({
        ...state,
        workspaceEditor: {
          ...state.workspaceEditor,
          tabs: state.workspaceEditor.tabs.map((tab) => ({
            ...tab,
            modelVersion: IdeMonacoModelVersion.make((tab.modelVersion ?? 1) + 1),
          })),
        },
      }),
    ).toBe("Model");
    expect(
      invalidate({
        ...state,
        workspaceEditor: {
          ...state.workspaceEditor,
          tabs: state.workspaceEditor.tabs.map((tab) => ({
            ...tab,
            incrementalSequence: IdeDocumentSequence.make((tab.incrementalSequence ?? 0) + 1),
          })),
        },
      }),
    ).toBe("Sequence");
    expect(
      invalidate({
        ...state,
        workspaceEditor: {
          ...state.workspaceEditor,
          tabs: state.workspaceEditor.tabs.map((tab) => ({
            ...tab,
            selection: { start: 1, end: 1 },
            selectionVersion: tab.selectionVersion + 1,
          })),
        },
      }),
    ).toBe("Selection");
    expect(
      invalidate({
        ...state,
        workspaceEditor: {
          ...state.workspaceEditor,
          tabs: state.workspaceEditor.tabs.map((tab) => ({
            ...tab,
            draft: `${tab.draft}// changed`,
          })),
        },
      }),
    ).toBe("Content");
  });

  test("does not mutate before ack and re-opens the canonical post-image into undo history", async () => {
    const initial = await sourceState();
    let acceptedCandidate = ideCursorFixtureCandidate();
    let opened = 0;
    const host: IdeCursorRendererHost = {
      snapshot: async () => emptyIdeCursorSnapshot(),
      command: async (command: IdeCursorCommand): Promise<IdeCursorCommandResult> => {
        if (command._tag === "Start") {
          acceptedCandidate = ideCursorFixtureCandidate(command.input.request, {
            anchor: command.input.request.anchor,
            replace: command.input.request.anchor.selection,
            text: "const answer = 42\n",
          });
          return {
            _tag: "Succeeded",
            snapshot: IdeCursorSnapshotSchema.make({
              ...emptyIdeCursorSnapshot(),
              latestSequence: command.input.request.sequence,
              activeRequestRef: command.input.request.requestRef,
              activeAttemptRef: command.input.request.attemptRef,
              candidates: [acceptedCandidate],
              finalDisclosure: ideCursorFixtureDisclosure(),
              state: "complete",
            }),
          };
        }
        if (command._tag !== "Decide" || command.decision._tag !== "Accept")
          throw new Error("unexpected command");
        const receipt = IdeCursorDecisionReceiptSchema.make({
          schemaVersion: "openagents.ide-cursor.v1",
          decision: command.decision,
          recordedAt: IdeTimestampSchema.make("2026-07-19T12:00:03.000Z"),
          previousContentDigest: acceptedCandidate.anchor.contentDigest,
          resultContentDigest: acceptedCandidate.resultDigest,
          proposalRef: null,
          proposalSubmitted: false,
          applied: true,
          staleRejected: false,
        });
        return {
          _tag: "Succeeded",
          snapshot: IdeCursorSnapshotSchema.make({
            ...emptyIdeCursorSnapshot(),
            latestSequence: command.decision.sequence,
            activeRequestRef: command.decision.requestRef,
            activeAttemptRef: acceptedCandidate.attemptRef,
            candidates: [acceptedCandidate],
            decisions: [command.decision],
            receipts: [receipt],
            state: "complete",
          }),
        };
      },
    };
    const state = await Effect.runPromise(SubscriptionRef.make(initial));
    const handlers = makeIdeCursorRendererHandlers(state, host, {
      openWorkspaceDocument: async () => {
        opened += 1;
        return {
          state: "available",
          document: document({
            content: "const answer = 42\n",
            revisionRef: "workspace.document.cursor.2",
          }),
        };
      },
      saveWorkspaceDocument: async () => null,
      saveWorkspaceDocumentAs: async () => null,
    });
    await Effect.runPromise(handlers.IdeCursorCompletionRequested("all"));
    const beforeAccept = await Effect.runPromise(SubscriptionRef.get(state));
    expect(beforeAccept.workspaceEditor.tabs[0]?.draft).toBe(document().content);
    expect(opened).toBe(0);
    await Effect.runPromise(
      handlers.IdeCursorDecisionRequested({
        action: "accept_all",
        candidateRef: acceptedCandidate.candidateRef,
      }),
    );
    const afterAccept = await Effect.runPromise(SubscriptionRef.get(state));
    expect(opened).toBe(1);
    expect(afterAccept.workspaceEditor.tabs[0]?.draft).toBe("const answer = 42\n");
    expect(afterAccept.workspaceEditor.tabs[0]?.undo.at(-1)).toBe(document().content);
    expect(afterAccept.workspaceEditor.tabs[0]?.modelVersion).toBe(2);
    expect(afterAccept.ideCursor.activeRequest).toBeNull();
  });

  test("reconciles main's asynchronous first candidate without requiring a manual refresh", async () => {
    const initial = await sourceState();
    let request: IdeCursorRequest | null = null;
    let snapshots = 0;
    const host: IdeCursorRendererHost = {
      command: async (command) => {
        if (command._tag !== "Start") throw new Error("unexpected command");
        request = command.input.request;
        return {
          _tag: "Succeeded",
          snapshot: IdeCursorSnapshotSchema.make({
            ...emptyIdeCursorSnapshot(),
            latestSequence: command.input.request.sequence,
            activeRequestRef: command.input.request.requestRef,
            activeAttemptRef: command.input.request.attemptRef,
            state: "running",
          }),
        };
      },
      snapshot: async () => {
        snapshots += 1;
        if (request === null) return emptyIdeCursorSnapshot();
        const candidate = ideCursorFixtureCandidate(request);
        return IdeCursorSnapshotSchema.make({
          ...emptyIdeCursorSnapshot(),
          latestSequence: request.sequence,
          activeRequestRef: request.requestRef,
          activeAttemptRef: request.attemptRef,
          candidates: snapshots > 1 ? [candidate] : [],
          state: "running",
        });
      },
    };
    const state = await Effect.runPromise(SubscriptionRef.make(initial));
    const handlers = makeIdeCursorRendererHandlers(state, host, {
      openWorkspaceDocument: async () => null,
      saveWorkspaceDocument: async () => null,
      saveWorkspaceDocumentAs: async () => null,
    });
    await Effect.runPromise(handlers.IdeCursorCompletionRequested("all"));
    const current = await Effect.runPromise(SubscriptionRef.get(state));
    expect(snapshots).toBeGreaterThan(1);
    expect(current.ideCursor.snapshot.candidates).toHaveLength(1);
    expect(current.ideCursor.snapshot.candidates[0]?._tag).toBe("Completion");
  });

  test("keeps editor bytes unchanged when the host refuses a decision", async () => {
    const initial = await sourceState();
    const state = await Effect.runPromise(SubscriptionRef.make(initial));
    const candidate = ideCursorFixtureCandidate();
    await Effect.runPromise(
      SubscriptionRef.update(state, (current) => ({
        ...current,
        ideCursor: IdeCursorRendererStateSchema.make({
          ...current.ideCursor,
          snapshot: IdeCursorSnapshotSchema.make({
            ...emptyIdeCursorSnapshot(),
            latestSequence: 1,
            candidates: [candidate],
            state: "complete",
          }),
        }),
      })),
    );
    let opened = 0;
    const handlers = makeIdeCursorRendererHandlers(
      state,
      {
        snapshot: async () => emptyIdeCursorSnapshot(),
        command: async () => ({
          _tag: "Refused",
          reason: "authority_stale",
          message: "stale",
          snapshot: IdeCursorSnapshotSchema.make({
            ...emptyIdeCursorSnapshot(),
            latestSequence: 1,
            candidates: [candidate],
            state: "failed",
          }),
        }),
      },
      {
        openWorkspaceDocument: async () => {
          opened += 1;
          return null;
        },
        saveWorkspaceDocument: async () => null,
        saveWorkspaceDocumentAs: async () => null,
      },
    );
    await Effect.runPromise(
      handlers.IdeCursorDecisionRequested({
        action: "accept_all",
        candidateRef: candidate.candidateRef,
      }),
    );
    const current = await Effect.runPromise(SubscriptionRef.get(state));
    expect(opened).toBe(0);
    expect(current.workspaceEditor.tabs[0]?.draft).toBe(document().content);
    expect(current.ideCursor.notice).toBe("stale");
  });
});
