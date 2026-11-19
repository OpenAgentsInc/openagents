# 1. A text input on the GPU

> **Status: partly implemented.** The shared Coax chat screen module
> (`apps/shared/chat/screen.coax`), the primary desktop consumer
> (`apps/openagents-desktop/chat_screen.coax`), and the mobile consumer
> (`apps/openagents-ios/composer_screen.coax`) pass their `#test` checks for
> the `tree` / `type composer.input hello` / `state draft` / `counts` trace
> and duplicate-ID refusal on `cpu`, beside
> `openagents/examples/cases/composer-type.ops`. Remaining for native hosts
> (`macos-arm64`, `linux-x86_64`, `ios-simulator`, `ios-device`): wiring the
> standalone `composer.input` screen into the desktop and iOS host shells and
> running their native accessibility UI tests.

## Premise

This is the first chapter, so there is nothing before it. You build the
smallest OpenAgents app that runs: one screen grounded in the primary
desktop app (`apps/openagents-desktop`)—and consumed by mobile
(`apps/openagents-ios`) from the same shared Coax module—with one text
input, the chat page's composer, drawn on the GPU by the shared component
kit. You can type into it with the keyboard, accessibility can find it, and
`coax drive` can type into it. Nothing else is in the app yet.

## Draft

The book is written first and the app is built to it
([README](README.md#book-driven-development)). So the app starts as small
as a chapter can make it, and each chapter adds one thing you can see.
Chapter 1 adds a text input.

The app is compiled Coax, linked into the native desktop binary (and the
mobile binary from the same shared module). There is no JavaScript, no
JavaScriptCore, no Wasm and no over-the-air update. Desktop is the primary
host from chapter 1 onward, with [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax)
keeping the screen logic shared across desktop and mobile.

The screen is one Metal view. The shared display-list kit in
[`components/`](../../components) draws every component on it
([GPU components on native](../../components/gpu-components.md)). The
platform supplies only three things a GPU surface can't: the editor of the
focused text field (the keyboard, input methods, AutoFill), the
accessibility tree, and system sheets. This chapter uses the first two.

One frame on the screen goes through four pieces, all on main today
(stage G0):

| Piece | What it does | File |
| --- | --- | --- |
| Frame context, `UiFrame` | Carries the sheet, the text measure, the skin, the input snapshot and the clock for one frame | [`components/kit/frame.coax`](../../components/kit/frame.coax) |
| Keyed state, `KeyedStore` | Keeps state that outlives a frame (entry motion, caret) under a stable ID | [`components/core/keyed.coax`](../../components/core/keyed.coax) |
| Semantic table, `ElTable` | One row per thing you can press, type into or read: ID, role, label, state, value, bounds, action | [`components/core/elements.coax`](../../components/core/elements.coax) |
| Surface commit, `UiSurface` | Commits the display list, the hit regions and the rows together, or refuses the frame and keeps the last one | [`components/paint/surface.coax`](../../components/paint/surface.coax) |

The composer is a component call with a stable ID, `composer.input`. It
measures itself, paints its frame, records one semantic row with the role
of a text field and the label "Message", and records its hit region over
the same box. It never calls the host and never changes app state.

When you tap the field, the host places the platform editor over the
row's committed bounds and makes it first responder. While the field has
focus, the editor draws the text, the placeholder and the caret in the
skin's face and colours, and the GPU paints only the field's frame. There
is one editing authority at a time: the platform owns the composition of an
input method, and the edit buffer in
[`components/core/edit.coax`](../../components/core/edit.coax) receives the
whole value after each edit. Two carets or the same glyphs drawn twice are a
defect.

The GPU login screen already works this way. Its native side,
[`NativeLogin.swift`](../../apps/shared/native/NativeLogin.swift), keeps one
`UITextField`, places it over the focused row and sends each edit's value
to the screen ([G1 as built](../../components/gpu-components.md#g1-as-built)).
This chapter reuses that overlay for the composer instead of writing a
second one.

Accessibility comes from the same rows. The host builds one
`UIAccessibilityElement` for each committed row and hides the Metal view
from VoiceOver. Activating an element sends the same action as a tap.

`coax drive` reads the same rows too. `tree` prints them, `type
composer.input TEXT` types into the field through the same path as the
editor, and `state draft` prints what the app holds. A host that can't do an
operation refuses it by name.

A still screen sleeps. With nothing typed and no motion due, no frame runs
and nothing is submitted to the GPU.

## What we build

Implemented on `cpu` in [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax), consumed by the primary desktop app ([`apps/openagents-desktop/chat_screen.coax`](../../apps/openagents-desktop/chat_screen.coax)) and the mobile app ([`apps/openagents-ios/composer_screen.coax`](../../apps/openagents-ios/composer_screen.coax)):

- The shared compiled Coax chat screen with a GPU composer surface, consumed
  by desktop as the primary host and by mobile as a second native host.
- The composer as a frame-context component with ID `composer.input`, its
  row and its hit region.
- The platform editor overlay on the focused field, shared with the login
  screen.
- One accessibility element per committed row.
- The driver operations `tree`, `type`, `state` and `counts` against the
  screen.

It builds on what exists today:

- [`components/gpu-components.md`](../../components/gpu-components.md)
- [`components/rendering.md`](../../components/rendering.md)
- [`components/kit/frame.coax`](../../components/kit/frame.coax)
- [`components/kit/form.coax`](../../components/kit/form.coax) (`ctx_field`)
- [`components/zeron/composer.coax`](../../components/zeron/composer.coax)
- [`components/core/edit.coax`](../../components/core/edit.coax)
- [`apps/shared/login/screen.coax`](../../apps/shared/login/screen.coax)
- [`apps/shared/native/NativeLogin.swift`](../../apps/shared/native/NativeLogin.swift)
- [`apps/shared/host/apple/ui-scene.m`](../../apps/shared/host/apple/ui-scene.m)
- [`apps/openagents-desktop`](../../apps/openagents-desktop)
- [`apps/openagents-ios`](../../apps/openagents-ios)

### The chapter's check

- **Source:** [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax),
  [`apps/openagents-desktop/chat_screen.coax`](../../apps/openagents-desktop/chat_screen.coax),
  [`apps/openagents-ios/composer_screen.coax`](../../apps/openagents-ios/composer_screen.coax),
  and [`openagents/examples/cases/composer-type.ops`](../examples/cases/composer-type.ops).
- **Trace:** `tree` prints `composer.input field "Message"`, with its bounds;
  `type composer.input hello` answers `typed`; `state draft` prints `hello`;
  `counts` prints `refused 0`.
- **Refusal:** a frame that records two rows with the ID `composer.input`
  is refused: the surface keeps its last committed frame and `counts`
  prints the refusal.
- **Independent check:** native accessibility UI tests on desktop and iOS
  that find the field through the accessibility tree and type into it with
  the system keyboard, with no `coax drive`, and read back the same value.
- **Commands:** `coax/bin/coax test apps/shared/chat/screen.coax`,
  `coax/bin/coax test apps/openagents-desktop/chat_screen.coax`, and
  `coax/bin/coax test apps/openagents-ios/composer_screen.coax`.

Implemented paths: [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax), [`apps/openagents-desktop/chat_screen.coax`](../../apps/openagents-desktop/chat_screen.coax), [`apps/openagents-ios/composer_screen.coax`](../../apps/openagents-ios/composer_screen.coax), [`openagents/examples/cases/composer-type.ops`](../examples/cases/composer-type.ops). The primary app is
[`apps/openagents-desktop`](../../apps/openagents-desktop), mobile consumes the
same shared logic in [`apps/openagents-ios`](../../apps/openagents-ios), and
components extend [`components/`](../../components).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, the same agent behind every surface
  - [Episode 196, *Ditch the TUI*](../../docs/transcripts/196.md): one interface for every device
- **Old code at `coder-v1.0.0`, crates:** `openagents-ui`
- **Also:**
  - [GPU components on native](../../components/gpu-components.md), stages G0 and G1
  - [Coax Native](../../coax/native.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1

## Open questions for the owner

1. Does the first TestFlight build of the lean app ship with only this
   chapter's text input, or does it wait until chapter 4 gives it a reply?
