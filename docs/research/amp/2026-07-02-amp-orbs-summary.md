# Amp Orbs — Summary (2026-07-02)

Sources (fetched 2026-07-02):

- https://ampcode.com/notes/putting-an-agent-in-an-orb
- https://ampcode.com/news/agents-in-orbs
- https://ampcode.com/manual/orbs#whats-in-an-orb

Companion doc: `2026-07-02-amp-orbs-adaptation-audit.md` (what OpenAgents should
adapt into our cloud backend and Khala Code).

## What an orb is

"Orbs are machines where agents can run without supervision." Amp (Sourcegraph's
coding agent) gives every new thread a fresh, ephemeral remote machine — an
"orb" — containing the repository clone, project configuration, plugins, and
the tools the agent might need. The pitch is that agents should operate
independently of user presence and single-machine limitations: launch many
agents on different bugs at once, convert bug reports into autonomous
investigations, run long optimization/testing loops, and prototype without
local resource constraints.

## Hardware, pricing, lifecycle economics

- Fixed shape today: **16 CPU cores, 32 GB memory** (resource customization
  "will be supported in the future").
- **$1.66/hour ($2.48/hour enterprise), billed by the minute.**
- Orbs start quickly and **pause automatically when inactive**; archiving a
  thread pauses its orb immediately. **A paused orb costs nothing.**

## What's in an orb

- OS: **Debian 12**.
- Pre-installed: authenticated `gh` (GitHub CLI) and authenticated `amp` CLI,
  Git, SSH, tmux, vim, jq, fzf, ripgrep, ast-grep, ffmpeg, ImageMagick, unzip,
  zstd, lsof, websocat.
- Databases: **PostgreSQL 17 and Redis** running in-box.
- Runtimes: Bun, Node.js, npm, pnpm, Yarn, Python/pip.
- `agent-browser` for browser automation inside the orb.
- Anything else is installable via `apt-get`; the agent's system prompt tells
  it that it is running in an orb and how to acquire additional tools.

## Repo lifecycle hooks

Two bash scripts at the repository root are the contract between the repo and
the orb runtime:

- **`.agents/setup`** — runs before the agent starts work on a fresh orb.
  Amp's own setup script is ~428 lines: initializes PostgreSQL with
  ephemeral-tuned settings (`fsync = off`, `synchronous_commit = off`,
  `autovacuum = off`), seeds test users, installs language toolchains via
  mise, installs dependencies, and writes orb-specific guidance to
  `~/.config/amp/AGENTS.md`.
- **`.agents/resume`** — runs on wake-from-pause to restore connectivity and
  correct system state; allowed to block **at most 10 seconds**.

## Snapshots

After `.agents/setup` completes, the orb state is **snapshotted and the
snapshot is reused for up to 24 hours**, so subsequent orbs for the same repo
skip the expensive setup phase. This is the key latency/cost optimization:
pay for setup once a day, not once a thread.

## Control surface (user side)

- `amp -ox "prompt"` — start an execute-mode thread remotely on a fresh orb
  (also launchable from the Amp TUI).
- `amp sync <thread-id>` — mirror the orb's file changes back to the local
  machine for iterative local work.
- Remote-with-local-feel: review file changes, browse the orb filesystem, and
  open a terminal into the orb from the client.
- Secrets and environment variables are defined per-project in ampcode.com
  settings and injected into orbs.

## Agent-ergonomics patterns (the interesting part)

The "putting an agent in an orb" note is mostly about making the *repo* a good
place for an unsupervised agent, not about the VM substrate:

- **Port registry instead of hardcoded ports**: the dev server writes port
  metadata to `.amp/dev-ports.json` so multiple checkouts run without
  collision.
- **Dev-only auth endpoints** kill OAuth friction for the agent:
  `/__dev/log-me-in/<email>` (instant authenticated session),
  `/__dev/preflight` (JSON readiness report), `/__dev/sudo` (passkey session
  testing).
- **Unified log inbox**: services log to `.amp/in/` (an agent "scratch-pad
  inbox"); browser console output is forwarded to server logs tagged
  `[browser]`, so one grep covers client and server.
- **Layered AGENTS.md**: 41 context-specific `AGENTS.md` files across the
  codebase, plus orb-specific guidance written at setup time.
- Stated philosophy: "every step had a paved path" — idempotent setup, cheap
  error recovery, anticipatory documentation, so the agent can "act, check,
  and correct instead of over-planning around mistakes."

## Takeaway framing

Orbs are three separable things bundled into one product:

1. A **per-thread ephemeral VM** with fixed shape, minute billing, and
   pause-free-resume economics.
2. A **repo-owned lifecycle contract** (`.agents/setup` / `.agents/resume` +
   24h snapshots) that amortizes environment cost.
3. A set of **agent-ergonomics conventions** (port registry, dev-auth
   endpoints, log inbox, layered agent docs) that make unsupervised execution
   actually converge.

The substrate (1) is commodity; the leverage is in (2) and (3).
