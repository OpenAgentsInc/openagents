import { sha256 } from "@noble/hashes/sha256";
import { Effect, Exit, Schema, SubscriptionRef } from "@effect-native/core/effect";
import { defineIntent } from "@effect-native/core";

import {
  IdeCursorAttemptRefSchema,
  IdeCursorCommandResultSchema,
  IdeCursorContextRefSchema,
  IdeCursorDecisionRefSchema,
  IdeCursorDecisionSchema,
  IdeCursorProviderInputSchema,
  IdeCursorRequestRefSchema,
  IdeCursorSelectionVersionSchema,
  IdeCursorSequenceSchema,
  IdeCursorSnapshotSchema,
  emptyIdeCursorSnapshot,
  type IdeCursorAnchor,
  type IdeCursorCandidate,
  type IdeCursorCandidateRef,
  type IdeCursorCommand,
  type IdeCursorCommandResult,
  type IdeCursorDecision,
  type IdeCursorIntentSchema,
  type IdeCursorProviderInput,
  type IdeCursorRequest,
  type IdeCursorSnapshot,
} from "../../ide/cursor-contract.ts";
import type { IdeAgentCodeSnapshot, IdeAgentContextItem } from "../../ide/agent-code-contract.ts";
import { IdeAttachmentRefSchema, IdeTimestampSchema } from "../../ide/project-contract.ts";
import { IdeDocumentSequence, IdeMonacoModelVersion } from "../../ide/monaco-document-contract.ts";
import {
  decodeWorkspaceDocumentResult,
  type DesktopWorkspaceDocumentResult,
} from "../../workspace-contract.ts";
import {
  withWorkspaceEditorCursorReconciled,
  type WorkspaceDocumentBridge,
  type WorkspaceEditorState,
} from "../workspace-editor.ts";
import {
  loadAgentCodeRendererSnapshot,
  unavailableIdeAgentCodeRendererHost,
  type IdeAgentCodeRendererHost,
} from "./agent-code.ts";

const boundedNotice = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000));
const prompt = Schema.String.check(Schema.isMaxLength(8_000));

export const IdeCursorRendererInvalidationSchema = Schema.TaggedUnion({
  Project: { detail: boundedNotice },
  Attachment: { detail: boundedNotice },
  Document: { detail: boundedNotice },
  Generation: { detail: boundedNotice },
  Model: { detail: boundedNotice },
  Sequence: { detail: boundedNotice },
  Selection: { detail: boundedNotice },
  Content: { detail: boundedNotice },
}).annotate({ identifier: "IdeCursorRendererInvalidation" });
export type IdeCursorRendererInvalidation = typeof IdeCursorRendererInvalidationSchema.Type;

export const IdeCursorRendererStateSchema = Schema.Struct({
  snapshot: IdeCursorSnapshotSchema,
  activeRequest: Schema.NullOr(IdeCursorProviderInputSchema.fields.request),
  selectedCandidateRef: Schema.NullOr(IdeCursorDecisionSchema.cases.Accept.fields.candidateRef),
  prompt,
  notice: Schema.NullOr(boundedNotice),
  invalidation: Schema.NullOr(IdeCursorRendererInvalidationSchema),
}).annotate({ identifier: "IdeCursorRendererState" });
export interface IdeCursorRendererState extends Schema.Schema.Type<
  typeof IdeCursorRendererStateSchema
> {}

export const emptyIdeCursorRendererState = (): IdeCursorRendererState =>
  IdeCursorRendererStateSchema.make({
    snapshot: emptyIdeCursorSnapshot(),
    activeRequest: null,
    selectedCandidateRef: null,
    prompt: "",
    notice: null,
    invalidation: null,
  });

export type IdeCursorRendererHost = Readonly<{
  snapshot: () => Promise<unknown>;
  command: (command: IdeCursorCommand) => Promise<unknown>;
}>;

export const unavailableIdeCursorRendererHost: IdeCursorRendererHost = {
  snapshot: async () => emptyIdeCursorSnapshot(),
  command: async () =>
    IdeCursorCommandResultSchema.cases.Refused.make({
      reason: "unavailable",
      message: "AI editing is unavailable.",
      snapshot: emptyIdeCursorSnapshot(),
    }),
};

