// CL-53: the Foldkit subscriptions for the desktop webview.
//
// A single persistent inbound stream carries the Electrobun → runtime pushes
// (node-state / notifications). The Electroview message handlers (main.ts) call
// `pushInbound` (bridge.ts); here we register the matching emitter against an
// Effect `Stream.callback` so those pushes become Messages the runtime processes.
//
// Mirrors the web app idiom (apps/openagents.com/apps/web/src/subscriptions.ts):
// `Stream.callback<Message>(queue => Effect.acquireRelease(register, release))`
// where the registered resource offers messages into the queue with
// `Queue.offerUnsafe`. `Subscription.persistent` is the right primitive: the
// stream's lifecycle is independent of the Model (it runs for the whole app),
// exactly like the web app's route-independent listeners.

import { Effect, Queue, Schema as S, Stream } from "effect"
import { Subscription } from "foldkit"
import {
  openAgentsDefaultInputProfile,
  openAgentsInputActionMapFromProfile,
  openAgentsInputActionSpecById,
  resolveOpenAgentsKeyboardEventActionBindings,
  type OpenAgentsInputActionMap,
  type OpenAgentsInputContext,
} from "@openagentsinc/input-bindings"

import { setEmit } from "./bridge.js"
import {
  EndedPaneDrag,
  GotChatWorldMultiplayer,
  GotChatWorldPaymentParticle,
  GotChatWorldScene,
  MovedPaneDragPointer,
  PressedKey,
  TickedOnboardingStatusRefresh,
  TickedVerseTrainingProjectionRefresh,
  type Message,
} from "./message.js"
import type { Model } from "./model.js"
import {
  subscribePaymentParticles,
  subscribePylonScene,
  subscribeCloudflareWorld,
} from "./chat-world-subscriptions.js"
import {
  chatWorldBuildFlags,
  chatWorldCharacterId,
  chatWorldMultiplayerFlag,
} from "../shared/chat-world-flags.js"
import { activateVerseGameScreen } from "../shared/verse-game-screen.js"
import { modelVerseGameScreenActive } from "./model.js"

// The inbound push stream. We stash a queue-backed emitter in the bridge so the
// Electroview handlers can feed messages in; teardown clears it.
const inboundStream: Stream.Stream<Message> = Stream.callback<Message>((queue) =>
  Effect.acquireRelease(
    Effect.sync(() => {
      setEmit((message) => {
        Queue.offerUnsafe(queue, message)
      })
      return { released: false }
    }),
    () => Effect.sync(() => setEmit(null)),
  ).pipe(Effect.flatMap(() => Effect.never)),
)

// #5465/#5946: the keyboard shortcut layer. A persistent `keydown` listener on
// the webview window turns key presses into the existing raw `PressedKey`
// compatibility message; the PURE reducer (update.ts) still decides what that
// means against the active pane + palette state.
//
// The important change from the old path is that forwarding is now driven by
// the shared input action profile rather than a hand-written raw-key whitelist.
// That keeps the real DOM path aligned with the tested action catalog; for
// example, Cmd/Ctrl-Shift-V now resolves because `hud.toggle_code_overlay` is in
// the profile instead of hoping `"v"` was remembered in a static Set.
const ACTION_BAR_SLOT_ACTION_IDS = Array.from(
  { length: 10 },
  (_, index) => `action_bar.slot_${index + 1}`,
)

const isActionBarSlotActionId = (actionId: string): boolean =>
  actionId.startsWith("action_bar.slot_")

// The hotbar slots wired to a real Verse effect (keyboard.ts + view.ts): slot 1
// coder session, slot 2 spawn scene, slot 3 toggle portal. ONLY these fire from
// (and are swallowed in) a focused field — the empty slots 4-10 stay normal
// keystrokes so a bare `4`-`0` still types into the Ask box.
const WIRED_ACTION_BAR_SLOT_ACTION_IDS = new Set([
  "action_bar.slot_1",
  "action_bar.slot_2",
  "action_bar.slot_3",
])

const isWiredActionBarSlotActionId = (actionId: string): boolean =>
  WIRED_ACTION_BAR_SLOT_ACTION_IDS.has(actionId)

const DESKTOP_SHORTCUT_ACTION_IDS = new Set([
  ...ACTION_BAR_SLOT_ACTION_IDS,
  "app.command_palette",
  "app.submit",
  "app.pane_next",
  "app.pane_previous",
  "hud.toggle_code_overlay",
  // Dev affordance (#6033 / #6041): ⌘⇧E spawn / ⌘⇧P portal in the live Verse.
  // These must be in the desktop forward allowlist so the keyboard subscription
  // forwards them into the reducer (interpretKey already maps them). Without
  // this they resolved to no action ids → forward=false → the keys were dropped
  // before interpretKey ran, which is exactly why #6041's keys did nothing.
  "verse.spawn_scene",
  "verse.toggle_scene_portal",
  "palette.close",
  "palette.run",
  "palette.move_up",
  "palette.move_down",
])

