import type {
  KhalaCodeDesktopKhalaSyncChatMessage,
  KhalaCodeDesktopKhalaSyncRuntimeMessage,
  KhalaCodeDesktopMessage,
} from "../shared/rpc"

/**
 * #8425 desktop render-gap closeout: pure merge logic for a Khala
 * Sync-sourced thread's rendered message list, extracted from `main.ts` (a
 * DOM-mounting entrypoint that isn't otherwise unit-testable) so this can be
 * covered directly — mirroring how `main-shell-model.ts` extracts other pure
 * shell-model logic out of the same file.
 *
 * Context: a turn dispatched from mobile (or any non-desktop-composer
 * surface) never produces a `chat_message` row for its reply — only
 * `runtime_turn`/`runtime_event` rows carry it. `khala-sync-service.ts`
 * folds those into `runtimeMessages` on the `khalaSyncChatMessages` RPC
 * result (see `khala-runtime-transcript-desktop-core.ts`); this module
 * merges that additive field with the existing `chat_message`-only
 * `messages` field into the one chronological list `activateCodexThread`
 * already knows how to render, without changing anything about how a turn
 * STARTED in desktop renders.
 */

export const khalaSyncChatMessageToDesktopMessage = (
  message: KhalaCodeDesktopKhalaSyncChatMessage,
): KhalaCodeDesktopMessage => ({
  body: message.body,
  id: message.messageId,
  role: "user",
})

export const khalaSyncRuntimeMessageToDesktopMessage = (
  message: KhalaCodeDesktopKhalaSyncRuntimeMessage,
): KhalaCodeDesktopMessage => ({
  body: message.body,
  id: `runtime-turn.${message.turnId}`,
  role: "assistant",
})

/** Interleaves chat_message rows (already ascending by `createdAt`) with
 * synthesized runtime-turn assistant replies (already ascending by their own
 * `sortKey`) into one chronological message list. Uses a stable sort so
 * within-list relative order is preserved on any timestamp tie. */
export const mergeKhalaSyncChatAndRuntimeMessages = (
  chatMessages: ReadonlyArray<KhalaCodeDesktopKhalaSyncChatMessage>,
  runtimeMessages: ReadonlyArray<KhalaCodeDesktopKhalaSyncRuntimeMessage>,
): ReadonlyArray<KhalaCodeDesktopMessage> =>
  [
    ...chatMessages.map(message => ({
      message: khalaSyncChatMessageToDesktopMessage(message),
      sortKey: message.createdAt,
    })),
    ...runtimeMessages.map(message => ({
      message: khalaSyncRuntimeMessageToDesktopMessage(message),
      sortKey: message.sortKey,
    })),
  ]
    .sort((a, b) => (a.sortKey < b.sortKey ? -1 : a.sortKey > b.sortKey ? 1 : 0))
    .map(entry => entry.message)