const decode = <S extends Schema.ConstraintDecoder<unknown, never>>(
  schema: S,
  value: unknown,
): S["Type"] | null => {
  const result = Schema.decodeUnknownExit(schema)(value);
  return Exit.isSuccess(result) ? result.value : null;
};

export const loadIdeCursorRendererSnapshot = async (
  host: IdeCursorRendererHost,
): Promise<IdeCursorSnapshot> =>
  decode(IdeCursorSnapshotSchema, await host.snapshot().catch(() => null)) ??
  IdeCursorSnapshotSchema.make({ ...emptyIdeCursorSnapshot(), state: "stopped" });

export const executeIdeCursorRendererCommand = async (
  host: IdeCursorRendererHost,
  command: IdeCursorCommand,
): Promise<IdeCursorCommandResult> =>
  decode(IdeCursorCommandResultSchema, await host.command(command).catch(() => null)) ??
  IdeCursorCommandResultSchema.cases.Refused.make({
    reason: "unavailable",
    message: "The AI-editing host returned no schema-valid result.",
    snapshot: IdeCursorSnapshotSchema.make({ ...emptyIdeCursorSnapshot(), state: "stopped" }),
  });

const waitForIdeCursorFirstCandidate = async (
  host: IdeCursorRendererHost,
  request: IdeCursorProviderInput["request"],
): Promise<IdeCursorSnapshot> => {
  const deadline = Date.now() + Math.min(30_000, request.budget.maxLatencyMs);
  let latest = await loadIdeCursorRendererSnapshot(host);
  while (
    Date.now() < deadline &&
    latest.activeRequestRef === request.requestRef &&
    latest.state === "running" &&
    !latest.candidates.some((candidate) => candidate.requestRef === request.requestRef)
  ) {
    await new Promise((resolve) => setTimeout(resolve, 16));
    latest = await loadIdeCursorRendererSnapshot(host);
  }
  return latest;
};

