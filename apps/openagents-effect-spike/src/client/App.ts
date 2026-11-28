// Firm reason: 1 (Root Effect UI App component wiring Query.StreamService, Transcript, ApprovalCard, TodosPanel, and isolated Composer for docs/effect/half-day-sprint.md)
import { UiSchema } from "@effect/ui"
import { Html } from "@effect/ui/html"
import { Schema } from "effect"
import { AsyncResult } from "effect/reactivity"
import { AppRoute } from "../shared/routes.ts"
import { ApprovalCard } from "./components/ApprovalCard.ts"
import { Composer } from "./components/Composer.ts"
import { renderTodosPanel } from "./components/TodosPanel.ts"
import { renderTranscript } from "./components/Transcript.ts"
import { SessionStream } from "./queries/SessionStream.ts"

export const AppModel = Schema.Struct({
  route: AppRoute,
  sessionId: Schema.String,
  model: Schema.String,
  effort: Schema.String,
  gated: Schema.Boolean
})
export type AppModel = typeof AppModel.Type

export const AppMessage = UiSchema.TaggedEnum({
  SelectedModel: { model: Schema.String },
  SelectedEffort: { effort: Schema.String },
  ToggledGated: {}
})
export type AppMessage = typeof AppMessage.Type

const statusDotClass = (status: string): string => {
  switch (status) {
    case "ready":
      return "bg-emerald-400"
    case "running":
      return "bg-sky-400 animate-pulse"
    case "waiting_approval":
      return "bg-amber-400 animate-pulse"
    case "error":
      return "bg-red-500"
    default:
      return "bg-zinc-500"
  }
}

export const App = Html.component({
  name: "App",
  props: Schema.Struct({ route: AppRoute }),
  model: AppModel,
  message: AppMessage,
  queries: {
    session: {
      query: SessionStream,
      args: ({ model }) => ({ sessionId: model.sessionId })
    }
  }
}).make({
  init: ($) => ({
    route: $.props.route,
    sessionId: "demo",
    model: "google/gemini-4-neon",
    effort: "low",
    gated: true
  }),
  update: {
    SelectedModel: ($, { model }) => $.evolve({ model }),
    SelectedEffort: ($, { effort }) => $.evolve({ effort }),
    ToggledGated: ($) => $.evolve({ gated: !$.model.gated })
  },
  view: (h, model) => {
    const queryResult = h.queries.session
    const snapshot = AsyncResult.isSuccess(queryResult) ? queryResult.value : null
    const streamError = AsyncResult.isFailure(queryResult)
      ? String(queryResult.cause)
      : null

    const status = snapshot?.status ?? "starting"
    const running = status === "running" || status === "waiting_approval"
    const stats = snapshot?.stats ?? null
    const tokPerSec =
      stats && stats.decodeMs > 0
        ? Math.round((stats.outputTokens / stats.decodeMs) * 1000)
        : null

    return h.div
      .class("flex h-screen flex-col bg-zinc-950 text-zinc-100")
      .children(
        h.header
          .class("flex flex-wrap items-center justify-between gap-3 border-b border-zinc-800 px-4 py-2.5")
          .children(
            h.div.class("flex items-center gap-2.5").children(
              h.span.class(`h-2.5 w-2.5 rounded-full ${statusDotClass(status)}`),
              h.h1.class("font-mono text-sm font-semibold text-zinc-100").children(
                snapshot?.title ? snapshot.title : "OpenAgents — Effect UI"
              ),
              h.span
                .attrs({ "data-testid": "worker-status" })
                .class("rounded bg-zinc-900 px-2 py-0.5 font-mono text-xs text-zinc-400")
                .children(
                  snapshot?.workerPid
                    ? `${status} · pid ${snapshot.workerPid}`
                    : status
                )
            ),
            h.div.class("flex flex-wrap items-center gap-2 font-mono text-xs").children(
              stats !== null
                ? h.span
                  .attrs({ "data-testid": "inference-stats" })
                  .class("rounded bg-zinc-900 px-2 py-1 text-zinc-400")
                  .children(
                    `TTFT ${stats.ttftMs}ms${tokPerSec !== null ? ` · ${tokPerSec} tok/s` : ""}`
                  )
                : null,
              h.select
                .attrs({ "aria-label": "Model", "data-testid": "model-select" })
                .class("rounded border border-zinc-800 bg-zinc-900 px-2 py-1 text-zinc-200")
                .value(model.model)
                .on("change", ({ value }) => AppMessage.SelectedModel({ model: value }))
                .children(
                  h.option.attr("value", "google/gemini-4-neon").children("google/gemini-4-neon"),
                  h.option.attr("value", "google/gemini-4-carbon").children("google/gemini-4-carbon"),
                  h.option.attr("value", "").children("local (Psionic Inkling)")
                ),
              h.select
                .attrs({ "aria-label": "Effort", "data-testid": "effort-select" })
                .class("rounded border border-zinc-800 bg-zinc-900 px-2 py-1 text-zinc-200")
                .value(model.effort)
                .on("change", ({ value }) => AppMessage.SelectedEffort({ effort: value }))
                .children(
                  h.option.attr("value", "low").children("effort: low"),
                  h.option.attr("value", "medium").children("effort: medium"),
                  h.option.attr("value", "high").children("effort: high")
                ),
              h.button
                .attrs({ type: "button", "data-testid": "gated-toggle" })
                .class(
                  model.gated
                    ? "rounded border border-amber-500/40 bg-amber-950/40 px-2 py-1 text-amber-200"
                    : "rounded border border-zinc-800 bg-zinc-900 px-2 py-1 text-zinc-400"
                )
                .on("click", AppMessage.ToggledGated())
                .children(model.gated ? "approvals: on" : "approvals: off")
            )
          ),

        streamError !== null
          ? h.div
            .class("border-b border-red-500/30 bg-red-950/40 px-4 py-2 font-mono text-xs text-red-300")
            .children(`Stream error: ${streamError}`)
          : null,

        snapshot?.lastError
          ? h.div
            .attrs({ "data-testid": "worker-error" })
            .class("border-b border-red-500/30 bg-red-950/40 px-4 py-2 font-mono text-xs text-red-300")
            .children(snapshot.lastError)
          : null,

        renderTranscript(h, snapshot?.messages ?? []),

        renderTodosPanel(h, snapshot?.todos ?? []),

        snapshot?.pendingPermit
          ? ApprovalCard(h, {
            key: `permit-${snapshot.pendingPermit.ticket}`,
            props: {
              sessionId: model.sessionId,
              permit: snapshot.pendingPermit
            }
          })
          : null,

        Composer(h, {
          props: {
            sessionId: model.sessionId,
            running,
            activeTurn: snapshot?.activeTurn ?? 0,
            model: model.model,
            effort: model.effort,
            gated: model.gated
          }
        })
      )
  }
})
