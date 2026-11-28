// Firm reason: 1 (Effect UI VirtualList transcript with tail-follow and h.lazy rows for docs/effect/half-day-sprint.md)
import { VirtualList, VirtualListScrollTarget } from "@effect/ui-kit"
import { Html } from "@effect/ui/html"
import type { ChatMessage } from "../../shared/wire.ts"
import { renderMessageRow } from "./MessageRow.ts"

export const renderTranscript = <Msg>(
  h: Html.H<Msg>,
  messages: ReadonlyArray<ChatMessage>
) => {
  if (messages.length === 0) {
    return h.div
      .attrs({ "data-testid": "transcript-empty" })
      .class("flex flex-1 items-center justify-center p-8 text-center text-sm text-zinc-500")
      .children(
        "Send a prompt below to run a live turn on the compiled Coax oa-worker."
      )
  }

  const keys = messages.map((m) => m.id)

  return VirtualList(h, {
    key: "transcript-list",
    props: {
      itemSize: 96,
      keys,
      contentAlignment: "end",
      followEnd: true,
      scrollTo: { target: VirtualListScrollTarget.End() }
    },
    view: (vl) =>
      vl.viewport(
        h.div
          .attrs({ "data-testid": "transcript-viewport", role: "log", "aria-live": "polite" })
          .class("flex-1 overflow-y-auto px-4 py-4")
      ).children(
        vl.list(h.ol.class("mx-auto flex max-w-3xl flex-col gap-3")).children(
          vl.items.map((index) => {
            const msg = messages[index]!
            return vl.item({
              element: h.li.class("flex flex-col").key(msg.id),
              index
            }).children(
              msg.streaming
                ? renderMessageRow(h, msg)
                : h.lazy(renderMessageRow, msg)
            )
          })
        )
      )
  })
}
