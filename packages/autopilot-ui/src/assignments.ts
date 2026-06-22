import { stylexAttrs } from "@openagentsinc/ui/stylex-foldkit"
import type { Html } from "foldkit/html"
import { html } from "foldkit/html"
import { domainStyles } from "./domain-styles.js"
import type { AutopilotUiMessage, ChipTone } from "./view.js"
import { statusChip } from "./view.js"

export type Assignment = Readonly<{
  ref: string
  state: "available" | "accepted" | "in_progress" | "completed"
  progress?: number
}>

const h = html<AutopilotUiMessage>()

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
    [
      ...stylexAttrs<AutopilotUiMessage>(domainStyles.list),
      h.DataAttribute("autopilot-assignment-list", ""),
    ],
    input.assignments.length === 0
      ? [
          h.p(stylexAttrs<AutopilotUiMessage>(domainStyles.empty), [
            "No assignments",
          ]),
        ]
      : input.assignments.map((assignment) => {
          const progress = Math.max(0, Math.min(100, assignment.progress ?? 0))
          const showAccept = assignment.state === "available" && input.readOnly !== true

          return h.article(
            [
              ...stylexAttrs<AutopilotUiMessage>(domainStyles.assignmentRow),
              h.DataAttribute("autopilot-assignment-ref", assignment.ref),
            ],
            [
              h.code(stylexAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [
                assignment.ref,
              ]),
              statusChip({
                label: assignment.state,
                tone: assignmentStateTone(assignment.state),
                attrs: [h.DataAttribute("autopilot-assignment-state", assignment.state)],
              }),
              h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.progressStack), [
                h.div(
                  [
                    ...stylexAttrs<AutopilotUiMessage>(domainStyles.progressTrack),
                    h.DataAttribute("autopilot-assignment-progress", String(progress)),
                  ],
                  [
                    h.div([
                      ...stylexAttrs<AutopilotUiMessage>(domainStyles.progressBar),
                      h.Style({ width: `${progress}%` }),
                    ], []),
                  ],
                ),
                h.span(stylexAttrs<AutopilotUiMessage>(domainStyles.muted), [
                  progressLabel(progress),
                ]),
              ]),
              ...(showAccept
                ? [
                    h.button(
                      [
                        ...stylexAttrs<AutopilotUiMessage>(domainStyles.actionButton),
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
