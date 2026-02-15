/**
 * Typed shapes for Convex API responses used by the Worker.
 * Keeps effuse-host free of "as any" when reading query/mutation results.
 */

export type ThreadSnapshotMessage = {
  messageId: string;
  role: string;
  status: string;
  text: string | null;
  runId: string | null;
  createdAtMs: number;
  updatedAtMs: number;
};

export type ThreadSnapshotPartRow = {
  messageId: string;
  runId: string;
  seq: number;
  part: unknown;
  createdAtMs: number;
};

export type GetRunPartsHeadResult = {
  ok: boolean;
  threadId: string;
  parts: ReadonlyArray<ThreadSnapshotPartRow>;
};


export type GetThreadSnapshotResult = {
  ok: boolean;
  threadId: string;
  messages: ReadonlyArray<ThreadSnapshotMessage>;
  parts: ReadonlyArray<unknown>;
};

export type GetBlueprintResult = {
  ok: boolean;
  blueprint: unknown;
  updatedAtMs: number;
};

export type CreateRunResult = {
  ok: boolean;
  runId: string;
  userMessageId: string;
  assistantMessageId: string;
};

export type EnsureOwnedThreadResult = {
  ok: boolean;
  threadId: string;
};

export type GetThreadTraceBundleResult = {
  ok: boolean;
  thread: {
    threadId: string;
    ownerId: string | null;
    createdAtMs: number;
    updatedAtMs: number;
  };
  blueprint: unknown;
  messages: ReadonlyArray<unknown>;
  parts: ReadonlyArray<unknown>;
  runs: ReadonlyArray<unknown>;
  receipts: ReadonlyArray<unknown>;
  featureRequests: ReadonlyArray<unknown>;
  dseBlobs: ReadonlyArray<unknown>;
  dseVars: ReadonlyArray<unknown>;
  summary: {
    messageCount: number;
    partCount: number;
    runCount: number;
    receiptCount: number;
    featureRequestCount: number;
    dseBlobCount: number;
    dseVarCount: number;
  };
};

export type IsCancelRequestedResult = {
  ok: boolean;
  cancelRequested?: boolean;
};

export type DseListExamplesResult = {
  examples?: ReadonlyArray<{
    exampleId?: string;
    inputJson?: unknown;
    expectedJson?: unknown;
    split?: string;
    tags?: ReadonlyArray<string>;
    meta?: unknown;
  }>;
};

export type DseGetReportResult = {
  report?: {
    compiled_id?: string;
    json?: unknown;
  } | null;
};

export type DseGetActiveResult = {
  compiled_id?: string | null;
};

export type DseGetArtifactResult = {
  artifact?: unknown;
};

export type DseCanaryResult = {
  canary?: {
    enabled?: boolean;
    rolloutPct?: number;
    salt?: string;
    canary_compiled_id?: string;
    control_compiled_id?: string;
  } | null;
};

export type DseStopCanaryResult = {
  existed?: boolean;
};
