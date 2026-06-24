import { classAttrs } from "@openagentsinc/ui/class-foldkit"
import type { Html } from "foldkit/html"
import { html } from "foldkit/html"
import { domainStyles } from "./domain-styles.js"
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
      ...classAttrs<AutopilotUiMessage>(domainStyles.panel),
      h.DataAttribute("autopilot-verify-status", input.status),
    ],
    [
      h.div(classAttrs<AutopilotUiMessage>(domainStyles.header), [
        h.h3(classAttrs<AutopilotUiMessage>(domainStyles.title), [
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
          ...classAttrs<AutopilotUiMessage>(domainStyles.commandBlock),
          h.DataAttribute("autopilot-verify-command", ""),
        ],
        [input.command.map(shellQuote).join(" ")],
      ),
      h.ul(
        [
          ...classAttrs<AutopilotUiMessage>(domainStyles.list),
          h.DataAttribute("autopilot-required-artifacts", ""),
        ],
        input.requiredArtifacts.map((artifact) =>
          h.li(
            [
              ...classAttrs<AutopilotUiMessage>(domainStyles.twoColumnRow),
              h.DataAttribute("autopilot-artifact-ref", artifact.ref),
            ],
            [
              h.code(classAttrs<AutopilotUiMessage>(domainStyles.codeMuted), [
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
