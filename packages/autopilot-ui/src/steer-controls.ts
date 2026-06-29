import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import type { AutopilotUiMessage } from "./view.js"

type SessionView = Readonly<{
  sessionRef: string
  state: "queued" | "running" | "paused" | "completed" | "failed" | "cancelled"
}>

type SteerAction = "steer" | "interrupt" | "pause" | "resume"

const h = html<AutopilotUiMessage>()

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const actionClass =
  "inline-flex h-8 items-center rounded-[4px] border border-[var(--outline,#525458)] px-3 font-mono text-xs font-bold text-[var(--primary,#fff)] disabled:opacity-45"

const isTerminalSessionState = (state: SessionView["state"]): boolean =>
  state === "completed" || state === "failed" || state === "cancelled"

const isActionDisabled = (input: {
  action: SteerAction
  session: SessionView
  readOnly: boolean
}): boolean => {
  if (input.readOnly || isTerminalSessionState(input.session.state)) return true
  if (input.action === "resume") return input.session.state !== "paused"
  return input.session.state !== "running"
}

export const SteerControls = (input: { session: SessionView; readOnly: boolean }): Html => {
  const actions = [
    ["steer", "Steer"],
    ["interrupt", "Interrupt"],
    ["pause", "Pause"],
    ["resume", "Resume"],
  ] as const satisfies ReadonlyArray<readonly [SteerAction, string]>

  return h.div(
    [
      className("flex flex-wrap gap-2"),
      h.DataAttribute("autopilot-steer-controls", input.session.sessionRef),
    ],
    actions.map(([action, label]) =>
      h.button(
        [
          className(actionClass),
          h.Type("button"),
          h.Disabled(isActionDisabled({ action, session: input.session, readOnly: input.readOnly })),
          h.DataAttribute("autopilot-action", action),
        ],
        [label],
      ),
    ),
  )
}
