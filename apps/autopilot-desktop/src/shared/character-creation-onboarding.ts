import type {
  OnboardingStatusResponse,
  OnboardingStep,
  OnboardingStepStatus,
} from "./onboarding-status"
import type { ChatWorldPylonScene } from "./chat-world-scene"

export type CharacterCreationBeatId =
  | "pylon-online"
  | "agent-warp-in"
  | "customize"
  | "forum-intro"
  | "work-search"

export type CharacterCreationBeat = Readonly<{
  id: CharacterCreationBeatId
  label: string
  status: OnboardingStepStatus
  message: string
  sourceRefs: ReadonlyArray<string>
}>

export type CharacterCreationOnboardingProjection = Readonly<{
  enabled: boolean
  complete: boolean
  currentBeatId: CharacterCreationBeatId
  mana: number
  pylonOnlineCount: number
  beats: ReadonlyArray<CharacterCreationBeat>
}>

const step = (
  status: OnboardingStatusResponse | null,
  id: string,
): OnboardingStep | null =>
  status?.steps.find(item => item.id === id) ?? null

const statusOf = (
  candidate: OnboardingStep | null,
  fallback: OnboardingStepStatus,
): OnboardingStepStatus => candidate?.status ?? fallback

const clamp01 = (value: number): number =>
  Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0

const doneCount = (beats: ReadonlyArray<CharacterCreationBeat>): number =>
  beats.filter(beat => beat.status === "done").length

export const projectCharacterCreationOnboarding = (input: {
  readonly flagEnabled: boolean
  readonly onboardingStatus: OnboardingStatusResponse | null
  readonly chatWorldScene: ChatWorldPylonScene | null
}): CharacterCreationOnboardingProjection => {
  const status = input.onboardingStatus
  const identity = step(status, "identity")
  const registered = step(status, "registered")
  const nodeOnline = step(status, "node-online")
  const wallet = step(status, "wallet")
  const presence = step(status, "presence")
  const tassadar = step(status, "tassadar")
  const claimed = step(status, "claimed")
  const earned = step(status, "earned")
  const onlineNow = input.chatWorldScene?.onlineNow ?? 0

  const beats: ReadonlyArray<CharacterCreationBeat> = [
    {
      id: "pylon-online",
      label: "Pylon online",
      status: statusOf(nodeOnline, status === null ? "active" : "pending"),
      message: nodeOnline?.message ?? "Waiting for the local Pylon to come online.",
      sourceRefs: ["desktop:onboarding-status", "api.public.pylon-stats"],
    },
    {
      id: "agent-warp-in",
      label: "Agent warp-in",
      status: statusOf(registered, "pending"),
      message: registered?.message ?? "Agent registration unlocks the warp-in beat.",
      sourceRefs: ["desktop:onboarding-status", "agent.registration"],
    },
    {
      id: "customize",
      label: "Customize",
      status:
        identity?.status === "done" && wallet?.status === "done"
          ? "done"
          : identity?.status === "done"
            ? "active"
            : statusOf(identity, "active"),
      message:
        identity?.status === "done"
          ? wallet?.status === "done"
            ? "Name, role, color, and compute pool are ready."
            : "Identity chosen; filling the compute pool."
          : (identity?.message ?? "Choose the agent identity."),
      sourceRefs: ["desktop:identity-choice", "wallet.status.receiveReady"],
    },
    {
      id: "forum-intro",
      label: "Forum intro",
      status:
        presence?.status === "done"
          ? "done"
          : registered?.status === "done"
            ? "active"
            : "pending",
      message:
        presence?.status === "done"
          ? "Presence is live; the intro marker can rise from the real agent flow."
          : "Waiting for registered presence before posting the intro.",
      sourceRefs: ["presence.heartbeat", "forum.product-promises"],
    },
    {
      id: "work-search",
      label: "Work search",
      status:
        earned?.status === "done"
          ? "done"
          : claimed?.status === "done" || tassadar?.status === "done"
            ? "active"
            : "pending",
      message:
        earned?.status === "done"
          ? earned.message
          : claimed?.status === "done"
            ? "Work claimed; searching settlement receipts."
            : (tassadar?.message ?? "Waiting for NIP-90 and promise work markers."),
      sourceRefs: ["nip90.assignments", "product-promises", "settlement.receipts"],
    },
  ]

  const current =
    beats.find(beat => beat.status !== "done") ?? beats[beats.length - 1]!

  return {
    enabled: input.flagEnabled,
    complete: beats.every(beat => beat.status === "done"),
    currentBeatId: current.id,
    mana: clamp01(doneCount(beats) / beats.length),
    pylonOnlineCount: onlineNow,
    beats,
  }
}
