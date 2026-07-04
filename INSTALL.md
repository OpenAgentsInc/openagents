# Install OpenAgents Software

This is the **canonical install guide** for what OpenAgents ships. If you are
a person, follow it top to bottom. If you are an AI agent installing on your
owner's behalf, aim at this file and follow it exactly — including the
[notes for agents](#notes-for-ai-agents) at the bottom. A fetchable copy of
the current install truth is also served at
<https://openagents.com/INSTALL.md>.
The public Khala Code install-truth page is
<https://openagents.com/code/download>; it mirrors the Codex requirement,
the npm `khala` CLI path, the source-build path, and the fact that the macOS
DMG is still receipt-gated.

Quick map — what do you want to install?

| Product | What it is | Fastest path |
| --- | --- | --- |
| **Khala Code** | The desktop coding app (the main product) | Build from source — [section 1](#1-khala-code-the-desktop-coding-app); public install truth at `/code/download` |
| **Pylon** | Headless contributor node (the agent path) | `npx @openagentsinc/pylon` — [section 2](#2-pylon-headless-contributor-node) |

## 1. Khala Code (the desktop coding app)

Khala Code wraps your own local Codex install and adds fleet/swarm
coordination on top. **There is no public installer yet — building from
source is the supported path** (tracked honestly in the `khala_code.*`
product promises). macOS is the primary target (Electrobun desktop shell).
The public download counter at
`/api/public/khala-code/download-counts` serves exact ledger rows only; if no
public-countable rows exist, it returns an empty `counts` array rather than a
synthetic install total.

**Prerequisites**

- [Bun](https://bun.sh) 1.3+ (`curl -fsSL https://bun.sh/install | bash`)
- Node 20+ (for the Codex CLI install below)
- **Codex CLI, logged in** — Khala Code requires it:

```sh
npm install -g @openai/codex
codex login
```

> Already using Codex? If `codex login` shows you're signed in, skip it.
> **Never re-run `codex login` on a machine with a live Codex session unless
> the owner asks** — starting the login flow wipes the stored Codex login in
> `~/.codex` and breaks active work.

**Install and run**

```sh
# 1. Shallow clone — do NOT clone the full history (~460 MB of .git you don't
#    need; a --depth 1 clone's .git is ~40 MB)
git clone --depth 1 https://github.com/OpenAgentsInc/openagents
cd openagents

# 2. Install workspace dependencies (must run at the repo ROOT, not in a subdir)
bun install

# 3. Launch Khala Code
bun run dev:khala-code-desktop
```

That's it. The app builds its UI and opens the desktop window wired to your
local `codex app-server`.

**Your fleet, from the terminal** (optional, pairs with Khala Code):

```sh
npm install -g @openagentsinc/khala     # Node 20+ or Bun
khala fleet connect     # link a Codex account (isolated home; paste-free device login)
khala fleet status      # see your fleet
```

`khala fleet connect` uses an **isolated per-account home** — it never
touches your live `~/.codex` session. Each distinct Codex account you connect
adds real concurrency for delegated coding work.

**Updating**

```sh
cd openagents
git pull            # shallow clones pull normally
bun install         # picks up dependency changes
bun run dev:khala-code-desktop
```

More detail (smokes, env flags, headless mode):
[`clients/khala-code-desktop/README.md`](clients/khala-code-desktop/README.md).

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
- **Run `bun install` at the repo root**, never inside
  `clients/khala-code-desktop`.
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
