import type { Command } from "foldkit"

import {
  LoadIdentityChoiceState,
  LoadInstallReadiness,
  LoadOnboardingStatus,
  LoadProofReplayBundle,
  LoadPublicActivityTimeline,
} from "./commands"
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
    publicActivityTimelinePending: true,
    publicActivityTimelineStatus: {
      text: "loading public activity...",
      tone: "info",
    },
    // AO-3/AO-4 (#5444/#5445): warm the first-run onboarding wizard on startup
    // so the "Get started" surface reflects the real identity-choice + chain
    // state immediately, not only after the user opens the pane.
    identityChoicePending: true,
    onboardingPending: true,
    onboardingStatusLine: { text: "loading onboarding status...", tone: "info" },
  })

  return [
    model,
    [
      LoadInstallReadiness(),
      LoadIdentityChoiceState(),
      LoadOnboardingStatus(),
      LoadProofReplayBundle({
        request: { mode: "catalog", slug: model.selectedProofReplaySlug },
      }),
      LoadPublicActivityTimeline(),
    ],
  ]
}
