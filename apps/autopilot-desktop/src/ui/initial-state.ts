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
    // AO-4 (#5441/#5445): the onboarding wizard is the FIRST screen on launch.
    // A fresh — or existing/adopted — user lands on the identity choice + the
    // live chain (registered → node → wallet → presence → Tassadar → earning),
    // NOT the replay/network viz. The sidebar nav lets them leave anytime.
    // Set here (the real app entry) rather than in `initialModel` so the shared
    // neutral base stays "network" for the view/update tests.
    pane: "onboarding",
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
