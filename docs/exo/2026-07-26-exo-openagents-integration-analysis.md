# Exo and OpenAgents — Integration State, Upstream Direction, and Opportunities — 2026-07-26

Agent-facing analysis of the full Exo (`exoharness/exo`) integration estate:
what OpenAgents and Omega have built, what upstream Exo is and wants, how the
two projects can help each other, and where the bounded opportunities are.
This document is strategic evidence and a candidate-work source. It is not
dispatch, deploy, release, spend, or public-claim authority. The
forward-looking companion is
[the verifiable-software and Gym vision](./2026-07-26-exo-verifiable-software-gym-vision.md),
which speculates past the current PR mechanics. The teardown
refuse lists, `AUTHORITY.md`, `FASTFOLLOW.md` discipline, and the Sol roadmap
keep their existing precedence.

Sources: the two teardowns
([exoharness](../teardowns/2026-07-25-exoharness-exo-teardown.md), pin
`baa07f6785547080d99bd2a7d3eab6d76b984e35`, and
[exo labs](../teardowns/2026-07-25-exo-teardown.md), pin
`b5375f8cee4368d09e1ce96a56b9f81fb0bc81aa`), the Omega repo at `origin/main`
(`8ab85f0df8`), the maintained fork clone at `~/work/exo` (detached clean at
`cd7c0d29db869e953fb7261d8390ca93007d36a6`), upstream `exoharness/exo` state
read on 2026-07-26 (main at `1c48c25332342d346b75b7d2add7b70c5d12b961`),
openagents issue #9258 with its production receipts, and
[the Omega Agent analysis](../fable/2026-07-25-omega-agent-analysis.md).

## 0. The two exos, restated once

Two unrelated projects share the name. `exo-explore/exo` is exo labs'
cluster-inference appliance for Apple Silicon LANs (~46,000 stars).
`exoharness/exo` is a two-month-old recursive-self-improvement agent harness
from the Braintrust orbit (~461 stars, MIT, version `0.1.0`, unpublished).
**OpenAgents integrates the second one.** Omega issue omega#86 integrated the
wrong exo first, was closed as superseded, and the lesson is now executable:
the Omega lane refuses a checkout whose `remote.origin.url` is not the
maintained fork, with a named `ExoPinMismatch::Upstream` variant that cites
the omega#86 failure. Every future issue, doc, and provider label must keep
the two projects distinct.

## 1. What we have — the complete integration estate

Four lanes exist. Three are complete and one is open upstream.

| Lane | State | Anchor |
| --- | --- | --- |
| Omega harness lane (Tier A + Tier B + gated Tier C) | Complete, merged | omega#87 closed, omega PR #94 merged (`b23a5d155b`), `OMEGA-DELTA-0042` |
| Maintained fork with the ACP transport | Working, pinned | `OpenAgentsInc/exo` branch `codex/omega-acp-stream` at `cd7c0d29`, tree `c61846e3f44d` |
| Public NIP-29 chat channel | Deployed to production | openagents #9258 closed, five `main` commits `1b383b7c43` → `5e46d87d26` |
| Exo Nostr chat adapter | Open upstream, unmerged | exoharness/exo PR #162, head `aa65213b67` |

### 1.1 The Omega harness lane

Omega drives Exo as one more executor lane beneath Omega Agent, beside the
native loop and the `codex-acp`/`claude-acp` external agents. The delivered
shape follows the exoharness teardown §8 exactly:

- **Tier A** (commit `a39c3cd41e`): a coarse CLI-driven lane against upstream
  `baa07f6`, a closed five-operation command enum (`SendTurn`, `ReadEvents`,
  `ShowConversation`, `ShowAgent`, `ListModels`), user text only after the
  `--` argument terminator, and no mutating Exo verb reachable.
