// Firm reason: 1 (Isolated Effect UI Composer component preventing transcript re-renders on typing for docs/effect/half-day-sprint.md)
import { Command, UiSchema } from "@effect/ui"
import { Html } from "@effect/ui/html"
import { Schema } from "effect"
import {
  CommandAck,
  StartTurnCommand,
  SteerCommand,
  StopCommand
} from "../commands/SessionCommands.ts"

export const ComposerProps = Schema.Struct({
  sessionId: Schema.String,
  running: Schema.Boolean,
  activeTurn: Schema.Number,
  model: Schema.String,
  effort: Schema.String,
  gated: Schema.Boolean
})
export type ComposerProps = typeof ComposerProps.Type

export const ComposerModel = Schema.Struct({
  draft: Schema.String,
  submitting: Schema.Boolean,
  error: Schema.NullOr(Schema.String)
})
export type ComposerModel = typeof ComposerModel.Type

export const ComposerMessage = UiSchema.TaggedEnum({
  UpdatedDraft: { value: Schema.String },
  Submitted: {},
  ClickedStop: {},
  SettledCommand: { result: CommandAck }
})
export type ComposerMessage = typeof ComposerMessage.Type

export const Composer = Html.component({
  name: "Composer",
  props: ComposerProps,
  model: ComposerModel,
  message: ComposerMessage
}).make({
  init: () => ({
    draft: "",
    submitting: false,
    error: null
  }),
  update: {
    UpdatedDraft: ($, { value }) => $.evolve({ draft: value, error: null }),

    Submitted: ($) => {
      const text = $.model.draft.trim()
      if (text.length === 0 || $.model.submitting) return $.model
      const cmd = $.props.running
        ? Command.map(
          SteerCommand.make({
            sessionId: $.props.sessionId,
            steer: text,
            turn: $.props.activeTurn
          }),
          (result) => ComposerMessage.SettledCommand({ result })
        )
        : Command.map(
          StartTurnCommand.make({
            sessionId: $.props.sessionId,
            task: text,
            model: $.props.model,
            effort: $.props.effort,
            gated: $.props.gated
          }),
          (result) => ComposerMessage.SettledCommand({ result })
        )
      return $.evolve({ draft: "", submitting: true, error: null }).run(cmd)
    },

    ClickedStop: ($) =>
      $.evolve({ error: null }).run(
        Command.map(
          StopCommand.make({ sessionId: $.props.sessionId }),
          (result) => ComposerMessage.SettledCommand({ result })
        )
      ),

    SettledCommand: ($, { result }) =>
      result._tag === "Ok"
        ? $.evolve({ submitting: false, error: null })
        : $.evolve({ submitting: false, error: result.message })
  },
  view: (h, model) => {
    const props = h.props
    const canSubmit = model.draft.trim().length > 0 && !model.submitting
    return h.form
      .class("border-t border-zinc-800 bg-zinc-950 px-4 py-3")
      .on("submit", ComposerMessage.Submitted(), { preventDefault: true })
      .children(
        model.error !== null
          ? h.p.class("mb-2 text-xs text-red-400").children(model.error)
          : null,
        h.div.class("flex items-end gap-2").children(
          h.textarea
            .attrs({
              "aria-label": "Message",
              "data-testid": "composer-input",
              rows: "2",
              placeholder: props.running
                ? "Steer the active turn… (Enter to steer, Shift+Enter for newline)"
                : "Ask OpenAgents… (Enter to send, Shift+Enter for newline)"
            })
            .class(
              "min-h-[44px] flex-1 resize-none rounded-lg border border-zinc-800 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-500 focus:border-zinc-600 focus:outline-none"
            )
            .value(model.draft)
            .on("input", ({ value }) => ComposerMessage.UpdatedDraft({ value }))
            .on(
              "keydown",
              ({ key, shift, composing }) =>
                key === "Enter" && !shift && !composing
                  ? ComposerMessage.Submitted()
                  : undefined,
              { preventDefault: true }
            ),
          props.running
            ? h.button
              .attrs({
                type: "button",
                "data-testid": "composer-stop"
              })
              .class(
                "rounded-lg border border-red-500/40 bg-red-950/60 px-3 py-2 text-xs font-medium text-red-200 hover:bg-red-900/60"
              )
              .on("click", ComposerMessage.ClickedStop())
              .children("Stop")
            : null,
          h.button
            .attrs({
              type: "submit",
              "data-testid": "composer-submit"
            })
            .prop("disabled", !canSubmit)
            .class(
              "rounded-lg bg-zinc-100 px-4 py-2 text-xs font-semibold text-zinc-950 hover:bg-white disabled:opacity-40"
            )
            .children(props.running ? "Steer" : "Send")
        )
      )
  }
})
