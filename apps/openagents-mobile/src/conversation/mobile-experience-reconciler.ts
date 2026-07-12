import type { MobileConversationSelection } from "./mobile-conversation"
import type { MobileSyncPhase } from "../screens/home-core"

/**
 * The personal scope is authoritative for the visible conversation only once it
 * reports the confirmed `live` phase. `bootstrapping`/`catching_up` are still
 * pre-live: the callable Sync host withholds `conversation()` and
 * `selectMobileConversation` returns the local fallback until the scope is live.
 */
const LIVE_PHASES: ReadonlySet<MobileSyncPhase> = new Set<MobileSyncPhase>(["live"])

export interface MobileAuthenticatedExperience {
  readonly conversation: MobileConversationSelection
}

export interface MobileExperienceReconciler {
  /**
   * Observe the latest observed sync phase. The authenticated experience is
   * selected once, synchronously, immediately after the verified session
   * connects — before the personal scope has caught up — so that first read is
   * necessarily the pre-live local fallback. This reconciler repairs that race:
   * when the scope first reaches a live phase while the current selection is
   * still that local fallback and an authenticated live scope exists, it
   * re-runs the authenticated experience selection exactly once and upgrades to
   * the confirmed sync selection.
   *
   * It is idempotent and single-flight: it never runs while a selection is
   * already in flight, never re-runs once the selection is sync, and the
   * underlying selection only reads confirmed rows, so a genuine local
   * fallback (the scope never becomes live) is preserved and no duplicate
   * conversation is ever created.
   */
  readonly observePhase: (phase: MobileSyncPhase | undefined) => void
  readonly close: () => void
}

export const openMobileExperienceReconciler = <
  Experience extends MobileAuthenticatedExperience,
>(
  input: Readonly<{
    currentMode: () => MobileConversationSelection["mode"]
    isAuthenticatedLive: () => boolean
    selectExperience: () => Promise<Experience>
    onUpgrade: (experience: Experience) => void
  }>,
): MobileExperienceReconciler => {
  let inFlight = false
  let closed = false
  return {
    observePhase: phase => {
      if (closed || inFlight) return
      if (phase === undefined || !LIVE_PHASES.has(phase)) return
      if (input.currentMode() === "sync") return
      if (!input.isAuthenticatedLive()) return
      inFlight = true
      void input.selectExperience().then(
        experience => {
          try {
            if (
              !closed &&
              experience.conversation.mode === "sync" &&
              input.currentMode() !== "sync"
            ) {
              input.onUpgrade(experience)
            }
          } finally {
            inFlight = false
          }
        },
        () => {
          inFlight = false
        },
      )
    },
    close: () => {
      closed = true
    },
  }
}