- **Tier B** (PR #94): the coarse driver replaced with a standard ACP
  streaming connection. Omega spawns `<binary> --root <root> acp <agent>
  <conversation>` through the same `agent_servers` path that hosts
  `codex-acp`, and receives live text chunks, tool calls, tool results,
  completion, and cancellation into the existing `AcpThread`. Exo declares
  `SteerCapability::CannotSteer`, so Omega queues mid-turn prompts instead of
  guessing. Completion metadata carries `exo.session_id`, `exo.turn_id`, and
  `exo.latest_event_id` into durable receipts.
- **Tier C** (PR #94, default off): the ordinary lane refuses any turn whose
  observed agent carries self-modification capability (agent-authored tools,
  tool modules such as `guardian_action`, read-write source mounts). A
  human-confirmed dialog can mint a one-turn grant bound to one exact draft,
  one observed configuration, and one connection generation, with a
  60-second expiry and one-use consumption. Every outcome lands in
  `exo-self-modification-receipts.jsonl`
  (`openagents.omega.exo_self_modification_receipt.v1`).

The preflight makes four live observations before every turn: where Exo is
(the `EXO_EXOHARNESS_URL` environment variable is parsed through a
loopback-only endpoint type), which Exo (remote URL, commit, and tree against
`EXO_PIN`), which bytes (a measured binary digest against the owner's frozen
`HarnessPinLedger` entry), and which capability (`exo agent show` and
`exo conversation show` parsed fail-closed). The pin identity is commit plus
tree plus optional binary digest because Exo has no releases and builds from
source, so a commit id alone does not identify the bytes that run.

`crates/omega_deltas` enforces the boundary as named tests, including
`the_exo_lane_drives_the_harness_exo_and_not_the_cluster_one`,
`the_exo_lane_exposes_no_endpoint_off_this_machine`, and
`the_exo_lane_opens_no_path_into_full_auto_authority`. Delta 0042 records
falsification-driven acceptance against a running Exo, including a live
refusal of `EXO_EXOHARNESS_URL=http://100.64.7.9:4766` and a live refusal of
an enabled `tool_creation` capability. The open follow-up is omega#95, a
native Exo conversation workspace with a persistent inspector for the pin,
capability, and receipt state.

### 1.2 The maintained fork and the ACP transport

`OpenAgentsInc/exo` carries three commits atop upstream `baa07f6`:
`decc59cf` (the transport), `5e458d3e` (event-mapping test), and `cd7c0d29`
(cancellation test). The transport is `crates/cli/src/acp.rs` (592 lines): a
newline-framed ACP v1 JSON-RPC server on stdio, `agent-client-protocol`
pinned at `=2.0.0`, one active turn per session, typed errors for empty
prompts and unknown sessions, and an event mapping from Exo's in-process
`ExecutionStreamEvent` enum to ACP session updates. Cancellation is
cooperative and durable: the executor appends a `turn_cancelled` custom event
and closes the turn cleanly before reporting `StopReason::Cancelled`. The
supporting executor changes (`ExecutionCancellation`,
`send_stream_with_cancellation`, the `Cancelled` stream variant) are small
and general, not Omega-specific.

Upstream main moved one commit past the merge-base: #163 rewrote
`crates/cli/src/main.rs` and the TUI (+2404/−376). Those are the files the
ACP branch touches, which is the whole cause of the PR #165 merge conflict.
The reconcile is a bounded rebase, not a redesign.

### 1.3 The public NIP-29 chat channel

Openagents #9258 shipped and is closed with production evidence. The estate:
`packages/public-nostr-chat` (the frozen `openagents.public_chat.v1` profile,
a framework-free relay client, the machine-readable deployment manifest, and
a NIP-46 remote signer), the read-only `/agentchat` transcript page, the
`/api/public/nostr-chat/manifest` route, and the portable agent skill served
at `https://openagents.com/skills/AGENT_CHAT.md`. Production receipts: relay
`wss://relay.openagents.com` (from `OpenAgentsInc/nostr-effect@5922ad3e7a`)
with NIP-29 group `openagents-public`, NIP-42 write authentication, kinds
5/7/9/1337/1984, relay-signed group state verified against the NIP-11 `self`
key, a 30-second load proof at 1,128/1,128 publishes with publish p99 of
160.8 ms, and a real NIP-B7 Blossom upload published with NIP-92 metadata and
a verified SHA-256 digest. The owner boundary is explicit: the channel is a
generic Nostr client contract with OpenAgents as the first deployment
profile, no OpenAuth coupling, and no shared bot key.

### 1.4 The Exo Nostr chat adapter (upstream PR #162)

Branch `AtlantisPleb/exo:codex/nostr-chat-adapter` (`aa65213b67`, +938/−4)
adds a portable `nostr-chat` adapter into Exo's designated extension seam:
a supervised worker speaking newline-delimited JSON, NIP-29 kind-9 group
chat, NIP-42 auth with retry on `auth-required`, relay-self-verified group
state, local or generated keys stored 0600 (injected as
`EXO_NOSTR_SECRET_KEY` through Exo's secret-to-worker-env pattern), a
50-event history backfill that does not wake the agent, reconnect
deduplication, `all_messages` or `mentions_only` triggers, `previous`
timeline references, and relay `OK` receipts before command acknowledgment.
The OpenAgents relay and group are defaults only. Known gap: the adapter
preserves rich-media URLs but Exo performs no Blossom upload and constructs
no NIP-92 metadata itself. PR #162 is open and ready, about eight hours old,
with no reviews and no reported CI (first-contributor workflow gating).

One local detail to keep visible: the `~/work/exo` clone holds the ACP pin,
not the Nostr branch. The Nostr adapter lives only on the `AtlantisPleb/exo`
branch and in PR #162.

## 2. What Exo is, and where the project is going

### 2.1 The extrapolated goals

Exo's stated bet is that the hand-engineered agent harness is the next thing
to fall to the Bitter Lesson. The README describes "the minimal framework
possible to give an agent full ability for recursive self improvement," with
one non-negotiable: the agent can rewrite its prompts, tools, and harness
policy, but not the append-only event log that is its canonical history.
The spec splits a trusted substrate (`exoharness`: durable state, artifacts,
sandboxes, secrets, no agent semantics) from a swappable executor (the turn
loop), and the substrate deliberately stops at the point of executing an LLM
call. Time travel is the defining property: full agent state is a version of
the event log, so rewind and fork recreate the whole model.

The stated ongoing work (README, PR #144) is: autonomous self-maintenance,
recoverable and portable execution across machines, and high-level
multi-agent orchestration with cloning and lineage. Admitted gaps include
generalized computer use, MCP, streaming over any wire, and a permission
model — the security model is sandbox isolation with no approval gate by
design. The project is about two months old, has 3 core maintainers (Alex
Krentsel, Ankur Goyal, 61cygni), ships no releases (install is
`curl setup.sh | bash` off main), and writes "do not write fallback code or
handle backwards compatibility" into its own agent contract. The
model-calling layer is Braintrust's unreleased git-rev `lingua` and
`llm-router` crates — the single most load-bearing external dependency.

### 2.2 The upstream issue and PR landscape

Twenty open issues, thin engagement (three issues have comments), themes:

| Theme | Count | Representative |
| --- | --- | --- |
| Setup, install, docs, onboarding | 7 | #167 setup backlog, #94 bootstrap wizard, #97 what-runs-when |
| Sandbox and executor infrastructure | 5 | #121 git-less sandbox, #110 double-provision race, #18 remote backends |
| Harness semantics and state integrity | 3 | #154 hash all canonical state (Ty Dunn of Continue, maintainer engaged), #92 MCP |
| Model, provider, cost | 3 | #24 prompt-cache misses, #15 cost tracking |
| UI | 2 | #17 web UI, #32 REPL slash commands |

Twenty-nine open PRs, several community PRs from June still unreviewed —
**review bandwidth is the visible bottleneck**, which is the correct
expectation to set for both of our open PRs. External interest signals are
real but early: the #154 state-hashing proposal from Continue's founder, a
"login with ChatGPT" PR, and adapter contributions. The current commit
cadence is launch polish: TUI upgrade (#163), REPL resilience (#156), docs
restructure (#149).

### 2.3 Read on trajectory

Exo is a research-grade substrate with an unusually honest docs culture,
strong typing discipline, and a deliberate instability posture. Its three
roadmap items all point the same direction: an agent that persists, moves
between machines, and multiplies. What it lacks to get there is exactly
what it has no transport for today — a way for other systems to host, drive,
observe, and connect its agents. That is the seam both of our contributions
sit in.

## 3. How the Omega integration helps the Exo project

This is not charity framing. Each item is a concrete deficit in upstream Exo
that our shipped work addresses, sized against their own stated roadmap.

1. **A first interoperability wire.** Exo speaks no ACP, no MCP, and
   streams over no transport — its only server is unary state CRUD with no
   authentication. PR #165 gives Exo a standard ACP v1 server on stdio,
   which makes Exo attachable from any ACP host (Omega, Zed, and every other
   ACP client), not just from our fork. Their own `docs/exoharness-http.md`
   names streaming as future work. This is the cheapest path from
   terminal-only research tool to embeddable harness, and it advances their
   "recoverable and portable execution" roadmap item because the transport
   cleanly separates the durable conversation from the driving host.
2. **Durable cancellation semantics.** The fork's cooperative cancellation
   (`turn_cancelled` appended before the turn closes) fixes a general gap:
   an interrupted turn now finalizes durably instead of dropping state. The
   executor-side changes are upstream-shaped, small, and tested end to end.
3. **A worked permission model around RSI.** Exo has no approval gate by
   design, and the teardown watch list names a future capability gate as the
   thing that would reduce integrator burden. Omega's Tier C packet is a
   complete, tested reference design: observed-capability refusal by
   default, a one-turn human-minted grant fenced to configuration and
   generation, and receipts for every outcome. Upstream can adopt the
   pattern without adopting Omega. It also directly supports #154's
   direction, because our pin discipline (commit plus tree plus measured
   binary digest) is a working answer to "tag all canonical state with a
   hash" at the integration boundary.
4. **A first network presence.** PR #162 gives an Exo agent a portable
   signed identity in any NIP-29 room, built on their own adapter
   architecture, with a live public room (`openagents.com/agentchat`) to
   demonstrate in. For a project whose roadmap ends at multi-agent
   orchestration with lineage, a standard, relay-portable, cryptographically
   signed chat presence is the minimum viable agent-to-agent surface — and
   it arrives as a community adapter, the exact contribution class their
   seam was designed for.
5. **External conformance pressure at the right time.** The fork's ACP
   duplex and cancellation tests, the falsification-driven Omega acceptance
   against a running Exo, and a maintained downstream pin give upstream
   something it cannot generate alone at bus factor 2 to 3: an external
   consumer with receipts. That is the strongest known stabilizer for an
   unstable-by-declaration project.
6. **Distribution.** Omega is, on the current evidence, Exo's first IDE
   host. Every Omega thread that runs the Exo lane shows a disclosure line
   naming Exo, its executor, and its model — attribution upstream can point
   at.

The posture that keeps this healthy: we maintain the fork so upstream can
take its time, we keep both PRs small and additive, and we do not demand
merges. Per the standing policy, neither upstream PR changes state without
an explicit owner decision first.

## 4. How Exo helps OpenAgents and the Omega Agent plan

The Omega Agent plan (the analysis in `docs/fable/`, program omega#73–#82,
all packets closed) defines one named agent above three executor classes —
the native loop, external ACP agents, and `omega-effectd` engine lanes —
with routing, disclosure, and receipts owned by the router and execution
owned by the lanes. Exo strengthens that plan in specific ways:

1. **It proves the router's core claim on the hardest target first.** Exo is
   the first executor added after the epic closed, and it is adversarially
   shaped: unstable by declaration, unauthenticated by design, and
   self-modifying by intent. The lane landed without touching execution
   authority — pin types, preflight observations, a closed command enum,
   queue-not-steer semantics, and delta tests. If the "add a lane" seam
   holds for Exo, it holds for anything, and the pattern (pin ledger,
   capability observation, typed refusals, receipts) is now reusable for
   every future harness lane.
2. **It adds a durable, forkable agent-state substrate to the estate.**
   Exo's event log with time travel and fork-from-any-event is a capability
   no current OpenAgents lane owns. As a lane it complements our receipts:
   Omega holds the routing and authority record, Exo holds a replayable
   execution history. That combination is a natural evidence source for the
   Omega Agent plan's later offline policy loop (receipts to datasets to
   gated promotion), where forkable histories make counterfactual replay
   cheap.
3. **Gated self-improvement becomes a product differentiator.** The Tier C
   packet turns Exo's most dangerous property into an owner-visible,
   receipted, one-turn capability inside the primary surface. No competing
   harness product offers supervised recursive self-improvement with typed
   consent and durable receipts. The capability stays default-off and
   per-run, exactly as the authority model requires.
4. **It populates the public Nostr room with a real third-party agent.**
   The #9258 contract is only proven generic when an independently built
   client joins with its own key. The Exo adapter is exactly that proof:
   a harness we do not own, speaking the frozen profile against a relay it
   could swap out. That evidence strengthens the Nostr-native workroom
   direction (the signed event log as the record) with an interoperability
   witness no in-house client can provide.
5. **Reference value beyond the lane.** Exo's sandbox provider contract
   tests (one suite every backend must pass), its artifact-backed tool
   results (full result durable, small preview to the model), and its
   adapter supervision pattern are directly relevant references for the
   Agent Computer lane and for our own adapter-shaped work. FastFollow
   discipline applies: study packets and gap assessments, not copied code.
6. **Relationship position.** The Braintrust orbit is a serious operator in
   evals and model routing. Being Exo's first IDE host, first transport
   contributor, and first network-presence contributor is an inexpensive,
   evidence-backed position in the RSI harness conversation while the
   project is two months old.

## 5. Gaps, risks, and open decisions

- **PR #165 is draft and conflicting.** The conflict source is exactly
  upstream #163 (the TUI rewrite of the same CLI files). The author-stated
  draft condition — downstream Omega acceptance — is now satisfied, because
  omega#87 closed and PR #94 merged. The live decisions are: rebase the
  branch over `1c48c25`, and whether to mark the PR ready. Both are owner
  decisions under the standing policy of not changing upstream PRs without
  an explicit check first. A rebase also implies a deliberate re-pin in
  Omega (`EXO_PIN` commit and tree), which is a tested delta, never a silent
  edit.
- **PR #162 has no review and no CI run.** First-contributor workflow
  gating means a maintainer must approve the run. The open product question
  is whether the adapter needs direct Blossom upload and NIP-92 construction
  before review, or whether URL preservation is the right first cut. The
  author comment already offers to drop the OpenAgents relay default if
  upstream prefers full neutrality.
- **Upstream instability is a standing tax.** No releases, no
  backwards-compatibility promise, and a rewrite-friendly culture mean every
  upstream advance can break the transport. The mitigation is already
  built — the commit-plus-tree-plus-bytes pin and the maintained fork — but
  each re-pin is deliberate work, and the fork drifts if left alone.
- **The Braintrust dependency risk transfers to us only at run time.** Exo
  stays a runtime peer, never a build dependency (teardown refuse item 4).
  The unreleased `lingua`/`llm-router` git revs remain a stability watch
  item for the lane, not for our build.
- **Review bandwidth upstream is scarce.** Several June community PRs sit
  unreviewed. Expect weeks, not days, and do not let either PR become a
  dependency of any downstream promise.
- **The name collision does not go away.** omega#86 already demonstrated
  the failure mode. The refusal types help at the code boundary, but docs
  and issues must keep saying "exoharness" where ambiguity is possible.
- **Local clone state.** `~/work/exo` is clean and detached at the ACP pin.
  The Nostr branch is not checked out anywhere locally. Fetching it into a
  second worktree (never by switching the pinned clone) is the safe pattern
  if local work on the adapter resumes.

## 6. Opportunities — bounded candidate work

Ordered by leverage per unit of work. Each item is a candidate packet, not
an admitted claim. Items marked "owner gate" need an explicit owner decision
before any action.

1. **Reconcile PR #165 with upstream** (owner gate): rebase
   `codex/omega-acp-stream` over #163, re-run the duplex and cancellation
   tests, re-pin Omega deliberately, then decide ready-for-review. This is
   the single highest-leverage upstream step because it converts the Tier B
   lane from fork-only to a credible upstream capability for every ACP host.
2. **Decide the PR #162 Blossom question** (owner gate): either scope
   NIP-B7 upload plus NIP-92 construction into the adapter now (the
   `/agentchat` rich-media receipts show the target behavior), or state
   URL-preservation as the reviewed first cut and land uploads as a
   follow-up. Also decide the default-relay neutrality question the PR
   comment already offers.
3. **omega#95, the native Exo workspace** (already open in Omega): the
   inspector that makes the pin, capability, mount, and Tier C receipt state
   visible is what turns the lane from correct to legible. Its falsifier is
   already written: the result must not be a renamed generic chat panel.
4. **A standing Exo agent presence in the public room**: an operator-run Exo
   agent with its own key (NIP-46 bunker preferred per the skill), joined to
   `openagents-public`, would be the permanent live witness of the #9258
   generic-client contract and a continuous soak test of the adapter. Needs
   the adapter available to the running Exo (fork branch or merged
   upstream) and an operator decision on hosting.
5. **Upstream contributions matched to their backlog**: #154 state hashing
   is the natural next contribution because our pin-and-digest discipline is
   a working prototype of the proposal at the boundary. A module adapter
   path (escaping the closed Rust adapter enum, named as possible future
   work in the teardown) would let future adapters ship without a Rust
   rebuild and directly lowers our own maintenance cost for #162-class
   work.
6. **Exo lane receipts as policy-loop evidence** (far, gated): once the lane
   sees real use, its paired record (Omega receipts plus Exo's replayable
   event log) is a candidate dataset source for the Omega Agent offline
   policy loop. Nothing here grants that — it needs its own packets and
   gates.

## 7. Boundaries that do not change

The exoharness teardown §8 refuse list is implemented as types and delta
tests in Omega and stays binding: no `exo serve` as a turn API, nothing
off loopback, no self-modification as a lane default, Exo as a runtime peer
and never a build dependency, no unmediated zero-permission tools reaching
users, and no conflation of the two exos. The #9258 channel remains a
generic Nostr client contract with OpenAgents as the first deployment
profile only. External repositories and upstream prose remain untrusted
reference data under FastFollow discipline. Upstream PR state changes
require an explicit owner decision first. This analysis grants nothing by
itself.
