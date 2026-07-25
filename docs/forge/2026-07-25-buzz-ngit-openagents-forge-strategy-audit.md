# Buzz, ngit, GRASP, and OpenAgents Forge strategy audit

- Date: 2026-07-25
- Class: source-grounded comparative audit and strategy recommendation
- Target revision: `40e704fbcde58af734f9f3e3244cddba574aad6b`
- Buzz revision: `5a3b8176aac5f4bced452ac8920477c5e059b828`
- Fast Follow lane: gap analysis
- Product status: evidence only
- Implementation status: no product code changed

## Executive conclusion

Buzz, ngit/GRASP, and the historical OpenAgents Forge plans address different
parts of one problem.

- Buzz joins a team workspace, agents, Git, workflows, and audit records around
  one relay and one community.
- ngit and GRASP separate repository discovery, signed ref intent, and ordinary
  Git object transport. This is the strongest existing interoperability base.
- The historical OpenAgents Forge plan adds work records, bounded execution,
  verification, promotion gates, receipts, and delivery state.

These systems converge on standard Git transport and signed collaboration
facts. They do not converge on authority. Buzz gives its relay and community
policy a large role. ngit gives maintainer-signed ref state a large role.
OpenAgents requires typed admission and evidence before a signed fact can cause
an effect.

**Recommendation:** OpenAgents.com should build a web-based Forge in the near
term. The MVP must make existing GitHub work legible and actionable through
OpenAgents.com. It must also own stable project, repository, change, work,
identity, action, and evidence records that permit a later transfer of
authority away from GitHub.

GitHub must remain the repository, ref, review, check, merge, and claim
authority in the MVP. The Forge must show this boundary. Forum can supply
discussion and participation, but it is one component of the Forge and not the
Forge MVP.

This surface can give maintainers and contributors a reason to return. It can
also prove the Omega workroom and evidence model in public. Later stages can
move collaboration, Git intake, refs, and promotion to OpenAgents only after
their product, assurance, security, recovery, and operations gates pass.

Do not restore the canceled standalone Forge. Do not deploy Buzz as a product
dependency. Do not operate GRASP hosting only because the reference software
is mature. Start with conformance, projections, and one public dogfood
repository. Add hosting only after measured demand and a new ProductSpec.

## Evidence labels

This audit uses three labels.

- **Fact** identifies a statement in the pinned source or current OpenAgents
  authority.
- **Inference** identifies a reasoned conclusion from those facts.
- **Unresolved** identifies a question that the sources or current proof do not
  answer.

Vision documents describe intended outcomes. Source files and tests describe
implemented mechanisms. Neither source class proves a production outcome by
itself.

## Authority and source boundary

The current
[Sol roadmap](../sol/MASTER_ROADMAP.md)
and the
[Omega accepted plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)
cancel the separate Buzz deployment and the standalone Nostr relay and Forge
program. The current direction keeps GitHub as the hosted review and merge
authority. Omega can add selected Nostr identity, Git, evidence, and workroom
projections.

The June files under `docs/forge/` are useful design evidence. They are not a
current product plan. Each June plan was first postponed. The July 23 Omega
decision then canceled the standalone product direction.

The [Fast Follow specification](../../FASTFOLLOW.md) admits Buzz lessons as
research input. It does not grant implementation, release, or public-claim
authority. The expanded request permits `docs/buzz/` or `docs/forge/`. This
document uses `docs/forge/` because its final subject is the OpenAgents public
Forge strategy. This location does not admit a product change.

The Buzz source is public Apache-2.0 source from Block. This audit paraphrases
it and links to the pinned revision. It does not copy Buzz code.

## Pinned source set

### OpenAgents

- All 15 files under `docs/forge/`
- [ngit lane README](../ngit/README.md)
- [ngit and GRASP source analysis](../ngit/2026-07-21-ngit-analysis.md)
- [Buzz teardown](../teardowns/2026-07-21-buzz-teardown.md)
- [Omega Buzz recommendation](../buzz/2026-07-24-omega-buzz-full-parity-recommendation.md)
- [Sol roadmap](../sol/MASTER_ROADMAP.md)
- [Omega accepted plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)

### Buzz

