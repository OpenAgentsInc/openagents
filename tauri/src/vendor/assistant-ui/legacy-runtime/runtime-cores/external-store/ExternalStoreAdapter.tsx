import type { ReadonlyJSONValue } from "assistant-stream/utils";

// Minimal local copy matching @assistant-ui/react ExternalStoreAdapter API surface
// Sourced from packages/react/src/legacy-runtime/runtime-cores/external-store/ExternalStoreAdapter.tsx

export type ExternalStoreThreadData<TState extends "regular" | "archived"> = {
  status: TState;
  id: string;
  remoteId?: string | undefined;
  externalId?: string | undefined;
  title?: string | undefined;
};

export type ExternalStoreThreadListAdapter = {
  threadId?: string | undefined;
  isLoading?: boolean | undefined;
  threads?: readonly ExternalStoreThreadData<"regular">[] | undefined;
  archivedThreads?: readonly ExternalStoreThreadData<"archived">[] | undefined;
  onSwitchToNewThread?: (() => Promise<void> | void) | undefined;
  onSwitchToThread?: ((threadId: string) => Promise<void> | void) | undefined;
  onRename?: (
    threadId: string,
    newTitle: string,
  ) => (Promise<void> | void) | undefined;
  onArchive?: ((threadId: string) => Promise<void> | void) | undefined;
  onUnarchive?: ((threadId: string) => Promise<void> | void) | undefined;
  onDelete?: ((threadId: string) => Promise<void> | void) | undefined;
};

export type ExternalStoreMessageConverter<T> = (
  message: T,
  idx: number,
) => any; // We avoid importing ThreadMessageLike to keep this file standalone

type ExternalStoreMessageConverterAdapter<T> = {
  convertMessage: ExternalStoreMessageConverter<T>;
};

type ExternalStoreAdapterBase<T> = {
  isDisabled?: boolean | undefined;
  isRunning?: boolean | undefined;
  isLoading?: boolean | undefined;
  messages?: readonly T[];
  messageRepository?: any; // ExportedMessageRepository
  suggestions?: readonly any[] | undefined; // ThreadSuggestion
  state?: ReadonlyJSONValue | undefined;
  extras?: unknown;

  setMessages?: ((messages: readonly T[]) => void) | undefined;
  onImport?: ((messages: readonly any[]) => void) | undefined; // ThreadMessage[]
  onLoadExternalState?: ((state: any) => void) | undefined;
  onNew: (message: any) => Promise<void>; // AppendMessage
  onEdit?: ((message: any) => Promise<void>) | undefined; // AppendMessage
  onReload?: ((parentId: string | null, config: any) => Promise<void>) | undefined; // StartRunConfig
  onResume?: ((config: any) => Promise<void>) | undefined; // ResumeRunConfig
  onCancel?: (() => Promise<void>) | undefined;
  onAddToolResult?: ((options: any) => Promise<void> | void) | undefined; // AddToolResultOptions
  onResumeToolCall?: ((options: { toolCallId: string; payload: unknown }) => void) | undefined;
  convertMessage?: ExternalStoreMessageConverter<T> | undefined;
  adapters?: {
    attachments?: any | undefined; // AttachmentAdapter
    speech?: any | undefined; // Speech adapter
    feedback?: any | undefined; // FeedbackAdapter
    threadList?: ExternalStoreThreadListAdapter | undefined;
  } | undefined;
  unstable_capabilities?: { copy?: boolean | undefined } | undefined;
};

export type ExternalStoreAdapter<T = any> = ExternalStoreAdapterBase<T> &
  (T extends any ? object : ExternalStoreMessageConverterAdapter<T>);

