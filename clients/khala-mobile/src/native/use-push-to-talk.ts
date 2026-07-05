import { useCallback, useEffect, useRef, useState } from "react"

import {
  describePushToTalkFailure,
  isPushToTalkPressable,
  phaseFromAvailability,
  type PushToTalkPhase,
  pushToTalkAccessibilityLabel
} from "./push-to-talk-core"
import { khalaNativeModules } from "./modules"

export type UsePushToTalkResult = Readonly<{
  phase: PushToTalkPhase
  pressable: boolean
  accessibilityLabel: string
  /** Tap-to-toggle: press once to start dictation, press again to stop and
   * resolve a transcript. See `chat-composer.tsx` for the call site — a
   * successful stop merges the transcript into the draft via
   * `mergeTranscriptIntoDraft`; a failure (either call currently always
   * rejects on both platforms, see `push-to-talk-core.ts`) surfaces through
   * `onError` instead of throwing into the caller. */
  press: () => void
}>

/**
 * Wires the `khala-push-to-talk-stt` Expo module into a tap-to-toggle mic
 * button. Checks `getAvailabilityAsync()` once on mount to decide whether the
 * button starts pressable; a press while idle calls `startRecognitionAsync`,
 * a press while recording calls `stopRecognitionAsync`. Both currently
 * always reject on real devices (the native module shells are linked but
 * unimplemented — see `push-to-talk-core.ts`'s `describePushToTalkFailure`
 * doc comment) — this hook does not pretend otherwise; it surfaces that
 * failure through `onError` and returns to the idle phase so the user can
 * try again rather than getting stuck.
 */
export const usePushToTalk = (input: {
  onTranscript: (transcript: string) => void
  onError: (message: string) => void
}): UsePushToTalkResult => {
  const { onError, onTranscript } = input
  const [phase, setPhase] = useState<PushToTalkPhase>("checking")
  const mountedRef = useRef(true)

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  useEffect(() => {
    let cancelled = false
    khalaNativeModules.pushToTalkStt
      .getAvailabilityAsync()
      .then(availability => {
        if (cancelled) return
        setPhase(phaseFromAvailability(availability))
      })
      .catch(() => {
        if (cancelled) return
        setPhase("unavailable")
      })
    return () => {
      cancelled = true
    }
  }, [])

  const press = useCallback(() => {
    setPhase(currentPhase => {
      if (!isPushToTalkPressable(currentPhase)) {
        onError(pushToTalkAccessibilityLabel(currentPhase))
        return currentPhase
      }

      if (currentPhase === "idle") {
        khalaNativeModules.pushToTalkStt
          .startRecognitionAsync()
          .then(result => {
            if (!mountedRef.current) return
            if (result.isFinal) {
              onTranscript(result.transcript)
              setPhase("idle")
            }
          })
          .catch((error: unknown) => {
            if (!mountedRef.current) return
            onError(describePushToTalkFailure(error))
            setPhase("idle")
          })
        return "recording"
      }

      // currentPhase === "recording"
      khalaNativeModules.pushToTalkStt
        .stopRecognitionAsync()
        .then(result => {
          if (!mountedRef.current) return
          onTranscript(result.transcript)
          setPhase("idle")
        })
        .catch((error: unknown) => {
          if (!mountedRef.current) return
          onError(describePushToTalkFailure(error))
          setPhase("idle")
        })
      return "recording"
    })
  }, [onError, onTranscript])

  return {
    accessibilityLabel: pushToTalkAccessibilityLabel(phase),
    phase,
    press,
    pressable: isPushToTalkPressable(phase)
  }
}
