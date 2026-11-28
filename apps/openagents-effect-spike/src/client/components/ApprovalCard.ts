// Firm reason: 1 (Effect UI ApprovalCard component with exhaustive Story.explore state machine for docs/effect/half-day-sprint.md)
import { Command, UiSchema } from "@effect/ui"
import { Html } from "@effect/ui/html"
import { Schema } from "effect"
import { PendingPermit } from "../../shared/wire.ts"
import { ApproveCommand, CommandAck } from "../commands/SessionCommands.ts"

export const ApprovalStatus = Schema.Literals([
  "idle",
  "submitting",
  "resolved",
  "failed"
])

export const ApprovalDecision = Schema.Literals(["allow", "deny"])

export const ApprovalCardModel = Schema.Struct({
  status: ApprovalStatus,
  decision: Schema.NullOr(ApprovalDecision),
  selectedOption: Schema.NullOr(Schema.String),
  error: Schema.NullOr(Schema.String)
})
export type ApprovalCardModel = typeof ApprovalCardModel.Type

export const ApprovalCardMessage = UiSchema.TaggedEnum({
  ClickedAllow: {},
  ClickedDeny: {},
  ClickedOption: { option: Schema.String },
  SettledApprove: { result: CommandAck }
})
export type ApprovalCardMessage = typeof ApprovalCardMessage.Type

export const ApprovalCardProps = Schema.Struct({
  sessionId: Schema.String,
  permit: PendingPermit
})
export type ApprovalCardProps = typeof ApprovalCardProps.Type

export const ApprovalCard = Html.component({
  name: "ApprovalCard",
  props: ApprovalCardProps,
  model: ApprovalCardModel,
  message: ApprovalCardMessage
}).make({
  init: () => ({
    status: "idle",
    decision: null,
    selectedOption: null,
    error: null
  }),
  update: {
    ClickedAllow: ($) => {
      if ($.model.status === "submitting" || $.model.status === "resolved") {
        return $.model
      }
      return $.evolve({
        status: "submitting",
        decision: "allow",
        selectedOption: null,
        error: null
      }).run(
        Command.map(
          ApproveCommand.make({
            sessionId: $.props.sessionId,
            ticket: $.props.permit.ticket,
            answer: true
          }),
          (result) => ApprovalCardMessage.SettledApprove({ result })
        )
      )
    },

    ClickedDeny: ($) => {
      if ($.model.status === "submitting" || $.model.status === "resolved") {
        return $.model
      }
      return $.evolve({
        status: "submitting",
        decision: "deny",
        selectedOption: null,
        error: null
      }).run(
        Command.map(
          ApproveCommand.make({
            sessionId: $.props.sessionId,
            ticket: $.props.permit.ticket,
            answer: false,
            why: "Denied by user"
          }),
          (result) => ApprovalCardMessage.SettledApprove({ result })
        )
      )
    },

    ClickedOption: ($, { option }) => {
      if ($.model.status === "submitting" || $.model.status === "resolved") {
        return $.model
      }
      return $.evolve({
        status: "submitting",
        decision: "allow",
        selectedOption: option,
        error: null
      }).run(
        Command.map(
          ApproveCommand.make({
            sessionId: $.props.sessionId,
            ticket: $.props.permit.ticket,
            answer: true,
            option
          }),
          (result) => ApprovalCardMessage.SettledApprove({ result })
        )
      )
    },

    SettledApprove: ($, { result }) => {
      if ($.model.status !== "submitting") return $.model
      if (result._tag === "Ok") {
        return $.evolve({ status: "resolved", error: null })
      }
      return $.evolve({ status: "failed", error: result.message })
    }
  },
  view: (h, model) => {
    const props = h.props
    const busy = model.status === "submitting" || model.status === "resolved"
    const options = props.permit.options
    return h.section
      .attrs({
        role: "region",
        "aria-label": "Approval required",
        "data-testid": "approval-card",
        "data-ticket": String(props.permit.ticket),
        "data-status": model.status
      })
      .class("mx-4 mb-3 rounded-lg border border-amber-500/40 bg-amber-950/30 p-3.5 text-sm")
      .children(
        h.div.class("flex items-center justify-between gap-2").children(
          h.span.class("font-semibold text-amber-200").children(
            `${props.permit.title} (#${props.permit.ticket})`
          ),
          h.span.class("rounded bg-amber-500/20 px-2 py-0.5 font-mono text-xs text-amber-300")
            .children(model.status)
        ),
        props.permit.command.length > 0
          ? h.pre
            .class(
              "mt-2 overflow-x-auto rounded border border-amber-500/20 bg-zinc-950/80 px-2.5 py-2 font-mono text-xs text-zinc-200"
            )
            .children(props.permit.command)
          : null,
        model.error !== null
          ? h.p.class("mt-2 text-xs text-red-400").children(model.error)
          : null,
        h.div.class("mt-3 flex flex-wrap items-center gap-2").children(
          ...(options.length > 0 && props.permit.question
            ? options.map((opt) =>
              h.button
                .key(opt)
                .attrs({ type: "button" })
                .prop("disabled", busy)
                .class(
                  "rounded-md bg-amber-500 px-3 py-1.5 text-xs font-medium text-zinc-950 hover:bg-amber-400 disabled:opacity-50"
                )
                .on("click", ApprovalCardMessage.ClickedOption({ option: opt }))
                .children(opt)
            )
            : [
              h.button
                .attrs({
                  type: "button",
                  "data-testid": "approve-allow"
                })
                .prop("disabled", busy)
                .class(
                  "rounded-md bg-emerald-500 px-3 py-1.5 text-xs font-medium text-zinc-950 hover:bg-emerald-400 disabled:opacity-50"
                )
                .on("click", ApprovalCardMessage.ClickedAllow())
                .children(
                  model.status === "submitting" && model.decision === "allow"
                    ? "Allowing…"
                    : "Allow"
                ),
              h.button
                .attrs({
                  type: "button",
                  "data-testid": "approve-deny"
                })
                .prop("disabled", busy)
                .class(
                  "rounded-md border border-zinc-700 bg-zinc-900 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-zinc-800 disabled:opacity-50"
                )
                .on("click", ApprovalCardMessage.ClickedDeny())
                .children(
                  model.status === "submitting" && model.decision === "deny"
                    ? "Denying…"
                    : "Deny"
                )
            ])
        )
      )
  }
})