const PALETTE_ACTION_IDS = new Set([
  "palette.close",
  "palette.run",
  "palette.move_up",
  "palette.move_down",
])

const desktopDefaultInputActionMap = openAgentsInputActionMapFromProfile(
  openAgentsDefaultInputProfile,
)

export const keyboardForwardDecision = (input: {
  readonly key: string
  readonly code?: string
  readonly meta: boolean
  readonly ctrl: boolean
  readonly shift?: boolean
  readonly alt?: boolean
  readonly inEditable: boolean
  // True only when the focused editable IS the in-world Verse Ask box (the
  // `.verse-khala-input`). The hotbar-focus fix is scoped to it so a bare slot
  // digit is never swallowed from any OTHER field (composer/terminal/palette),
  // where typing `1`/`2`/`3` must still insert the digit.
  readonly inVerseAskInput?: boolean
}, actionMap: OpenAgentsInputActionMap = desktopDefaultInputActionMap): {
  readonly forward: boolean
  readonly preventDefault: boolean
} => {
  const key = input.key
  const actionIds = resolveDesktopKeyboardActionIds(input, actionMap)
  if (input.inEditable) {
    const paletteEditingAction = actionIds.some((actionId) =>
      PALETTE_ACTION_IDS.has(actionId)
    )
    // #6045 follow-up (hotbar-focus bug): the Verse hotbar number keys are the
    // MMO action bar. The owner hit a bug where, with the in-world Ask box
    // focused, a bare `2`/`3` typed a digit and the hotbar "did nothing" (editable
    // focus dropped every non-palette key here). The wired hotbar slots are
    // dedicated game keys in the verse, so when the focused field IS the Verse Ask
    // box we FORWARD + preventDefault them: preventDefault stops the digit from
    // polluting the box, and forwarding lets the reducer fire the slot (the
    // matching `interpretKey` change resolves the slot intent in-editable). This
    // is scoped to the Ask box via `inVerseAskInput`, so a `1`/`2`/`3` in any
    // other field still types normally.
    const editableWiredSlot =
      input.inVerseAskInput === true &&
      actionIds.some(isWiredActionBarSlotActionId)
    if (editableWiredSlot) {
      return { forward: true, preventDefault: true }
    }
    return {
      forward: paletteEditingAction,
      preventDefault: paletteEditingAction && key === "Escape",
    }
  }
  const modifiedShortcut = actionIds.some((actionId) =>
    actionId === "app.command_palette" ||
    actionId === "app.submit" ||
    actionId === "hud.toggle_code_overlay" ||
    actionId === "verse.spawn_scene" ||
    actionId === "verse.toggle_scene_portal"
  )
  const escapeKey = actionIds.includes("palette.close")
  const bareNavKey = actionIds.some((actionId) =>
    actionId === "app.pane_next" || actionId === "app.pane_previous"
  )
  const actionBarSlot = actionIds.some(isActionBarSlotActionId)
  const forward = actionIds.length > 0
  return {
    forward,
    preventDefault: forward && (modifiedShortcut || escapeKey || bareNavKey || actionBarSlot),
  }
}

const resolveDesktopKeyboardActionIds = (
  input: {
    readonly key: string
    readonly code?: string
    readonly meta: boolean
    readonly ctrl: boolean
    readonly shift?: boolean
    readonly alt?: boolean
    readonly inEditable: boolean
  },
  actionMap: OpenAgentsInputActionMap,
): ReadonlyArray<string> => {
  const activeContexts = desktopKeyboardContexts(input.inEditable)
  const event = input.code === undefined
    ? {
      key: input.key,
      metaKey: input.meta,
      ctrlKey: input.ctrl,
      shiftKey: input.shift === true,
      altKey: input.alt === true,
    }
    : {
      key: input.key,
      code: input.code,
      metaKey: input.meta,
      ctrlKey: input.ctrl,
      shiftKey: input.shift === true,
      altKey: input.alt === true,
    }
  return resolveOpenAgentsKeyboardEventActionBindings(
    actionMap,
    event,
    { allowExtraModifiers: false },
  )
    .map((match) => match.actionId)
    .filter((actionId) => DESKTOP_SHORTCUT_ACTION_IDS.has(actionId))
    .filter((actionId) => {
      const spec = openAgentsInputActionSpecById.get(actionId)
      return spec?.contexts.some((context) => activeContexts.includes(context)) === true
    })
}

