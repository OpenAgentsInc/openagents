# 10. The logged-in shell

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. The web shell and settings it describes run today; the
> shared app version is the plan.

## Premise

You are logged in and the API answers. This chapter builds what you see next:
the shell around every logged-in screen, with its side list, and the settings
page, on the web and the iPhone from one set of actions.

## Draft

The web already has the shell ([the app shell](../../docs/app-shell.md)).
Logged in, you see one side down the left: the mark and a link home, three
folds for Chats, Projects and Worlds, and at the foot who is logged in, a
link to Settings and Log out. Settings is a page of the shell with its
sections: Account, Agents and models, Appearance, Limits and About. The
pieces are the settings set in
[`components/kit/settings.coax`](../../components/kit/settings.coax).

This chapter moves the shell's state and actions into the shared app, so
the iPhone gets the same structure. The side is a route-owning surface: a
press on a chat opens `conversation(id)`, a press on Settings opens
`settings(section)`, and Back moves between them on both hosts. On a narrow
screen the side folds away, as it does on the web below 760 px.

Settings shows only what works. A switch changes something you can see, a
row that has nothing behind it is not drawn, and every limit appears with
its number (Settings, Limits). That is a product rule across the whole app.

What the shell keeps is split by owner. Which folds are shut and whether the
side is open are this device's preferences. Your account, your chats and
your projects come from the API and are the same on every device.

## What we build

A plan for what this chapter ships once the draft is approved:

- The shell's state and actions in the shared app: the side, its folds, the
  account foot and Log out.
- Settings sections as routes, with the Account and Limits sections first.
- The iPhone shell drawn from the same actions, as UIKit controllers until
  the native control backend is qualified (chapter 3).

It builds on what exists today:

- [`docs/app-shell.md`](../../docs/app-shell.md)
- [`components/kit/settings.coax`](../../components/kit/settings.coax)
- [`apps/openagents-web/settings-web.coax`](../../apps/openagents-web/settings-web.coax)
- [`apps/openagents-web/settings`](../../apps/openagents-web/settings)

### The chapter's check

- **Source:** the shell module in `apps/shared/`.
- **Trace:** after chapter 8's login case, `tree` for the shell, then `tap
  side.settings`, `tap settings.limits` and `route.back`, with the route
  after each.
- **Refusal:** opening `settings(account)` with no session lands on `login`
  with settings as its return target.
- **Independent check:** the web shell's existing page checks still pass
  against the shared module.

Planned paths: none new. The shell module goes in
[`apps/shared`](../../apps/shared).

## Sources

- **Also:**
  - [The app shell and settings](../../docs/app-shell.md)
  - [Components spec](../../components/spec.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1
