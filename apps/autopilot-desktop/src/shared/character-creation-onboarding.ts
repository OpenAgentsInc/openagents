import type {
  OnboardingStatusResponse,
  OnboardingStep,
} from "./onboarding-status.js"
import type { ChatWorldPylonScene } from "./chat-world-scene.js"

export type CharacterCreationBeatId =
  | "pylon-online"
  | "agent-warp-in"
  | "customize"
  | "forum-intro"
  | "work-search"

export type CharacterCreationBeatStatus =
  | "pending"
  | "active"
  | "done"
  | "blocked"
  | "accepted"
  | "rejected"

export type CharacterCreationBeat = Readonly<{
  id: CharacterCreationBeatId
  label: string
  status: CharacterCreationBeatStatus
  message: string
  sourceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  required: boolean
}>

export type CharacterCreationOnboardingProjection = Readonly<{
  enabled: boolean
  complete: boolean
  currentBeatId: CharacterCreationBeatId
  mana: number
  pylonOnlineCount: number
  beats: ReadonlyArray<CharacterCreationBeat>
}>

export type CharacterCreationForumReadiness = Readonly<{
  ok: boolean
  agentTokenPresent: boolean
  forumTopicsUrl: string
  blockerRefs: ReadonlyArray<string>
}>

const step = (
  status: OnboardingStatusResponse | null,
  id: string,
): OnboardingStep | null =>
  status?.steps.find(item => item.id === id) ?? null

const statusOf = (
  candidate: OnboardingStep | null,
  fallback: CharacterCreationBeatStatus,
): CharacterCreationBeatStatus => {
  const status = candidate?.status
  return status === "failed" ? "blocked" : (status ?? fallback)
}

const clamp01 = (value: number): number =>
  Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0

const beatComplete = (beat: CharacterCreationBeat): boolean =>
  beat.status === "done" || beat.status === "accepted"

const doneCount = (beats: ReadonlyArray<CharacterCreationBeat>): number =>
  beats.filter(beatComplete).length

const publicSafeRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  refs
    .map(ref => ref?.trim() ?? "")
    .filter(ref =>
      ref.length > 0 &&
      !ref.startsWith("/") &&
      !ref.includes("/Users/") &&
      !ref.includes("\\"),
    )

const blockersFor = (candidate: OnboardingStep | null): ReadonlyArray<string> =>
  candidate?.status === "failed" ? [`onboarding.${candidate.id}`] : []

const forumIntroBeat = (input: {
  readonly registered: OnboardingStep | null
  readonly presence: OnboardingStep | null
  readonly forumReadiness: CharacterCreationForumReadiness | null | undefined
}): CharacterCreationBeat => {
  const registeredDone = input.registered?.status === "done"
  const presenceDone = input.presence?.status === "done"
  const readiness = input.forumReadiness

  if (!registeredDone) {
    return {
      id: "forum-intro",
      label: "Forum intro",
      status: "pending",
      message: "Waiting for registered presence before preparing the public intro.",
      sourceRefs: ["agent.registration", "forum.product-promises"],
      blockerRefs: [],
      required: false,
    }
  }

  if (!presenceDone) {
    return {
      id: "forum-intro",
      label: "Forum intro",
      status: "active",
      message: "Presence is converging; the public intro stays queued until allowed.",
      sourceRefs: ["presence.heartbeat", "forum.product-promises"],
      blockerRefs: blockersFor(input.presence),
      required: false,
    }
  }

  if (readiness === null || readiness === undefined) {
    return {
      id: "forum-intro",
      label: "Forum intro",
      status: "active",
      message: "Checking Forum posting permission; no intro is posted automatically.",
      sourceRefs: ["desktop:promise-surfacing-readiness", "forum.product-promises"],
      blockerRefs: ["desktop.promise-surfacing-readiness.pending"],
      required: false,
    }
  }

  if (!readiness.ok || !readiness.agentTokenPresent) {
    return {
      id: "forum-intro",
      label: "Forum intro",
      status: "blocked",
      message: "Forum intro needs explicit agent posting permission; nothing is posted yet.",
      sourceRefs: publicSafeRefs([
        "desktop:promise-surfacing-readiness",
        readiness.forumTopicsUrl,
      ]),
      blockerRefs: publicSafeRefs(readiness.blockerRefs),
      required: false,
    }
  }

  return {
    id: "forum-intro",
    label: "Forum intro",
    status: "active",
    message: "Forum intro is ready to prepare; posting still waits for explicit permission.",
    sourceRefs: publicSafeRefs([
      "desktop:promise-surfacing-readiness",
      readiness.forumTopicsUrl,
    ]),
    blockerRefs: [],
    required: false,
  }
}