const desktopKeyboardContexts = (
  inEditable: boolean,
): ReadonlyArray<OpenAgentsInputContext> =>
  inEditable
    ? // While a field is focused we resolve the command-palette context (Escape /
      // Enter / arrows drive the palette) AND `verse_explore` — but ONLY so the
      // hotbar action-bar slots resolve here. `keyboardForwardDecision`'s editable
      // branch forwards just the action-bar slot keys (everything else stays a
      // normal keystroke), so adding verse_explore does NOT leak other Verse keys
      // into focused fields; it only lets the hotbar fire with the Ask box focused
      // (the hotbar-focus bug).
      ["command_palette", "verse_explore"]
    : [
        "global",
        "managed_pane",
        "command_palette",
        "verse_explore",
        "verse_code_overlay",
      ]

const isEditableTarget = (target: EventTarget | null): boolean => {
  const el = target as { tagName?: string; isContentEditable?: boolean } | null
  if (!el) return false
  if (el.isContentEditable) return true
  const tag = (el.tagName ?? "").toLowerCase()
  return tag === "input" || tag === "textarea" || tag === "select"
}

// True only when the focused field is the in-world Verse Ask box. The hotbar
// keys fire (and swallow their digit) from this field only — never from the
// composer, terminal, palette, or any other input.
const isVerseAskInputTarget = (target: EventTarget | null): boolean => {
  const el = target as { classList?: { contains?: (c: string) => boolean } } | null
  return el?.classList?.contains?.("verse-khala-input") === true
}

const keyboardStream: Stream.Stream<Message> = Stream.callback<Message>((queue) =>
  Effect.acquireRelease(
    Effect.sync(() => {
      const target = globalThis as unknown as {
        addEventListener?: (t: string, h: (e: unknown) => void, o?: unknown) => void
        removeEventListener?: (t: string, h: (e: unknown) => void) => void
      }
      const handler = (raw: unknown): void => {
        const event = raw as {
          key?: string
          code?: string
          metaKey?: boolean
          ctrlKey?: boolean
          shiftKey?: boolean
          target?: EventTarget | null
          preventDefault?: () => void
          stopPropagation?: () => void
          stopImmediatePropagation?: () => void
        }
        const key = event.key ?? ""
        const code = event.code
        const meta = event.metaKey ?? false
        const ctrl = event.ctrlKey ?? false
        const inEditable = isEditableTarget(event.target ?? null)
        const inVerseAskInput = isVerseAskInputTarget(event.target ?? null)
        const shortcutInput = code === undefined
          ? {
              key,
              meta,
              ctrl,
              shift: event.shiftKey ?? false,
              inEditable,
            }
          : {
              key,
              code,
              meta,
              ctrl,
              shift: event.shiftKey ?? false,
              inEditable,
            }
        // Only forward actions the reducer might act on. Native edit/movement
        // commands (Cmd-C/V/X/A/Z, Cmd-arrow, etc.) keep reaching WebKit/AppKit.
        // `inVerseAskInput` is passed to the gate only (it scopes the hotbar-focus
        // swallow to the Ask box); the PressedKey message keeps its existing shape.
        const decision = keyboardForwardDecision({
          ...shortcutInput,
          inVerseAskInput,
        })
        if (!decision.forward) return
        if (decision.preventDefault) {
          event.preventDefault?.()
          event.stopPropagation?.()
          event.stopImmediatePropagation?.()
        }
        Queue.offerUnsafe(
          queue,
          PressedKey(shortcutInput),
        )
        // Palette focus management. The reducer toggles the command palette on
        // Cmd/Ctrl-K (and closes it on Escape / after running a command via
        // Enter), but `autofocus` doesn't fire on a dynamically-mounted input and
        // the subscriptions are static (not model-reactive), so we move focus
        // here: after the reducer re-renders, focus the palette input if it's now
        // open, otherwise return focus to the shell chat input. Runs only on the
        // keys that open/close the palette; querying after a frame so the DOM has
        // updated. (No-op where the targets don't exist, e.g. inside a pane.)
        const modified = meta || ctrl
        const affectsPalette =
          (key.toLowerCase() === "k" && modified) ||
          key === "Escape" ||
          key === "Enter"
        if (affectsPalette && typeof requestAnimationFrame === "function") {
          requestAnimationFrame(() => {
            const palette = document.querySelector(
              ".palette-input",
            ) as HTMLElement | null
            if (palette) {
              palette.focus()
              return
            }
            const shellInput = document.querySelector(
              ".shell-input",
            ) as HTMLElement | null
            shellInput?.focus()
          })
        }
      }
      target.addEventListener?.("keydown", handler)
      return { handler, target }
    }),
    ({ handler, target }) => Effect.sync(() => target.removeEventListener?.("keydown", handler)),
  ).pipe(Effect.flatMap(() => Effect.never)),
)

