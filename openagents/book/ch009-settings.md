# 9. Settings

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 8's chat page has everything you need to send work. This chapter
adds the one page you open from the sidebar's foot: Settings, with your
account, your limits and Log out, as a scrolling page of GPU components.

## Draft

Settings is a route, `settings(section)`, that you open from the account
foot of the sidebar. Its sections come first as rows: Account, Limits and
About. Each is a page of the kit's settings set, drawn on the GPU like the
chat page. Log out is a row on the Account page and runs chapter 5's logout.

Settings shows only what works. A switch changes something you can see; a
row with nothing behind it is not drawn. Every limit appears with its
number before you reach it. That is a product rule across the whole app.

This is the first page that scrolls as a page rather than as a transcript,
so it settles the scroll driver for every later page. Coax owns the offset
as keyed state, and the host drives it from the platform's scroll physics.
Settings is also where large text has to work: the rows lay out from the
text size iOS reports, and a row that can't fit wraps rather than clips.
Chapter 5's login card draws at a fixed scale today, and the same rule
applies to it.

What a page keeps is split by owner. Which section you last opened is this
device's preference. Your account's settings come from the service and are
the same on every device.

## What we build

A plan for what this chapter ships once the draft is approved:

- The `settings(section)` route, opened from the sidebar's account foot.
- Account, Limits and About sections from the kit's settings set.
- The page scroll driver and large-text layout.

It builds on what exists today:

- [`components/kit/settings.coax`](../../components/kit/settings.coax)
- [`docs/app-shell.md`](../../docs/app-shell.md)
- [`components/gpu-components.md`](../../components/gpu-components.md#scrolling)
- [`components/core/scroll.coax`](../../components/core/scroll.coax)

### The chapter's check

- **Source:** the settings route in the lean app, with its checks, and
  `openagents/examples/cases/settings.ops`.
- **Trace:** `tap side.account`, `tree` lists the three sections; `tap
  settings.limits` prints each limit with its number; `tap settings.back`;
  `state route` prints the chat page's route.
- **Refusal:** a deep link to `settings(account)` with no session gives the
  login card with settings as its return target.
- **Independent check:** an XCUITest at the largest accessibility text size
  that reads every row's full label through the accessibility tree.

Planned paths: `openagents/examples/cases/settings.ops`. The app is
[`apps/openagents-ios`](../../apps/openagents-ios).

## Sources

- **Also:**
  - [The app shell and settings](../../docs/app-shell.md)
  - [GPU components on native](../../components/gpu-components.md), stage G2
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1
