// Dialog stack + toasts for the Pylon TUI (issue #4738), following the shape
// of opencode's ui/dialog.tsx: a stack store, a centered modal over a dimmed
// overlay, promise-based open* APIs, focus capture while open, and focus
// restore on close. Editing keys inside an open dialog (escape/return/arrows/
// typing) are component-local, like textarea editing; every app-level action
// stays in the keymap command registry (commands.tsx).

import type { InputRenderable, Renderable } from "@opentui/core"
import { For, Show, createMemo, createSignal } from "solid-js"
import { createStore, produce } from "solid-js/store"
import { theme } from "./theme"

export type SelectItem = {
  id: string
  label: string
  detail?: string
}

type DialogSpec =
  | { kind: "alert"; title: string; body: string; resolve: () => void }
  | {
      kind: "confirm"
      title: string
      body: string
      confirmLabel: string
      resolve: (accepted: boolean) => void
    }
  | {
      kind: "prompt"
      title: string
      placeholder: string
      resolve: (value: string | null) => void
    }
  | {
      kind: "select"
      title: string
      items: SelectItem[]
      resolve: (id: string | null) => void
    }

type DialogEntry = DialogSpec & { id: number }

const [dialogStack, setDialogStack] = createStore<DialogEntry[]>([])
let nextDialogId = 1
let restoreFocus: (() => void) | null = null
let captureFocusedRenderable: (() => (() => void) | null) | null = null

// The view registers how to capture/restore focus (app.tsx wires this to
// the composer/scrollbox refs). Kept indirect so this module stays free of
// pane-specific knowledge.
export function registerDialogFocusHooks(hooks: { capture: () => (() => void) | null }) {
  captureFocusedRenderable = hooks.capture
}

export function dialogOpen(): boolean {
  return dialogStack.length > 0
}

function pushDialog(spec: DialogSpec): void {
  if (dialogStack.length === 0) {
    restoreFocus = captureFocusedRenderable?.() ?? null
  }
  setDialogStack(produce((stack) => stack.push({ ...spec, id: nextDialogId++ } as DialogEntry)))
}

function popDialog(): void {
  setDialogStack(produce((stack) => stack.pop()))
  if (dialogStack.length === 0) {
    restoreFocus?.()
    restoreFocus = null
  }
}

export function openAlert(options: { title: string; body: string }): Promise<void> {
  return new Promise((resolve) => {
    pushDialog({ kind: "alert", title: options.title, body: options.body, resolve })
  })
}

export function openConfirm(options: {
  title: string
  body: string
  confirmLabel?: string
}): Promise<boolean> {
  return new Promise((resolve) => {
    pushDialog({
      kind: "confirm",
      title: options.title,
      body: options.body,
      confirmLabel: options.confirmLabel ?? "Confirm",
      resolve,
    })
  })
}

export function openPrompt(options: { title: string; placeholder?: string }): Promise<string | null> {
  return new Promise((resolve) => {
    pushDialog({ kind: "prompt", title: options.title, placeholder: options.placeholder ?? "", resolve })
  })
}

export function openSelect(options: { title: string; items: SelectItem[] }): Promise<string | null> {
  return new Promise((resolve) => {
    pushDialog({ kind: "select", title: options.title, items: options.items, resolve })
  })
}

// Subsequence fuzzy match with a light score: contiguous prefix matches rank
// above scattered ones; non-matches are excluded.
export function fuzzyScore(query: string, candidate: string): number | null {
  const q = query.toLowerCase()
  const c = candidate.toLowerCase()
  if (!q) return 0
  let qi = 0
  let score = 0
  let streak = 0
  for (let ci = 0; ci < c.length && qi < q.length; ci += 1) {
    if (c[ci] === q[qi]) {
      qi += 1
      streak += 1
      score += streak
    } else {
      streak = 0
    }
  }
  return qi === q.length ? score : null
}

export function filterSelectItems(items: readonly SelectItem[], query: string): SelectItem[] {
  return items
    .map((item) => ({ item, score: fuzzyScore(query, `${item.label} ${item.detail ?? ""}`) }))
    .filter((entry): entry is { item: SelectItem; score: number } => entry.score !== null)
    .sort((a, b) => b.score - a.score)
    .map((entry) => entry.item)
}

// --- Toasts -----------------------------------------------------------------

export type Toast = { id: number; message: string; tone: "info" | "error" }

const [toasts, setToasts] = createStore<Toast[]>([])
let nextToastId = 1

export function showToast(message: string, tone: "info" | "error" = "info", durationMs = 4000): void {
  const id = nextToastId++
  setToasts(produce((list) => list.push({ id, message, tone })))
  setTimeout(() => {
    setToasts(produce((list) => {
      const index = list.findIndex((toast) => toast.id === id)
      if (index >= 0) list.splice(index, 1)
    }))
  }, durationMs)
}

export function resetDialogState(): void {
  setDialogStack([])
  setToasts([])
  restoreFocus = null
}

// --- Components -------------------------------------------------------------

function DialogFrame(props: { title: string; children: any; width?: number }) {
  return (
    <box
      position="absolute"
      left="15%"
      top={4}
      width={props.width ?? "70%"}
      zIndex={3000}
      border
      borderStyle="single"
      borderColor={theme.colors.border}
      title={` ${props.title} `}
      titleColor={theme.colors.title}
      flexDirection="column"
    >
      {props.children}
    </box>
  )
}