- [README at the pinned revision](https://github.com/block/buzz/blob/5a3b8176aac5f4bced452ac8920477c5e059b828/README.md)
- [platform vision](https://github.com/block/buzz/blob/5a3b8176aac5f4bced452ac8920477c5e059b828/VISION.md)
- [Forge vision](https://github.com/block/buzz/blob/5a3b8176aac5f4bced452ac8920477c5e059b828/VISION_PROJECTS.md)
- [sovereignty vision](https://github.com/block/buzz/blob/5a3b8176aac5f4bced452ac8920477c5e059b828/VISION_SOVEREIGN.md)
- [agent vision](https://github.com/block/buzz/blob/5a3b8176aac5f4bced452ac8920477c5e059b828/VISION_AGENT.md)
- [architecture](https://github.com/block/buzz/blob/5a3b8176aac5f4bced452ac8920477c5e059b828/ARCHITECTURE.md)
- Git policy, Smart HTTP, NIP-34 builders, CLI, and live Git test sources at
  that revision

### Source identity and freshness

| Corpus | Identity | Evidence limit |
| --- | --- | --- |
| OpenAgents | commit `40e704fbcde58af734f9f3e3244cddba574aad6b` | Current tree and current authority at 2026-07-25 |
| Buzz | commit `5a3b8176aac5f4bced452ac8920477c5e059b828` | Source inspection only. No Buzz tests ran in this audit |
| ngit lane | pinned commits in the ngit analysis and later GRASP addendum | The July 22 addendum supersedes two stale July 21 findings |

The July 21 ngit analysis says `ngit-grasp` was unavailable and says
gitworkshop had no license file. The later
[GRASP addendum](2026-07-22-grasp-ecosystem-prior-art-addendum.md)
supersedes both statements. It records an acquired and tested MIT
`ngit-grasp` revision. It also records the later gitworkshop license result.

## Comparison at a glance

| Dimension | Buzz Forge | ngit and GRASP | Historical OpenAgents Forge | Current OpenAgents direction |
| --- | --- | --- | --- | --- |
| Primary thesis | The relay is the workspace and forge | Signed Git collaboration across replaceable hosts | Evidence-gated software factory and owned coordination | Omega workrooms with GitHub authority |
| Main object | Community, channel, event, repository | Repository coordinate, signed state, Git objects | Work record, change, lease, verification, promotion | Durable workroom and thread |
| Git transport | Smart HTTP through Buzz | Smart HTTP through any listed server | Smart HTTP through owned intake | Existing Zed Git and GitHub |
| Ref authority | Object-store and relay-managed Git state | Maintainer-signed kind 30618 | Canonical store with ref locks and promotion gates | GitHub until a separate cutover |
| Identity | Human or agent Nostr keys plus community membership | Maintainer keys and signer custody | Tenant identity, scoped tokens, executor identity | OpenAgents identity plus isolated signer and adapters |
| Review model | Branch channel and NIP-34 projections | Patches, PR events, comments, labels, web clients | Change inspector, review guide, verification ladder | Native Omega review with GitHub writeback |
| CI model | Relay coordinates external agents | CI facts can be Nostr events | Named verification receipts and proof rungs | Existing checks and OpenAgents receipts |
| Multi-tenancy | Host-selected community boundary | Open network and replaceable hosts | Tenant-scoped control and Git planes | Existing OpenAgents tenancy and audience law |
| Maturity | Git substrate exists. Integrated Forge flow is partial | Live CLI, server, and client ecosystem | Significant retained substrate. Standalone product canceled | Active Omega plan. No new Git host |

## 1. Product thesis

### Buzz

**Fact:** Buzz presents one self-hostable workspace where people and agents
share rooms. Messages, reviews, workflows, and Git facts use signed Nostr
events.

**Fact:** The Forge vision says a branch channel becomes the pull request, CI
dashboard, discussion, and permanent reason-for-code record.

**Inference:** Buzz differentiates through one collaboration surface, not
through new Git object mechanics.

**Unresolved:** The sources do not show that one chronological channel can
replace mature diff, review, stack, rebase, and merge-queue interfaces at
scale.

### ngit and GRASP

**Fact:** ngit makes Nostr a discovery and collaboration plane for Git.
GRASP is a server convention that joins a relay with standard Git Smart HTTP.

**Fact:** ngit separates three planes:

1. kind 30617 announces a repository,
2. kind 30618 states signed refs,
3. Git servers transport objects.

**Inference:** ngit is the best fit for host-independent Git interoperability.
It is not a complete team workspace or software factory.

### Historical OpenAgents Forge

**Fact:** The June plan joined an owned Git coordination layer with a software
factory. The factory tracked intake, scope, assignment, execution,
verification, promotion, delivery, and retention.

**Fact:** Its core objects included work records, changes, leases, evidence
bundles, verification receipts, promotion decisions, and delivery receipts.

**Inference:** Historical Forge covered more lifecycle authority than Buzz or
ngit. It also carried much more product and operations cost.

### Current fit

**Fact:** Omega makes the durable OpenAgents thread and workroom the product
object. The editor, channel, Git view, and agent panel are projections.

**Recommendation:** Preserve this product thesis. Do not create a second
canonical work object called a Forge repository, branch room, or relay thread.

## 2. Core architecture

### Buzz

**Fact:** Buzz uses one Axum relay as the main coordination point. Postgres
stores events and search data. Redis carries pub/sub. Object storage holds
media and Git data.

**Fact:** The relay authenticates users, verifies signatures, applies
membership rules, stores events, fans out updates, indexes search, and starts
workflows.

**Inference:** Buzz uses a centralized service with portable event formats.
It is not a peer-to-peer runtime.

### ngit and GRASP

**Fact:** Maintainer-signed state gives ref intent. Servers supply Git objects.
Several servers can mirror the same repository.

**Fact:** Current `ngit-grasp` uses an embedded relay, standard Git processes,
inline receive-pack authorization, contributor PR storage, and persistent
purgatory.

Purgatory holds a collaboration event until its referenced Git objects arrive.
This prevents a visible proposal from pointing to missing objects.

**Inference:** This architecture has the cleanest separation between identity,
ref intent, and object availability.

### Historical OpenAgents Forge

**Fact:** The June plan used a typed control plane, Smart HTTP intake, pack
archives, canonical ref rows, ref locks, a virtual queue, verification
receipts, and promotion decisions.

**Fact:** Pack archives were evidence. They were not ref authority.

**Fact:** Promotion was a gated fast-forward. It was not a metadata state
change and was not a GitHub PR merge.

### Current fit

**Recommendation:** Use the ngit three-plane model for public interoperability.
Keep OpenAgents admission and receipts above it. Keep GitHub and Git as the
current mutation authority.

## 3. Identity, authentication, and trust

### Buzz

**Fact:** People and agents have separate secp256k1 keys. NIP-OA can record
owner authorization without replacing the agent as event author.

**Fact:** Buzz combines identity, community membership, channel role, Git
policy, and branch protection.

**Fact:** Current Git policy can treat a managed agent as its owner for
repository authority. It also promotes a Bot to Member for baseline Git
access.

**Risk:** Channel membership can become broader Git authority than an
OpenAgents work packet permits.

### ngit and GRASP

**Fact:** ngit supports NIP-46 remote signing and NIP-49 encrypted local keys.
It does not require a plaintext private key on disk.

**Fact:** The GRASP state-match rule accepts a push when the new ref matches
the latest authorized signed state.

**Fact:** gitworkshop derives a trusted maintainer set from mutual listings and
one selected trust anchor. This reduces unilateral maintainer-listing attacks.

**Risk:** A valid signature proves control of a key. It does not prove a
person, organization, role, safe change, or accepted outcome.

### Historical and current OpenAgents

**Fact:** Historical Forge separated `git:*` transport scopes from `forge:*`
control-plane scopes. It bound each scope to a tenant and failed closed across
the boundary.

**Fact:** Current OpenAgents law treats signed Nostr facts as inputs. A signed
fact does not become a command, accepted outcome, or receipt.

**Recommendation:** Keep agent authorship and owner authorization separate.
Compose Nostr evidence with OpenAgents identity, work packet, generation,
policy, and target authority.

**Unresolved:** A public profile still needs key recovery, rotation,
revocation, organization binding, and dispute rules.

## 4. Git transport and interoperability

### Common ground

All three designs preserve ordinary Git object transport. None needs to put
large Git objects in Nostr events.

### Buzz facts

- Smart HTTP clone and push routes exist.
- NIP-98 protects reads and writes.
- NIP-34 builders exist for repository, state, patch, issue, PR, and status
  events.
- Branch policy checks fast-forward, deletion, force-push, role, and patch
  requirements.
- Object-store compare-and-swap prevents a losing push from publishing success
  state.

Buzz does not implement a `nostr://` remote helper. Its implemented Git routes
are workspace routes, and the inspected transport says there are no public
repositories in version 1.

### ngit and GRASP facts

- `git-remote-nostr` implements the standard Git remote-helper protocol.
- A repository has a host-independent NIP-34 coordinate.
- GRASP-01 offers normal HTTPS Smart HTTP beside a relay.
- GRASP-06 lets contributors upload PR objects without canonical push access.
- kind 10317 lists preferred GRASP servers.
- Unknown announcement tags round-trip for forward compatibility.

### Interoperability gaps

**Fact:** The pinned Buzz builder has materially converged with the ngit
pointer model. It writes kind 1618 and 1619 events with a repository
coordinate, tip commit, clone URLs, optional branch name, and optional merge
base. It does not write the older `target-branch` tag.

**Fact:** The current comparison has two main proposal forms:

1. standard kind 1617 patch events,
2. kind 1618 and 1619 pointer PRs, with profile details that still need
   conformance tests.

Earlier Buzz research found a `target-branch` profile at an older revision.
This audit does not treat that historical profile as the pinned Buzz design.

**Fact:** NIP-22 kind 1111 is the current common comment path. Older
OpenAgents research identified a kind 1622 mismatch.

**Unresolved:** The ecosystem still needs a stable PR profile, a CI profile, a
merge receipt, private read rules, and multi-host disagreement handling.

**Recommendation:** OpenAgents can add high value through conformance and
harmonization. It should not add a new dialect.

## 5. Branch, PR, review, CI, and workflow model

### Buzz vision and implementation

**Fact:** The vision defines branch creation, an automatic branch channel,
external CI, signed review, merge, and channel archive.

**Fact:** The source contains Git transport, PR and issue builders, CLI
commands, workflow triggers, conditions, schedules, and trace events.

**Fact:** Workflow approval can suspend a step, but persistence and resume are
not complete. The architecture says such runs currently fail.

**Fact:** The merge coordinator and web-of-trust reputation remain designed
work in the vision status table.

**Unresolved:** The inspected source did not prove automatic branch-channel
creation, merge-time archive, required review counts, stale review dismissal,
or a complete merge train.

### ngit and GRASP

**Fact:** ngit supports issue and PR commands. gitworkshop supplies browser
review, inline suggestions, labels, and CI event rendering.

**Fact:** GRASP stores and transports proposal objects. It does not decide
whether a change is correct.

### Historical OpenAgents Forge

**Fact:** Verification receipts bind base, head, pack digest, executor,
command, exit code, artifact refs, timestamps, and log digest.

**Fact:** Promotion receipts bind the queue position, gate results, blockers,
actor, target ref, and promoted head.

**Fact:** The planned verification ladder separated tests, exact replay,
model review, second-agent review, human review, and owner acceptance.

**Recommendation:** Reuse OpenAgents review and receipt semantics in the public
surface. Use Nostr events for portable proposals and facts. Do not let relay
events replace verification or promotion state.

## 6. Agent roles and permissions

### Buzz

**Fact:** Buzz proposes triage, review, docs, merge coordinator, and coding
agent roles.

**Fact:** Agents use their own keys and histories. Agent tools can still run at
the operator's shell trust level.

### ngit and GRASP

**Fact:** ngit distinguishes maintainers from contributors through signed
state and contributor object namespaces. It does not define a complete agent
runtime or execution policy.

### OpenAgents

**Fact:** Historical Forge separated lifecycle authority from execution.
Pylon ran bounded work. Forge tracked change and promotion state. Artanis could
select or diagnose work only through typed gates.

**Recommendation:** The public surface must show these separate roles:
author, owner, agent, reviewer, verifier, approver, and merger. One actor can
hold several roles, but the UI must not collapse them.

**Recommendation:** A public contribution should create a proposal. It must not
grant shell, merge, deploy, payment, or release authority.

## 7. Events, audit, evidence, and reputation

### Buzz

**Fact:** Buzz signs platform events and keeps a hash-chain audit record.

**Fact:** The implemented audit actions are generic. They cover event,
channel, member, authentication, and rate-limit actions. The list does not
name dedicated Git push, review, CI, merge, or release actions. NIP-42
authentication events and ephemeral events are not in this audit log.

**Fact:** Its proposed reputation view uses contribution and approval history.
The status table says this system is designed, not shipped.

**Unresolved:** A derived kind 30618 event can record ref state, but it is not
the same as a Git effect receipt. The inspected sources do not prove a complete
audit chain from push through review, verification, merge, and release.

### ngit and GRASP

**Fact:** NIP-34 gives portable repository, patch, issue, PR, comment, label,
and status facts.

**Inference:** The maintainer graph and merged contribution history can supply
reputation evidence.

**Risk:** Event count, signed history, or maintainer proximity is not a quality
score. Sybil resistance, decay, context, disputes, and negative evidence still
need rules.

### OpenAgents

**Fact:** OpenAgents receipts distinguish attempts, observed effects,
verification, acceptance, delivery, release, and public claims.

**Recommendation:** Build a public contribution evidence card before a public
reputation score. Show exact facts:

- who signed the proposal,
- which owner or organization bound the actor,
- which commit and repository were in scope,
- which checks ran,
- which reviewer made each decision,
- which authority merged the change,
- which receipt confirms the final effect.

Do not compute a single reputation number in the near term.

## 8. Multi-tenancy and sovereignty

### Buzz

**Fact:** Buzz selects a community from the request host before it processes
auth, events, media, workflows, search, or Git.

**Fact:** Its default model is one community per relay. Its hosted model can
place several communities on shared infrastructure.

**Fact:** Identity keys can move across communities. Profiles, membership,
messages, repositories, and audit records remain community-local.

**Unresolved:** The source set does not define a complete operator-to-operator
export of Git objects, events, media, audit state, and workspace mappings.

### ngit and GRASP

**Fact:** Repositories can name several object hosts and several relays.
Maintainer keys retain ref intent across host changes.

**Fact:** Public reads are part of the normal GRASP posture.

**Risk:** This is a good fit for public open source. It is not a private
customer tenancy model.

### OpenAgents

**Fact:** Historical Forge designed tenant-bound Git and control scopes.
Current OpenAgents already has separate audience, tenancy, privacy, and receipt
rules.

**Recommendation:** Limit the first public surface to public repositories and
public-safe evidence. Private repository hosting needs a separate security,
retention, billing, backup, moderation, and recovery design.

## 9. Implementation maturity

### Buzz maturity

The following mechanisms exist in the inspected tree:

- Git Smart HTTP routes
- NIP-98 authentication
- NIP-34 event builders
- agent-oriented JSON CLI commands
- object-store Git state
- ref policy checks
- audit and workflow infrastructure
- ignored live Git end-to-end tests

The following outcomes remain partial, ambiguous, or designed:

- persistent workflow approval and resume
- automatic branch rooms
- complete code-review approval semantics
- merge coordination
- web-of-trust reputation
- public anonymous Git hosting

No Buzz tests ran in this audit. These are source-inspection findings.

### ngit and GRASP maturity

**Fact:** ngit CLI is a large active MIT Rust client with tests.

**Fact:** The current `ngit-grasp` addendum records a pinned MIT server,
production instances, persistent purgatory, synchronization, and successful
source acquisition through GRASP.

**Fact:** gitworkshop is a mature web client. Shakespeare proves a production
browser AI-builder use of the substrate.

**Limit:** These facts prove protocol and hosting maturity. They do not prove
OpenAgents verification, tenant, private repository, or software factory
requirements.

### OpenAgents maturity

**Fact:** The tree retains `packages/forge-protocol`, control-plane routes,
Git intake, token stores, canonical stores, mirror receipts, migrations, and
tests.

**Fact:** `apps/forge/` is absent.

**Fact:** The current roadmap cancels the standalone product and keeps the
retained substrate as evidence and reusable code.

**Conclusion:** It is inaccurate to call OpenAgents Forge either unbuilt or
active. Significant substrate exists. The standalone product does not.

## 10. Main gaps and risks

### Authority drift

A signed event can look final when it is only a proposal. A relay acceptance
can look like delivery. A test pass can look like acceptance.

**Control:** Show proposal, observed effect, verification, acceptance, merge,
delivery, and release as separate states.

### Split truth

Git refs, GitHub PRs, Nostr events, OpenAgents workrooms, and receipt stores can
disagree.

**Control:** Name one writable owner for every field. Show source identity and
projection freshness. Fail closed on conflicting mutation state.

### Dialect fragmentation

Standard patches and pointer PRs use different proposal shapes. The pinned
Buzz and ngit pointer profiles are now close, but exact conformance remains
unproven. Historical Buzz data can also contain the older target-branch form.

**Control:** Publish a conformance matrix. Read all known profiles. Write only
one admitted profile. Work upstream before a custom extension.

### Event and object races

A proposal can reach a client before its Git objects.

**Control:** Use persistent purgatory or an equivalent object-availability
gate. Do not publish a usable proposal projection before object proof.

### Identity overclaim

A key is not a person. Owner attestation is not employment. Contribution
history is not code quality.

**Control:** Present exact evidence and uncertainty. Keep organization binding,
role, and action authority separate.

### Duplicate product risk

GitHub, ngit, GRASP, and gitworkshop already solve mature parts of the problem.

**Control:** Build the OpenAgents differentiator. That differentiator is the
work, agent, evidence, and outcome record.

### Operations burden

Public Git hosting brings disks, backups, garbage collection, abuse,
moderation, takedowns, private access, support, and on-call work.

**Control:** Do not host in the first phase. Require measured demand and a new
admitted plan before any server commitment.

### Empty return loop

A public surface with few active changes will not create repeat visits.

**Control:** Start with one active dogfood repository and one recurring public
work stream. Do not launch a general directory before the first work stream is
useful.

## 11. Should OpenAgents.com build a public Forge surface?

### Recommended disposition

**Yes. Build a GitHub-backed web Forge now and design an explicit path to an
OpenAgents-owned Forge.**

The MVP is not a new Git host. It is also not a Forum view with a Forge label.
It is a web product that joins repository state, changes, diffs, agent work,
reviews, checks, decisions, and evidence.

The MVP must use GitHub as the repository and merge authority. It must own the
stable OpenAgents records and authority seams that prevent permanent coupling
to GitHub.

This is a strategy recommendation. The owner authorized one GitHub epic for
this scope on 2026-07-25. The issue does not by itself admit implementation,
release, or a public replacement claim. The current route contract does not
include `/forge`. A ProductSpec, AssuranceSpec, route decision, Sol
reconciliation, and bounded claims remain dependencies.

Do not restore the deleted standalone `apps/forge/` service. The intended web
surface belongs in the single `apps/openagents.com` application after route
admission. Forum can provide linked or embedded discussion with its existing
audience and moderation rules.

### Audience and value

#### Maintainers

Maintainers need one place to see:

- the current repository and change state,
- what agents are doing,
- which change needs attention,
- which evidence exists,
- who made each decision,
- what can safely happen next.

They also need bounded actions without a context switch. The MVP can write
selected commands through to GitHub and confirm the resulting GitHub state.
Merge remains on GitHub.

The value is review compression with evidence and action, not faster code
generation.

#### Contributors

Contributors need:

- repository and change views that explain the exact base and head,
- public work with clear scope,
- a portable proposal path,
- visible review state,
- exact feedback,
- durable credit for accepted work.

The value is a trusted path from contribution to outcome.

#### Agent and tool builders

Builders need:

- a public interoperability target,
- conformance results,
- typed work and evidence records,
- a way to attach their agent without surrendering its identity or custody.

The value is integration proof, not a new vendor account.

#### Observers and potential users

Observers need a legible answer to one question: can OpenAgents complete useful
work with agents and prove what happened?

The public record can answer that question better than a marketing page.

### Why users could return

The return loop should come from changing work state:

- a branch or change moves,
- a new proposal arrives,
- an agent starts or stalls,
- verification finishes,
- a reviewer requests a change,
- a decision becomes ready,
- a merge or delivery receipt appears,
- a contributor builds a verified record.

Do not use generic feeds or activity counts as the main retention mechanism.

## 12. Credible near-term scope

### Can credibly ship after normal admission

A first public slice can:

1. provide Forge home, project, and repository views for one public dogfood
   repository,
2. show repository identity, README, tree, branches, commits, and projection
   health from GitHub,
3. show a change view with an exact base, head, file diff, conversation,
   reviews, checks, agent work, decisions, and receipts,
4. show a work view that links scope, assignee, agent thread, blockers,
   verification, and target change,
5. show an attention queue for failed checks, review requests, stale state,
   authority conflicts, and owner decisions,
6. show exact actor roles and public contribution evidence without a
   reputation score,
7. permit selected GitHub-backed actions through typed action intents,
   idempotency, authority checks, and read-after-write receipts,
8. expose signed NIP-34 announcements and proposal projections,
9. link to GitHub for authoritative repository, review, check, claim, and merge
   state,
10. render source revision, freshness, disagreement, and recovery state.

The first slice should use one OpenAgents dogfood repository. It should be
useful without OpenAgents-hosted Git. It must not use a GitHub-specific object
model that prevents a later authority transfer.

### Can credibly prove now as research

The team can produce these receipts before a product surface:

- OpenAgents announcement readable by ngit and gitworkshop
- ngit clone or fetch from an announced repository
- OpenAgents reader for standard patches plus the current ngit and pinned Buzz
  pointer profiles
- kind 1111 comment conformance
- object-before-projection failure test
- signed event to GitHub-authoritative change cross-reference

### Remains vision or later work

Do not claim these outcomes in the first phase:

- OpenAgents-hosted Git repositories
- private repository support
- multi-host recovery
- complete GitHub or GitLab replacement
- native merge authority
- mature code reputation
- paid public agent marketplace
- general tenant onboarding
- complete branch-room automation
- public release or deployment authority

### Core owned model

The MVP needs stable OpenAgents records for:

- project,
- repository,
- ref snapshot,
- change,
- work,
- actor binding,
- review,
- check,
- evidence receipt,
- decision receipt,
- action intent,
- projection state.

Each field must have one writable owner. Imported GitHub state,
OpenAgents-native state, and portable NIP-34 state must remain distinguishable.

The repository record must name its authority mode. The initial value is
`github_authoritative`. Later values such as
`openagents_collaboration_authoritative` or `openagents_git_authoritative`
require their own admitted gates.

## 13. How the Forge folds into current plans

### Omega

Omega remains the authoring and supervision client. Its workroom should produce
the same public-safe work, review, and evidence records that the Forge renders.

### Agent work

Existing agents attach through explicit adapters. They keep their homes,
credentials, memory, tools, and sessions. The public surface shows capability
gaps and effective authority.

### Evidence

OpenAgents receipts remain the differentiator. Nostr facts add portable
identity and proposal evidence. They do not replace effect receipts.

### Identity

Use an isolated signer and explicit identity bindings. Preserve agent
authorship. Show owner authorization as separate evidence.

### Website

Use one Forge experience in `apps/openagents.com`. A canonical `/forge` route is
the intended destination. The current public route contract does not admit that
route, so route admission is an explicit dependency.

Forum can provide discussion, notifications, moderation, and participation.
It must not replace repository, tree, commit, diff, review, check, work, or
evidence views.

### GitHub

GitHub remains canonical for repository objects, refs, claims, GitHub review,
required checks, and merge in the MVP.

OpenAgents owns the Forge projection, work and agent records, identity
bindings, evidence, action intents, and transition contract. A Forge action is
not complete when GitHub accepts an API request. OpenAgents must read the
resulting authoritative state and record a receipt.

## 14. Recommended phased roadmap

### Phase 0: contract and conformance

**Goal:** prove the data model before a public product claim.

- Freeze the project, repository, ref, work, change, actor, review, check,
  receipt, action-intent, and projection-state models.
- Name one writable owner for each field.
- Freeze authority modes and the GitHub adapter contract.
- Test standard patches and the current ngit and pinned Buzz pointer profiles.
- Test the historical Buzz target-branch profile only if retained data requires
  compatibility.
- Resolve kind 1111 comments.
- Test event-before-object failure.
- Publish a conformance report.

**Exit gate:** one pinned matrix passes against live clients. Every field has
one owner. Every mismatch has an explicit loss or rejection. Stale projection
and event-before-object tests fail closed.

### Phase 1: GitHub-backed web Forge

**Goal:** create a useful and actionable Forge without new Git hosting.

- Add Forge home, project, repository, change, work, attention, actor/evidence,
  and interoperability-health views.
- Render GitHub-authoritative objects, refs, changes, reviews, checks, claims,
  and merges.
- Link agent work, checks, review, and receipts.
- Add bounded GitHub write-through actions with typed intent, idempotency,
  authority checks, and read-after-write confirmation.
- Use Forum for discussion and notifications where its contracts fit.

**Exit gate:** maintainers use the view for one real feature from issue to
merge. The record agrees with GitHub and target receipts. The Forge has a
useful action path and is not only a dashboard.

### Phase 2: OpenAgents-owned collaboration authority

**Goal:** remove agent coordination and evidence from the GitHub API critical
path.

- Make OpenAgents work, claims, agent leases, native reviews, verification,
  decisions, and collaboration records authoritative in an owned typed store.
- Project selected facts to GitHub and NIP-34.
- Keep GitHub authoritative for Git objects, refs, and merge.

**Exit gate:** one agent work stream completes without GitHub coordination
reads. Repository mutations still use GitHub.

### Phase 3: portable proposals and owned Git intake

**Goal:** accept a contribution without requiring a GitHub pull request.

- Accept one tested ngit or NIP-34 proposal path.
- Add standard Smart HTTP intake behind scoped credentials and ref
  restrictions.
- Validate objects and keep unresolved pointer proposals in purgatory.
- Review and verify the change in Forge.
- Mirror the accepted proposal to GitHub.

**Exit gate:** one proposal enters without a GitHub pull request and reaches an
accepted or rejected terminal state. GitHub remains the protected target and
mirror.

### Phase 4: OpenAgents ref and promotion authority

**Goal:** make OpenAgents the canonical Git mutation authority.

- Add canonical object and ref storage.
- Add ref locks and protected-ref policy.
- Add a deterministic merge queue.
- Gate promotion on exact verification and decision receipts.
- Fast-forward the canonical ref after gate success.
- Mirror the promoted commit to GitHub.
- Prove backup, restore, signer recovery, and failover.

Required inputs:

- measured clone and proposal demand
- private versus public repository demand
- moderation and abuse design
- backup and disaster recovery design
- signer and key recovery design
- Git maintenance and on-call budget
- multi-host conformance
- new ProductSpec and AssuranceSpec
- owner admission

**Exit gate:** one admitted promotion updates an OpenAgents canonical ref and
GitHub receives the same commit as a verified projection.

### Phase 5: GitHub-optional operation

**Goal:** decide whether OpenAgents can make GitHub optional.

- Add tenant isolation and private-repository controls.
- Add import, export, mirror, and disaster recovery.
- Add moderation, abuse, takedown, and support operations.
- Add governance and ecosystem integration parity for the admitted audience.

**Exit gate:** a separate launch decision proves the required Git forge
capabilities without GitHub. The decision can remain "GitHub required."

## 15. Success measures

Measure useful return behavior, not activity volume.

- weekly returning maintainers for the selected work stream
- time from proposal to first grounded review
- time from failed check to a new verified result
- share of displayed changes with exact source and receipt refs
- share of attention items that reach an explicit terminal state
- external proposals that complete the full review path
- conformance pass rate across supported NIP-34 profiles
- projection disagreement and stale-data rate
- owner-reported review time saved
- share of selected web actions that reach a confirmed authority receipt
- GitHub API reads removed from agent coordination after Phase 2
- successful import, mirror, and recovery drills before any authority transfer

Do not use event count, agent turn count, or generated line count as success.

## 16. Strategic lessons

1. **Keep Git boring.** Use standard Git for objects and refs.
2. **Make collaboration portable.** Use NIP-34 for public proposals and facts.
3. **Keep authority typed.** A signature is evidence, not permission.
4. **Make object availability explicit.** Use purgatory before projection.
5. **Show the full decision chain.** Connect intent, work, review, proof, and
   effect.
6. **Preserve agent authorship.** Record owner authorization separately.
7. **Prefer conformance leadership to dialect growth.** Do not add a new PR
   format.
8. **Build a Forge, not a renamed Forum.** Use Forum as a participation
   component.
9. **Delay reputation scores.** Start with exact contribution evidence.
10. **Own the seams before the servers.** Stable identifiers, authority modes,
    action receipts, and projections make staged replacement possible.
11. **Let proof earn authority.** Mature reference code does not create product
    authority or customer need.

## Final recommendation

OpenAgents should build a public web Forge around one active repository. It
should connect GitHub repository state, Omega workrooms, agent activity,
review, CI, identity, actions, and receipts in one product.

The MVP must keep GitHub authoritative and say so. It must also own the object
model, identity bindings, action receipts, evidence, projection health, and
authority modes that let later stages move collaboration, Git intake, refs, and
promotion to OpenAgents.

Use Forum for participation, not as the whole Forge. Use ngit and GRASP as
tested interoperability peers and possible later infrastructure. Use Buzz as a
source of signed-workspace, agent, workflow, and audit lessons.

Do not claim a GitHub replacement in the MVP. Make the replacement path
credible through typed boundaries, conformance, dogfood evidence, and explicit
stage gates.
