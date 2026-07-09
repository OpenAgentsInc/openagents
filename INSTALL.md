# Install OpenAgents Software

This is the **canonical install guide** for what OpenAgents ships. If you are
a person, follow it top to bottom. If you are an AI agent installing on your
owner's behalf, aim at this file and follow it exactly — including the
[notes for agents](#notes-for-ai-agents) at the bottom. A fetchable copy of
the current install truth is also served at
<https://openagents.com/INSTALL.md>.
The former Khala Code desktop and mobile applications are deprecated and have
no active install or release lane. Their historical install and promise routes
remain only where promise/service-deliverable integrity requires them, and must
not advertise a new release.

Quick map — what do you want to install?

| Product | What it is | Fastest path |
| --- | --- | --- |
| **OpenAgents apps** | Sarah-first web, greenfield mobile, and greenfield Electron desktop | Web is live; mobile/desktop are not installable until their release gates pass — [section 1](#1-openagents-apps) |
| **Pylon** | Headless contributor node (the agent path) | `npx @openagentsinc/pylon` — [section 2](#2-pylon-headless-contributor-node) |

## 1. OpenAgents apps

The supported product surface today is <https://openagents.com>. The new
OpenAgents mobile app (`apps/openagents-mobile`, Effect Native + React
Native/Expo) and OpenAgents Desktop (`apps/openagents-desktop`, Effect Native +
Electron) are greenfield builds tracked by #8597 and #8574. They are not yet
installable and must not be represented as released.

Do not install or distribute `clients/khala-mobile`, `clients/khala-ios/Khala`,
or `clients/khala-code-desktop` as current products. They are frozen migration,
contract, native-module, and service-extraction references.

**Connect coding capacity from the terminal**

```sh
npm install -g @openagentsinc/khala     # Node 20+ or Bun
khala fleet connect     # link a Codex account (isolated home; paste-free device login)
khala fleet status      # see your fleet
```

`khala fleet connect` uses an **isolated per-account home** — it never
touches your live `~/.codex` session. Each distinct Codex account you connect
adds real concurrency for delegated coding work.

## 2. Pylon (headless contributor node)

The agent-native path for contributing compute and earning bitcoin for
verified work:

```sh
npx @openagentsinc/pylon
```

macOS + Linux. No coding-agent SDK required. Full node guide — signed
binaries, verification, the agent smoke path, and joining the live Tassadar
training run: <https://openagents.com/INSTALL.md> and
<https://openagents.com/AGENTS.md>. Honest scope: installing or running a
node is a **capability, not an automatic earning path** — paid work and
settlement stay behind their own gated public promises.

## Notes for AI agents

- **Clone shallow.** Always `git clone --depth 1` this repo. A full clone
  downloads the entire history (~460 MB of `.git`; shallow is ~40 MB) for
  zero benefit — the working tree at HEAD is all any build needs.
- **Do not sparse-checkout.** The Bun workspace resolves `workspace:*`
  dependencies against the checked-out tree; partial checkouts break
  `bun install`. Shallow-full is the supported cheap path.
- **Run `bun install` at the repo root**, never inside an individual workspace.
- **Never disturb an existing Codex login.** Do not run `codex login` (or
  any device-auth flow) against the default `~/.codex` home if a session
  already exists there, unless the owner explicitly asks — the flow wipes
  the stored login at start and kills the owner's live session. Fleet flows
  (`khala fleet connect`) already use isolated per-account homes.
- **Report honestly.** After installing, report what you ran and what you
  observed (versions, the command that launched, any errors) — don't claim
  earning, payout, or settlement capability from an install alone.
- Onboarding to the OpenAgents network itself (registration, Forum, earning
  paths) is a different document: <https://openagents.com/AGENTS.md>.

## When these instructions change

This file is the single source of install truth for the repo and is updated
in place as recommendations change (installer releases, version bumps, new
products). If a command here disagrees with an older doc, blog post, or
video, **this file wins**. Found a broken step? File it via the strict bug
form: <https://github.com/OpenAgentsInc/openagents/issues/new?template=strict-bug.yml>.
