# Roadmap: from the transcript archive to one plan

Status: proposal, 2026-09-25, for
[#9598](https://github.com/OpenAgentsInc/openagents/issues/9598). It reads the
episode transcripts in [`transcripts/`](transcripts/), mainly episodes 200 to
288 and the 2024 plugin episodes (48 to 107), against what this repository
holds today, and proposes one ordered plan that folds the legacy threads into
the current direction. Episode numbers are cited as "ep N". The transcripts are
machine-generated, so quotes are approximate.

## The vision this plan serves

Episode 288 states it:

> "really good reliable coding agent … coding agent as primitive that you just
> rely on and then put in any other workflow using system one heavily bringing
> speed and cost way down keeping quality on par with the big models. Make it
> reliable embeddable in any surface fully open source and extensible
> increasingly drawing on our extensible plugin slash program system … I think
> we've got the shape of it. We just got to fill in the pieces."

Around that core, the archive keeps returning to four commitments that predate
Coder and still stand:

1. **Pay the people who contribute**, in Bitcoin (ep 1 onward; "PAY THE
   PEOPLE", ep 230; "the members of our ecosystem … will be paid the most",
   ep 230, ep 237).
2. **Open source on open protocols**: Nostr and Bitcoin as the neutral
   meeting point (ep 200, ep 286), "100% of all code that we ship is open
   source" (ep 255).
3. **A network, not a chatbot**: "the moat is a network" (ep 269); plugins and
   programs whose first author gets paid when others reuse them (ep 269,
   ep 275).
4. **Verification as the product**: "the atomic unit … is the accepted
   outcome" (ep 237); "verifiable software" (ep 259); receipts and public
   traces (ep 272).

## What the history teaches

The archive is also a record of how plans went wrong. The plan below is built
around these lessons.

- **Too many anchors.** Between ep 199 and ep 288 the anchor product changed
  at least eight times: MechSuit on Claude Code, Autopilot (four different
  forms), Khala, Khala Code, OpenAgents Desktop, Omega, Sarah, the Agent Forge,
  and Coder. The speaker names the problem himself: "We never had one stable
  product to anchor it all" (ep 262), and "Is it just a character flaw of mine
  that I just cannot do simply scoped MVP?" (ep 254). **Lesson:** one anchor,
  Coder, and everything else either serves it or waits.
- **Supply without demand.** The 2024 plugin marketplace had uploads, reviews,
  and per-use fees, and "market forces did not really figure it out" (ep 138);
  about 20 developers were paid "but we didn't really have the use case"
  (ep 165). GPUtopia had the same shape: "oversupply of sellers and no
  compelling buy-side" (ep 201). The pre-reset Coder plugins went unused: with
  14 installed, the model called none of them (the 2026-08-28 A/B review,
  removed in `dabc08102f`). **Lesson:** build the demand first. A plugin or a
  market is worth building only when a program that already runs calls it.
- **Promises outrun delivery.** Dated promises recur and lapse: the January 14
  mainnet (ep 202), the five weekly markets (ep 213, paused in ep 216), "the
  largest decentralized training run" (ep 222 to ep 236), a working plugin
  marketplace "by the end of this week" (ep 058). **Lesson:** keep a public
  ledger, and retire promises out loud instead of letting them lapse.
- **Stack churn.** Rust, then Effect and TypeScript on Bun and Cloudflare
  (ep 233), then Bun removed (ep 253), then Elixir and Phoenix (ep 270), then
  Rust again (ep 279). The repository is now Rust by rule (`AGENTS.md`).
  **Lesson:** the stack is settled; don't reopen it.
- **Claims ahead of evidence.** Clip 288g announced a thesis win that the
  record later withdrew (`terminal-bench/2026-09-24-microluna-two-targets.md`).
  **Lesson:** a claim names the pinned policy, held-out tasks, repeated
  results, and all-in cost, or it isn't made
  ([assessment](coder/design/2026-09-24-assessment.md)).

## The legacy map

Every thread the archive opened, its state, and where it goes in this plan.

| Thread | Episodes | State today | Disposition |
| --- | --- | --- | --- |
| Coding agent core: MechSuit, Probe, Autopilot's coding mode, Khala Code, Coder One, Microluna | 199, 206, 218, 228, 246, 275, 287, 288 | Coder, Coder One, Microluna, and Jev are built; one task passes repeatedly; held-out tasks 0 of 4 | **The anchor.** Phases 1 and 2. |
| Typed decisions: the DSPy "guidance module", Blueprint, System One, Jev | 206, 211, 242, 250, 286, 287 | Jev is the decision model; the gateway serves it | **Kept.** Jev stays narrow and calibrated (thesis). |
| Autopilot, Sarah, Omega, OpenAgents Desktop, Khala as product brands | 199 to 274 | Folded or dropped ("some versions of these will all be folded into the open agents product suite", ep 274) | **Retire the names.** The product is Coder. |
| The Agent Forge and GetAfter, a GitHub replacement | 243, 270 to 274, 281 | Paused ("too big of an apple", ep 281) | **Parked** until phase 5. GitHub stays the host. |
| 2024 Extism Wasm plugins, the plugin registry, the agent store | 048 to 107 | Removed with that codebase; the current Wasm host (`crates/plugin`) replaces the runtime | **Superseded** by the program and module system. Network-capable guests stay excluded by design. |
| Per-use fees to plugin authors, lifetime revenue share, the "1 million sats to the first 100 developers" bounty | 054, 085, 087, 102, 107, 212, 245, 269 | Nothing in the repository pays an author | **Deliver once, in phase 4**, through one mechanism, or retire explicitly. |
| Pre-reset Coder evidence plugins (code search, git facts, repo maps, session search) | removed in `dabc08102f` | Deleted; the model never called them | **Revive as program steps** in phase 3, called by code, not offered to the model. |
| Skills: monetized agent skills, the skills NIP, `tenancy::skills`, `.agents/skills/` | 199, 211, 212, 214 | A free, reviewed skill directory exists; "scoped skills" are designed but not connected | **Connect** in phase 3; payment follows phase 4's mechanism. |
| Client plugins for Claude Code and Codex, and MCP servers (`oak-mcp`) | 165, 255 | Implemented for the decision API | **Kept** as adapters; a discovered MCP server grants nothing. |
| Nostr: NIP-90 jobs, Nexus, NIP-28 and NIP-29, the Immortal relay, NIP-MKT, NIP-CJ, NIP-OPT, NIP-PRG, NIP-CAP, NIP-EXT | 200, 203, 209, 263, 266, 267, 287 | `relay.openagents.com` runs; NIP-CJ conversation jobs work; registries are files, not yet relay events | **Kept.** Publish the registries in phase 3; NIP-OPT in phase 4. NIP-90 stays deprecated. |
| Bitcoin payments: Spark, then LDK, MoneyDevKit, L402, the Economy Kernel, the five markets | 201, 207, 212 to 216, 227, 230, 235 | No payment code in this repository; `tenancy::billing` bills callers, not contributors | **Phase 4**, scoped to paying contributors for accepted outcomes. The five markets stay parked. |
| Compute: GPUtopia, Pylon, Psionic, Tassadar, Apple Silicon, the Foundation Models bridge | 201 to 238, 275 | Lev (Apple on-device model) is built; Pylon and Psionic are outside this repository | **Phase 5**, except System One on local models, which phase 2 can use through Lev. |
| Verification and receipts: Economy Kernel, product promises, AssuranceSpec, Observer, ATIF traces, the Gym | 230, 234, 237, 252, 259, 272, 288 | ATIF, receipts, the Gym, the issue-flow evaluation set, truthful checks (#9584) | **The spine of every phase.** |
| Forum, Moltbook, Shout, agent social network | 209, 212, 230, 231, 235, 275 | Not in this repository | **Parked** (phase 5). |
| Games: the agentic MMORPG, the Verse, XP and progression, StarCraft UI | 189, 199, 237, 240, 284 | `crates/voyager` explores open-ended agents in Minecraft; progression docs only | **Research only.** Voyager continues as a test bed, not a product. |
| Coder Cloud ($1 an hour), the $20 pro plan, the free plan, mobile sync, Coder OS | 276 to 281 | Outside this repository or paused | **Phase 2** decides which surface ships, after the core is reliable. |

## The plan

Each phase has exit criteria. A phase starts only when the one before it meets
its exit criteria, except for work marked as preparation.

### Phase 0: an honest ledger (now, days)

1. **A public promise ledger.** One file listing every public promise in the
   archive that is still open, from the tables in the source notes: the ones
   to keep, the ones kept, and the ones retired, each retired with a reason.
   The 2024 plugin-author payments and the "1 million sats" bounty are the
   most important entries; they get a stated disposition, not silence.
2. **Fix the documents that contradict the code.** Done in 68ac39fed6.
   `extensions/plugins.md`, `glossary.md`, and `programs.md` said
   the Wasm host and the program `module` step weren't built; both landed on
   2026-09-21 (#9519).
3. **Disambiguate the words.** "Plugin" means a Wasm guest in this repository,
   a client package for Claude Code or Codex under `plugins/`, and an Extism
   module in 2024; "skill" means a `SKILL.md` guide and, in Voyager, a Lua
   program. The glossary says which is which. Done in 68ac39fed6:
   [Plugins and skills](glossary.md#plugins-and-skills).

**Exit:** the ledger is published, and no document contradicts the code.

### Phase 1: the primitive works (now, weeks)

This is the Luna pivot and the determinism thesis, and it's the current work
([assessment](coder/design/2026-09-24-assessment.md)).

1. **A signal that separates passing work from failing work** (#9584). Nothing
   else can stop, select, or retry well without it.
2. **The pre-registered Luna-sized family**
   ([protocol](../bench/terminal-bench/experiments/2026-09-25-luna-sized-family/protocol.md)):
   does the cheaper-work claim hold beyond one task?
3. **The lean loop in the product** (#9624), decided by the issue-flow
   evaluation set (#9625).
4. **Stall detection** (#9627) and **best-of-N** (#9587), each in a matched run
   once the signal exists.
5. **The capability-gap log** stays current
   ([capability-gaps.md](terminal-bench/capability-gaps.md)); tasks in it
   aren't tuned on.

**Exit:** a pre-registered win on held-out Terminal-Bench tasks (repeated
passes at a stated fraction of Fable 5.1's cost per pass), and the issue flow
passing a stated share of its held-out issues. Without that, nothing below
ships as a claim.

### Phase 2: embeddable (after phase 1, weeks)

"Put it in any other workflow" (ep 288).

1. **One contract for a coding turn**, the same in every surface: `coder -p`,
   the terminal, the issue flow, a NIP-CJ execution job on the relay, and an
   HTTP route on the gateway beside `POST /v1/systemone`. Each returns a
   receipt: what ran, what changed, what the checks said, and what it cost.
2. **A Rust SDK** for that contract, next to `crates/jev`.
3. **System One on local models where it pays**: Jev-shaped judgments through
   Lev on Apple silicon, measured like any other judgment.
4. **Choose the hosted surface once**: Coder Cloud, a pro plan, or neither,
   decided from phase 1's cost figures, not before.

**Exit:** one external workflow (for example a CI job that turns labeled
issues into draft pull requests) uses the contract for a month with its
results in the Gym.

### Phase 3: extensible through programs (overlaps phase 2's end, weeks)

"Increasingly drawing on our extensible plugin slash program system" (ep 288).
The 2024 lesson decides the shape: code calls a plugin as a program step; the
model is never handed a toolbox to choose from.

1. **Revive the evidence plugins as program steps**: code search, git facts,
   repo maps, and session search, as `SnapshotRead` Wasm guests run by
   `module` steps, each measured on the issue-flow evaluation set and kept only
   if it helps. Started in #9630: a repository map, code search, and a
   test-report parser, run by `programs/evidence-guests.json` from Coder
   One's probe stage behind a manifest switch that's off by default. The
   [measurement plan](extensions/plugins.md#measurement-plan) decides which
   stay.
2. **Programs that compose** (the specified `program` and `invoke` steps) and
   **local packages with locks** (built) become the way to share a workflow.
3. **Publish the registries on the relay**: programs, capabilities, question
   sets, and packages as NIP-PRG, NIP-CAP, and NIP-EXT events, so another host
   can find and run them.
4. **Connect the skill directory to scoped skills**, so a reviewed `SKILL.md`
   can be pinned into a program's context.

**Exit:** at least one program from outside the core team runs in another
host, by digest, from the relay.

### Phase 4: pay the contributors (after phase 3)

This is where the oldest promise gets delivered, once, and narrowly.

1. **One payment mechanism**: a receipt shows which published programs,
   modules, and skills an accepted outcome used; a Lightning payout pays their
   authors a stated share. Accepted means verified by the checks from phase 1,
   not claimed by a model.
2. **Start where demand already exists**: pay only for use inside runs that
   users already pay for or that the project funds. No marketplace listing
   pays anything until something calls it.
3. **NIP-OPT**: shared optimizations (signatures and measured improvements)
   published as events, credited to their authors, paid the same way.
4. **Close the ledger's payment entries**: the 2024 plugin-author promise and
   the bounty are delivered through this mechanism or retired with a reason.

**Exit:** a month of payouts to outside authors, each traceable to receipts.

### Phase 5: the parked markets (only on demand)

Compute (Pylon, Psionic, Tassadar), the five markets, NIP-MKT swaps, the
Agent Forge, the forum, and the games return only when a phase 2 to 4 product
creates demand for them, stated in an issue with the measurement that shows
it. Each one gets its own pre-registered test, the same as a Coder policy.

## What not to do

- Don't start a new product name or brand. The product is Coder, the lab is
  OpenAgents.
- Don't reopen the language or runtime.
- Don't build a marketplace, a market, or a payout before something calls
  what it sells.
- Don't route to or escalate toward stronger models as the plan; the Luna
  pivot holds until its evidence says otherwise, and each gap is logged.
- Don't announce a result before its pre-registered test says so.

## Sources

The four archive readings behind this plan covered episodes 200 to 229, 230 to
259, 260 to 288, and the plugin episodes 048 to 107 with the current extension
design (`extensions/`, `programs.md`, `crates/plugin*`, `plugins/`,
`tenancy::skills`). The archive's own guide is
[`transcripts/README.md`](transcripts/README.md).
