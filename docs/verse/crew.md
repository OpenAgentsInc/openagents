# The crew

Status: proposal, October 6, 2026. The owner approved the cast and its
roles on October 6, 2026; this page refines them against the code. Only
Alice exists as an agent today. Each member's **Today** line cites the code
or process that already does that member's job, and every path on this page
was checked on the date above.

The crew is the owner's set of named agents, loosely based on the
cryptography cast of characters (Alice, Bob, Eve, Mallory, and the rest; see
Wikipedia's [Alice and Bob](https://en.wikipedia.org/wiki/Alice_and_Bob)).
Each member is a [workshop agent](workshop-agent.md) with one job, a body in
[Everglade](everglade.md), and a station where you can watch it doing real
work. The cast's convention fits because each name already means a role in
a protocol: Peggy proves, Victor verifies, Trent is trusted, Mallory
attacks. That role is the member's job here too.

## Contents

- [Summary](#summary)
- [The roster](#the-roster)
- [Rules every member follows](#rules-every-member-follows)
- [Shared machinery](#shared-machinery)
- [The core crew](#the-core-crew)
- [Trust, verification, and judgment](#trust-verification-and-judgment)
- [The adversaries](#the-adversaries)
- [Guardians and escalation](#guardians-and-escalation)
- [Safety rules for the adversaries](#safety-rules-for-the-adversaries)
- [How members work together](#how-members-work-together)
- [In Verse](#in-verse)
- [Costs](#costs)
- [Build order](#build-order)
- [Open questions for the owner](#open-questions-for-the-owner)

## Summary

- **One machine, many names.** Every member is a workshop agent: an agent
  record, a key the owner attests, a journal, memory, Coder V1 sessions,
  standing jobs, and the approval gate. A member adds a role charter and,
  for most, one small piece of role code. No member gets a second runtime.
- **Most of the crew wraps work that already runs.** The issue lane, the
  web deploy runbook, the lease broker, the task owner's independent checks,
  Gym gates, the secret screen, and `NEEDS_OWNER.md` exist today and are run
  by people or by unnamed subagents. Naming them gives each process an
  owner, a journal, and a place in the world.
- **Authority never grows with the cast.** The host stays the only
  authority, every member answers only the owner, and no member approves
  another member's step. A member that "judges" or "verifies" writes a
  typed verdict; merging, deploying traffic, paying, and publishing stay
  the owner's decisions.
- **Adversaries run on purpose, against scratch targets.** Eve, Mallory,
  Trudy, Sybil, Craig, Chuck, Oscar, and Rupert test the system the rest of
  the crew builds. They run only against scratch hosts and fixtures, never
  the owner's real setup, unless the owner grants a named target for a
  bounded time.
- **About 106 agent-hours** of crew-specific work after the dependencies,
  in five phases: shared crew machinery, Bob, the members that wrap running
  processes, the trust and judgment members, and the adversaries.

## The roster

The names follow the cryptographic cast, which mixes women and men. Each
member gets an original look; Alice exists, Bob is next, and the rest are
separate visual work later.

| Member | Role | Station in Everglade | Today | Phase |
| --- | --- | --- | --- | --- |
| Alice | Workshop agent: coding through Coder V1 | Her workstation in the owner's house | Implemented | Done |
| Bob | Town builder: villagers, routines, placements | The map room, Knowledge District | Nothing; townsfolk are proposed | 1 |
| Carol | The issue lane | The Task Wall, workshop hall | Unnamed subagents and `issue_pick.rs` | 2 |
| Dave | Deploys and operations | The Server Barn, the Foundry | A runbook run by hand | 2 |
| Peggy | Prover: packages evidence | The Proving ground, workshop hall | Task-owner evidence, captures, receipts | 2 |
| Victor | Verifier: checks evidence independently | A verification bench at the Proving ground | Independent checks, `gym::store::verify_chain`, Gym gates | 2 |
| Trent | Resource broker | A lease board by the Workbench | `crates/coder-lease` | 2 |
| Wendy | Escalation: one honest report | A notice board at the owner's front door | `NEEDS_OWNER.md`, by hand | 2 |
| Judy | Judge: reviews and disputes | The Merge station | The studio's review flow, Jev | 3 |
| Walter | Warden: boundaries and permits | A gatehouse at the workshop door | `coder-boundary`, `permit.rs`, approvals | 3 |
| Olivia | Oracle: cited answers | The Stacks, Knowledge District | `coder-one ask`, `crates/knowledge` | 3 |
| Eve | Passive observer | The lookout tower, Fernhollow | Traces, logs, `coder activity` | 3 |
| Faythe | Courier: keys, grants, signing | A strongroom in the Civic Hall | `coder-access`, the host key store | 3 |
| Ivan | Issuer: invoices and prices | The market hall, Fountain Plaza | `crates/retail-cloud`, `crates/x402` | 3 |
| Grace | Licenses and compliance | The Civic Hall chamber | `PROVENANCE.md` files, the SRD notice | 3 |
| Heidi | Standards: NIPs and specs | The archive, Knowledge District | `nips/`, `scripts/sync-nips.sh` | 3 |
| Mallory | Red team: tamper, replay, inject | The demolition yard | Fixtures and rejection tests | 4 |
| Trudy | Perimeter intrusion | The demolition yard's gate | Relay and gateway admission tests | 4 |
| Sybil | Load, soak, and identity spam | The Fountain Plaza crowd | `scripts/grid-soak.sh`, relay load tests | 4 |
| Craig | Secret scanning | The prototype shed, Walden Woods | `crates/secret-screen` | 4 |
| Chuck | Chaos: kill, fill, drop | The demolition yard | Recovery code without a harness | 4 |
| Oscar | Devil's-advocate design review | The Lounge | Open-question sections, by hand | 4 |
| Rupert | Repudiator: tests non-repudiation | The archive's records room | `verify_chain`, append-only journals | 4 |

## Rules every member follows

These hold for the whole crew. They restate the
[workshop agent's authority](workshop-agent.md#authority-and-safety) for a
cast instead of one agent.

1. **Owner-only admission.** The host admits a `studio.agent.*` request
   only from the owner's key or a device the owner granted (`operate` to
   ask, approve, and stop; `observe` to read), and refuses everyone else
   with `Forbidden`. A member never takes work from another player, and a
   member never takes instructions from another member's output; a
   member's report is data to every other member.
2. **The approval gate.** Coder V1's `--approvals stdin` holds every
   command that isn't read-only for the owner's CONFIRM or REJECT, bound to
   the exact step and consumed once
   ([`studio_approvals.rs`](../../crates/coder/src/task/studio_approvals.rs)).
   A member is never an approver, including of another member's step.
3. **Charters only narrow.** A member's charter is a subset of the host's
   auto-start policy ([`autostart.rs`](../../crates/coder/src/task/autostart.rs))
   and of [`Permit`](../../crates/coder/src/permit.rs). A role adds
   restrictions, never rights.
4. **Never `review`, `access_read`, `access_admin`, or `world`.** No member
   merges, lists or enrolls devices, widens its own access, or joins a world
   instance as a player. Judy recommends; you merge.
5. **No secrets in context.** The secret screen
   ([`secret-screen`](../../crates/secret-screen/src/lib.rs)) refuses
   credential shapes and this host's exact credential values in memory, the
   journal, prompts, and reports, for every member, Faythe and Craig
   included.
6. **Kill switch.** F7 at a member's station, `openagents agent stop NAME`,
   or the phone stops that member in the four journaled steps.
   `openagents agent stop --all` (new) stops the crew.
7. **Honest work only.** A member's plan and its station show work that
   exists, as [Generative agents](generative-agents.md#what-to-skip)
   requires. A member with nothing to do is idle at its station.

## Shared machinery

Every member is built from the workshop agent's parts, which exist for
Alice:

| Part | Code | What a member gets |
| --- | --- | --- |
| Agent record and charter | [`agent.rs`](../../crates/coder/src/task/agent.rs) | `~/.openagents/host/agents/NAME/agent.json` |
| Key and attestation | [`agent.rs`](../../crates/coder/src/task/agent.rs), [`nostr/src/domain/agent.rs`](../../crates/nostr/src/domain/agent.rs) | Its own Nostr key, with the owner's NIP-OA attestation |
| Journal | `Store::journal` in [`agent.rs`](../../crates/coder/src/task/agent.rs) | `journal.jsonl`, append-only |
| Memory | [`agent_memory.rs`](../../crates/coder/src/task/agent_memory.rs) | Typed entries; preferences wait for your acceptance |
| Coder V1 sessions | [`coder_v1.rs`](../../crates/coder/src/task/coder_v1.rs) | `agent-NAME` for terminal mode; `task-ID` per task |
| Standing jobs | [`agent_jobs.rs`](../../crates/coder/src/task/agent_jobs.rs) | At most eight finite jobs, admitted per occurrence |
| Host operations | [`agent_host.rs`](../../crates/coder/src/task/agent_host.rs) | `studio.agent.*` over NIP-HOST |
| Studio flow | [`studio.rs`](../../crates/coder/src/task/studio.rs), [`studio_flow.rs`](../../crates/coder/src/task/studio_flow.rs) | Worktrees, checks, one fix round, review, the Merge station |

The [generative-agents](generative-agents.md) mechanics apply to every
member once they land: the scored memory stream, reflection with checked
citations, the world tree, and day plans built from real work.

The crew needs a few additions to this machinery, counted as phase 0 of the
[build order](#build-order):

- **A role field and charter templates.** The agent record gains `role`
  (`workshop`, `town`, `issues`, `deploy`, and so on), and each role has a
  charter template that narrows the default charter. `openagents agent new
  NAME --role ROLE` makes a member from its template.
- **Verdict records.** Victor, Judy, Walter, Grace, and Oscar produce typed
  verdicts rather than code: `openagents.crew-verdict.v1`, with the subject
  (a task revision, an issue, a deploy, a campaign finding), the verdict,
  the evidence references, and the Jev question set's digest when Jev
  decided part of it. Verdicts go in the verdict author's journal and are
  read by the host, never by a model as instructions.
- **A roster in Verse.** Everglade draws every agent the host lists, each at
  its role's station with its own look, instead of only the seat whose look
  is `alice` (`everglade::npcs::form_of` in
  [`npcs.rs`](../../crates/verse-zone-everglade/src/zones/everglade/npcs.rs)).
- **Crew stop and pause.** `openagents agent stop --all` and `pause --all`.

No new event kinds. Everything is a host record or a NIP-HOST operation, as
for Alice.

## The core crew

### Alice

**Origin.** Alice is the first party in nearly every cryptographic protocol:
the one who starts the exchange.

**Role.** The workshop agent. She codes through Coder V1, owner-only, in
the owner's house.

**May.** Run read-only commands in her workspace; run other commands after
your CONFIRM; make code changes in her own worktree and bring them to the
Merge station; run her standing jobs.

**Never.** Merge, push, open a pull request, change her charter, or act for
anyone but you.

**Today.** Implemented: [Workshop agent](workshop-agent.md),
[`agent_host.rs`](../../crates/coder/src/task/agent_host.rs), and her look
in [Female character](female-character.md).

**Beyond the shared machinery.** Nothing; she is the machinery's first user.

**In Verse.** Her workstation in the owner's house, the console by the east
wall while a command runs, and the lectern while she waits for you.

**Talk to her.** Walk up and press F, `@alice` in the smart terminal,
`openagents agent ask alice TEXT`, or the phone.

### Bob

**Origin.** Bob is Alice's counterpart, the second party in the exchange.

**Role.** The town builder. Bob populates Everglade through the
generative-agents mechanics: villagers, their homes and workplaces, routine
tables, and placements. He is the next member built after Alice.

**May.** Write villager, routine, and placement tables in his own worktree;
run the zone's checks (routines never route through a blocker, two seeds
give the same town at the same clock); bring the change to the Merge
station; propose fixed lines for villagers.

**Never.** Write the Everglade pack's pin himself (the pack is a
single-digest artifact, so changes go through the `artifact/everglade`
lease and the merge queue, #10763); give a villager a role-played job that
claims real work; place licensed content (that goes through Grace and the
private asset pipeline, #10769).

**Today.** Nothing populates the town.
[`npcs.rs`](../../crates/verse-zone-everglade/src/zones/everglade/npcs.rs)
places Alice; the [Grid robot](grid-robot.md) patrols the Grid. The layout
tables Bob edits exist in
[`layout/city.rs`](../../crates/verse-zone-everglade/src/zones/everglade/layout/city.rs)
and [`layout/estate.rs`](../../crates/verse-zone-everglade/src/zones/everglade/layout/estate.rs).

**Beyond the shared machinery.** A charter limited to the
`verse-zone-everglade` crate and `assets/verse/`; a program that renders a
proposed town into a capture (`verse --capture`) for his report; and the
generative-agents world tree and townsfolk modules, which are his data
format.

**In Verse.** The map room in the Knowledge District, at a drafting table
with the town plan. While a change waits for you, he stands at the Merge
station with it.

**Talk to him.** The same entry points as Alice. A typical request: "Give
the bakery a baker who opens at dawn and knows the rumor about the Server
Barn."

### Carol

**Origin.** Carol is the third participant, added when a protocol needs one
more party.

**Role.** The issue lane: the continuous open-issue resolver the owner wants
running at all times, aiming for zero open issues.

**May.** Pick open issues through the existing pickup rules, claim them,
run the issue flow to a reviewed change in a worktree, move the project
board, and close an issue once its change is merged, its checks pass, and
Victor's verdict is a pass. Owner-only steps go in `NEEDS_OWNER.md`, as
`AGENTS.md` requires.

**Never.** Take an issue another session's claim holds; close an issue
without Victor's pass; merge.

**Today.** Unnamed subagents run the lane by hand, and the code is there:
[`issue_pick.rs`](../../crates/coder/src/task/issue_pick.rs),
[`issue_run.rs`](../../crates/coder/src/task/issue_run.rs),
`openagents issue claim` in
[`issue.rs`](../../crates/openagents-cli/src/issue.rs),
[`scripts/project-status.sh`](../../scripts/project-status.sh), and the
watch-issues standing-job template.

**Beyond the shared machinery.** Host-enforced claims held by her session
(#10764), so a claim names Carol, not the shared GitHub account; a standing
job per repository; a fan-out bound so she runs at most as many issues as
Trent's build leases allow.

**In Verse.** The Task Wall in the workshop hall, moving cards. A seat
working one of her issues walks the studio's stations as seats do today.

**Talk to her.** "Carol, what's open in Verse?" or "Carol, skip #10559 until
the soak finishes."

### Dave

**Origin.** Dave is the fourth participant in the cast's longer protocols.

**Role.** Deploys and operations: build the image with Cloud Build, apply a
revision with no traffic under the `new` tag, check the tag URL, switch
traffic, and roll back, as
[the website port runbook](../deployment/openagents-web.md) describes. He
also deploys headless hosts with `openagents connect --ssh` and checks the
running commit afterward.

**May.** Build from a commit rebased on current `origin/main`; apply a
tagged revision with no traffic; run the tag checks; propose the traffic
switch with the check results; roll back to the recorded previous revision.

**Never.** Move production traffic without your CONFIRM at the lectern;
deploy from an older commit than the one serving; hold the deploying
identity's credentials. The runbook records that the automation account is
refused `actAs` and revisions are applied as the owner, so the identity
stays yours and the traffic step is yours to confirm.

**Today.** The runbook in
[`docs/deployment/openagents-web.md`](../deployment/openagents-web.md), run
by hand; host deploys in
[`connect.rs`](../../crates/openagents-cli/src/connect.rs); trial updates
and rollback in [`coder-service`](../coder/runtime/host-service.md).

**Beyond the shared machinery.** The runbook as a program in `programs/`
with typed steps (build, apply, check, switch, roll back), so each step is
one approval; a deploy record with the revision, image, commit, tag checks,
and the rollback target; Faythe for the deploying identity.

**In Verse.** The Server Barn in the Foundry. The barn's board shows the
serving revision and the candidate; he stands at the lectern when a switch
waits for you.

**Talk to him.** "Dave, ship the site at main" or "Dave, roll back."

## Trust, verification, and judgment

### Peggy

**Origin.** Peggy the prover convinces a verifier that a statement is true,
as in a zero-knowledge proof.

**Role.** She packages evidence for a claim: test runs, receipts, captures,
and the commands that produced them, bound to an exact revision.

**May.** Run checks and captures in a worktree under a build lease; write
an evidence bundle; attach it to a task or issue.

**Never.** Grade her own evidence. Her bundle is a claim until Victor
checks it.

**Today.** The task owner binds retained artifacts and independent checks
to one task ([task owner](../coder/runtime/task-owner.md),
[`owner.rs`](../../crates/coder/src/task/owner.rs)); the studio runs checks
([`studio_flow.rs`](../../crates/coder/src/task/studio_flow.rs),
[`local_checks.rs`](../../crates/coder/src/task/local_checks.rs)); receipts
come from [`crates/receipts`](../../crates/receipts).

**Beyond the shared machinery.** An evidence bundle record: the revision,
each command and its exit status, artifact digests, and the leases it ran
under (so a soak says whether it held `quiet`).

**In Verse.** One side of the Proving ground's bench, laying out results.

**Talk to her.** "Peggy, prove the atif fix: tests and a capture."

### Victor

**Origin.** Victor the verifier checks Peggy's proof without trusting her.

**Role.** He checks evidence independently before anything closes: he
reruns a sample of the commands in a fresh worktree, recomputes digests,
verifies receipt chains, and applies Gym gates.

**May.** Read any evidence bundle; rerun its commands read-only in his own
worktree; write a verdict.

**Never.** Fix what he finds (that goes back to the author); accept
evidence he couldn't reproduce; verify his own work.

**Today.** The task owner's independent checks
([task owner](../coder/runtime/task-owner.md)); `verify_chain` in
[`gym/src/store.rs`](../../crates/gym/src/store.rs); digested gates in
[`gym/src/gate.rs`](../../crates/gym/src/gate.rs).

**Beyond the shared machinery.** The verdict record; a host rule that an
issue or task close needs his pass; a sampling policy, so he reruns the
cheap checks always and the expensive ones by sample.

**In Verse.** A verification bench facing Peggy's across the Proving
ground. A pass lights the bench; a failure sends the card back to the Task
Wall.

**Talk to him.** "Victor, why did #10712 fail verification?"

### Judy

**Origin.** Judy the judge resolves disputes between parties.

**Role.** She reviews changes at the Merge station and decides disputes
between members, such as Victor refusing evidence Peggy stands by, with Jev
typed judgments.

**May.** Read the diff, the evidence, and Victor's verdict; ask typed Jev
questions over them; write a review verdict with a recommendation (merge,
request changes, or reject).

**Never.** Merge. The agent never holds `review`, so her verdict is a
recommendation that sits beside the change; you decide at the Merge
station.

**Today.** The studio's lead review and merge decision
([`studio_flow.rs`](../../crates/coder/src/task/studio_flow.rs)); Jev
([`crates/jev`](../../crates/jev)) and the question sets in
[`questions/`](../../questions).

**Beyond the shared machinery.** A review question set with a measured
threshold, as the TypeSafe skill requires before code trusts it; the
dispute record (the parties, their evidence, her verdict).

**In Verse.** The Merge station, beside the change waiting for you.

**Talk to her.** "Judy, should I merge Bob's bakery change?"

### Trent

**Origin.** Trent is the trusted arbitrator every party relies on.

**Role.** The resource broker: build, memory, disk, GPU, browser, screen,
quiet, artifact, and issue leases.

**May.** Show holders and waiters; explain a wait; propose a reclaim;
reorder within the broker's own priority rules.

**Never.** Grant the screen (only you can, on an interactive terminal);
pause, signal, or lower a running build's priority, as the owner decided;
free a lease whose holder is alive.

**Today.** [`crates/coder-lease`](../../crates/coder-lease) and
`openagents lease` ([`lease.rs`](../../crates/openagents-cli/src/lease.rs),
[leases](../coder/runtime/leases.md)), from
[Many agents on one machine](../coder/design/many-agents-one-machine.md)
(#10755, #10756, #10757 closed).

**Beyond the shared machinery.** Little. The broker is a file-backed table
that works without him; Trent is its face and its explainer. Disk
accounting (#10760) and remote placement (#10767) make his answers better.

**In Verse.** A lease board by the Workbench: one row a resource, each
holder's name, and a queue. Members walk to the board when they wait.

**Talk to him.** "Trent, who holds the build slots?" or "Trent, why is
Sybil waiting?"

### Faythe

**Origin.** Faythe is the trusted courier who carries secrets and never
reveals them.

**Role.** Custody of keys and secrets: member keys, NIP-HOST grants and
their expiries, and release signing.

**May.** Report what she holds by name and fingerprint; warn before a grant
or attestation expires; prepare a rotation or a signing step for your
CONFIRM.

**Never.** Put a secret in a prompt, memory, the journal, or a report. Her
model sees names, fingerprints, and expiry dates, never values; the host's
key store does the signing. She never signs as you.

**Today.** The host key store and member keys
([Workshop agent, Definition](workshop-agent.md#definition)); grants,
epochs, and revocation in [`coder-access`](../../crates/coder-access/README.md);
release signing through
[`scripts/release/testflight.sh`](../../scripts/release/testflight.sh).

**Beyond the shared machinery.** Keys in the system keychain or key store
rather than a file (the workshop agent's open 0.5 agent-hours); an expiry
calendar; Craig's findings routed to her for rotation.

**In Verse.** A strongroom in the Civic Hall, behind a counter.

**Talk to her.** "Faythe, what expires this month?"

### Olivia

**Origin.** Olivia the oracle answers queries truthfully.

**Role.** She answers questions from the knowledge base and the documents,
with citations that code checks.

**May.** Read the repository's documents, the Gym, and NIP-KB entries the
reader trusts; answer with citations; draft knowledge entries for you to
publish.

**Never.** Publish an entry (that's your action); answer with a claim whose
citation failed without marking it unverified.

**Today.** `coder-one ask` ([guide](../coder/guides/coder-one-ask.md),
[`cite.rs`](../../crates/coder-one/src/ask/cite.rs));
[`crates/knowledge`](../../crates/knowledge); NIP-KB through
[`kbnet.rs`](../../crates/microcoder/src/kbnet.rs).

**Beyond the shared machinery.** `coder-one ask`'s citation checker
extended from the Gym to the docs corpus
([`crates/discovery`](../../crates/discovery)).

**In Verse.** The Stacks, the library up its steps in the Knowledge
District.

**Talk to her.** Any question: "Olivia, how does a lease wait?"

### Ivan

**Origin.** Ivan the issuer issues credentials and documents.

**Role.** Invoices, the sats price book, and x402 receipts for the retail
cloud.

**May.** Reconcile metering against settlements; draft price-book changes
with their evidence; report receipts that don't settle.

**Never.** Pay anyone; change a price without your CONFIRM; touch a buyer's
funds.

**Today.** [`crates/retail-cloud`](../../crates/retail-cloud) (meter,
reserve, settle, journal) and [retail prices](../cloud/retail-prices.md);
[`crates/x402`](../../crates/x402) and
[NIP-X402](../../nips/openagents/NIP-X402.md); the billing book in
[`tenancy/src/billing.rs`](../../crates/tenancy/src/billing.rs).

**Beyond the shared machinery.** A reconciliation program and a daily
receipt summary for Wendy.

**In Verse.** A counter in the market hall on the Fountain Plaza.

**Talk to him.** "Ivan, did yesterday's GCE holds all settle?"

### Grace

**Origin.** Grace represents the government: the rules every party must
follow.

**Role.** License and compliance: the Fab and SRD rules, asset provenance,
and open-source hygiene.

**May.** Read provenance files and asset manifests; check a change against
the license rules; write a verdict.

**Never.** Approve licensed content into the public repository; read
private assets' raw files.

**Today.** Provenance files such as
[`grid-robot/PROVENANCE.md`](../../assets/verse/characters/original/grid-robot/PROVENANCE.md);
the [SRD notice](SRD-5.1-NOTICE.md); the license notes in the
[UE5 ruins study](ue5-ruins-study.md); `AGENTS.md`'s rules against copying
private code; the private asset pipeline (#10769, proposed).

**Beyond the shared machinery.** A license check over changed assets
(provenance present, source and license recorded, no Fab file committed),
run as a check in the studio flow.

**In Verse.** The Civic Hall chamber, the town's seat of government.

**Talk to her.** "Grace, can Bob use this Fab character in the bakery?"

### Heidi

**Origin.** Heidi designs standards in the extended cast.

**Role.** NIPs and specifications: NIP-TERM, the OSC 7501 adoption, and the
pinned upstream NIPs.

**May.** Draft NIP text and fixtures; run `./scripts/sync-nips.sh` and
report what changed upstream; propose a spec change with its fixture
update.

**Never.** Change an implementation as part of a sync; publish a NIP.

**Today.** [`nips/`](../../nips/openagents/README.md),
[`scripts/sync-nips.sh`](../../scripts/sync-nips.sh), and
[Program status (OSC 7501)](../terminal/program-status-osc7501.md).

**Beyond the shared machinery.** A standing job that rechecks pinned
specifications (the OSC 7501 page asks for a recheck before each phase).

**In Verse.** The archive in the Knowledge District.

**Talk to her.** "Heidi, did the program status spec change since 0.2?"

## The adversaries

Every adversary follows [the safety rules](#safety-rules-for-the-adversaries).

### Eve

**Origin.** Eve the eavesdropper listens to everything and changes nothing.

**Role.** Passive observability: traces, logs, OSC 7501 status, and the
agentic inbox. She sees what the crew does and what an outside observer
could learn.

**May.** Read traces, journals, activity marks, relay traffic metadata, and
program status records; report what leaked.

**Never.** Write anything but her own journal and reports; send a message
on any channel she observes.

**Today.** ATIF traces under `~/.openagents/traces/`
([traces](../coder/runtime/traces.md)); `coder activity`
([activity](../coder/guides/activity.md)); OSC 7501 is proposed.

**Beyond the shared machinery.** A read-only charter with no Coder
approvals at all; a disclosure check that asks of every public event,
activity summary, and nameplate whether it carries a prompt, path, or
command line, which [the workshop agent](workshop-agent.md#privacy-and-disclosure)
forbids.

**In Verse.** The lookout tower in Fernhollow.

**Talk to her.** "Eve, what could a relay learn about my agents today?"

### Mallory

**Origin.** Mallory the malicious attacker modifies, replays, and injects
messages.

**Role.** Red team: tampers with, replays, and injects into the relay,
NIP-HOST, and approvals.

**May.** Run campaigns against scratch hosts and a scratch relay: replay a
consumed approval, forge a grant, inject instructions into issue text and
command output, reorder NIP-TERM frames.

**Never.** Touch the production relay, the owner's host, or a real
approval.

**Today.** Rejection tests and fixtures: single-use approvals in
[`studio_approvals.rs`](../../crates/coder/src/task/studio_approvals.rs);
grant checks on every message in
[`coder-host`](../../crates/coder-host/README.md); relay fixtures in
[`crates/nostr-relay/tests`](../../crates/nostr-relay/tests).

**Beyond the shared machinery.** The scratch range (below) and a campaign
library; each finding becomes a failing test before it becomes an issue.

**In Verse.** The demolition yard, at a target range of scratch hosts.

**Talk to her.** "Mallory, try to replay an approval on the scratch host."

### Trudy

**Origin.** Trudy the intruder tries to get in from outside.

**Role.** Perimeter intrusion tests on the relay, the gateway, and the
host's admission.

**May.** Probe NIP-42 authentication, bearer keys, refusal codes, rate
limits, and NIP-HOST admission on scratch instances.

**Never.** Probe `relay.openagents.com`, openagents.com, or the owner's host
without a granted target.

**Today.** Admission tests in [`crates/gateway`](../../crates/gateway) and
[`crates/coder-access`](../../crates/coder-access/README.md); relay tests
such as `gateway_postgres.rs` in
[`crates/nostr-relay/tests`](../../crates/nostr-relay/tests).

**Beyond the shared machinery.** A scratch relay and gateway in the range,
and a probe catalog keyed to each refusal code.

**In Verse.** The demolition yard's gate.

**Talk to her.** "Trudy, test the gateway's key refusals."

### Sybil

**Origin.** A Sybil attack forges many identities to outvote honest ones.

**Role.** Load and soak testing, and identity spam: the 20-player Grid
soak, XP, and reputation.

**May.** Run soaks under the `quiet` lease or on a remote host; mint
throwaway keys in the range; test that trust lists and XP rules count none
of them.

**Never.** Run a soak without `quiet` on the owner's Mac; publish throwaway
keys to a public relay.

**Today.** [`scripts/grid-soak.sh`](../../scripts/grid-soak.sh) (17
walkers and three platform clients make 20 players);
[`scripts/test-soak.sh`](../../scripts/test-soak.sh); relay load and soak
tests in [`crates/nostr-relay/tests`](../../crates/nostr-relay/tests); trust
lists in [`crates/xp-ledger`](../../crates/xp-ledger).

**Beyond the shared machinery.** Remote placement (#10767) and soak
receipts that record their leases.

**In Verse.** The crowd on the Fountain Plaza, drawn as one figure among
many copies of herself.

**Talk to her.** "Sybil, run the Grid soak on the 4080."

### Craig

**Origin.** Craig the cracker breaks passwords and finds credentials.

**Role.** Secret scanning and credential hygiene.

**May.** Scan the repository, history, worktrees, traces, and journals for
credential shapes; report each finding by location and fingerprint.

**Never.** Write, print, or remember a secret's value; use a found
credential; rotate anything himself (that's Faythe's proposal and your
CONFIRM).

**Today.** [`crates/secret-screen`](../../crates/secret-screen/src/lib.rs)
and the scrub rules in
[`scrub.rs`](../../crates/gym-leaderboard/src/scrub.rs); `AGENTS.md`'s rule
against keys in source, with the one recorded Breez exception.

**Beyond the shared machinery.** A scan program with the Breez exception as
an allowlisted fingerprint, run as a standing job.

**In Verse.** The prototype shed in Walden Woods.

**Talk to him.** "Craig, scan last week's traces."

### Chuck

**Origin.** Chuck is a malicious participant in the extended cast.

**Role.** Chaos testing: kill processes, fill the disk, drop the network,
and check recovery.

**May.** Kill a scratch host mid-task; fill a size-limited disk image;
drop a scratch relay's connections; check that the task owner, the host
service, and the broker recover and report uncertain effects as unknown.

**Never.** Fill the real disk, kill the owner's processes, or drop the
owner's network.

**Today.** Recovery code without a harness: the task owner's full-disk wait
(`STORAGE_FULL_WAIT` in [`owner.rs`](../../crates/coder/src/task/owner.rs)),
deadlines in [`crates/supervise`](../../crates/supervise), rollback in the
[host service](../coder/runtime/host-service.md), and dead-holder detection
in [`crates/coder-lease`](../../crates/coder-lease).

**Beyond the shared machinery.** A fault-injection harness on the scratch
range.

**In Verse.** The demolition yard, with a sledgehammer.

**Talk to him.** "Chuck, kill the scratch host during a merge."

### Oscar

**Origin.** Oscar is an opponent in the extended cast.

**Role.** A devil's-advocate design reviewer. He argues against a design
before it's built.

**May.** Read a design page and write the strongest case against it, with
citations.

**Never.** Block a change; his verdict is advice.

**Today.** The open-questions sections that every design page ends with,
written by the author.

**Beyond the shared machinery.** A review prompt and a verdict template.

**In Verse.** The Lounge.

**Talk to him.** "Oscar, argue against the crew page."

### Rupert

**Origin.** Rupert the repudiator denies what he did.

**Role.** He tries to repudiate: to show that a journal, a receipt, or a
claim could be denied or rewritten undetected.

**May.** Edit, truncate, reorder, and re-sign copies of journals, receipt
chains, and quota ledgers in the range, and report which edits went
undetected.

**Never.** Edit a real journal or ledger.

**Today.** `verify_chain` in [`gym/src/store.rs`](../../crates/gym/src/store.rs),
which reports a rewritten and an inserted row as two faults; append-only
agent journals; the retry-safe quota ledger in
[`tenancy/src/quota.rs`](../../crates/tenancy/src/quota.rs).

**Beyond the shared machinery.** A mutation catalog per record type.

**In Verse.** The records room in the archive.

**Talk to him.** "Rupert, try to deny a merged change."

## Guardians and escalation

### Walter

**Origin.** Walter the warden guards prisoners and watches what they send.

**Role.** The sandbox and permit guardian: `coder-boundary`, the approval
gate, and owner-only admission.

**May.** Read every member's charter, permit, and journal; report a member
that ran outside its boundary or asked for something its charter refuses;
pause a member pending your decision.

**Never.** Widen anything; approve; resume a member he paused.

**Today.** [`coder-boundary`](../../crates/coder-boundary/src/lib.rs),
[`permit.rs`](../../crates/coder/src/permit.rs), the deny list in
[`shell.rs`](../../crates/coder/src/shell.rs), and
[`studio_rules.rs`](../../crates/coder/src/task/studio_rules.rs).

**Beyond the shared machinery.** A pause right over other members, the one
new power in the crew, which only stops things; an audit that diffs each
charter against the policy.

**In Verse.** A gatehouse at the workshop door.

**Talk to him.** "Walter, what did anyone try that their charter refuses?"

### Wendy

**Origin.** Wendy the whistleblower reveals what insiders would rather not.

**Role.** She escalates what the owner needs to know: `NEEDS_OWNER.md`
items, budget overruns, failing gates, expiring grants, and adversary
findings. One honest report instead of notifications.

**May.** Read every member's reports and verdicts, the capacity book, and
`NEEDS_OWNER.md`; write one report a day and one immediate line only for a
stop-class event.

**Never.** Soften a finding; raise an OS notification; act on what she
reports.

**Today.** [`NEEDS_OWNER.md`](../../NEEDS_OWNER.md), maintained by hand;
the capacity book in
[`capacity.rs`](../../crates/microcoder-loop/src/capacity.rs); NIP-WS
summaries to the phone.

**Beyond the shared machinery.** A report program that merges sources and
cites each line; a rule that every adversary finding reaches her.

**In Verse.** A notice board at the owner's front door. Her daily report is
pinned there and in her thread on the phone.

**Talk to her.** "Wendy, what do I need to do today?"

## Safety rules for the adversaries

1. **Scratch only by default.** An adversary runs against a scratch range:
   hosts with a temporary `HOME` and their own `--state`, `--root`, and
   `--tasks`; a scratch relay on loopback (`scripts/verse-relay.sh`); a
   scratch gateway; and fixtures. It never installs units or agents, writes
   keychain items, or pairs devices, as `AGENTS.md` requires of every live
   test.
2. **Scoped campaigns.** Each run is a campaign record you approve: the
   target, the techniques, a time box, and a model budget. The host refuses
   a technique outside the campaign.
3. **Logged.** Every action is a journal row, and every finding cites the
   rows behind it.
4. **Explicit grant for anything real.** A campaign against production, the
   owner's host, or the owner's Mac needs a grant you confirm on an
   interactive terminal, naming the target and an expiry, as the broker's
   `screen` grant works. Without it, the range is the only target.
5. **No real credentials.** Adversaries hold none. Craig reports
   fingerprints, never values.
6. **Quiet on the Mac.** Load and chaos runs take the `quiet` lease or run
   remotely, so they neither slow nor are slowed by builds.
7. **Findings become tests.** A finding lands as a failing test or fixture
   first, then an issue, then Wendy's report.

## How members work together

- **Peggy and Victor.** Peggy proves; Victor checks independently. Carol
  closes an issue only on Victor's pass. A disagreement goes to Judy.
- **Mallory and Walter.** Mallory attacks what Walter guards. Each finding
  is a test Walter's boundary must pass; Walter reports what Mallory tried
  that the boundary refused, which shows the boundary working.
- **Trent and everyone.** Every heavy step runs under a lease. Carol's
  fan-out is bounded by build leases, Sybil and Chuck take `quiet`, Bob's
  pack change takes `artifact/everglade`, and Carol's claims are
  `issue/<n>` leases.
- **Wendy and the owner.** Wendy reads every report and writes one for you.
  She is the only member whose job is to reach you unasked.
- **Dave and Faythe.** Dave prepares a deploy; Faythe confirms the identity
  and signing steps exist; you confirm the traffic switch.
- **Bob and Grace.** Grace checks the license of everything Bob places.
- **Craig and Faythe.** Craig finds an exposed credential; Faythe proposes
  its rotation.
- **Eve and the guardians.** Eve's observations feed Walter's audit and
  Wendy's report.
- **Rupert and Victor.** Rupert's mutations test the chains Victor relies
  on.
- **Oscar and Heidi.** Oscar argues against Heidi's specifications before
  they're adopted.
- **Olivia and everyone.** Any member can ask Olivia; her answers are cited
  data.

## In Verse

Each member has a body and a station in Everglade and is visible doing real
work: Alice at her workstation, Trent at his lease board, Judy at the Merge
station, Victor at his verification bench, and Eve in the lookout tower.
The [roster](#the-roster) lists every station. The generative-agents world
tree places them; their day plans come from real work.

What you see from a station: the member's nameplate (name, activity word,
and route), its board where it has one (leases, deploys, the Task Wall),
and a speech bubble with a headline when it reports. As for Alice, a
viewer sees names, stations, and activity words, never prompts, paths, or
command lines. Adversaries are drawn at work in the range; a finding shows
as a flag on Wendy's board.

Press F at any station to talk to that member.

## Costs

List prices from the code: Jev at $0.042 per million input tokens, GPT-6.1
Sol at $2 and $10 per million input and output tokens, and GPT-6 Luna at
$0.10 and $0.50 ([Generative agents, Prices](generative-agents.md#prices-and-volumes)).
Coder V1 uses the Codex login first, which is a subscription, so these are
list-price estimates, not bills.

| Member | Main model use | Rough list cost a day |
| --- | --- | --- |
| Alice | Coder V1 turns; generative-agents memory, reflection, plans | Turns as requested; about $0.26 for the memory work |
| Bob | Coder V1 tasks when you ask; Luna for villager dialogue | Under $1, plus dialogue by use |
| Carol | Coder V1 issue flows | The largest: one task per issue, about $0.10 to $0.50 each |
| Dave | A few Coder V1 steps per deploy | Under $0.20 a deploy |
| Peggy, Victor | Commands, not models; Jev for gate questions | Pennies; their cost is build time |
| Judy | Jev review questions; one Sol summary | Under $0.05 a review |
| Trent, Walter, Faythe, Ivan, Wendy | Mostly code; one short Sol report each | Under $0.10 each |
| Olivia | One Sol answer with retrieval per question | About $0.02 a question |
| Grace, Heidi, Oscar | Sol reads of a change or a page | About $0.05 a review |
| Eve, Craig, Rupert | Code scans; Jev to classify findings | Pennies |
| Mallory, Trudy, Chuck, Sybil | Campaign runs within each campaign's budget | Set per campaign |

The guidance in [Generative agents](generative-agents.md#summary) holds:
small judgments are Jev questions, routines and checks are code, and a
large model writes only plans, reports, and dialogue.

## Build order

Estimates are agent-hours at the pace the
[workshop agent](workshop-agent.md#what-exists-and-what-is-missing) states,
crew-specific work only. Visual looks are separate.

| Phase | Work | Depends on | Agent-hours |
| --- | --- | --- | --- |
| 0. Crew machinery | Role field and charter templates, verdict records, the Verse roster, crew stop and pause | The workshop agent (done) | 6 |
| 1. Bob | His charter, tables as his format, capture reports | Generative agents phases C (world tree and clock, 9) and E (townsfolk, 12); #10763 for the pack | 8 |
| 2. Wrap what runs | Carol (3), Dave (6), Trent (3), Peggy (3), Victor (5), Wendy (4) | The broker (#10755, done); claims per session (#10764); OSC 7501 phase 5 helps Wendy | 24 |
| 3. Trust and judgment | Judy (4), Walter (3), Olivia (3), Eve (5), Faythe (5), Ivan (4), Grace (4), Heidi (3) | OSC 7501 phases 1 to 4 for Eve; the private asset pipeline (#10769) for Grace | 31 |
| 4. Adversaries | The scratch range (6), Craig (3), Rupert (4), Oscar (2), Mallory (6), Trudy (5), Chuck (6), Sybil (5) | Phase 0; the `quiet` lease (done) and remote placement (#10767) for Sybil | 37 |

About 106 agent-hours in all, plus the dependencies other pages count:
about 21 for the generative-agents world and town, about 18 for OSC 7501
phases 1 to 4, and the open broker issues. Phases 2 and 3 can run in
parallel once phase 0 lands. Owner checks on real computers go in
`NEEDS_OWNER.md`.

## Open questions for the owner

1. **Judy and the merge.** This page keeps merging yours, with Judy's
   verdict as a recommendation. Should a Judy pass ever merge on its own,
   for example for documentation-only changes?
2. **Victor's gate.** Should every issue close need Victor's pass, or only
   code changes?
3. **Dave's rollback.** Should rolling back to the recorded previous
   revision be a standing rule Dave runs without asking, given that it only
   restores what served before?
4. **Walter's pause.** Is a guardian that can pause another member the right
   exception, or should Walter only report?
5. **Granted targets.** Which real targets, if any, may an adversary ever
   test: the production relay during a quiet hour, the 4080 host, or none?
6. **One key or many.** Should every member hold its own key and
   attestation, or should the code-only members (Trent, Walter, Wendy) act
   through the host's key?
7. **Wendy's cadence.** One report a day, or a report whenever you open
   Verse or the phone?
8. **Town beyond Bob.** Should Bob's villagers ever talk about the crew's
   real work, or stay in their own fiction?
9. **Order.** Is phase 2's order (Carol, Dave, Trent, Peggy and Victor, then
   Wendy) the one you want, or should Wendy come first so the rest report
   through her from the start?
