// External Store Runtime (for ACP integration)
export { useExternalStoreRuntime } from "./legacy-runtime/runtime-cores/external-store/useExternalStoreRuntime";
export type { ExternalStoreAdapter } from "./legacy-runtime/runtime-cores/external-store/ExternalStoreAdapter";
export type { AppendMessage } from "./types/AssistantTypes";
export type { ThreadMessageLike } from "./legacy-runtime/runtime-cores/external-store/ThreadMessageLike";
export { ExportedMessageRepository } from "./legacy-runtime/runtime-cores/utils/MessageRepository";

// Runtime Provider
export { AssistantRuntimeProvider } from "./legacy-runtime/AssistantRuntimeProvider";

// Local Runtime (for Ollama adapter)
export { useLocalRuntime } from "./legacy-runtime/runtime-cores/local/useLocalRuntime";
export type { ChatModelAdapter } from "./legacy-runtime/runtime-cores/local/ChatModelAdapter";

// Attachment Types
export type { AttachmentAdapter } from "./types/AttachmentTypes";

// Primitives
export * as ActionBarPrimitive from "./primitives/actionBar";
export * as AssistantModalPrimitive from "./primitives/assistantModal";
export * as AttachmentPrimitive from "./primitives/attachment";
export * as BranchPickerPrimitive from "./primitives/branchPicker";
export * as ComposerPrimitive from "./primitives/composer";
export * as MessagePartPrimitive from "./primitives/messagePart";
export * as ErrorPrimitive from "./primitives/error";
export * as MessagePrimitive from "./primitives/message";
export * as ThreadPrimitive from "./primitives/thread";
export * as ThreadListPrimitive from "./primitives/threadList";
export * as ThreadListItemPrimitive from "./primitives/threadListItem";

// Primitive Hooks
export { useMessagePartText } from "./primitives/messagePart/useMessagePartText";
export { useMessagePartReasoning } from "./primitives/messagePart/useMessagePartReasoning";
export { useMessagePartSource } from "./primitives/messagePart/useMessagePartSource";
export { useMessagePartFile } from "./primitives/messagePart/useMessagePartFile";
export { useMessagePartImage } from "./primitives/messagePart/useMessagePartImage";
export { useThreadViewportAutoScroll } from "./primitives/thread/useThreadViewportAutoScroll";
export { useScrollLock } from "./primitives/reasoning";

// Context Hooks
export {
  useAssistantApi,
  useExtendedAssistantApi,
  type AssistantApi,
} from "./context/react/AssistantApiContext";
export { useAssistantState } from "./context/react/hooks/useAssistantState";
export { useAssistantEvent } from "./context/react/hooks/useAssistantEvent";
export {
  useThreadViewport,
  useThreadViewportStore,
} from "./context/react/ThreadViewportContext";
export {
  useAssistantRuntime,
  useThreadList,
} from "./legacy-runtime/hooks/AssistantContext";
export {
  useAttachmentRuntime,
  useAttachment,
  useThreadComposerAttachmentRuntime,
  useThreadComposerAttachment,
  useEditComposerAttachmentRuntime,
  useEditComposerAttachment,
  useMessageAttachment,
  useMessageAttachmentRuntime,
} from "./legacy-runtime/hooks/AttachmentContext";
export {
  useComposerRuntime,
  useComposer,
} from "./legacy-runtime/hooks/ComposerContext";
export {
  useMessageRuntime,
  useEditComposer,
  useMessage,
} from "./legacy-runtime/hooks/MessageContext";
export {
  useMessagePartRuntime,
  useMessagePart,
} from "./legacy-runtime/hooks/MessagePartContext";
export {
  useThreadRuntime,
  useThread,
  useThreadComposer,
  useThreadModelContext,
} from "./legacy-runtime/hooks/ThreadContext";
export {
  useThreadListItemRuntime,
  useThreadListItem,
} from "./legacy-runtime/hooks/ThreadListItemContext";

// Model Context / Tools
export { makeAssistantTool } from "./model-context/makeAssistantTool";
export { makeAssistantToolUI } from "./model-context/makeAssistantToolUI";
export { makeAssistantVisible } from "./model-context/makeAssistantVisible";
export { useAssistantInstructions } from "./model-context/useAssistantInstructions";
export { useAssistantTool } from "./model-context/useAssistantTool";
export { useAssistantToolUI } from "./model-context/useAssistantToolUI";

// Types
export type { ToolCallMessagePartComponent } from "./types/MessagePartComponentTypes";

// Internal (for INTERNAL.generateId, etc.)
export * as INTERNAL from "./internal";
