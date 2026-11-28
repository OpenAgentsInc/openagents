// Firm reason: 1 (Effect UI TodosPanel rendering MWW_TODOS checklist for docs/effect/half-day-sprint.md)
import { Html } from "@effect/ui/html"
import type { TodoItem } from "../../shared/wire.ts"

const badgeFor = (status: string): string => {
  switch (status) {
    case "completed":
    case "done":
      return "[x]"
    case "in_progress":
      return "[~]"
    default:
      return "[ ]"
  }
}

export const renderTodosPanel = <Msg>(
  h: Html.H<Msg>,
  todos: ReadonlyArray<TodoItem>
) => {
  if (todos.length === 0) return null
  return h.aside
    .attrs({ "aria-label": "Agent tasks", "data-testid": "todos-panel" })
    .class("mx-4 mb-2 rounded-lg border border-zinc-800 bg-zinc-900/60 px-3 py-2 text-xs")
    .children(
      h.div.class("mb-1 font-mono font-semibold text-zinc-400").children(
        `Tasks (${todos.length})`
      ),
      h.ul.class("space-y-1 font-mono").children(
        ...todos.map((item) =>
          h.li
            .key(item.id || item.content)
            .class(
              item.status === "completed" || item.status === "done"
                ? "text-zinc-500 line-through"
                : item.status === "in_progress"
                  ? "text-emerald-300"
                  : "text-zinc-300"
            )
            .children(`${badgeFor(item.status)} ${item.content}`)
        )
      )
    )
}
