// Full-input-path test harness (#6041 class of bugs).
//
// WHY: the original ⌘⇧E / ⌘⇧P bug (#6041) shipped with a GREEN reducer test
// because the test started at `update(model, PressedKey(...))` — AFTER the part
// that was broken. The live desktop path is:
//
//   keydown (DOM)
//     → keyboardForwardDecision(...)   FORWARD GATE (subscriptions.ts)
//       → PressedKey queued            (only if forward === true)
//         → interpretKey(model, ev)    (keyboard.ts)
//           → reducer update(...)      (update.ts)
//             → next Model
//               → verseSceneVisualization(model)  (view.ts — feeds the renderer)
//
// The bug lived in the FORWARD GATE: the spawn keys resolved to zero action ids,
// so `forward` was false and the keydown was dropped before `interpretKey` ran.
// A reducer-only test can never see that.
//
// THIS HARNESS drives the WHOLE path in one call, in the SAME order the live DOM
// handler in subscriptions.ts uses, calling the REAL `keyboardForwardDecision`,
// the REAL reducer, and the REAL visualization projection. If the gate drops the
// key, `dispatched` is false and `nextModel === model` — i.e. the #6041 bug is
// directly observable. Any new input feature tested through this util is tested
// end to end, so we stop shipping keybindings whose reducer test is green but
// whose live key does nothing.

import type { OpenAgentsInputActionMap } from "@openagentsinc/input-bindings"

import { keyboardForwardDecision } from "../ui/subscriptions.js"
import { interpretKey, type KeyEvent, type KeyIntent } from "../ui/keyboard.js"
import { update } from "../ui/update.js"
import { PressedKey } from "../ui/message.js"
import type { Model } from "../ui/model.js"
import { verseSceneVisualization } from "../ui/view.js"
import { VERSE_SPAWNED_SCENE_NODE_PREFIX } from "../shared/verse-spawned-scene.js"

export type FullInputPathOptions = Readonly<{
  // Inject an alternate input action map to prove the harness fails on a
  // deliberately-broken binding set and passes on the real one. Defaults to the
  // production desktop action map (whatever keyboardForwardDecision uses).
  actionMap?: OpenAgentsInputActionMap
}>

export type FullInputPathOutcome = Readonly<{
  // The forward gate (subscriptions.ts). `false` here is exactly the #6041 bug.
  forwarded: boolean
  preventDefault: boolean
  // Did a PressedKey actually reach the reducer? Mirrors the live handler:
  // the message is only queued when the gate forwards.
  dispatched: boolean
  // What interpretKey resolved the key to (only meaningful when dispatched).
  intent: KeyIntent
  // Reducer output. Identical to the input model when the gate dropped the key.
  nextModel: Model
  // The world visualization the renderer consumes for `nextModel`.
  visualization: ReturnType<typeof verseSceneVisualization>
  // Convenience projection: scene ids spawned in the resulting model.
  spawnedSceneIds: ReadonlyArray<string>
  // Convenience projection: spawned-scene entity ids present in the rendered viz.
  spawnedVizEntityIds: ReadonlyArray<string>
  // Convenience projection: crackling_arc beams present in the rendered viz.
  cracklingArcBeamCount: number
}>

const NO_INTENT: KeyIntent = { kind: "none" }

// Drive a raw keyboard event + model state through the REAL full input path and
// return the observable outcome. This is the single source of truth a test
// should use to assert an input feature works end to end.
export const runKeyEventThroughFullPath = (
  model: Model,
  event: KeyEvent,
  options: FullInputPathOptions = {},
): FullInputPathOutcome => {
  // 1. FORWARD GATE — the exact decision the DOM keydown handler makes. We pass
  //    the optional override map through to the real function so a test can
  //    inject a broken map.
  const decision =
    options.actionMap === undefined
      ? keyboardForwardDecision({
          key: event.key,
          ...(event.code === undefined ? {} : { code: event.code }),
          meta: event.meta,
          ctrl: event.ctrl,
          shift: event.shift,
          inEditable: event.inEditable,
        })
      : keyboardForwardDecision(
          {
            key: event.key,
            ...(event.code === undefined ? {} : { code: event.code }),
            meta: event.meta,
            ctrl: event.ctrl,
            shift: event.shift,
            inEditable: event.inEditable,
          },
          options.actionMap,
        )

  // 2. If the gate does not forward, the live handler returns early and NO
  //    message is queued — the reducer never runs. The model is unchanged.
  if (!decision.forward) {
    return {
      forwarded: false,
      preventDefault: decision.preventDefault,
      dispatched: false,
      intent: NO_INTENT,
      nextModel: model,
      visualization: verseSceneVisualization(model),
      spawnedSceneIds: model.verseSpawnedScenes.map((scene) => scene.sceneId),
      spawnedVizEntityIds: [],
      cracklingArcBeamCount: 0,
    }
  }

  // 3. The gate forwarded — build the SAME PressedKey the subscription queues and
  //    run the REAL reducer. interpretKey is reported for visibility (the reducer
  //    also calls it internally; we surface it so a test can assert the mapping).
  const intent = interpretKey(model, event)
  const pressed =
    event.code === undefined
      ? PressedKey({
          key: event.key,
          meta: event.meta,
          ctrl: event.ctrl,
          shift: event.shift,
          inEditable: event.inEditable,
        })
      : PressedKey({
          key: event.key,
          code: event.code,
          meta: event.meta,
          ctrl: event.ctrl,
          shift: event.shift,
          inEditable: event.inEditable,
        })
  const [nextModel] = update(model, pressed)

  // 4. Project the REAL visualization the renderer consumes.
  const visualization = verseSceneVisualization(nextModel)
  const entities = visualization.entities ?? []
  const beams = visualization.beams ?? []

  return {
    forwarded: true,
    preventDefault: decision.preventDefault,
    dispatched: true,
    intent,
    nextModel,
    visualization,
    spawnedSceneIds: nextModel.verseSpawnedScenes.map((scene) => scene.sceneId),
    spawnedVizEntityIds: entities
      .map((entity) => entity.id)
      .filter((id) => id.startsWith(VERSE_SPAWNED_SCENE_NODE_PREFIX)),
    cracklingArcBeamCount: beams.filter((beam) => beam.style === "crackling_arc")
      .length,
  }
}