// HUD H3 (#5501): the managed pane-layer drag tracker. A title-bar / resize-handle
// pointerdown captures the gesture (StartedPaneDrag, in view.ts); the actual MOVE
// must be tracked at the WINDOW level so the pointer can leave the small handle
// without dropping the drag — exactly how Commander's `@use-gesture` drag worked
// (audit §4.6). A persistent `pointermove`/`pointerup` listener feeds the pure
// reducer: `MovedPaneDragPointer` only while a button is held (so idle pointer
// motion costs nothing), and `EndedPaneDrag` on release. The reducer no-ops both
// when no drag is in flight, so this is safe even with no panes open.
const pointerStream: Stream.Stream<Message> = Stream.callback<Message>((queue) =>
  Effect.acquireRelease(
    Effect.sync(() => {
      const target = globalThis as unknown as {
        addEventListener?: (t: string, h: (e: unknown) => void, o?: unknown) => void
        removeEventListener?: (t: string, h: (e: unknown) => void) => void
      }
      const onMove = (raw: unknown): void => {
        const event = raw as { clientX?: number; clientY?: number; buttons?: number }
        // Only forward motion while a button is held — that bounds the message
        // rate to an active drag. The reducer ignores it if no pane drag exists.
        if ((event.buttons ?? 0) === 0) return
        Queue.offerUnsafe(
          queue,
          MovedPaneDragPointer({ pointerX: event.clientX ?? 0, pointerY: event.clientY ?? 0 }),
        )
      }
      const onUp = (): void => {
        Queue.offerUnsafe(queue, EndedPaneDrag())
      }
      target.addEventListener?.("pointermove", onMove)
      target.addEventListener?.("pointerup", onUp)
      // A cancelled gesture (window blur / pointer capture loss) also ends it.
      target.addEventListener?.("pointercancel", onUp)
      return { target, onMove, onUp }
    }),
    ({ target, onMove, onUp }) =>
      Effect.sync(() => {
        target.removeEventListener?.("pointermove", onMove)
        target.removeEventListener?.("pointerup", onUp)
        target.removeEventListener?.("pointercancel", onUp)
      }),
  ).pipe(Effect.flatMap(() => Effect.never)),
)

// P2.5 chat-world wiring (#5730): connect the P1/P2 live feeds (#5736/#5737)
// into the reducer. The chat-world scene mounts/unmounts with the chat pane, but
// the desktop runtime owns ONE persistent inbound stream per concern (mirroring
// `inbound`/`keyboard`/`paneDrag`), so we model each chat-world feed as a
// persistent Foldkit subscription whose acquire calls the hook and whose release
// calls the returned unsubscribe() — the same mount/unmount contract the old
// TODO described, expressed in Effect acquireRelease.
//
// Both feeds are FLAG-GATED on the SAME build flags the view uses
// (chatWorldBuildFlags), passed explicitly into the hooks so there is no hidden
// global. With the flags OFF the hooks return noop immediately (no fetch, no
// EventSource, no timers), so the only cost of registering them is two acquired
// resources that do nothing — and the reducer never sees a chat-world message,
// keeping the flag-OFF model + view byte-identical to current main.
//
// Evidence-bound (§5): subscribePaymentParticles only ever dispatches particles
// that carry a real sourceRef (activityEventToParticle drops the rest), so every
// GotChatWorldPaymentParticle the reducer stores is clickable to a real receipt.

const pylonSceneStream: Stream.Stream<Message> = Stream.callback<Message>((queue) =>
  Effect.acquireRelease(
    Effect.sync(() =>
      subscribePylonScene(
        (scene) => Queue.offerUnsafe(queue, GotChatWorldScene({ scene })),
        { flags: chatWorldBuildFlags() },
      ),
    ),
    (unsubscribe) => Effect.sync(() => unsubscribe()),
  ).pipe(Effect.flatMap(() => Effect.never)),
)

const paymentParticleStream: Stream.Stream<Message> = Stream.callback<Message>((queue) =>
  Effect.acquireRelease(
    Effect.sync(() =>
      subscribePaymentParticles(
        (particle) =>
          Queue.offerUnsafe(queue, GotChatWorldPaymentParticle({ particle })),
        { flags: chatWorldBuildFlags() },
      ),
    ),
    (unsubscribe) => Effect.sync(() => unsubscribe()),
  ).pipe(Effect.flatMap(() => Effect.never)),
)

