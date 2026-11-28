// Firm reason: 1 (Effect UI MessageRow with @effect/ui-kit Collapsible thinking/tools and Markdown.view for docs/effect/half-day-sprint.md)
import { UiSchema } from "@effect/ui"
import { Collapsible } from "@effect/ui-kit"
import { Html, Markdown, MarkdownParser } from "@effect/ui/html"
import { Schema } from "effect"
import { ChatMessage, ToolCallEntry } from "../../shared/wire.ts"

const ToggleMessage = UiSchema.TaggedEnum({
  Toggled: { open: Schema.Boolean }
})

const ThinkingBlock = Html.component({
  name: "ThinkingBlock",
  props: Schema.Struct({ text: Schema.String, streaming: Schema.Boolean }),
  model: Schema.Struct({ open: Schema.Boolean }),
  message: ToggleMessage
}).make({
  init: () => ({ open: false }),
  update: {
    Toggled: ($, { open }) => $.evolve({ open })
  },
  view: (h, model) => {
    const props = h.props
    return Collapsible(h, {
      props: { open: model.open },
      onEmit: {
        Toggled: ({ open }) => ToggleMessage.Toggled({ open }),
        Settled: () => undefined
      },
      view: (c) =>
        c.root(h.div.class("mb-2 rounded-md border border-zinc-800/80 bg-zinc-900/50")).children(
          c.trigger(
            h.button
              .attrs({ type: "button" })
              .class(
                "flex w-full items-center justify-between px-2.5 py-1.5 text-left font-mono text-xs text-zinc-400 hover:text-zinc-200"
              )
          ).children(
            h.span(
              props.streaming
                ? `Thinking… (${props.text.length} chars)`
                : `Thought (${props.text.length} chars)`
            ),
            h.span(model.open ? "−" : "+")
          ),
          c.panel(
            h.pre
              .class(
                "max-h-60 overflow-y-auto whitespace-pre-wrap border-t border-zinc-800/80 px-2.5 py-2 font-mono text-xs text-zinc-400"
              )
              .children(props.text)
          )
        )
    })
  }
})

const ToolCallPill = Html.component({
  name: "ToolCallPill",
  props: Schema.Struct({ tool: ToolCallEntry }),
  model: Schema.Struct({ open: Schema.Boolean }),
  message: ToggleMessage
}).make({
  init: () => ({ open: false }),
  update: {
    Toggled: ($, { open }) => $.evolve({ open })
  },
  view: (h, model) => {
    const tool = h.props.tool
    const label =
      tool.kind === "view"
        ? `View ${tool.path}${tool.found ? "" : " (not found)"}`
        : `$ ${tool.command} [exit ${tool.exit ?? "?"}${tool.timedOut ? " timeout" : ""}]`
    const hasDetails = tool.output.length > 0
    if (!hasDetails) {
      return h.div
        .class(
          "rounded border border-zinc-800 bg-zinc-900/60 px-2.5 py-1 font-mono text-xs text-zinc-400"
        )
        .children(label)
    }
    return Collapsible(h, {
      props: { open: model.open },
      onEmit: {
        Toggled: ({ open }) => ToggleMessage.Toggled({ open }),
        Settled: () => undefined
      },
      view: (c) =>
        c.root(h.div.class("rounded border border-zinc-800 bg-zinc-900/60")).children(
          c.trigger(
            h.button
              .attrs({ type: "button" })
              .class(
                "flex w-full items-center justify-between px-2.5 py-1 text-left font-mono text-xs text-zinc-300 hover:text-zinc-100"
              )
          ).children(
            h.span.class("truncate").children(label),
            h.span.class("ml-2 shrink-0 text-zinc-500").children(model.open ? "−" : "+")
          ),
          c.panel(
            h.pre
              .class(
                "max-h-56 overflow-auto border-t border-zinc-800 px-2.5 py-2 font-mono text-xs text-zinc-400"
              )
              .children(tool.output)
          )
        )
    })
  }
})

export const renderMessageRow = <Msg>(h: Html.H<Msg>, msg: ChatMessage) => {
  if (msg.role === "user") {
    return h.article
      .attrs({ "data-role": "user", "data-msg-id": msg.id })
      .class("ml-auto max-w-[82%] rounded-xl border border-zinc-700 bg-zinc-800/80 px-3.5 py-2.5 text-sm text-zinc-100")
      .children(h.p.class("whitespace-pre-wrap").children(msg.text))
  }

  const parsedDoc = msg.text.length > 0 ? MarkdownParser.parseMarkdown(msg.text) : null

  return h.article
    .attrs({
      "data-role": "assistant",
      "data-msg-id": msg.id,
      "data-streaming": String(msg.streaming)
    })
    .class("max-w-[92%] rounded-xl border border-zinc-800 bg-zinc-900/50 px-4 py-3 text-sm text-zinc-100")
    .children(
      msg.thinking.length > 0
        ? ThinkingBlock(h, {
          key: `think-${msg.id}`,
          props: { text: msg.thinking, streaming: msg.streaming }
        })
        : null,
      msg.tools.length > 0
        ? h.div.class("mb-2 flex flex-col gap-1.5").children(
          ...msg.tools.map((tool) =>
            ToolCallPill(h, {
              key: tool.id,
              props: { tool }
            })
          )
        )
        : null,
      parsedDoc !== null
        ? Markdown.view(h, parsedDoc).class("oa-prose leading-relaxed text-zinc-100")
        : msg.streaming
          ? h.p.class("font-mono text-xs text-zinc-500").children("Streaming…")
          : null,
      msg.ending !== null && msg.ending !== "finished"
        ? h.div
          .class("mt-2 inline-block rounded bg-zinc-800 px-2 py-0.5 font-mono text-[11px] text-zinc-400")
          .children(`ended: ${msg.ending}`)
        : null
    )
}
