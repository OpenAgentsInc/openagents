// Firm reason: 1 (Effect UI Command.Service browser HTTP mutations for docs/effect/half-day-sprint.md)
import { Command, UiSchema } from "@effect/ui"
import { Effect, Layer, Schema } from "effect"

export const CommandAck = UiSchema.TaggedEnum({
  Ok: {},
  Failed: { message: Schema.String }
})
export type CommandAck = typeof CommandAck.Type

const postJson = (
  url: string,
  body: Record<string, unknown>
): Effect.Effect<CommandAck> =>
  Effect.promise(async (): Promise<CommandAck> => {
    try {
      const res = await fetch(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body)
      })
      if (!res.ok) {
        const text = await res.text().catch(() => "")
        return CommandAck.Failed({
          message: text.slice(0, 200) || `HTTP ${res.status}`
        })
      }
      return CommandAck.Ok()
    } catch (err) {
      return CommandAck.Failed({
        message: err instanceof Error ? err.message : String(err)
      })
    }
  })

export class StartTurnCommand extends Command.Service<StartTurnCommand>()(
  "oa-spike/StartTurn",
  {
    args: {
      sessionId: Schema.String,
      task: Schema.String,
      model: Schema.String,
      effort: Schema.String,
      gated: Schema.Boolean
    },
    success: CommandAck,
    mode: "queue"
  }
) {
  static readonly layerBrowser = this.toLayer(({ sessionId, task, model, effort, gated }) =>
    postJson(`/api/sessions/${encodeURIComponent(sessionId)}/turn`, {
      task,
      model,
      effort,
      gated
    })
  )
  static readonly layerNoop = this.toLayer(() => Effect.succeed(CommandAck.Ok()))
}

export class ApproveCommand extends Command.Service<ApproveCommand>()(
  "oa-spike/Approve",
  {
    args: {
      sessionId: Schema.String,
      ticket: Schema.Number,
      answer: Schema.Boolean,
      why: Schema.optional(Schema.String),
      option: Schema.optional(Schema.String)
    },
    success: CommandAck,
    mode: "switch"
  }
) {
  static readonly layerBrowser = this.toLayer(({ sessionId, ticket, answer, why, option }) =>
    postJson(`/api/sessions/${encodeURIComponent(sessionId)}/approve`, {
      ticket,
      answer,
      ...(why !== undefined ? { why } : {}),
      ...(option !== undefined ? { option } : {})
    })
  )
  static readonly layerNoop = this.toLayer(() => Effect.succeed(CommandAck.Ok()))
}

export class SteerCommand extends Command.Service<SteerCommand>()(
  "oa-spike/Steer",
  {
    args: {
      sessionId: Schema.String,
      steer: Schema.String,
      turn: Schema.Number
    },
    success: CommandAck,
    mode: "queue"
  }
) {
  static readonly layerBrowser = this.toLayer(({ sessionId, steer, turn }) =>
    postJson(`/api/sessions/${encodeURIComponent(sessionId)}/steer`, {
      steer,
      turn
    })
  )
  static readonly layerNoop = this.toLayer(() => Effect.succeed(CommandAck.Ok()))
}

export class StopCommand extends Command.Service<StopCommand>()(
  "oa-spike/Stop",
  {
    args: {
      sessionId: Schema.String
    },
    success: CommandAck,
    mode: "switch"
  }
) {
  static readonly layerBrowser = this.toLayer(({ sessionId }) =>
    postJson(`/api/sessions/${encodeURIComponent(sessionId)}/stop`, {})
  )
  static readonly layerNoop = this.toLayer(() => Effect.succeed(CommandAck.Ok()))
}

export const SessionCommandsBrowser = Layer.mergeAll(
  StartTurnCommand.layerBrowser,
  ApproveCommand.layerBrowser,
  SteerCommand.layerBrowser,
  StopCommand.layerBrowser
)

export const SessionCommandsNoop = Layer.mergeAll(
  StartTurnCommand.layerNoop,
  ApproveCommand.layerNoop,
  SteerCommand.layerNoop,
  StopCommand.layerNoop
)
