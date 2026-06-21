import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import type { AutopilotUiMessage, ChipTone } from "./view.js"
import { statusChip } from "./view.js"

export type VerifyState = Readonly<{
  command: ReadonlyArray<string>
  status: "pending" | "passed" | "failed"
  requiredArtifacts: ReadonlyArray<
    Readonly<{
      ref: string
      present: boolean
    }>
  >
}>

const h = html<AutopilotUiMessage>()

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const verifyStatusTone = (status: VerifyState["status"]): ChipTone => {
  switch (status) {
    case "passed":
      return "success"
    case "failed":
      return "danger"
    case "pending":
      return "warning"
  }
}

const artifactLabel = (present: boolean): "present" | "missing" => (present ? "present" : "missing")

const shellQuote = (part: string): string =>
  /^[A-Za-z0-9_./:=@%+-]+$/.test(part) ? part : `'${part.replaceAll("'", "'\\''")}'`

export const VerifyStatus = (input: VerifyState): Html =>
  h.section(
    [
      className(
        "grid gap-3 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-4 text-[var(--text,#d7d8e5)]",
      ),
      h.DataAttribute("autopilot-verify-status", input.status),
    ],
    [
      h.div([className("flex flex-wrap items-center justify-between gap-2")], [
        h.h3([className("m-0 font-mono text-sm font-bold text-[var(--primary,#fff)]")], [
          "verify",
        ]),
        statusChip({
          label: input.status,
          tone: verifyStatusTone(input.status),
          attrs: [h.DataAttribute("autopilot-verify-state", input.status)],
        }),
      ]),
      h.code(
        [
          className(
            "block min-w-0 overflow-x-auto whitespace-pre rounded-[4px] border border-[var(--outline,#525458)] bg-[var(--bg,#0b0b0c)] px-3 py-2 font-mono text-xs text-[var(--primary,#fff)]",
          ),
          h.DataAttribute("autopilot-verify-command", ""),
        ],
        [input.command.map(shellQuote).join(" ")],
      ),
      h.ul(
        [className("grid gap-2"), h.DataAttribute("autopilot-required-artifacts", "")],
        input.requiredArtifacts.map((artifact) =>
          h.li(
            [
              className(
                "grid gap-2 border border-[var(--outline,#525458)] bg-transparent p-3 sm:grid-cols-[minmax(0,1fr)_7rem] sm:items-center",
              ),
              h.DataAttribute("autopilot-artifact-ref", artifact.ref),
            ],
            [
              h.code([className("min-w-0 truncate font-mono text-xs text-[var(--primary,#fff)]")], [
                artifact.ref,
              ]),
              statusChip({
                label: artifactLabel(artifact.present),
                tone: artifact.present ? "success" : "danger",
                attrs: [
                  h.DataAttribute("autopilot-artifact-status", artifactLabel(artifact.present)),
                ],
              }),
            ],
          ),
        ),
      ),
    ],
  )
