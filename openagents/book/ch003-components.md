# 3. Components

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 2's app has state and no face. This chapter draws it with the one
component system the repository already has, and makes every element
something you can find by ID, read aloud and press.

## Draft

The app does not write its own buttons. [`components/`](../../components)
holds one set of components for every app, in layers: `core/` for state and
rules (text, the edit buffer, the one Markdown parser), `layout/` for
measuring, `paint/` and `skin/` for looks, `backend/` for drawing, and `kit/`
for the components themselves ([the components spec](../../components/spec.md)).
This book uses them. When one falls short, the fix goes into `components/`,
under [the reuse rule](../../docs/reuse.md).

A view reads committed state and builds elements. It never starts a request,
reads a clock or changes state. Each element has a stable ID, a role, a label
that a screen reader speaks, its state, the action it sends and its parent.
The element table
([`components/core/elements.coax`](../../components/core/elements.coax))
records them as the screen lays out, and `coax drive tree` prints them:

```text
account.email field "Email" at 24 180 size 342x44
account.submit button "Send code" at 24 236 size 342x44
```

The same view draws in more than one way. On the web, a text surface such as
a form or a transcript goes to the browser as real DOM through
[`components/backend/dom.coax`](../../components/backend/dom.coax), so the
browser handles text input, selection and accessibility. A world goes to the
GPU. On the iPhone, the first native control set (text, button, field,
toggle, scroll and keyed list) is a proposed Coax Native backend; until it
exists, the app's screens are UIKit controllers and the shared GPU
components. The book says which one a screen uses and never claims that the
GPU kit already runs as UIKit controls.

Accessibility is part of the component, not a later pass. A label is
required. An accessibility action dispatches the same action as a tap. Large
text, reduced motion and the keyboard are facts the host reports, and a
screen that can't honor one says so.

## What we build

A plan for what this chapter ships once the draft is approved:

- The login screen of chapter 2, built from `components/kit/` on the DOM
  backend for the web and the declared iPhone presentation.
- A tree check: the IDs, roles and labels of the screen, the same on both
  hosts.
- A refusal: two elements with one ID stop the update before it becomes
  interactive.

It builds on what exists today:

- [`components/spec.md`](../../components/spec.md)
- [`components/core/tree.coax`](../../components/core/tree.coax)
- [`components/core/elements.coax`](../../components/core/elements.coax)
- [`components/core/edit.coax`](../../components/core/edit.coax)
- [`components/core/markdown.coax`](../../components/core/markdown.coax)
- [`components/backend/dom.coax`](../../components/backend/dom.coax)
- [`components/kit/controls.coax`](../../components/kit/controls.coax)
- [`apps/shared/host/apple`](../../apps/shared/host/apple)

### The chapter's check

- **Source:** `openagents/examples/login-screen.coax`, a view over chapter
  2's state.
- **Trace:** the tree, printed in the element table's format, for the empty
  form and for the form after a typed email.
- **Refusal:** a view that gives two rows the same ID refuses the update; the
  previous tree stays and the refusal prints its code.
- **Independent check:** `coax/web/tool.sh components-check` (one Markdown
  parser, no looks in a component) and the Chrome layout oracle for the flex
  subset ([the components spec](../../components/spec.md)).

Planned paths: `openagents/examples/login-screen.coax`.

## Sources

- **Episodes:**
  - [Episode 196, *Ditch the TUI*](../../docs/transcripts/196.md): one interface for every device
- **Also:**
  - [The components spec](../../components/spec.md)
  - [The chat zero-base audit](../../docs/audits/2026-11-11-chat-zero-base-audit.md), sections 10 and 11: text surfaces use the browser
  - [Coax Native, "Elements and presentation"](../../coax/native.md#elements-and-presentation)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1

## Open questions for the owner

1. Which screens of the iPhone app move to the native control backend first,
   and which stay UIKit controllers until it is qualified?
