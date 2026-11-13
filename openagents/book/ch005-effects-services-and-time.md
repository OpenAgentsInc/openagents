# 5. Effects, services and time

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

The app can move between screens but can't reach the world. This chapter
adds effects: requests the app asks a host to run, answers that come back as
actions, fake services for tests, and a clock the tests control.

## Draft

An effect is a request with an operation ID, a declared capability and a
bounded payload. The action that wants it commits the request with its state
change. The host then runs it, or refuses it, and the answer arrives later as
an ordinary action
([Coax Native, "Services and native modules"](../../coax/native.md#services-and-native-modules)).
Network calls, the Keychain, passkeys, the payment SDK, files and the
microphone are all effects.

State says what an effect is doing. A request is pending, done, failed or
unknown. Unknown is real: a network write or a payment can be sent and never
answered. An unknown outcome never retries on its own. The service that owns
the operation ID decides whether a second try is safe.

Cancellation works the same way on every host. Cancelling a request stops
its answer from changing the screen at once. A late answer that arrives after
you cancel, log out or leave the screen is dropped, because it carries the
owner it was sent for and that owner is gone. Chapter 11 uses this to cancel
a pending reply.

Tests never reach a real service. A fake service answers with recorded
replies, in a declared order, so a run is the same every time. A fake never
holds a real key and can't spend real money.

Time is controlled too. The app keeps UI time, simulation time, deadlines
and the wall clock apart. In a test, `coax drive clock +MS` advances the
app's time, and `clock settle` runs scheduled work to a bound. The pinned
test environment is seed 13096, 2026-01-01T00:00:00Z, en-US and UTC, the
same values the engine's agent pin uses
([`metaverse-engine/agent/pin.coax`](../../metaverse-engine/agent/pin.coax)).
The clock never fast-forwards a real network or payment timeout into a
success.

## What we build

A plan for what this chapter ships once the draft is approved:

- The login request of chapter 2 as an effect: `POST /api/auth/email/start`,
  with its pending, done, failed and unknown states.
- A fake auth service with recorded answers, and a held answer the test can
  release later.
- Cancellation and the late-answer fence.

It builds on what exists today:

- [`coax/native.md`](../../coax/native.md)
- [`coax/EFFECTS.md`](../../coax/EFFECTS.md)
- [`apps/shared/app-login-flow.coax`](../../apps/shared/app-login-flow.coax)
- [`docs/metaverse/agent-clock.md`](../../docs/metaverse/agent-clock.md)
- [`metaverse-engine/agent/pin.coax`](../../metaverse-engine/agent/pin.coax)

### The chapter's check

- **Source:** `openagents/examples/effects.coax`.
- **Trace:** submit an email, print the request the app committed, release
  the fake answer, print the new state; then submit again, advance the clock
  past the deadline and print the timeout state.
- **Refusal:** cancel a pending request, then release its held answer; the
  state is unchanged and the trace prints `late answer dropped`.
- **Independent check:** the login flow's existing checks in
  [`apps/shared/app-login-flow.coax`](../../apps/shared/app-login-flow.coax)
  still pass against the same recorded answers.

Planned paths: `openagents/examples/effects.coax`.

## Sources

- **Also:**
  - [Coax Native, "Time and lifecycle"](../../coax/native.md#time-and-lifecycle)
  - [The agent clock](../../docs/metaverse/agent-clock.md)
  - [Effects in Coax](../../coax/EFFECTS.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1
