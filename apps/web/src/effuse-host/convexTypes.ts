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
