import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import type { AutopilotUiMessage, ChipTone } from "./view.js"
import { statusChip } from "./view.js"

export type Assignment = Readonly<{
  ref: string
  state: "available" | "accepted" | "in_progress" | "completed"
  progress?: number
}>

const h = html<AutopilotUiMessage>()

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const assignmentStateTone = (state: Assignment["state"]): ChipTone => {
  switch (state) {
    case "available":
      return "info"
    case "accepted":
    case "in_progress":
      return "warning"
    case "completed":
      return "success"
  }
}

const progressLabel = (progress: number | undefined): string => `${progress ?? 0}%`

export const AssignmentList = (input: {
  assignments: ReadonlyArray<Assignment>
  readOnly?: boolean
}): Html =>
  h.section(
    [className("grid gap-2"), h.DataAttribute("autopilot-assignment-list", "")],
    input.assignments.length === 0
      ? [
          h.p([className("m-0 text-sm text-[var(--text-secondary,#8a8c93)]")], [
            "No assignments",
          ]),
        ]
      : input.assignments.map((assignment) => {
          const progress = Math.max(0, Math.min(100, assignment.progress ?? 0))
          const showAccept = assignment.state === "available" && input.readOnly !== true

          return h.article(
            [
              className(
                "grid gap-3 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-3 text-[var(--text,#d7d8e5)] sm:grid-cols-[minmax(0,1fr)_8rem_minmax(8rem,12rem)_auto] sm:items-center",
              ),
              h.DataAttribute("autopilot-assignment-ref", assignment.ref),
            ],
            [
              h.code([className("min-w-0 truncate font-mono text-sm text-[var(--primary,#fff)]")], [
                assignment.ref,
              ]),
              statusChip({
                label: assignment.state,
                tone: assignmentStateTone(assignment.state),
                attrs: [h.DataAttribute("autopilot-assignment-state", assignment.state)],
              }),
              h.div([className("grid gap-1")], [
                h.div(
                  [
                    className(
                      "h-2 overflow-hidden rounded-[4px] border border-[var(--outline,#525458)] bg-[var(--bg,#0d0d0d)]",
                    ),
                    h.DataAttribute("autopilot-assignment-progress", String(progress)),
                  ],
                  [
                    h.div([
                      className("h-full bg-[var(--info,#3ea6ff)]"),
                      h.Style({ width: `${progress}%` }),
                    ], []),
                  ],
                ),
                h.span([className("font-mono text-xs text-[var(--text-secondary,#8a8c93)]")], [
                  progressLabel(progress),
                ]),
              ]),
              ...(showAccept
                ? [
                    h.button(
                      [
                        className(
                          "inline-flex h-8 items-center justify-center rounded-[4px] border border-[var(--outline,#525458)] px-3 font-mono text-xs font-bold text-[var(--primary,#fff)] disabled:opacity-45",
                        ),
                        h.Type("button"),
                        h.DataAttribute("autopilot-assignment-action", "accept"),
                      ],
                      ["Accept"],
                    ),
                  ]
                : []),
            ],
          )
        }),
  )