function AlertDialog(props: { entry: Extract<DialogEntry, { kind: "alert" }> }) {
  const close = () => {
    props.entry.resolve()
    popDialog()
  }
  return (
    <DialogFrame title={props.entry.title}>
      <text fg={theme.colors.text} width="100%">{` ${props.entry.body}`}</text>
      <input
        focused
        width="100%"
        height={1}
        onKeyDown={(key: any) => {
          if (key.name === "return" || key.name === "escape") {
            close()
            key.preventDefault?.()
            key.stopPropagation?.()
          }
        }}
      />
      <text fg={theme.colors.textMuted} width="100%">{" return/esc close"}</text>
    </DialogFrame>
  )
}

function ConfirmDialog(props: { entry: Extract<DialogEntry, { kind: "confirm" }> }) {
  const close = (accepted: boolean) => {
    props.entry.resolve(accepted)
    popDialog()
  }
  return (
    <DialogFrame title={props.entry.title}>
      <text fg={theme.colors.text} width="100%">{` ${props.entry.body}`}</text>
      <input
        focused
        width="100%"
        height={1}
        onKeyDown={(key: any) => {
          if (key.name === "y" || key.name === "return") {
            close(true)
          } else if (key.name === "n" || key.name === "escape") {
            close(false)
          } else {
            return
          }
          key.preventDefault?.()
          key.stopPropagation?.()
        }}
      />
      <text fg={theme.colors.textMuted} width="100%">
        {` y/return ${props.entry.confirmLabel.toLowerCase()} · n/esc cancel`}
      </text>
    </DialogFrame>
  )
}

function PromptDialog(props: { entry: Extract<DialogEntry, { kind: "prompt" }> }) {
  let current = ""
  const close = (value: string | null) => {
    props.entry.resolve(value)
    popDialog()
  }
  return (
    <DialogFrame title={props.entry.title}>
      <input
        ref={(renderable: InputRenderable) => {
          queueMicrotask(() => renderable.focus())
        }}
        focused
        width="100%"
        height={1}
        placeholder={props.entry.placeholder}
        onInput={(value: string) => {
          current = value
        }}
        onKeyDown={(key: any) => {
          if (key.name === "return") {
            close(current.trim() || null)
          } else if (key.name === "escape") {
            close(null)
          } else {
            return
          }
          key.preventDefault?.()
          key.stopPropagation?.()
        }}
      />
      <text fg={theme.colors.textMuted} width="100%">{" return accept · esc cancel"}</text>
    </DialogFrame>
  )
}

function SelectDialog(props: { entry: Extract<DialogEntry, { kind: "select" }> }) {
  const [query, setQuery] = createSignal("")
  const [cursor, setCursor] = createSignal(0)
  const filtered = createMemo(() => filterSelectItems(props.entry.items, query()))
  const close = (id: string | null) => {
    props.entry.resolve(id)
    popDialog()
  }
  const visible = createMemo(() => {
    const list = filtered()
    const index = Math.min(cursor(), Math.max(0, list.length - 1))
    const start = Math.max(0, index - 7)
    return { list: list.slice(start, start + 10), offset: start, index }
  })
  return (
    <DialogFrame title={props.entry.title}>
      <input
        ref={(renderable: InputRenderable) => {
          queueMicrotask(() => renderable.focus())
        }}
        focused
        width="100%"
        height={1}
        placeholder="type to filter..."
        onInput={(value: string) => {
          setQuery(value)
          setCursor(0)
        }}
        onKeyDown={(key: any) => {
          if (key.name === "return") {
            const selected = filtered()[Math.min(cursor(), filtered().length - 1)]
            close(selected ? selected.id : null)
          } else if (key.name === "escape") {
            close(null)
          } else if (key.name === "down") {
            setCursor((value) => Math.min(value + 1, Math.max(0, filtered().length - 1)))
          } else if (key.name === "up") {
            setCursor((value) => Math.max(value - 1, 0))
          } else {
            return
          }
          key.preventDefault?.()
          key.stopPropagation?.()
        }}
      />
      <For each={visible().list}>
        {(item, index) => (
          <text
            width="100%"
            fg={visible().offset + index() === visible().index ? theme.colors.accent : theme.colors.text}
          >
            {`${visible().offset + index() === visible().index ? " > " : "   "}${item.label}${item.detail ? `  ·  ${item.detail}` : ""}`}
          </text>
        )}
      </For>
      <Show when={filtered().length === 0}>
        <text fg={theme.colors.textMuted} width="100%">{"   no matches"}</text>
      </Show>
      <text fg={theme.colors.textMuted} width="100%">{" ↑/↓ select · return accept · esc cancel"}</text>
    </DialogFrame>
  )
}

export function DialogHost() {
  const top = createMemo(() => dialogStack[dialogStack.length - 1])
  return (
    <>
      <Show when={top()}>
        {(entry) => (
          <>
            <Show when={entry().kind === "alert"}>
              <AlertDialog entry={entry() as Extract<DialogEntry, { kind: "alert" }>} />
            </Show>
            <Show when={entry().kind === "confirm"}>
              <ConfirmDialog entry={entry() as Extract<DialogEntry, { kind: "confirm" }>} />
            </Show>
            <Show when={entry().kind === "prompt"}>
              <PromptDialog entry={entry() as Extract<DialogEntry, { kind: "prompt" }>} />
            </Show>
            <Show when={entry().kind === "select"}>
              <SelectDialog entry={entry() as Extract<DialogEntry, { kind: "select" }>} />
            </Show>
          </>
        )}
      </Show>
      <Show when={toasts.length > 0}>
        <box position="absolute" right={1} top={1} width={40} zIndex={3500} flexDirection="column">
          <For each={toasts}>
            {(toast) => (
              <text width="100%" fg={toast.tone === "error" ? theme.colors.error : theme.colors.accent}>
                {` ${toast.message}`}
              </text>
            )}
          </For>
        </box>
      </Show>
    </>
  )
}
