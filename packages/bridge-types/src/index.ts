// Temporary hand-authored TS types for WS payloads and Tinyvex rows.
// In Phase 2 these will be overwritten by ts-rs generated files from the Rust bridge.

export interface ThreadSummaryTs {
  id: string;
  thread_id?: string | null;
  title: string;
  project_id?: string | null;
  resume_id?: string | null;
  rollout_path?: string | null;
  source?: string | null;
  created_at: number;
  updated_at: number;
  message_count?: number | null;
  last_message_ts?: number | null;
}

export interface MessageRowTs {
  id: number;
  thread_id: string;
  role?: 'user' | 'assistant' | undefined | null;
  kind: 'message' | 'reason' | string;
  text?: string | null;
  item_id?: string | null;
  partial?: 0 | 1 | null;
  seq?: number | null;
  ts: number;
  created_at: number;
  updated_at?: number | null;
}

export interface ToolCallRowTs {
  thread_id: string;
  tool_call_id: string;
  title?: string | null;
  kind?: string | null;
  status?: string | null;
  content_json?: string | null;
  locations_json?: string | null;
  created_at: number;
  updated_at: number;
}

export interface SyncWatchedDirTs {
  path: string;
  provider: string;
  last_read?: number | null;
}

export interface SyncStatusTs {
  enabled: boolean;
  two_way: boolean;
  watched: SyncWatchedDirTs[];
  last_read?: number | null;
}

export type TinyvexStreamName = 'threads' | 'messages' | 'toolCalls' | 'plan' | 'state';

export interface TinyvexSnapshot<T> {
  type: 'tinyvex.snapshot';
  stream: TinyvexStreamName;
  rows: T[];
}

export interface TinyvexQueryResult<T> {
  type: 'tinyvex.query_result';
  name: string;
  args?: unknown;
  rows: T[];
}

export interface TinyvexUpdate {
  type: 'tinyvex.update';
  stream: TinyvexStreamName;
  op: 'upsert' | 'update' | 'insert' | 'upsert_streamed' | 'finalize_streamed';
  thread_id?: string;
  item_id?: string;
  tool_call_id?: string;
  updated_at?: number;
}

export type BridgeEvent<T = unknown> =
  | TinyvexSnapshot<ThreadSummaryTs>
  | TinyvexSnapshot<MessageRowTs>
  | TinyvexSnapshot<ToolCallRowTs>
  | TinyvexQueryResult<ThreadSummaryTs>
  | TinyvexQueryResult<MessageRowTs>
  | TinyvexQueryResult<ToolCallRowTs>
  | TinyvexUpdate
  | T;