const hex = (bytes: Uint8Array): string =>
  [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");

const digest = (value: string): `sha256:${string}` =>
  `sha256:${hex(sha256(new TextEncoder().encode(value)))}`;

const suffix = (value: string): string =>
  digest(value).slice("sha256:".length, "sha256:".length + 32);

const offsetPosition = (
  content: string,
  offset: number,
): Readonly<{ line: number; column: number }> => {
  const bounded = Math.max(0, Math.min(content.length, Math.trunc(offset)));
  let line = 1;
  let lineStart = 0;
  for (let index = 0; index < bounded; index += 1) {
    if (content.charCodeAt(index) !== 10) continue;
    line += 1;
    lineStart = index + 1;
  }
  return { line, column: bounded - lineStart + 1 };
};

const contextSource = (
  item: IdeAgentContextItem,
): "selection" | "diagnostic" | "symbol" | "history" | "workspace" => {
  switch (item.source._tag) {
    case "Range":
      return "selection";
    case "Diagnostic":
      return "diagnostic";
    case "Symbol":
      return "symbol";
    case "RecentEdit":
      return "history";
    default:
      return "workspace";
  }
};

const currentManifest = (agentCode: IdeAgentCodeSnapshot) => {
  const attachment = agentCode.attachment;
  if (attachment === null) return null;
  return (
    [...agentCode.manifests]
      .reverse()
      .find(
        (manifest) =>
          manifest.attachment.agentAttachmentRef === attachment.agentAttachmentRef &&
          manifest.attachment.attachmentGeneration === attachment.attachmentGeneration,
      ) ?? null
  );
};

const authorityFileItem = (agentCode: IdeAgentCodeSnapshot, pathRef: string) =>
  currentManifest(agentCode)?.items.find(
    (item) =>
      item.source._tag === "File" &&
      item.source.pathRef === pathRef &&
      item.disposition._tag === "Included" &&
      item.freshness === "current",
  ) ?? null;

export type IdeCursorRendererSourceState = Readonly<{
  ideCursor: IdeCursorRendererState;
  agentCode: IdeAgentCodeSnapshot;
  workspaceEditor: WorkspaceEditorState;
  workspaceBrowser: Readonly<{ grantRef: string | null }>;
}>;

const anchorFromState = (state: IdeCursorRendererSourceState): IdeCursorAnchor | null => {
  const attachment = state.agentCode.attachment;
  const manifest = currentManifest(state.agentCode);
  const pathRef = state.workspaceEditor.activePathRef;
  if (pathRef === null) return null;
  const tab = state.workspaceEditor.tabs.find((candidate) => candidate.pathRef === pathRef) ?? null;
  if (
    attachment === null ||
    manifest === null ||
    tab === null ||
    tab.phase !== "ready" ||
    tab.document === null ||
    tab.documentRef === undefined ||
    tab.generation === undefined
  )
    return null;
  const file = authorityFileItem(state.agentCode, pathRef);
  if (file === null || file.source._tag !== "File") return null;
  const selection = {
    start: offsetPosition(tab.draft, tab.selection.start),
    end: offsetPosition(tab.draft, tab.selection.end),
  };
  return {
    projectRef: attachment.projectRef,
    rootRef: attachment.rootRef,
    worktreeRef: attachment.worktreeRef,
    attachmentRef: IdeAttachmentRefSchema.make(
      `ide.attachment.cursor.${suffix(attachment.agentAttachmentRef)}`,
    ),
    attachmentGeneration: attachment.attachmentGeneration,
    sessionRef: attachment.sessionRef,
    sourceDocumentRef: tab.documentRef,
    sourceDocumentGeneration: tab.generation,
    fileRef: file.source.fileRef,
    documentRef: file.source.documentRef,
    documentGeneration: file.source.documentGeneration,
    documentSequence: tab.incrementalSequence ?? IdeDocumentSequence.make(0),
    modelVersion: tab.modelVersion ?? IdeMonacoModelVersion.make(1),
    selectionVersion: IdeCursorSelectionVersionSchema.make(tab.selectionVersion),
    pathRef,
    selection,
    contentDigest: digest(tab.draft),
  };
};

const sameRange = (
  left: IdeCursorAnchor["selection"],
  right: IdeCursorAnchor["selection"],
): boolean =>
  left.start.line === right.start.line &&
  left.start.column === right.start.column &&
  left.end.line === right.end.line &&
  left.end.column === right.end.column;

const invalidationFor = (
  previous: IdeCursorAnchor,
  current: IdeCursorAnchor | null,
): IdeCursorRendererInvalidation | null => {
  if (current === null)
    return IdeCursorRendererInvalidationSchema.cases.Document.make({
      detail: "The active authoritative document is unavailable.",
    });
  if (
    previous.projectRef !== current.projectRef ||
    previous.rootRef !== current.rootRef ||
    previous.worktreeRef !== current.worktreeRef ||
    previous.sessionRef !== current.sessionRef
  ) {
    return IdeCursorRendererInvalidationSchema.cases.Project.make({
      detail: "The project, worktree, or session changed.",
    });
  }
  if (
    previous.attachmentRef !== current.attachmentRef ||
    previous.attachmentGeneration !== current.attachmentGeneration
  ) {
    return IdeCursorRendererInvalidationSchema.cases.Attachment.make({
      detail: "The IDE-08 attachment changed.",
    });
  }
  if (
    previous.pathRef !== current.pathRef ||
    previous.sourceDocumentRef !== current.sourceDocumentRef ||
    previous.fileRef !== current.fileRef ||
    previous.documentRef !== current.documentRef
  ) {
    return IdeCursorRendererInvalidationSchema.cases.Document.make({
      detail: "The source or authoritative document changed.",
    });
  }
  if (
    previous.sourceDocumentGeneration !== current.sourceDocumentGeneration ||
    previous.documentGeneration !== current.documentGeneration
  ) {
    return IdeCursorRendererInvalidationSchema.cases.Generation.make({
      detail: "The source or authoritative document generation changed.",
    });
  }
  if (previous.modelVersion !== current.modelVersion)
    return IdeCursorRendererInvalidationSchema.cases.Model.make({
      detail: "The Monaco model version changed.",
    });
  if (previous.documentSequence !== current.documentSequence)
    return IdeCursorRendererInvalidationSchema.cases.Sequence.make({
      detail: "The document edit sequence changed.",
    });
  if (
    previous.selectionVersion !== current.selectionVersion ||
    !sameRange(previous.selection, current.selection)
  ) {
    return IdeCursorRendererInvalidationSchema.cases.Selection.make({
      detail: "The editor selection changed.",
    });
  }
  if (previous.contentDigest !== current.contentDigest)
    return IdeCursorRendererInvalidationSchema.cases.Content.make({
      detail: "The document content changed.",
    });
  return null;
};

export const invalidateIdeCursorRendererState = (
  cursor: IdeCursorRendererState,
  state: IdeCursorRendererSourceState,
): IdeCursorRendererState => {
  const request = cursor.activeRequest;
  if (request === null) return cursor;
  const attachment = state.agentCode.attachment;
  const invalidation =
    attachment === null
      ? IdeCursorRendererInvalidationSchema.cases.Attachment.make({
          detail: "The IDE-08 attachment is unavailable.",
        })
      : request.anchor.projectRef !== attachment.projectRef ||
          request.anchor.rootRef !== attachment.rootRef ||
          request.anchor.worktreeRef !== attachment.worktreeRef ||
          request.anchor.sessionRef !== attachment.sessionRef
        ? IdeCursorRendererInvalidationSchema.cases.Project.make({
            detail: "The project, worktree, or session changed.",
          })
        : request.anchor.attachmentGeneration !== attachment.attachmentGeneration ||
            request.anchor.attachmentRef !==
              IdeAttachmentRefSchema.make(
                `ide.attachment.cursor.${suffix(attachment.agentAttachmentRef)}`,
              )
          ? IdeCursorRendererInvalidationSchema.cases.Attachment.make({
              detail: "The IDE-08 attachment changed.",
            })
          : invalidationFor(request.anchor, anchorFromState(state));
  return invalidation === null
    ? cursor
    : IdeCursorRendererStateSchema.make({
        ...cursor,
        activeRequest: null,
        selectedCandidateRef: null,
        invalidation,
        notice: invalidation.detail,
      });
};

export const IdeCursorBuildResultSchema = Schema.TaggedUnion({
  Ready: { input: IdeCursorProviderInputSchema },
  Unavailable: { reason: boundedNotice },
}).annotate({ identifier: "IdeCursorBuildResult" });
export type IdeCursorBuildResult = typeof IdeCursorBuildResultSchema.Type;

export const buildIdeCursorProviderInput = (
  state: IdeCursorRendererSourceState,
  intent: typeof IdeCursorIntentSchema.Type,
  requestedAt: string,
): IdeCursorBuildResult => {
  const anchor = anchorFromState(state);
  const manifest = currentManifest(state.agentCode);
  if (anchor === null || manifest === null)
    return IdeCursorBuildResultSchema.cases.Unavailable.make({
      reason:
        "Attach the current file through the IDE-08 context tray before requesting AI editing.",
    });
  const sequence = IdeCursorSequenceSchema.make(state.ideCursor.snapshot.latestSequence + 1);
  const key = `${anchor.projectRef}:${anchor.sourceDocumentRef}:${anchor.documentSequence}:${anchor.modelVersion}:${anchor.selectionVersion}:${sequence}`;
  const evidence = {
    _tag: "Observed" as const,
    evidenceRef: manifest.manifestRef,
    observedAt: manifest.createdAt,
  };
  const runtime = manifest.effectiveRuntime;
  const execution = {
    harness: { value: runtime.harnessRef, evidence },
    provider: { value: runtime.providerRef, evidence },
    model: { value: runtime.modelRef, evidence },
    account: { value: runtime.accountRef, evidence },
    placementRef: runtime.placementRef,
    placementGeneration: runtime.placementGeneration,
    indexPosture:
      runtime.semanticRetrieval === "disabled"
        ? ("disabled" as const)
        : runtime.semanticRetrieval === "local"
          ? ("local" as const)
          : ("remote" as const),
    // Provider execution and semantic indexing are independent dimensions.
    // A Fable/Codex/ACP model call is networked even when optional semantic
    // retrieval is disabled; collapsing these would falsely reject the
    // remote-embeddings-disabled mode required by IDE-09.
    networkPosture: "networked" as const,
  };
  const context = manifest.items
    .flatMap((item) => {
      if (
        item.disposition._tag !== "Included" ||
        item.excerpt === null ||
        item.sensitivity === "private" ||
        item.sensitivity === "secret"
      )
        return [];
      const contextKey = `${item.contextItemRef}:${item.source.sourceGeneration}`;
      return [
        {
          contextRef: IdeCursorContextRefSchema.make(
            `ide.cursor-context.item.${suffix(contextKey)}`,
          ),
          source: contextSource(item),
          text: item.excerpt,
          contentDigest: digest(item.excerpt),
          freshness: item.freshness === "current" ? ("current" as const) : ("stale" as const),
          sensitivity: item.sensitivity === "public" ? ("public" as const) : ("workspace" as const),
        },
      ];
    })
    .slice(0, 128);
  const proposalBases = manifest.items.flatMap((item) => {
    if (item.source._tag !== "File" || item.disposition._tag !== "Included" ||
      item.excerpt === null || item.truncated) return [];
    const source = item.source;
    const editorDocument = state.workspaceEditor.tabs.find(
      (tab) => tab.pathRef === source.pathRef,
    )?.document;
    const content = item.excerpt;
    const withoutCrlf = content.replaceAll("\r\n", "");
    const hasCrlf = content.includes("\r\n");
    const hasLf = withoutCrlf.includes("\n");
    return [{
      fileRef: source.fileRef,
      pathRef: source.pathRef,
      base: {
        existed: true,
        content,
        diskRevisionRef: source.diskRevisionRef,
        documentRef: source.documentRef,
        documentGeneration: source.documentGeneration,
        gitSnapshotRef: null,
        gitSnapshotGeneration: null,
        checkpointRef: null,
        contentDigest: digest(content),
        encoding: editorDocument?.encoding ?? "utf-8" as const,
        lineEnding: hasCrlf && hasLf ? "mixed" as const
          : hasCrlf ? "crlf" as const
            : hasLf ? "lf" as const
              : "none" as const,
        mode: "regular" as const,
      },
    }];
  }).slice(0, 128);
  if (proposalBases.length === 0)
    return IdeCursorBuildResultSchema.cases.Unavailable.make({
      reason: "The current IDE-08 manifest has no complete version-bound file preimage.",
    });
  const request: IdeCursorRequest = {
    schemaVersion: "openagents.ide-cursor.v1",
    requestRef: IdeCursorRequestRefSchema.make(`ide.cursor-request.renderer.${suffix(key)}`),
    attemptRef: IdeCursorAttemptRefSchema.make(
      `ide.cursor-attempt.renderer.${suffix(`${key}:attempt`)}`,
    ),
    sequence,
    requestedAt: IdeTimestampSchema.make(requestedAt),
    anchor,
    intent,
    identity: {
      requested: execution,
      admitted: execution,
      effective: execution,
      substitution: { _tag: "None" },
    },
    contextRef: IdeCursorContextRefSchema.make(
      `ide.cursor-context.renderer.${suffix(`${key}:context`)}`,
    ),
    contextDigest: digest(JSON.stringify(context)),
    budget: { maxLatencyMs: 30_000, maxInputTokens: 50_000, maxOutputTokens: 8_000 },
  };
  return IdeCursorBuildResultSchema.cases.Ready.make({
    input: IdeCursorProviderInputSchema.make({
      request,
      proposalContext: {
        attachment: manifest.attachment,
        manifestRef: manifest.manifestRef,
        turnRef: manifest.turnRef,
        conversationThreadRef: manifest.conversationThreadRef,
        bases: proposalBases,
      },
      documentText:
        state.workspaceEditor.tabs.find((tab) => tab.pathRef === anchor.pathRef)?.draft ?? "",
      context,
    }),
  });
};

export const IdeCursorCompletionRequested = defineIntent(
  "IdeCursorCompletionRequested",
  Schema.Literals(["word", "line", "all"]),
);
export const IdeCursorNextEditRequested = defineIntent("IdeCursorNextEditRequested", Schema.Null);
export const IdeCursorPromptChanged = defineIntent("IdeCursorPromptChanged", prompt);
export const IdeCursorAskRequested = defineIntent("IdeCursorAskRequested", Schema.Null);
export const IdeCursorEditRequested = defineIntent("IdeCursorEditRequested", Schema.Null);
export const IdeCursorGenerateRequested = defineIntent("IdeCursorGenerateRequested", Schema.Null);
export const IdeCursorCandidateSelected = defineIntent(
  "IdeCursorCandidateSelected",
  Schema.NullOr(IdeCursorDecisionSchema.cases.Accept.fields.candidateRef),
);
export const IdeCursorDecisionRequested = defineIntent(
  "IdeCursorDecisionRequested",
  Schema.Struct({
    action: Schema.Literals([
      "accept_word",
      "accept_line",
      "accept_all",
      "reject",
      "compare",
      "retry",
      "undo",
      "cancel",
    ]),
    candidateRef: Schema.NullOr(IdeCursorDecisionSchema.cases.Accept.fields.candidateRef),
  }),
);
export const IdeCursorRefreshed = defineIntent("IdeCursorRefreshed", Schema.Null);

export const ideCursorRendererIntents = [
  IdeCursorCompletionRequested,
  IdeCursorNextEditRequested,
  IdeCursorPromptChanged,
  IdeCursorAskRequested,
  IdeCursorEditRequested,
  IdeCursorGenerateRequested,
  IdeCursorCandidateSelected,
  IdeCursorDecisionRequested,
  IdeCursorRefreshed,
] as const;

export type IdeCursorRendererCapableState = IdeCursorRendererSourceState;

const candidateFor = (
  cursor: IdeCursorRendererState,
  candidateRef: string | null,
): IdeCursorCandidate | null =>
  candidateRef === null
    ? null
    : (cursor.snapshot.candidates.find((candidate) => candidate.candidateRef === candidateRef) ??
      null);

const decisionFor = (
  cursor: IdeCursorRendererState,
  action: "accept_word" | "accept_line" | "accept_all" | "reject" | "compare" | "retry" | "undo" | "cancel",
  candidate: IdeCursorCandidate | null,
  now: string,
  ordinal: number,
): IdeCursorDecision | null => {
  const request =
    cursor.activeRequest ??
    (candidate === null
      ? null
      : {
          requestRef: candidate.requestRef,
          sequence: candidate.sequence,
        });
  if (request === null) return null;
  const common = {
    decisionRef: IdeCursorDecisionRefSchema.make(`ide.cursor-decision.renderer.${ordinal}`),
    requestRef: request.requestRef,
    sequence: request.sequence,
  };
  if (action === "cancel")
    return IdeCursorDecisionSchema.cases.Cancel.make({
      ...common,
      candidateRef: candidate?.candidateRef ?? null,
      decidedAt: IdeTimestampSchema.make(now),
      reason: "The renderer invalidated or cancelled this exact AI-editing request.",
    });
  if (candidate === null) return null;
  if (action === "reject")
    return IdeCursorDecisionSchema.cases.Reject.make({
      ...common,
      candidateRef: candidate.candidateRef,
      decidedAt: IdeTimestampSchema.make(now),
      reason: "The user rejected this exact candidate.",
    });
  if (action === "compare")
    return IdeCursorDecisionSchema.cases.Compare.make({
      ...common,
      candidateRef: candidate.candidateRef,
      decidedAt: IdeTimestampSchema.make(now),
    });
  if (action === "retry")
    return IdeCursorDecisionSchema.cases.Retry.make({
      ...common,
      candidateRef: candidate.candidateRef,
      decidedAt: IdeTimestampSchema.make(now),
    });
  if (action === "undo")
    return IdeCursorDecisionSchema.cases.Undo.make({
      ...common,
      candidateRef: candidate.candidateRef,
      decidedAt: IdeTimestampSchema.make(now),
      resultDigest: candidate.anchor.contentDigest,
    });
  return IdeCursorDecisionSchema.cases.Accept.make({
    ...common,
    candidateRef: candidate.candidateRef,
    acceptedAt: IdeTimestampSchema.make(now),
    granularity: action === "accept_word" ? "word" : action === "accept_line" ? "line" : "all",
    resultDigest: candidate.resultDigest,
  });
};

export const makeIdeCursorRendererHandlers = <S extends IdeCursorRendererCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  host: IdeCursorRendererHost = unavailableIdeCursorRendererHost,
  documents: WorkspaceDocumentBridge,
  agentCodeHost: IdeAgentCodeRendererHost = unavailableIdeAgentCodeRendererHost,
) => {
  let decisionOrdinal = 0;

  const updateCursor = (mutate: (cursor: IdeCursorRendererState) => IdeCursorRendererState) =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      ideCursor: mutate(current.ideCursor),
    }));

  const start = Effect.fn("IdeCursorRenderer.start")(function* (
    intent: typeof IdeCursorIntentSchema.Type,
  ) {
    const current = yield* SubscriptionRef.get(state);
    const built = buildIdeCursorProviderInput(current, intent, new Date().toISOString());
    if (built._tag === "Unavailable") {
      yield* updateCursor((cursor) =>
        IdeCursorRendererStateSchema.make({ ...cursor, notice: built.reason }),
      );
      return;
    }
    const result = yield* Effect.promise(() =>
      executeIdeCursorRendererCommand(host, { _tag: "Start", input: built.input }),
    );
    yield* updateCursor((cursor) =>
      IdeCursorRendererStateSchema.make({
        ...cursor,
        snapshot: result.snapshot,
        activeRequest: result._tag === "Succeeded" ? built.input.request : null,
        selectedCandidateRef: null,
        notice: result._tag === "Succeeded" ? null : result.message,
        invalidation: null,
      }),
    );
    if (
      result._tag === "Succeeded" &&
      result.snapshot.state === "running" &&
      !result.snapshot.candidates.some(
        (candidate) => candidate.requestRef === built.input.request.requestRef,
      )
    ) {
      // Main starts provider consumption in its scoped Effect fiber so a
      // Cancel decision can interrupt it. Reconcile the first independently
      // schema-decoded post-start snapshot here; otherwise the renderer would
      // remain stuck on the immediate `running` acknowledgement until a
      // manual Refresh even though main already owns a candidate. The exact
      // active request guard prevents a late poll from reviving superseded or
      // cancelled output.
      const observed = yield* Effect.promise(() =>
        waitForIdeCursorFirstCandidate(host, built.input.request),
      );
      yield* updateCursor((cursor) =>
        cursor.activeRequest?.requestRef !== built.input.request.requestRef
          ? cursor
          : IdeCursorRendererStateSchema.make({ ...cursor, snapshot: observed }),
      );
    }
  });

  const decide = Effect.fn("IdeCursorRenderer.decide")(function* (payload: {
    readonly action: "accept_word" | "accept_line" | "accept_all" | "reject" | "compare" | "retry" | "undo" | "cancel";
    readonly candidateRef: string | null;
  }) {
    const before = yield* SubscriptionRef.get(state);
    const candidate = candidateFor(before.ideCursor, payload.candidateRef);
    const decision = decisionFor(
      before.ideCursor,
      payload.action,
      candidate,
      new Date().toISOString(),
      ++decisionOrdinal,
    );
    if (decision === null) {
      yield* updateCursor((cursor) =>
        IdeCursorRendererStateSchema.make({
          ...cursor,
          notice: "The selected AI-editing candidate is unavailable or stale.",
        }),
      );
      return;
    }
    const result = yield* Effect.promise(() =>
      executeIdeCursorRendererCommand(host, { _tag: "Decide", decision }),
    );
    yield* updateCursor((cursor) =>
      IdeCursorRendererStateSchema.make({
        ...cursor,
        snapshot: result.snapshot,
        activeRequest: decision._tag === "Cancel" ? null : cursor.activeRequest,
        notice: result._tag === "Succeeded" ? null : result.message,
      }),
    );
    if (result._tag === "Succeeded" && decision._tag === "Retry") {
      const intent = before.ideCursor.activeRequest?.intent;
      if (intent !== undefined) yield* start(intent);
      return;
    }
    if (result._tag === "Succeeded" && decision._tag === "Compare") {
      yield* updateCursor((cursor) => IdeCursorRendererStateSchema.make({
        ...cursor,
        notice: `Compare ${cursor.snapshot.candidates.length} version-bound candidate(s); select a candidate to inspect its exact provenance.`,
      }));
      return;
    }
    if (
      result._tag !== "Succeeded" ||
      candidate === null ||
      (decision._tag !== "Accept" && decision._tag !== "Undo")
    )
      return;
    const receipt = result.snapshot.receipts.findLast(
      (item) => item.decision.decisionRef === decision.decisionRef,
    );
    if (receipt?.proposalSubmitted === true) {
      const agentCode = yield* Effect.promise(() => loadAgentCodeRendererSnapshot(agentCodeHost));
      yield* SubscriptionRef.update(state, (current) => ({
        ...current,
        agentCode,
        ideCursor: IdeCursorRendererStateSchema.make({
          ...current.ideCursor,
          activeRequest: null,
          notice: "Multi-file AI edit submitted to the canonical IDE-08 proposal review plane.",
        }),
      }));
      return;
    }
    if (receipt?.applied !== true) return;
    const latest = yield* SubscriptionRef.get(state);
    const grantRef = latest.workspaceBrowser.grantRef;
    if (grantRef === null) {
      yield* updateCursor((cursor) =>
        IdeCursorRendererStateSchema.make({
          ...cursor,
          notice: "The canonical workspace grant is unavailable after apply.",
        }),
      );
      return;
    }
    const raw = yield* Effect.promise(() =>
      documents
        .openWorkspaceDocument({ grantRef, pathRef: candidate.anchor.pathRef })
        .catch(() => null),
    );
    const opened = decodeWorkspaceDocumentResult(raw);
    if (opened === null || opened.state !== "available") {
      yield* updateCursor((cursor) =>
        IdeCursorRendererStateSchema.make({
          ...cursor,
          notice: "The canonical post-image could not be re-opened after apply.",
        }),
      );
      return;
    }
    yield* SubscriptionRef.update(state, (current) => ({
      ...current,
      workspaceEditor: withWorkspaceEditorCursorReconciled(
        current.workspaceEditor,
        candidate.anchor.pathRef,
        opened,
      ),
      ideCursor: IdeCursorRendererStateSchema.make({
        ...current.ideCursor,
        activeRequest: null,
        notice:
          decision._tag === "Undo"
            ? "AI edit undone from the canonical workspace."
            : "AI edit accepted into the canonical workspace.",
      }),
    }));
  });

  return {
    IdeCursorCompletionRequested: (granularity: "word" | "line" | "all") =>
      start({ _tag: "Complete", acceptance: granularity }),
    IdeCursorNextEditRequested: () => start({ _tag: "NextEdit" }),
    IdeCursorPromptChanged: (value: string) =>
      updateCursor((cursor) =>
        IdeCursorRendererStateSchema.make({ ...cursor, prompt: value.slice(0, 8_000) }),
      ),
    IdeCursorAskRequested: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state);
        if (current.ideCursor.prompt.trim() === "") return;
        yield* start({ _tag: "Ask", question: current.ideCursor.prompt.trim() });
      }),
    IdeCursorEditRequested: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state);
        if (current.ideCursor.prompt.trim() === "") return;
        yield* start({ _tag: "Edit", instruction: current.ideCursor.prompt.trim() });
      }),
    IdeCursorGenerateRequested: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state);
        if (current.ideCursor.prompt.trim() === "") return;
        yield* start({ _tag: "Generate", instruction: current.ideCursor.prompt.trim() });
      }),
    IdeCursorCandidateSelected: (candidateRef: IdeCursorCandidateRef | null) =>
      updateCursor((cursor) =>
        IdeCursorRendererStateSchema.make({ ...cursor, selectedCandidateRef: candidateRef }),
      ),
    IdeCursorDecisionRequested: decide,
    IdeCursorRefreshed: () =>
      Effect.gen(function* () {
        const snapshot = yield* Effect.promise(() => loadIdeCursorRendererSnapshot(host));
        yield* updateCursor((cursor) => IdeCursorRendererStateSchema.make({ ...cursor, snapshot }));
      }),
  };
};

export const cancelInvalidatedIdeCursor = async (
  host: IdeCursorRendererHost,
  previous: IdeCursorRendererState,
  invalidated: IdeCursorRendererState,
  ordinal: number,
): Promise<IdeCursorCommandResult | null> => {
  if (previous.activeRequest === null || invalidated.invalidation === null) return null;
  const decision = decisionFor(previous, "cancel", null, new Date().toISOString(), ordinal);
  return decision === null
    ? null
    : executeIdeCursorRendererCommand(host, { _tag: "Decide", decision });
};

export const cursorReconcileResult = (
  editor: WorkspaceEditorState,
  pathRef: string,
  result: DesktopWorkspaceDocumentResult,
): WorkspaceEditorState => withWorkspaceEditorCursorReconciled(editor, pathRef, result);
