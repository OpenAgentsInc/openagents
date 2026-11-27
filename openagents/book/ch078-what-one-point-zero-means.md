# 78. What 1.0 means

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

This last chapter builds nothing. It states what has to be true before we call
any of this version 1.0, and it lists what we will still not have shown.

## Draft

A version number is a promise, and this project has made promises it did not
keep on time. [Episode 234](../../docs/transcripts/234.md) admits the gap
between what was said and what was running, and starts a registry that marks
each claim as live, gated or withdrawn. Version 1.0 uses the same discipline.

The test for 1.0 is the list this book began from. Episode 289 makes sixteen
promises, and the audit numbers them P1 to P16. Each is covered by chapters
here. 1.0 is the point at which, for every one of the sixteen, the chapters
that cover it run on the platforms they name, and a person outside the company
can do the thing promised.

Some promises can be checked by a program: the same runtime on every surface,
a payout that matches its receipts. Some need a person: a stranger publishes a
plugin and is paid. The list says which is which.

The general-agent essay ends by separating what was shown from what was not,
and that section stays true until we change it. On `main` today we have
shipped and verified:

- **Unified pure-Coax agent core & native desktop surface:** the 24-layer
  [`openagents/core/`](../../openagents/core) runtime (`oa-worker.coax`,
  `oa-subagent.coax`, `#14050`–`#14057`, `#14096`), typed `RoutePlan` routing
  (`coax/lib/agent/router/`, `#14056`), bypass-immune `WalletPay` and closed
  `OpenAgentsCli`/`Metaverse` tool packs (`#14057`), and the unified
  [`apps/openagents-desktop/`](../../apps/openagents-desktop) app across macOS
  Metal 4, Linux Vulkan, and Windows Win32/Vulkan (`#14046`) with File Viewer,
  Embedded Browser Pane (`#14105`–`#14110`), Desktop Spark Wallet pane, Voice
  pane (`#14065`), Always-On Usage HUD (`#14064`), and Embedded Metaverse &
  GameBuilder pane (`#14038`–`#14046`).
- **Cloud + local serving & web stack:** OpenAgents Cloud retail pricing and
  usage APIs (`#14069`–`#14072`), local `$0.00` `psionic-inkling-serve` on
  Metal 4 and CUDA (`#14028`, `#14087`), **Coax Web** with Coax UI Core/Kit/Router,
  native SSR/Hydration, and 0-KB-Wasm SSR books, and the locked **WorkOS hybrid
  `acct_` auth** SDK (`coax/lib/workos/`, `#14097`–`#14103`).
- **OpenAgents Executor Network (`#13912`–`#13928`, `#13960`–`#13963`):** the
  full compounding loop proven on our own hardware ([master capstone](../../docs/books/capstones/executor-network-flywheel.md))—exact
  compiled WASM/VM transformers across Apple Metal 4 and NVIDIA RTX 4080 CUDA,
  `Inkling-2-Mini` hybrid routing with `0` silent hallucinations, multi-device
  replay verification with live Bitcoin mainnet 3-leg Breez Spark settlement
  (Worker + Validator + Module Author royalty `<= 25%`, `#13961`), Byzantine-safe
  DiLoCo training (`7/7` attacks and `3/3` regressions blocked), and signed
  shadow-to-promote rollouts ([H7 qualification record](../../docs/verification/executor-network-effect-h7.md)).

What remains **open and tracked** before 1.0 (not yet shown at full product
scale):

- Shared knowledge base & semantic retrieval (`#14058`, `NIP-KB`, chapters 20–22).
- OpenAgents Studio, conversational plugin/skill authoring interview, and
  sandboxed third-party plugin runtime (`#14059`, chapters 38–42).
- General public `NIP-REG` / `NIP-EVAL` / `NIP-PAY` plugin economy and
  book-reconciled cross-host conformance (`#14060`, chapters 43–61).
- Deep session introspection, borderless cross-window control, full mobile
  feature parity on iOS/Android, and Metaverse Engine Windows host conformance
  (`#14066`–`#14068`, `#14073`–`#14094`, chapters 10, 75, 77).
- Independent outside contributors operating Pylons and submitting winning
  capabilities at scale, sustained real-world compounding across public users,
  and composition working outside software. 1.0 does not need all three of
  these long-horizon proofs; it needs each stated plainly next to the version
  number, with the measurement that would settle it.

The essay also says what would show the thesis is wrong. If members that pass
every check do not make the agent better on new work, the method has failed,
and we should say so.

This book gives no dates. The order of work is in the plan, and each step is
done when its chapters run.

## What we build

A plan for what this chapter ships once the draft is approved:

- A 1.0 checklist: each of P1 to P16, the chapters that cover it, the
  platforms, and whether a program or a person checks it.
- A public list of what is live, gated or not yet built, kept next to the
  download page.
- A page of what we have not shown, each item with the measurement that would
  settle it.

It builds on what exists today:

- [`docs/audits/2026-11-10-episode-289-port-audit.md`](../../docs/audits/2026-11-10-episode-289-port-audit.md)
- [`docs/transcripts/README.md`](../../docs/transcripts/README.md)
- [`README.md`](../../README.md)

### The chapter's check

- **Source:** `openagents/book/ONE-POINT-ZERO.md`: P1 to P16, each with the
  chapters that cover it, the platforms they name, and whether a program or
  a person checks it. It gathers those chapters' checks.
- **Trace:** one line per promise: each covering chapter's check result per
  platform, and for a person check, the dated record of who did the thing
  and what they saw.
- **Refusal:** a promise with a failing or missing check, or a person check
  with no record, is marked not done, and 1.0 is refused until every line
  passes.
- **Independent check:** the checklist is compared line by line with the
  sixteen promises in [the
  audit](../../docs/audits/2026-11-10-episode-289-port-audit.md), and each
  person check is done by someone outside the company.

Planned paths: `openagents/book/ONE-POINT-ZERO.md`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 4:00, version 1.0
  - [Episode 288, *Three DevDays Later*](../../docs/transcripts/288.md): 2:15, three years into one product
  - [Episode 234, *Product Promises*](../../docs/transcripts/234.md)
  - [Episode 237, *You Must Construct Additional Pylons*](../../docs/transcripts/237.md): an earlier 1.0
  - [Episode 248, *Predictable Software*](../../docs/transcripts/248.md)
  - [Episode 259, *Verifiable Software*](../../docs/transcripts/259.md)
  - [Episode 138, *Year One Recap*](../../docs/transcripts/138.md): a year-one recap
  - [Episode 100, *Looking Ahead*](../../docs/transcripts/100.md)
- **Also:**
  - *The Return of the General Agent* (`git show coder-v1.0.0:docs/essays/2026-10-01-the-return-of-the-general-agent.md`), Part V and "What would show this is wrong"
  - [The port audit](../../docs/audits/2026-11-10-episode-289-port-audit.md), sections 1 and 4
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P16

## Open questions for the owner

1. Is 1.0 one release of everything, or does each surface reach 1.0 when its
   own chapters run?
2. Which of the sixteen promises may ship marked "gated" at 1.0, if any?
