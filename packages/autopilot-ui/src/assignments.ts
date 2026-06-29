import { classAttrs } from "@openagentsinc/ui/class-foldkit"
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
      ...classAttrs<AutopilotUiMessage>(domainStyles.list),
      h.DataAttribute("autopilot-assignment-list", ""),
    ],
    input.assignments.length === 0
      ? [
          h.p(classAttrs<AutopilotUiMessage>(domainStyles.empty), [
            "No assignments",
          ]),
        ]
      : input.assignments.map((assignment) => {
          const progress = Math.max(0, Math.min(100, assignment.progress ?? 0))
          const showAccept = assignment.state === "available" && input.readOnly !== true

          return h.article(
            [
              ...classAttrs<AutopilotUiMessage>(domainStyles.assignmentRow),
              h.DataAttribute("autopilot-assignment-ref", assignment.ref),
            ],
            [
              h.code(classAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [
                assignment.ref,
              ]),
              statusChip({
                label: assignment.state,
                tone: assignmentStateTone(assignment.state),
                attrs: [h.DataAttribute("autopilot-assignment-state", assignment.state)],
              }),
              h.div(classAttrs<AutopilotUiMessage>(domainStyles.progressStack), [
                h.div(
                  [
                    ...classAttrs<AutopilotUiMessage>(domainStyles.progressTrack),
                    h.DataAttribute("autopilot-assignment-progress", String(progress)),
                  ],
                  [
                    h.div([
                      ...classAttrs<AutopilotUiMessage>(domainStyles.progressBar),
                      h.Style({ width: `${progress}%` }),
                    ], []),
                  ],
                ),
                h.span(classAttrs<AutopilotUiMessage>(domainStyles.muted), [
                  progressLabel(progress),
                ]),
              ]),
              ...(showAccept
                ? [
                    h.button(
                      [
                        ...classAttrs<AutopilotUiMessage>(domainStyles.actionButton),
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
