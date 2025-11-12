// Local vendor shim for assistant-ui external-store runtime
// We expose the hook and types from a single location within our repo
// to make it easier to migrate the implementation in the future.

export { useExternalStoreRuntime } from "@assistant-ui/react";
export type { ExternalStoreAdapter } from "@assistant-ui/react";
export type { AppendMessage } from "@assistant-ui/react";
export type { ThreadMessageLike as AUIThreadMessageLike } from "@assistant-ui/react";
export { ExportedMessageRepository } from "@assistant-ui/react";
