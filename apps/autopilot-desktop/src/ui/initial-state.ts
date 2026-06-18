import type { Command } from "foldkit"

import { LoadInstallReadiness, LoadProofReplayBundle } from "./commands"
import type { Message } from "./message"
import { initialModel, Model } from "./model"

type InitialRuntimeState = readonly [
  Model,
  ReadonlyArray<Command.Command<Message>>,
]

export const initialRuntimeState = (): InitialRuntimeState => {
  const model = Model.make({
    ...initialModel,
    proofReplayPending: true,
    proofReplayStatus: {
      text: "loading public replay bundle...",
      tone: "info",
    },
  })

  return [
    model,
    [
      LoadInstallReadiness(),
      LoadProofReplayBundle({ slug: model.selectedProofReplaySlug }),
    ],
  ]
}
