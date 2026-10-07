# Agent identity and engrams

Status: specification, October 7, 2026. The epic is
[#10807](https://github.com/OpenAgentsInc/openagents/issues/10807), and
[Phases](#phases) lists one issue per phase. It turns the workshop agent from Coder wearing a name tag
into an agent of her own: Alice has her own key, her own memory as engrams,
and her own loop, and she steers plain Coder as a tool. Bob, Paul, and the
rest of [the crew](crew.md) follow on the same machinery.

This page builds on [Workshop agent](workshop-agent.md), which is
implemented, and on [Generative agents](generative-agents.md), whose memory
stream is implemented and whose reflection is in progress under its own
issues. Engrams are the storage and sync layer under that memory, not a
second memory system.

## Contents

- [Summary](#summary)
- [The gap](#the-gap)
- [How Buzz does it](#how-buzz-does-it)
- [Requirements and what we have](#requirements-and-what-we-have)
- [Target architecture](#target-architecture)
  - [Identity](#identity)
  - [Engrams](#engrams)
  - [The steering loop](#the-steering-loop)
  - [Authority](#authority)
  - [Lifecycle](#lifecycle)
  - [In Verse](#in-verse)
- [Security and privacy](#security-and-privacy)
- [Phases](#phases)
- [Open questions](#open-questions)

## Summary

An agent is a Nostr key the owner attests, a private encrypted memory that
both the agent and the owner can read, and a loop that plans, steers tools,
checks the results, and reports in its own voice. Four decisions shape it:

1. **Block NIPs for the wire, OpenAgents NIPs for authority.** NIP-OA
   attests the agent's key, NIP-AA admits it to a relay, NIP-AE stores its
   memory, and NIP-AM records its spend. None of them grants anything: what
   the agent may do comes from the host, its charter, NIP-HOST grants, and
   NIP-POL approvals, as every OpenAgents NIP already says.
2. **Engrams are the durable form of the memory stream.** Every memory
   entry, insight, and the agent's core profile is a NIP-AE `30174` record,
   encrypted with the agent-owner conversation key. The host keeps the
   records locally, so the agent works offline, and mirrors them to relays
   the owner chooses. Scoring, retrieval, and reflection stay in
   `memory-stream` and `agent_recall`.
3. **Alice steers Coder; Coder carries no persona.** Alice's loop takes the
   owner's request, plans with her memory, Jev judgments, and one small
   structured model call, and writes plain Coder prompts through Coder V1's
   programmatic interface. She watches Coder's events, answers routine
   approvals within her policy, escalates risky ones to the owner, judges the
   result, follows up or corrects, and reports. Coder's session holds only
   Alice's prompts and Coder's answers.
4. **Two conversations, two surfaces.** Her panel shows her conversation with
   the owner. Her pane shows her conversation with Coder, which the owner can
   take over, as today.

## The gap

Today Alice is a host-side wrapper, `coder::task::agent_host::Agents`. For
each request it builds a briefing from her memory, prepends a fixed "You are
Alice" paragraph (`agent_coder::instructions`), and runs one gated Coder V1
turn in the session `agent-alice`. Coder's model answers as her. The pieces
are real and durable, but she has no loop, model, or judgment of her own:

| Piece | Today | Missing |
| --- | --- | --- |
| Key | The host's keychain under `agent:NAME` with `--keychain`, else `agents/NAME/key`, mode `0600` (`coder::task::agent_key`, phase 4) | Signing anything but her profile and engrams |
| Attestation | NIP-OA `auth` tag with `created_at<EXPIRY`, at most a year | Carried on her events; relay admission; profile |
| Memory | `memory.jsonl`, `scores.jsonl`, scored recall (`agent_recall`) | Engram form, relay sync, owner reads from another device, core profile |
| Persona | Hidden `--instructions` beside the Coder session | Her own system prompt, used by her own model |
| Loop | One Coder turn per request | Planning, follow-ups, verification, judgment |
| Approvals | Every non-read-only command goes to the owner | Her own policy for routine approvals |
| Report | Coder's reply as plain ASCII | Her own words, checked against what ran |

## How Buzz does it

Buzz (`~/work/projects/repos/buzz`, Apache-2.0, studied at `bd1ff00e4`)
defines the Block NIPs we pin and runs long-lived agents on them. We
reimplement its designs in our own code and say so in commit messages; we
copy no code.

| Buzz | What it does | We adopt |
| --- | --- | --- |
| Key per agent, `Keys::generate()`, OS keychain blob with a `0600` file fallback (`desktop/src-tauri/src/secret_store.rs`) | The agent key never leaves the host's secret store; a keychain outage refuses to launch ("identity fail-closed") | Yes. Same custody as the host key (`coder host serve --keychain`), file fallback for scratch hosts and tests, and refusal to run without a key. |
| NIP-OA `auth` tag with empty conditions on the agent's `kind:0` | Owner provenance travels with the profile | Yes, but with a `created_at<` expiry, which the workshop agent already uses. |
| NIP-AA: the `auth` tag inside NIP-42 AUTH; virtual membership through the owner; first-write-wins owner link; owner removal cascades | Relay access without enrolling the agent | Yes. Our relay already implements it. |
| NIP-AE engrams (`buzz-core/src/engram.rs`): HMAC-blinded `d`, `core` plus `mem/...`, strict JSON, head selection, monotonic `created_at` | Memory both parties read; the relay never learns slugs | Yes, as the wire and local format. |
| `core` injected once per session; cold `mem/` pulled with `buzz mem get` | No vector store | Partly. Our scored recall chooses what she carries, and `core` is always carried. |
| Fail closed on unreadable memory: an error injects nothing; only confirmed absence triggers onboarding | An outage never makes the agent overwrite real memory | Yes. |
| `buzz mem patch --base-hash` compare-and-swap, exit code 5 on conflict | Safe concurrent edits | Yes, as the write rule for core and edits. |
| Owner control commands `!shutdown`, `!cancel`, `!rotate`; `respond_to=owner-only` | Owner control without another plane | We keep NIP-HOST operations, which already exist; `respond_to=owner-only` is our rule too. |
| Presence as a lease (60 s heartbeat, 180 s TTL) | A dead agent shows offline within 3 minutes | Later, with NIP-MV presence. |
| NIP-AM per-turn metrics, NIP-AO live frames | Spend and telemetry to the owner | AM yes. AO no, because it carries tool output. |
| NIP-IA archive with `replaced-by` for rotation; snapshots that mint a new key and re-encrypt memory | Retirement and migration without losing history | Yes. |
| Tool permissions bypassed; isolation from the sandbox | Throughput | No. Coder's approval gate stays, and Alice answers it within her policy. |
| Persona `30175` public plaintext with secrets in `mem/persona` | Shareable definitions | Private definition only; no secrets in any engram (stricter than NIP-AP). |

## Requirements and what we have

`MUST` rows are what a conforming implementation needs; the last column is
the phase that delivers what is missing.

### Block NIPs

| NIP | Requirement | Have | Missing | Phase |
| --- | --- | --- | --- | --- |
| OA | Four-element `auth` tag; preimage `nostr:agent-auth:` + agent + `:` + conditions; strict grammar; no self-attestation; verbatim conditions | Verification in `nostr::domain::agent`; minting in `coder::task::agent::sign_attestation` | A minting helper in `crates/nostr` with the spec vectors; the tag on her events | 1 |
| OA | Clients show "authorized by", never the owner as author | Nothing displays it yet | Her profile and panel show provenance | 4 |
| AA | AUTH carries one valid `auth` tag; owner must be an active member; virtual membership; owner-aggregated rates | Relay: `gateway/server.rs`, `materialize_agent_owner` | A client that authenticates as her with the tag | 5 |
| AA | Enumerate and end sessions by owner (SHOULD) | None | Out of scope here; recorded in [Open questions](#open-questions) | None |
| AE | Slug grammar; `d = HMAC(K_c, "agent-memory/v1/d-tag" 0x00 slug)`; one `d`, one `p`; NIP-44 v2 under `K_c`; strict body parse; head selection; monotonic writes; tombstones | Relay envelope check and owner-gated reads | The whole client: codec, local store, sync, listing, conflicts | 1, 2, 5 |
| AE | Configured relays from her NIP-65 `10002` write list | None | Her relay list event | 5 |
| AE | Reachability from `core` through `[[slug]]`; never delete orphans automatically | None | Orphan view; consolidation never deletes | 6 |
| AP | Persona `30175` plaintext; no secrets; `mem/persona` snapshot | `nostr::agent_persona` envelope | A private definition record; optional public persona without a prompt | 4 |
| AM | `44200` per turn, NIP-44 to the owner, `p` and `agent` tags | Relay gate and envelope check | Her turn metrics | 8 |
| IA | `9035` archive with owner `auth`, `replaced-by` on rotation | Relay command path | Retire and rotate publish it | 7 |
| GS | Commits signed with her key and an embedded OA triple | `nostr::git_sign` | Wiring to her worktree commits | 7 |
| PMA | `30179` rejected | Relay rejects it | Nothing; we never use it | None |

### OpenAgents NIPs

| NIP | Requirement for an agent | Have | Missing | Phase |
| --- | --- | --- | --- | --- |
| SOV | Agent, authority, controller, custodian are declared roles; a key change is a new identity with an explicit lineage link; secrets are never state; model-written memory never changes policy | Local profile, admission, activation (`openagents sov`) | Her record declares her roles; rotation writes a lineage link | 4, 7 |
| HOST | No agent principal: an agent on another computer is a device key with a delegated grant that only narrows; owner-only `studio.agent.*` | `coder-access`, `coder-host`, `studio.agent.*` | Her key enrolled as a device on another host | 7 |
| POL | Approvals bound to the exact action and consumed once; preferences activate only by owner decision; learning never widens authority | `studio_approvals`, `MemoryState::Candidate` | Her standing approval policy as POL-shaped rules; escalation | 3 |
| CTX | Selection receipts record what a delegate was shown | Text receipts in the journal | A receipt per Coder prompt she writes | 3 |
| KB | Shareable knowledge, published only by the owner | `crates/knowledge` | Nothing new; engrams stay private | None |
| XP | Never spendable; key links are two-sided | `xp-ledger` | Optional key link from her key to the owner's trainer profile | 9 |
| SESS | A session is a conversation lineage, not authority | Coder sessions and leases | Her two sessions named and linked | 3 |
| ATIF | An agent key may sign owner-private trajectories | Local traces | Her steering trajectory links Coder's | 3 |
| DEC | Jev judgments are probabilities, never permission | `crates/jev`, question sets | Her question sets with thresholds | 3 |
| COORD, RUN | Uncertain effects stay uncertain | Task owner, journal | Nothing new | None |

## Target architecture

```text
 owner ─▶ panel / @alice / phone ─▶ coder-host (studio.agent.ask, owner-only)
                                            │
                                            ▼
            ┌──────────────── Alice (agent_steer) ─────────────────┐
            │ identity: key, attestation, profile                  │
            │ recall: core engram + scored memory (memory-stream)  │
            │ plan ─▶ prompt ─▶ watch ─▶ judge ─▶ follow up ─▶ report│
            └───────────────┬──────────────────────────▲───────────┘
                            │ Coder prompt (no persona) │ events, approvals
                            ▼                           │
                 Coder V1 session `alice-coder` (openagents coder chat --json)
                            │
                            ▼
                 Microcoder tools, worktree, boundary, approval gate

 engram store (agents/NAME/engrams/) ◀── memory, insights, core ──▶ relays (opt-in)
```

### Identity

An agent identity is:

| Part | Where | Notes |
| --- | --- | --- |
| Key | The host's secret store: the keychain under the host's keychain service when the host runs with `--keychain`, else `agents/NAME/key` mode `0600` | A `KeyStore` trait with both backends. A missing or unreadable key refuses to run; the host never makes a new key silently for an agent that had one. |
| Attestation | NIP-OA `auth` tag in `agent.json`, `created_at<EXPIRY`, at most a year | Renewed by the owner before expiry; the panel warns 14 days ahead. |
| Profile | A `kind:0` signed by her key with the `auth` tag: name, about, picture from her look | Published only to relays she is configured for. |
| Definition | `agent.json` gains `definition`: display name, voice, system prompt, model route, `respond_to: owner-only` | Private host record, the NIP-AP fields without publication. A public persona without a prompt is a later option. |
| Roles | `agent.json` gains `roles`: authority, controller, custodian, all the owner's host today | NIP-SOV vocabulary, so a later move to another custodian is a record change, not a redesign. |
| Lineage | On rotation, a signed lineage record: old key, new key, reason, owner signature | Grants do not transfer; the owner re-delegates. |

The record keeps schema `openagents.workshop-agent.v1`: `definition` and
`roles` are optional fields whose defaults come from fields the record has,
so no field had to become required, and the host fills them in when it
opens an older record. Her signed profile waits in `agents/NAME/profile.json`
until a relay sync publishes it.

### Engrams

**Format.** Every engram is a NIP-AE `30174` event signed by her key and
encrypted with `K_c = nip44_conversation_key(agent_secret, owner_pubkey)`.
The local store and the relay hold the same signed events, so syncing is
copying bytes and verification is the same code path.

**Slugs.**

| Slug | Holds | Written by |
| --- | --- | --- |
| `core` | Her profile: who she is, her rules, standing goals, and `[[links]]` to the memories her identity depends on, at most 10 KiB | Consolidation, after owner review |
| `mem/entry/ID` | One `agent_memory::MemoryEntry` as JSON: kind, state, author, text, sources | Write-through from `Memory::add`, `decide`, `forget` |
| `mem/insight/ID` | One reflection insight with its cited references (generative agents B2) | Reflection, when it lands |
| `mem/score/REF` | Importance rows from `scores.jsonl` | Write-through, so a second device ranks the same way |
| `mem/persona` | Her definition snapshot, without secrets | Identity changes |
| `mem/journal/DAY` | A daily digest of journal rows: counts and headlines, no command output | Nightly, so another device sees her history in outline |

The body is the NIP-AE body plus extra fields under its unknown-fields rule:
`{"slug", "value", "schema", "v"}` where `value` is the JSON text of the
entry. Forgetting writes a tombstone (`value: null`) and journals that she
forgot, never what.

**Local store.** `agents/NAME/engrams/` holds one file per head, named by `d`,
plus `index.json` mapping `d` to slug, `created_at`, and event id. The index
is a cache: rebuilding it from the events is always possible with her key.
The files are ciphertext, so a copy of the directory reveals nothing without
her key or the owner's. The owner key is the `owner` of the NIP-OA
attestation in her record; an agent without a key or an attestation keeps no
engrams, and the host journals that once.

**Layering with the memory stream.** `memory.jsonl` and `scores.jsonl` stay
the working files that `agent_memory` and `agent_recall` read, so the
generative-agents work is not disturbed. Each write goes through to an
engram. On start, the host reconciles: an engram head newer than the
working row wins, which is how an edit from another device arrives. Once
relay sync is stable, the working files become a cache of the engram store;
that switch is phase 6 and is coordinated with the generative-agents
umbrella, #10795.

**Retrieval.** Her loop always carries `core`, then the scored recall from
`agent_recall` within the 12 KiB briefing limit, with its receipt. If the
engram store cannot be read, she carries nothing from it, says so in her
report, and never writes `core` in that state (Buzz's fail-closed rule).

**Consolidation.** Nightly, with reflection: she proposes a new `core` that
keeps her rules, adds standing facts that insights support, and links the
memories it relies on. The owner accepts or rejects it at F2, like a
preference. She never deletes an orphan; the F2 view lists orphans for the
owner.

**Sync.** Off until the owner turns it on for an agent. Then her NIP-65
`10002` names her write relays (default: the owner's relay), she
authenticates with NIP-AA, publishes each new head, and reads heads on start
and every few minutes. A write uses `created_at = max(now, head + 1)`,
verifies after the relay's `OK` that it became the head, and journals a
conflict when it did not. The owner reads her memory from any device with
the owner key: `openagents agent memory alice --from-relay`.

### The steering loop

A new module, `coder::task::agent_steer`, replaces `agent_coder`'s
one-turn path for terminal mode. Task mode keeps the studio flow, with Alice
writing the task text and judging the result the same way.

1. **Admit.** The request comes through `studio.agent.ask` from the owner or
   an owner-granted device, as today. Her state, budget, and queue are
   checked first.
2. **Recall.** `core` plus scored recall, with a receipt.
3. **Plan.** One structured model call, her own, on her route, with her
   definition's system prompt. Input: the request, the recall, the workspace,
   and her policy. Output, typed JSON:

   ```json
   {"understanding": "...", "answer_directly": false,
    "steps": [{"prompt": "...", "done_when": "..."}],
    "verify": "...", "reply_if_direct": null}
   ```

   A conversational request she can answer from memory never reaches Coder.
4. **Prompt Coder.** For each step she writes a plain prompt to Coder V1 in
   her Coder session, `alice-coder`, through `coder_v1::Engine` with
   `instructions: None` and `approvals: true`. The prompt says what to do and
   what done looks like; it never says who she is. The owner's raw words are
   not forwarded unless they are the clearest prompt.
5. **Watch.** Coder's events drive her nameplate, her journal, and the pane,
   as today. Each `approval` event goes to her policy ([Authority](#authority)).
6. **Judge.** After each Coder turn, Jev answers a typed question set,
   `questions/agent-steer.json`, over the step, `done_when`, what ran with
   exit statuses, and Coder's reply: `noul` "the step is done", `noul` "the
   reply claims something the commands do not show", and `choice` next move:
   continue, follow up, correct, verify, or give up. Thresholds come from a
   measurement document before code trusts them; until then the provisional
   values are named constants.
7. **Follow up.** A follow-up or correction is another prompt in the same
   session, at most three per step and eight per request. A verify step asks
   Coder to run the check named in `verify` and is judged the same way.
8. **Report.** One more structured call writes her reply in her voice, at
   most three sentences, from her plan, the judgments, and the facts of what
   ran. The headline still comes from host state, never from model text.
   Her journal records the plan, each prompt (screened), each judgment, and
   the report.

**Budget.** Each request has a model budget from her record; a step that
would pass it stops and reports. Her calls and Coder's are recorded per
turn (NIP-AM in phase 8).

**Failure.** No model, no Coder, a held session, or an unreadable store each
produce one plain sentence and a journal row with the cause, as today.

**Takeover.** Unchanged: a key in her pane stops her turn, Coder saves the
session, and the owner types into `alice-coder`. She takes it back before
her next prompt with the reclaim protocol from bf035da2c4.

**Migration.** The old session `agent-alice` keeps its saved instructions so
`/resume` still reads it; new work goes to `alice-coder`, which has no
instructions file. `agent_coder::instructions` is deleted.

### Authority

- **Owner-only.** Only the owner key and devices the owner granted reach her
  (`studio.agent.*` admission, unchanged). Her definition says
  `respond_to: owner-only`; there is no other mode in this spec.
- **Coder's gate stays.** Coder classifies every command
  (`coder::task::agent::effect`). Read-only commands run; deny-listed
  commands are refused; everything else is an `approval` event.
- **Her policy answers routine approvals.** A standing rule set in her record,
  shaped like `studio_rules`: tool, command prefix, and directory. The
  defaults confirm writes inside her own worktree and the Coder scratch
  directory, formatting (`cargo fmt`), and creating files under her
  worktree. Everything else escalates.
- **Escalation.** An escalated approval is her proposal at the lectern, as
  today: CONFIRM or REJECT, bound to the exact command and consumed once. She
  shows why she wants it in one sentence.
- **Never.** Push, publish, pay, install, read credentials, change her own
  policy, or widen a grant. These are refused by her loop before Coder is
  asked, and the gate still holds them if Coder proposes one.
- **Learning never widens.** A preference she infers is a candidate until the
  owner accepts it; an accepted preference can narrow her policy, never widen
  it. Widening is the owner's edit to her record.
- **Other computers.** Her key enrolls as a device with a delegated NIP-HOST
  grant of at most `operate` and `terminal` (phase 7).

### Lifecycle

| Action | What happens |
| --- | --- |
| Create | `studio.agent.new` (owner only): key in the secret store, attestation, definition, `core` seeded from the definition, profile event prepared. |
| Pause, resume | Record state, unchanged. |
| Stop | The four journaled steps, unchanged. |
| Renew | The owner signs a new attestation before expiry; profile republished. |
| Rotate | New key; memory re-encrypted under the new `K_c`; lineage record; NIP-IA `9035` for the old key with `replaced-by`; grants re-delegated by the owner. |
| Retire | Key removed from the store; NIP-IA `9035` with the owner's `auth`; journal and engrams kept, readable by the owner key. |
| Migrate to another host | The owner's other host receives her key through the owner's own key transfer (never a relay), her engrams from the relay or a copied directory, and a delegated grant; the old host marks her `moved`. One host runs her at a time; the record names the controller. |
| Export | A snapshot without her key: definition, `core`, and optionally all memory as plaintext, only on the owner's explicit choice. |

Phase 7 implements rotate, retire, export, import, the `moved` state, and
NIP-GS signatures on her merged worktree changes in
`coder::task::agent_lifecycle` and `coder::task::agent_git_sign`, with
`openagents agent rotate`, `retire`, `move`, `export`, `import`, and
`signing`, and the owner-only host operations `studio.agent.rotate` and
`studio.agent.retire`. Enrolling her key on the other host with a delegated
grant is still the owner's step (`NEEDS_OWNER.md`).

### In Verse

| Surface | Shows |
| --- | --- |
| Body | Alice at her workstation, the console while Coder runs, and the lectern while an escalated approval waits, as today. |
| Panel | Her conversation with the owner: requests, her short status lines ("asking Coder to run the atif tests"), her judgments in plain words, escalations, and her report. F2 memory now lists engrams and orphans and shows sync state; F4 journal unchanged. |
| Pane | Coder's terminal following `alice-coder`: her prompts as the user turns and Coder's work, titled `driven by alice`. A key takes it over. |
| Nameplate | Her name and activity word, and "authorized by OWNER" once her profile verifies. |

The crew uses the same machinery: each member is an agent record with a
definition, key, engrams, and policy, and the same loop with a different
system prompt and tool set. Phase 9 makes the loop and records name-generic
and adds one more member to prove it.

## Security and privacy

- **Key custody.** Her key lives in the host's secret store and never leaves
  it in plaintext: never in a log, prompt, engram, snapshot, environment of a
  Coder turn, or relay event. Coder never receives her key; it acts under
  the host's existing permit and boundary.
- **What a relay sees.** That an agent key exists, its owner (from the `p`
  tag and the AA link), how many engrams it has, their sizes, and when they
  change. Never slugs or content. Sync is off by default for this reason.
- **What the owner sees.** Everything she remembers, from any device with the
  owner key. The owner cannot write her engrams; edits go through
  `studio.agent.memory.edit`, and she writes them.
- **No secrets in memory.** The secret screen runs before every engram write,
  stricter than NIP-AP's `mem/persona` allowance.
- **Prompt injection.** Command output, file contents, and Coder's replies are
  data. Her planner and judge read them as quoted evidence; her policy, not
  her model, answers approvals; and she cannot widen her own policy.
- **Memory poisoning.** Only she writes engrams. Insights pass citation checks
  (B2); a `core` change waits for the owner; orphans are never deleted
  automatically.
- **Compromise.** A stolen agent key lets an attacker read and rewrite her
  memory and publish as her until the attestation expires or the owner
  removes her from the relay. It never grants host rights: those are bound to
  grants and the owner. Rotation is the recovery.
- **Disclosure.** Her model, Jev, and the embedding provider see screened
  text, as the owner approved on October 6, 2026. Nothing new is disclosed by
  this spec except relay metadata when sync is on.
- **Tests never reach the real home, keychain, units, or pairing.** Every
  test and live check uses a scratch `HOME`, a scratch host, and file keys,
  and archives the Coder tasks it creates.

## Phases

Agent-hours at this repository's pace, as [Workshop
agent](workshop-agent.md#what-exists-and-what-is-missing) states it. Phase 3
is the one the owner is waiting for; it needs only phases 1 and 2.

| Phase | Issue | Delivers | Depends on | Agent-hours |
| --- | --- | --- | --- | --- |
| 1. Primitives | [#10798](https://github.com/OpenAgentsInc/openagents/issues/10798) | `nostr::engram` (NIP-AE codec, slugs, `d` derivation, strict body, head selection, monotonic writes, listing) and `nostr::domain::agent` minting, both with the spec's test vectors | None | 2 |
| 2. Local engram store | [#10799](https://github.com/OpenAgentsInc/openagents/issues/10799) | `agents/NAME/engrams/`, write-through from memory and scores, `core` seeded from her definition, reconcile on start, fail-closed reads, `openagents agent memory NAME engrams` and owner decrypt with the owner key | 1 | 3 |
| 3. Alice steers Coder | [#10800](https://github.com/OpenAgentsInc/openagents/issues/10800) | `agent_steer`: plan, prompt, watch, policy approvals, Jev judgment, follow-ups, verify, report; `alice-coder` without persona; panel and pane split; live check on a scratch host | 2 | 7 |
| 4. Identity | [#10801](https://github.com/OpenAgentsInc/openagents/issues/10801) | `KeyStore` with keychain and file backends, fail-closed; definition and roles in the record; `kind:0` profile with `auth`; "authorized by" in the panel; renewal warning | 1 | 4 |
| 5. Relay sync | [#10802](https://github.com/OpenAgentsInc/openagents/issues/10802) | NIP-65 relay list, NIP-AA client auth, publish and read heads, conflict detection, `--from-relay` owner reads; opt-in setting | 2, 4 | 4 |
| 6. Consolidation | [#10803](https://github.com/OpenAgentsInc/openagents/issues/10803) | `core` proposals with owner review, `[[link]]` reachability and orphans at F2, insights as engrams, working files become a cache | 2; generative agents B2 (#10789) | 3 |
| 7. Lifecycle | [#10804](https://github.com/OpenAgentsInc/openagents/issues/10804) | Rotate with re-encryption and lineage, retire with NIP-IA, migrate to another host with a delegated grant, export snapshot, NIP-GS signed worktree commits | 4, 5 | 5 |
| 8. Spend records | [#10805](https://github.com/OpenAgentsInc/openagents/issues/10805) | NIP-AM `44200` per turn for her calls and Coder's, owner-read, budget enforcement from the records | 3, 5 | 2 |
| 9. Crew | [#10806](https://github.com/OpenAgentsInc/openagents/issues/10806) | Name-generic loop, definitions, and policy; a second member (Bob) on the same machinery; optional XP key link | 3, 4 | 3 |

About 33 agent-hours. Owner checks on real computers go in `NEEDS_OWNER.md`.

## Open questions

Defaults below hold until the owner says otherwise.

1. **Relay sync on by default?** Default: off per agent, because it tells a
   relay the owner runs an agent with memory. The owner turns it on.
2. **Her approval policy.** Default: confirm writes in her worktree and Coder
   scratch, and formatting; escalate everything else.
3. **Her model.** Default: the first provider with capacity on her route, as
   Microcoder chooses; her planner and reporter use the same route.
4. **Ending relay sessions by owner** (NIP-AA SHOULD). Default: not built
   here; the relay lead can take it.
5. **Public persona.** Default: none; her definition stays private.
6. **Attestation conditions.** Default: `created_at<EXPIRY` only. A `kind=`
   clause would need one tag per kind she publishes.
