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

import { setEmit } from "./bridge"
import {
  EndedPaneDrag,
  MovedPaneDragPointer,
  PressedKey,
  type Message,
} from "./message"
import type { Model } from "./model"

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
// We translate only the keys the reducer cares about (Cmd/Ctrl-K, Cmd/Ctrl-1..5,
// Cmd/Ctrl-Enter, Escape, Arrow Up/Down, Enter, j/k) and `preventDefault` those
// so the webview never swallows them; everything else passes through untouched.
// `inEditable` reflects focus so the reducer can ignore bare nav keys mid-typing.
// Digit keys 1..9 are forwarded both as Cmd/Ctrl chords (group jump) AND, for
// HUD H1 (#5499), as BARE hotbar-slot hotkeys outside an editable field. Cover
// the full 1..9 range so a future 6th+ nav group automatically gets its slot
// hotkey without another edit here (the registry stays the source of truth).
const DIGIT_KEYS = ["1", "2", "3", "4", "5", "6", "7", "8", "9"] as const
const KEYBOARD_KEYS = new Set([
  "k",
  ...DIGIT_KEYS,
  "Enter",
  "Escape",
  "ArrowUp",
  "ArrowDown",
  "j",
])
const isDigitKey = (key: string): boolean => key.length === 1 && key >= "1" && key <= "9"

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
        // Only consider keys the reducer might act on (cheap pre-filter). A bare
        // letter (j/k) is a candidate too, but only acted on outside inputs.
        const modified = meta || ctrl
        // A bare 1..9 outside an input is the HUD H1 hotbar hotkey (#5499); a
        // bare j/k navigates sub-panes. Both are only candidates outside inputs.
        const bareNavKey =
          !inEditable && (key === "j" || key === "k" || isDigitKey(key))
        const isCandidate =
          KEYBOARD_KEYS.has(key) &&
          (modified || key === "Escape" || key === "ArrowUp" || key === "ArrowDown" ||
            key === "Enter" || bareNavKey)
        if (!isCandidate) return
        // Stop the webview from acting on shortcut chords (e.g. Cmd-K) and on
        // bare hotbar / sub-pane nav keys. Bare keys in an editable field are
        // left alone (so typing digits/letters in the text bar is unaffected).
        if (modified || bareNavKey) {
          event.preventDefault?.()
        }
        Queue.offerUnsafe(
          queue,
          PressedKey({ key, meta, ctrl, shift: event.shiftKey ?? false, inEditable }),
        )
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

export const subscriptions = Subscription.make<Model, Message>()(() => ({
  inbound: Subscription.persistent(inboundStream),
  // #5465: route window keydown into the reducer as PressedKey.
  keyboard: Subscription.persistent(keyboardStream),
  // HUD H3 (#5501): route window pointer move/up into the pane-layer drag reducer.
  paneDrag: Subscription.persistent(pointerStream),
}))