const chatWorldSubscriptionFlags = () => ({
  ...chatWorldBuildFlags(),
  CHAT_WORLD_MULTIPLAYER: chatWorldMultiplayerFlag(),
})

const cloudflareWorldStream: Stream.Stream<Message> = Stream.callback<Message>((queue) =>
  Effect.acquireRelease(
    Effect.sync(() =>
      subscribeCloudflareWorld(
        (world) => Queue.offerUnsafe(queue, GotChatWorldMultiplayer({ world })),
        // Pass the character resolver LAZILY (a getter), not an eager string.
        // The Bun host injects globalThis.__OA_CHARACTER (chatWorldCharacterId
        // reads it) and the dom-ready injection may land after this subscription
        // mounts; resolving at join/move time — after the async Cloudflare world
        // connect — makes the value deterministic regardless of inject timing.
        { flags: chatWorldSubscriptionFlags(), characterId: () => chatWorldCharacterId() },
      ),
    ),
    (unsubscribe) => Effect.sync(() => unsubscribe()),
  ).pipe(Effect.flatMap(() => Effect.never)),
)

export const verseTrainingProjectionRefreshMs = 10_000

export const onboardingStatusRefreshMs = 5_000

const onboardingStatusRefreshStream: Stream.Stream<Message> = Stream.callback<Message>((queue) =>
  Effect.acquireRelease(
    Effect.sync(() => {
      const handle = globalThis.setInterval(
        () => Queue.offerUnsafe(queue, TickedOnboardingStatusRefresh()),
        onboardingStatusRefreshMs,
      )
      return handle
    }),
    (handle) => Effect.sync(() => globalThis.clearInterval(handle)),
  ).pipe(Effect.flatMap(() => Effect.never)),
)

const verseTrainingProjectionStream: Stream.Stream<Message> = Stream.callback<Message>((queue) =>
  Effect.acquireRelease(
    Effect.sync(() => {
      const handle = globalThis.setInterval(
        () => Queue.offerUnsafe(queue, TickedVerseTrainingProjectionRefresh()),
        verseTrainingProjectionRefreshMs,
      )
      return handle
    }),
    (handle) => Effect.sync(() => globalThis.clearInterval(handle)),
  ).pipe(Effect.flatMap(() => Effect.never)),
)

// M8 "playable-in-our-world": a Model-keyed lifecycle subscription. While
// `verseGameScreenActive` is true, the iframe game host is booted, the game is
// started, and a window keydown forwarder feeds game keys into it; when it flips
// false the host is torn down. The stream emits NO messages — it owns a DOM/iframe
// side effect via acquireRelease, gated on the dependency. (No-op headless without
// a DOM: `activateVerseGameScreen` returns an inert teardown.)
const verseGameScreenStream = (active: boolean): Stream.Stream<Message> =>
  !active
    ? Stream.empty
    : Stream.callback<Message>(() =>
        Effect.acquireRelease(
          Effect.sync(() => activateVerseGameScreen()),
          (teardown) => Effect.sync(() => teardown()),
        ).pipe(Effect.flatMap(() => Effect.never)),
      )

export const subscriptions = Subscription.make<Model, Message>()((entry) => ({
  inbound: Subscription.persistent(inboundStream),
  // #5465: route window keydown into the reducer as PressedKey.
  keyboard: Subscription.persistent(keyboardStream),
  // HUD H3 (#5501): route window pointer move/up into the pane-layer drag reducer.
  paneDrag: Subscription.persistent(pointerStream),
  // #5730: live pylon scene + Bitcoin payment particles behind chat. Both noop
  // (no I/O) unless their build flag is on, so flag-OFF behavior is unchanged.
  chatWorldScene: Subscription.persistent(pylonSceneStream),
  chatWorldPayments: Subscription.persistent(paymentParticleStream),
  chatWorldCloudflare: Subscription.persistent(cloudflareWorldStream),
  onboardingStatusRefresh: Subscription.persistent(onboardingStatusRefreshStream),
  verseTrainingProjection: Subscription.persistent(verseTrainingProjectionStream),
  // M8: boot/teardown the in-world Khala crossy-road arcade screen with its toggle.
  verseGameScreen: entry(
    { active: S.Boolean },
    {
      modelToDependencies: (model) => ({
        active: modelVerseGameScreenActive(model),
      }),
      dependenciesToStream: ({ active }) => verseGameScreenStream(active),
    },
  ),
}))