const workSearchBeat = (input: {
  readonly tassadar: OnboardingStep | null
  readonly claimed: OnboardingStep | null
  readonly earned: OnboardingStep | null
}): CharacterCreationBeat => {
  const blockerRefs = publicSafeRefs([
    ...blockersFor(input.tassadar),
    ...blockersFor(input.claimed),
    ...blockersFor(input.earned),
  ])

  if (input.earned?.status === "done") {
    return {
      id: "work-search",
      label: "Work search",
      status: "accepted",
      message: input.earned.message,
      sourceRefs: ["assignments.poll", "wallet.status.balanceSats", "settlement.receipts"],
      blockerRefs: [],
      required: true,
    }
  }

  if (input.earned?.status === "failed") {
    return {
      id: "work-search",
      label: "Work search",
      status: "rejected",
      message: input.earned.message,
      sourceRefs: ["assignments.poll", "settlement.receipts"],
      blockerRefs,
      required: true,
    }
  }

  if (blockerRefs.length > 0) {
    return {
      id: "work-search",
      label: "Work search",
      status: "blocked",
      message:
        input.claimed?.message ??
        input.tassadar?.message ??
        "Work search is blocked.",
      sourceRefs: ["assignments.poll", "nip90.assignments"],
      blockerRefs,
      required: true,
    }
  }

  if (input.claimed?.status === "done") {
    return {
      id: "work-search",
      label: "Work search",
      status: "active",
      message: "Work claimed; searching settlement receipts.",
      sourceRefs: ["assignments.poll", "nip90.assignments", "settlement.receipts"],
      blockerRefs: [],
      required: true,
    }
  }

  if (input.tassadar?.status === "done") {
    return {
      id: "work-search",
      label: "Work search",
      status: "active",
      message: "Joined Tassadar; searching for claimable work.",
      sourceRefs: ["assignments.poll", "nip90.assignments"],
      blockerRefs: [],
      required: true,
    }
  }

  return {
    id: "work-search",
    label: "Work search",
    status: "pending",
    message: input.tassadar?.message ?? "Waiting for NIP-90 and promise work markers.",
    sourceRefs: ["nip90.assignments", "product-promises", "settlement.receipts"],
    blockerRefs: [],
    required: true,
  }
}

export const projectCharacterCreationOnboarding = (input: {
  readonly flagEnabled: boolean
  readonly onboardingStatus: OnboardingStatusResponse | null
  readonly chatWorldScene: ChatWorldPylonScene | null
  readonly forumReadiness?: CharacterCreationForumReadiness | null
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
      blockerRefs: blockersFor(nodeOnline),
      required: true,
    },
    {
      id: "agent-warp-in",
      label: "Agent spawned",
      status: statusOf(registered, "pending"),
      message: registered?.message ?? "Agent registration unlocks the spawn beat.",
      sourceRefs: ["desktop:onboarding-status", "agent.registration"],
      blockerRefs: blockersFor(registered),
      required: true,
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
      blockerRefs: publicSafeRefs([
        ...blockersFor(identity),
        ...blockersFor(wallet),
      ]),
      required: true,
    },
    forumIntroBeat({
      registered,
      presence,
      forumReadiness: input.forumReadiness,
    }),
    workSearchBeat({ tassadar, claimed, earned }),
  ]

  const requiredBeats = beats.filter(beat => beat.required)
  const current =
    beats.find(beat => beat.required && !beatComplete(beat)) ??
    beats.find(beat => beat.status === "blocked" || beat.status === "rejected") ??
    beats[beats.length - 1]!

  return {
    enabled: input.flagEnabled,
    complete: requiredBeats.every(beatComplete),
    currentBeatId: current.id,
    mana: clamp01(doneCount(requiredBeats) / requiredBeats.length),
    pylonOnlineCount: onlineNow,
    beats,
  }
}
