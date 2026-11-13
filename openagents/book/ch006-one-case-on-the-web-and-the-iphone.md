# 6. One case on the web and the iPhone

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapters 2 to 5 built an app with state, components, routes and effects. This
chapter runs it on two hosts and proves they agree: one acceptance case,
written once as `coax drive` operations, passes in headless Chrome and in the
iPhone app.

## Draft

An acceptance case is a list of operations with expected answers. The
operations are the common `coax drive` vocabulary: `tree`, `tap ID`,
`type ID TEXT`, `state`, `layout`, `clock`, `logs` and `screenshot`
([Coax Native, "The agent interface and verification"](../../coax/native.md#the-agent-interface-and-verification)).
`tap` and `type` find a live element by ID and go through the real input
path. They never write state directly.

The first case opens the login screen, types an email, requests a code,
releases a fake answer, enters the code and checks that the account state and
the route changed. A second case cancels the request and releases a late
answer after you leave the screen.

```text
tap home.login
type account.email you@example.com
tap account.submit
clock data release auth.email_start
state account.step
```

Assertions run on state and the semantic tree first. A picture comes last,
and only to catch a drawing fault that the tree can't show. The web and the
iPhone may look different. Their action and result sequences must match.

On the web, `coax/web/tool.sh drive web` already runs these operations in
headless Chrome. On the iPhone, the shared native driver
([`apps/shared/host/drive.c`](../../apps/shared/host/drive.c)) speaks the
same operations, but the iPhone app's shell does not use it yet. Its UI tests
use XCTest identifiers, coordinates and timed waits. Connecting the app's
launch path to the driver is stage 1 of
[Coax Native's migration](../../coax/native.md#migration-and-implementation-stages),
and this chapter is its first consumer.

A host that can't do an operation says so. A driver that implements only
`screenshot` and `tap` refuses `type` and does not claim the whole
interface. Before a run, the driver checks that the app it attached to was
built from the source under test; a stale build refuses with the command
that rebuilds it.

The simulator is evidence for the simulator. Keyboard input methods,
VoiceOver, backgrounding and the platform SDKs need a run on a real device
before a chapter names `ios-device` as a target
([CONTRACT.md](CONTRACT.md#targets)). When the work reaches that point, the
device run goes in the workspace's `NEEDS_OWNER.md` if it needs the owner's
phone or key.

## What we build

A plan for what this chapter ships once the draft is approved:

- The login case and the cancellation case as operation files.
- The iPhone app's shell connected to `apps/shared/host/drive.c`, reporting
  the operations it supports.
- A runner that plays one operation file against the web build and the
  simulator build and compares the two traces line by line.

It builds on what exists today:

- [`coax/web/drive.coax`](../../coax/web/drive.coax)
- [`coax/web/drive-dom.coax`](../../coax/web/drive-dom.coax)
- [`apps/shared/host/drive.c`](../../apps/shared/host/drive.c)
- [`apps/shared/host/drive.h`](../../apps/shared/host/drive.h)
- [`apps/openagents-ios`](../../apps/openagents-ios)
- [`docs/metaverse/fast-shots.md`](../../docs/metaverse/fast-shots.md)

### The chapter's check

- **Source:** `openagents/examples/cases/login.ops` and
  `openagents/examples/cases/login-cancel.ops`, over the app of chapters 2
  to 5.
- **Trace:** each operation and its answer, one line each; the web trace and
  the iPhone trace are byte-equal after the host's name line.
- **Refusal:** the cancellation case; the late answer changes nothing, on
  both hosts.
- **Independent check:** the iPhone app's existing XCTest sign-in tests
  still pass, until the portable cases cover their behavior.

Planned paths: `openagents/examples/cases/`.

## Sources

- **Episodes:**
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat on several devices
- **Also:**
  - [Coax Native](../../coax/native.md), stages 1 and 2
  - [Fast shots](../../docs/metaverse/fast-shots.md): one headless browser per checkout
  - [The iOS app's tests](../../apps/openagents-ios/README.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P16

## Open questions for the owner

1. Which transport does the driver use to reach an app on a physical
   iPhone, and who may attach to it?
