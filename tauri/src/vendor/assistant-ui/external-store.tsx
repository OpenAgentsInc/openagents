// Local vendor shim for assistant-ui external-store runtime
// Route imports through our vendored package sources so we fully control types.

export { useExternalStoreRuntime } from "../../../../packages/assistant-ui-runtime/src/minimal/useExternalStoreRuntime";
export type { ExternalStoreAdapter } from "../../../../packages/assistant-ui-runtime/src/legacy-runtime/runtime-cores/external-store/ExternalStoreAdapter";
export type { AppendMessage } from "../../../../packages/assistant-ui-runtime/src/types/AssistantTypes";
export type { ThreadMessageLike as AUIThreadMessageLike } from "../../../../packages/assistant-ui-runtime/src/legacy-runtime/runtime-cores/external-store/ThreadMessageLike";
export { ExportedMessageRepository } from "../../../../packages/assistant-ui-runtime/src/legacy-runtime/runtime-cores/utils/MessageRepository";
