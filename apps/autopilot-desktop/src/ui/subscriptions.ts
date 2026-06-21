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

import { Effect, Queue, Stream } from "effect"
import { Subscription } from "foldkit"

import { setEmit } from "./bridge.js"
import {
  EndedPaneDrag,
  GotChatWorldMultiplayer,
  GotChatWorldPaymentParticle,
  GotChatWorldScene,
  MovedPaneDragPointer,
  PressedKey,
  type Message,
} from "./message.js"
import type { Model } from "./model.js"
import {
  subscribePaymentParticles,
  subscribePylonScene,
  subscribeSpacetimeWorld,
} from "./chat-world-subscriptions.js"
import {
  chatWorldBuildFlags,
  chatWorldMultiplayerFlag,
} from "../shared/chat-world-flags.js"

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

// #5465: the keyboard shortcut layer. A persistent `keydown` listener on the
// webview window turns each key press into a single raw `PressedKey` message;
// the PURE reducer (update.ts) decides what it means against the active pane +
// palette state, so the whole shortcut layer is unit-testable without a DOM.
//
// We translate only the keys the reducer cares about (Cmd/Ctrl-K,
// Cmd/Ctrl-Enter, Escape, Arrow Up/Down, Enter, j/k) and `preventDefault` those
// so the webview never swallows them; everything else passes through untouched.
// `inEditable` reflects focus so the reducer can ignore bare nav keys mid-typing.
// The hotbar's numbered cells are intentionally inert, so digit keys are not
// captured here.
const KEYBOARD_KEYS = new Set([
  "k",
  "Enter",
  "Escape",
  "ArrowUp",
  "ArrowDown",
  "j",
])

export const keyboardForwardDecision = (input: {
  readonly key: string
  readonly meta: boolean
  readonly ctrl: boolean
  readonly inEditable: boolean
}): { readonly forward: boolean; readonly preventDefault: boolean } => {
  const modified = input.meta || input.ctrl
  const key = input.key
  const modifiedShortcut = modified && (
    key.toLowerCase() === "k" || key === "Enter"
  )
  const escapeKey = key === "Escape"
  const paletteKey = !modified && (
    key === "ArrowUp" || key === "ArrowDown" || key === "Enter"
  )
  const bareNavKey = !modified && !input.inEditable && (key === "j" || key === "k")
  const forward =
    KEYBOARD_KEYS.has(key) && (modifiedShortcut || escapeKey || paletteKey || bareNavKey)
  return {
    forward,
    preventDefault: forward && (modifiedShortcut || escapeKey || bareNavKey),
  }
}

const isEditableTarget = (target: EventTarget | null): boolean => {
  const el = target as { tagName?: string; isContentEditable?: boolean } | null
  if (!el) return false
  if (el.isContentEditable) return true
  const tag = (el.tagName ?? "").toLowerCase()
  return tag === "input" || tag === "textarea" || tag === "select"
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
          metaKey?: boolean
          ctrlKey?: boolean
          shiftKey?: boolean
          target?: EventTarget | null
          preventDefault?: () => void
        }
        const key = event.key ?? ""
        const meta = event.metaKey ?? false
        const ctrl = event.ctrlKey ?? false
        const inEditable = isEditableTarget(event.target ?? null)
        // Only forward keys the reducer might act on. Modified keys are limited
        // to Cmd/Ctrl-K and Cmd/Ctrl-Enter so native edit/movement commands
        // (Cmd-C/V/X/A/Z, Cmd-arrow, etc.) keep reaching WebKit/AppKit.
        const decision = keyboardForwardDecision({ key, meta, ctrl, inEditable })
        if (!decision.forward) return
        if (decision.preventDefault) {
          event.preventDefault?.()
        }
        Queue.offerUnsafe(
          queue,
          PressedKey({ key, meta, ctrl, shift: event.shiftKey ?? false, inEditable }),
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

const spacetimeWorldStream: Stream.Stream<Message> = Stream.callback<Message>((queue) =>
  Effect.acquireRelease(
    Effect.sync(() =>
      subscribeSpacetimeWorld(
        (world) => Queue.offerUnsafe(queue, GotChatWorldMultiplayer({ world })),
        { flags: chatWorldSubscriptionFlags() },
      ),
    ),
    (unsubscribe) => Effect.sync(() => unsubscribe()),
  ).pipe(Effect.flatMap(() => Effect.never)),
)

export const subscriptions = Subscription.make<Model, Message>()(() => ({
  inbound: Subscription.persistent(inboundStream),
  // #5465: route window keydown into the reducer as PressedKey.
  keyboard: Subscription.persistent(keyboardStream),
  // HUD H3 (#5501): route window pointer move/up into the pane-layer drag reducer.
  paneDrag: Subscription.persistent(pointerStream),
  // #5730: live pylon scene + Bitcoin payment particles behind chat. Both noop
  // (no I/O) unless their build flag is on, so flag-OFF behavior is unchanged.
  chatWorldScene: Subscription.persistent(pylonSceneStream),
  chatWorldPayments: Subscription.persistent(paymentParticleStream),
  chatWorldSpacetime: Subscription.persistent(spacetimeWorldStream),
}))
