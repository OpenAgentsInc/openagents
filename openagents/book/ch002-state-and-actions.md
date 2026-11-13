# 2. State and actions

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 1 named the app. This chapter gives it a body: one state record, a
set of actions that change it, and one dispatcher that every input goes
through. Nothing is drawn yet. You drive the app from a test and read its
state back.

## Draft

An app on Coax Native is its state and its actions
([Coax Native, "Application state and dispatch"](../../coax/native.md#application-state-and-dispatch)).
The state is an explicit record the app owns: the current route, the fields
a person has typed, what is loading and what came back. An action is a named,
typed change to that record, such as `account.open_login` or
`composer.send`.

Every input reaches the app as an action. A tap, a key, a timer, a deep link,
an accessibility action and the answer to a network request all enter one
serialized dispatcher. A host never changes state on its own, and it never
calls back into Coax from the middle of a platform call. It queues an event
and wakes the dispatcher.

An action is all or nothing. It checks its inputs, works out the whole new
state and the effects it wants, and then commits both. If it refuses, the
person sees no half-changed screen and no effect is sent. Use the existing
[checked state](../../coax/checked-state.md) mechanism for this.

An action never does the effect itself. It asks for one, as a request with an
operation ID, and the request goes out after the commit. The answer comes
back later as another action. Chapter 5 builds that path. Here the app has no
effects at all, which keeps the first checks exact.

Stable IDs start here. Every piece of state that a person can see or press
gets an ID made from its role and its domain key, such as `composer.send` or
`session.5e55a0e1`. An ID never comes from a list index or a pixel position.
Chapter 3 hands these IDs to the components, and `coax drive` finds things
by them.

## What we build

A plan for what this chapter ships once the draft is approved:

- The app's state record and its first actions: open the login screen,
  type into a field, submit and go back.
- One dispatcher with a bounded queue. A full queue refuses with a named
  result; it never drops an event silently.
- Checks that drive the actions and read the state, with no host.

It builds on what exists today:

- [`coax/native.md`](../../coax/native.md)
- [`coax/checked-state.md`](../../coax/checked-state.md)
- [`apps/shared/app-login-flow.coax`](../../apps/shared/app-login-flow.coax)
- [`apps/shared/app-login.coax`](../../apps/shared/app-login.coax)

### The chapter's check

- **Source:** `openagents/examples/app-state.coax`, with its checks beside
  it, over the planned app module in `apps/shared/`.
- **Trace:** the program dispatches `account.open_login`, `field.type
  account.email "you@example.com"` and `account.submit_email`, and prints
  the route and the fields after each, one line per action.
- **Refusal:** `account.submit_email` with an empty field leaves the state
  unchanged, requests no effect and prints the refusal's name.
- **Independent check:** a `#forall` check that any sequence of refused
  actions leaves the state equal to its starting value.

Planned paths: `openagents/examples/app-state.coax`. Generic dispatch code
goes in the proposed `coax/lib/app/` that
[Coax Native](../../coax/native.md#architecture) describes, not in this book.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): one product behind every surface
- **Also:**
  - [Coax Native](../../coax/native.md) and the [Exact research](../../docs/research/exact/README.md) it draws on
  - [Exact: what to adapt](../../docs/research/exact/03-what-to-adapt.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1
