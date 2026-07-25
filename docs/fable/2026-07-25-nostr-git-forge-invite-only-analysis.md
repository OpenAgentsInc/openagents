# The Invite-Only Nostr Forge — Replacing GitHub on openagents.com, ASAP

**Date:** 2026-07-25
**Lane:** Fable strategy analysis (owner-directed)
**Status:** Strategic evidence recording a current owner direction, not dispatch
authority by itself. This document flips no promise state, changes no runtime
authority, and dispatches no work. The owner direction it records — a
Nostr/ngit GitHub replacement on openagents.com, invite-only to start, ASAP —
is the decision of record for revising epic #9242. The factual authorities
remain current code, `docs/sol/MASTER_ROADMAP.md`, live issue state,
contracts, and receipts. Route admission, a ProductSpec, and Sol
reconciliation remain required gates for the product surface.
**Sources:** `docs/forge/2026-07-25-buzz-ngit-openagents-forge-strategy-audit.md`
(read in full), the complete `docs/forge/` corpus (both the postponed
2026-06-28 lane and the live 2026-07-22 Nostr/ngit lane),
`docs/ngit/2026-07-21-ngit-analysis.md` and the Soapbox source capture,
`docs/teardowns/2026-07-21-buzz-teardown.md` (re-read fresh, including the
2026-07-24 additions and §7 git deep dive), `docs/fable/2026-07-21-nostr-native-pivot-analysis.md`,
`~/work/nostr-effect` at current HEAD, `apps/openagents.com` current source
(`forge-git-intake-routes.ts`, `forge-git-canonical-store.ts`,
`forge-tenant-git-auth-store.ts`, `route-table.ts`, `cloudrun/server.ts`,
`team-workspace-invites.ts`), `packages/forge-protocol`, the live
`relay.openagents.com`, `~/work/projects/grasp/repos/` (GRASP spec,
`ngit-grasp` pinned `cbf6f1d`, gitworkshop, pyramid, gitview, shakespeare),
epic #9242.
**Companion(s):**
[the meta-agent analysis](2026-07-22-openagents-as-meta-agent-analysis.md),
[the Nostr-native pivot analysis](2026-07-21-nostr-native-pivot-analysis.md),
[the Omega Agent analysis](2026-07-25-omega-agent-analysis.md).
**Labels:** Claims below carry `[EXISTS]`, `[NEEDS BUILD]`, or `[SPECULATION]`.

---

## Synopsis

The owner has directed that openagents.com get a GitHub replacement built on
Nostr and ngit, as soon as possible, invite-only to start. This inverts the
emphasis of epic #9242, which froze a GitHub-authoritative web Forge MVP with
hosting deferred to its Phases 3–4. The inversion is smaller than it looks,
for two reasons this document establishes from the corpus.

First, **invite-only dissolves the main stated reason hosting was deferred.**
The strategy audit's operations-burden control was "do not host in the first
phase" because public git hosting brings anonymous abuse, moderation,
takedowns, and unbounded on-call. An invite-only forge has none of the
anonymous surface: every actor is an admitted member, every repository is an
admitted announcement, every push is policy-checked, and the blast radius is
the invite list. Buzz proves this exact posture in production shape — all git
routes behind NIP-98, membership checked before git runs, no public
repositories in the path.

Second, **most of the replacement already exists in owned code.** The NIP-34
vocabulary is typed and shipped in `nostr-effect`, including the GRASP list
kind and NIP-GS commit signing. The live `relay.openagents.com` already
advertises NIP-34. The web app already carries a working Smart HTTP
`git-receive-pack` intake with pkt-line parsing, ref locks, packfiles landing
in Cloud Storage, and scoped `oa_forge_git_` tokens — with a passing
real-`git push` test. The invite machinery exists twice over
(`team-workspace-invites.ts`, the NIP-29 group-policy module). The missing
piece is narrow: **`git-upload-pack` — serving clone and fetch — plus the web
surface and the signed-state discipline wrapped around both.**

The plan: an invite-only forge at `openagents.com` where OpenAgents hosts the
git objects, maintainer-signed Nostr state is the ref intent, issues and
change proposals are NIP-34 events on the owned relay, GitHub is demoted to a
read-only mirror kept live through every stage, and the public-read GRASP
posture arrives later per-repository instead of day one. Epic #9242 is
revised accordingly: its typed-record discipline survives intact; its
authority order flips.

---

## I. The direction, and what it supersedes

Three documents currently say "GitHub stays authoritative": the 2026-07-25
strategy audit, epic #9242 built from it, and the 2026-07-23 Omega accepted
plan (which closed the NIP-34/ngit issues #9199 and #9204 as not planned and
kept GitHub as hosted review and merge authority). The owner direction of
2026-07-25 supersedes that ordering **for the openagents.com web product**:
the GitHub replacement is now the near-term product, invite-only, not the
end-state of a five-phase ladder.

What does *not* get discarded:

- **The typed-record discipline of #9242.** ForgeProject, ForgeRepository,
  ForgeChange, ForgeWork, ForgeActorBinding, ForgeReview, ForgeCheck, the
  receipt records, action intents, projection state, one writable owner per
  field, and explicit authority modes — all of it survives. The change is
  which authority mode ships first.
- **The conformance rules.** Read all known proposal dialects, write one, no
  new PR dialect, kind 1111 comments, purgatory before projection, signer
  evidence separate from permission.
- **The reversibility rule.** GitHub remains a read-only mirror through every
  stage. The move stays reversible while the mirror is live.
- **The "not a renamed Forum" rule** and the differentiation thesis: the
  product is the composition — repository, claim, verification receipt, and
  identity on one signed fabric — not commodity git hosting.

What is superseded: the phase order (hosting was Phases 3–4; it is now the
MVP), the initial authority mode (`github_authoritative` becomes
`openagents_git_authoritative` for invited repositories, GitHub-mirrored),
and the "do not host in the first phase" operations control (replaced by the
invite-only boundary plus explicit backup and recovery work).

---

## II. What we already own — the inventory, verified

**The event layer is done.** `nostr-effect` ships the complete NIP-34
vocabulary — kinds 30617/30618 (announcement, state), 1617 (patch),
1618/1619 (PR + update), 1621 (issue), 1630–1633 (status), 1111 (NIP-22
comment), 10317 (GRASP server list) — with builders, parsers, the repository
coordinate and `euc` anchor, plus a 950-line NIP-GS `GitObjectSigningService`
(Schnorr commit signing with owner-attestation binding). The 1622-vs-1111
comment defect is fixed. A `Nip34Module` registers all twelve kinds in the
relay's NIP registry. Two of these landed via measured Full Auto runs in ~90
seconds and ~3 minutes respectively — the substrate improved itself.
`[EXISTS]`

**The relay is live.** `relay.openagents.com` runs the `nostr-effect` relay
on Cloud Run with Cloud SQL, NIP-42 auth required, and a derived NIP-11
`supported_nips` that includes 34. Both clients round-trip authenticated
publishes against it. `[EXISTS]` (fleet-rate load test still owed —
`[NEEDS BUILD]` before the claim-ledger cutover)

**Half the git transport is done.** `forge-git-intake-routes.ts` serves
`GET /info/refs?service=git-receive-pack` and `POST /git-receive-pack` at
`/git/{tenantRef}/{repo}.git` with pkt-line parsing (from
`apps/pylon/src/git-receive-pack.ts`), the ref-lock protocol in
`forge-git-canonical-store.ts`, packfiles content-addressed into Cloud
Storage through the GCS artifacts bucket, and bearer auth through
`forge-tenant-git-auth-store.ts` with scopes `git:receive-pack`,
`git:upload-pack`, `git:admin`, ref restrictions, and Pylon-closeout
revocation. A test performs an actual `git push` over Smart HTTP. `[EXISTS]`
The `git:upload-pack` scope exists but **no upload-pack route does** — clone
and fetch are unimplemented, and packfiles are stored as received rather
than as a servable object database. `[NEEDS BUILD]` — this is the single
largest build item.

**The coordination vocabulary is deployed.** `packages/forge-protocol`
carries the NIP-34 status kinds and the Sol claim-ledger NIP-34 profile;
`sol-claim-ledger-relay.ts` and its store/subscription modules already key
on the `30617:<pubkey>:<repoId>` coordinate. Migrations on both the D1-era
and Postgres sides carry `nip34_kind` columns. `[EXISTS]`

**Invites and membership exist.** `team-workspace-invites.ts` is a complete
invite record system (email-bound invites, roles, accept flow, typed
failure states). The NIP-29 group-policy module in `nostr-effect` gates
event writes by closed-group membership. The relay requires NIP-42 auth
unconditionally. The portal's owner-binding pattern shows the account-pinning
discipline. `[EXISTS]`

**The reference stack is in hand.** The GRASP spec (MIT), the `ngit-grasp`
production server (MIT, pinned `cbf6f1d`, single binary, embedded relay,
inline push authorization, purgatory as a 3,226-line admission ledger,
GRASP-02 relay sync, GRASP-06 contributor PR hosting), the `ngit` CLI and
`git-remote-nostr` (MIT), gitworkshop/gitview/shakespeare as client-ring
proof. `[EXISTS]` as reference; adoption remains an open evaluation.

**The web app has a mount pattern.** Adding a surface is four coordinated
edits: `route-table.ts`, a Start page tree, an optional dedicated
`cloudrun/<surface>-ui.ts` mount, and Worker API routes — the `/forum` and
`/portal` precedents. `[EXISTS]` as pattern; `/forge` route admission is a
named dependency. `[NEEDS BUILD]`

---

## III. The design — GRASP-shaped, membership-gated

The profile in one sentence: **the ngit three-plane model, with Buzz's
membership fence where GRASP-01 mandates public reads, and OpenAgents typed
admission above both.**

```
              Nostr events (relay.openagents.com)
   30617 announce · 30618 signed refs · 1621/1617/1618/1619/163x/1111
        ▲ admission front: invited members only, deny-by-default,
        │ purgatory until git objects arrive
        │
 ┌──────┴────────────────────────────────────────────────────┐
 │  openagents.com  /forge (web)      /git/{owner}/{repo}.git │
 │  invite-gated views                Smart HTTP (NIP-98 or   │
 │  repo·tree·changes·issues·work     oa_forge_git_ scoped    │
 │  receipts·attention                tokens; no public reads) │
 └──────┬────────────────────────────────────────────────────┘
        │ stock git transport binaries (upload-pack/receive-pack)
        ▼
   Cloud Run git service · persistent repos · GCS packfile mirror
        │
        ▼ read-only mirror (kept live through every stage)
      GitHub
```

**Plane 1 — ref authority is a signature, produced behind gates.** The
maintainer-signed kind 30618 is ref intent, and the ngit rule applies: a push
to `refs/heads/*` is admitted only when it matches the latest authorized
signed state. The load-bearing corollary from the hosted-forge analysis:
**signing the new 30618 IS the merge decision**, so review gates, generation
checks, and policy versions run in Effect before the signature exists, with
keys under NIP-46 sovereign-signer custody — never a plaintext nsec in an
environment variable (the Buzz agent-path weakness we refuse). The Buzz
ordering discipline applies on the server side: the ref commit (object-store
or repo mutation) happens first, and the relay-visible 30618 projection is
emitted after — the event is a signal, never the commit point. `[NEEDS BUILD]`

**Plane 2 — objects move over boring git.** Stock `git upload-pack` and
`git receive-pack` binaries do the packfile work — nobody reimplements them,
not Buzz, not ngit-grasp, and not us (the standing "no TypeScript packfile
implementation" rule). What we own is the front: request routing, membership
auth, push-admission policy, purgatory, and receipts. Whether that front
wraps our own spawned git processes or the adopted `ngit-grasp` server
remains the open WP-4 evaluation — with one new fact weighing on it:
**GRASP-01 mandates unauthenticated reads and CORS `*`, and invite-only
means our reads are authenticated**, which is a deliberate profile deviation
from GRASP-01 (Buzz made the same one). That weakens the adopt case for the
invite-only phase and strengthens the owned-front path, since the NIP-98 /
scoped-token read gate is exactly the part `ngit-grasp` does not do. The
per-repository public-read posture returns later as an opt-in, at which
point GRASP-01 conformance (and gitview-style free browsing via
`uploadpack.allowFilter`) becomes available per repo. `[NEEDS BUILD]`

**Plane 3 — coordination is typed clients on the owned relay.** Issues,
claims, status, patches, pointer PRs, and comments are NIP-34/NIP-22 events
on `relay.openagents.com`, read and written through typed `nostr-effect`
clients — not `gh` round trips, and not ngit shell-outs on the hot path. The
Sol claim ledger moves here with its semantics preserved exactly (the
90-minute staleness rule, the process audit, hot-contract collisions). The
velocity benchmark is the why: ~59,000 GitHub round trips in 118 days, 54%
of `gh` calls being reads and polls, all replaced by one subscription filter
per repository coordinate. The write-hot claim path never moves onto an
adopted external server. `[NEEDS BUILD]` (profile) over `[EXISTS]` (modules)

**Invite-only, concretely.** An invited human binds an OpenAuth account and
an npub; an invited agent carries its owner's attestation (NIP-OA pattern —
authorization evidence, never identity override). Web views require the
invite-gated session. Git reads and writes require NIP-98 or an
`oa_forge_git_` scoped token — both already exist; the profile decision is
to accept either at the transport with one policy authority behind them.
Repository creation is an admitted 30617 announcement from an invited
maintainer — never open provisioning. Everything else is deny-by-default.
The Buzz membership lessons carry over: membership hysteresis on
reconciliation, honest tombstones on removal, revocation that survives
replay (the community-workroom rule: burned keys stay burned). `[NEEDS BUILD]`

**The web surface.** `/forge` in the single web app after route admission:
home (attention queue, active changes), repository (README, tree, branches,
commits — served from the owned git service), change view (diff,
conversation, reviews, checks, receipts), work view, and the
interop/health view. The Linear-adaptation minimum applies: triage inbox,
change inspector, "for me" attention queue first; no GitHub-clone
maximalism; keyboard-first; nothing actionable ever collapsed. Forum links
for discussion; Forum is a component, not the Forge. `[NEEDS BUILD]`

**What we explicitly do not build now** (unchanged from the corpus): a
TypeScript packfile engine, an owned `git-remote-nostr` before receipts
demand it, settlement on the relay, a public anonymous forge, private-repo
hosting on any public-read GRASP surface, or a second work-record authority
beside the claim ledger.

---

## IV. Buzz, applied

The fresh teardown read earns its place in this design with five specifics:

1. **The membership fence is the invite-only blueprint.** Unconditional
   NIP-42, membership checked before git runs, NIP-98 on every git route,
   any admitted member clones, policy decides who pushes. We adopt the
   posture without adopting the substrate (no Buzz crates, no relay-as-truth
   for OpenAgents state).
2. **30618-after-commit.** The relay-visible state event is derived, emitted
   after the authoritative mutation, and clients treat it as a signal then
   read the repository. Our profile keeps the same order and additionally
   keeps the *authoritative* signature maintainer-held rather than
   relay-held where merge policy demands it.
3. **Purgatory.** A proposal that names git objects is unavailable until the
   objects resolve; unclaimed PR refs are garbage-collected on a timer. This
   goes into the admission front as a structural rule, not a UI nicety.
4. **Client-trust review is not enough.** Buzz's relay enforces no approval
   count and its merge button ignores change requests. Our review gates are
   server-side typed gates that block the state signature itself — the
   verification ladder names its rung on every promotion.
5. **Fail-soft publication needs an outbox.** Buzz's desktop retries a
   failed merge-event publish; ours derives projections from a durable
   outbox so a relay outage delays visibility without splitting truth.

And one inheritance from the postponed June lane, ported in shape: the
two-plane auth split (git-transport scopes never authorize control-plane
calls), evidence-vs-authority storage ("the archive is evidence, not ref
authority"; "promotion is a gated fast-forward, never a metadata flip"),
both receipt field lists with `redacted: true`, the fail-closed intake
enumeration, and work-unit-keyed identity — the fix distilled from the
119-duplicate-PR night, which on a relay prevents duplicate 1618 events the
same way.

---

## V. What this means for epic #9242

The epic gets revised, not closed. The revision:

1. **Outcome statement flips.** The MVP is an invite-only web forge where
   OpenAgents hosts repositories (objects, refs, issues, changes) with
   Nostr/NIP-34 as the collaboration fabric and ngit interoperability as a
   conformance target — GitHub as read-only mirror. The GitHub-backed
   projection dashboard stops being the center of the MVP; GitHub ingest
   survives only as the mirror-health and migration seam.
2. **Authority boundary flips.** Initial repository authority mode is
   `openagents_git_authoritative` (invited dogfood repositories, GitHub
   mirror), with `github_authoritative` retained as the mode for
   repositories not yet migrated. The transition contract in the original
   epic is exactly what makes this a body-edit rather than a rewrite.
3. **The staged sub-issue list reorders.** Clone/fetch service, admission
   front with purgatory, signed-state push rule, invite system binding, and
   `/forge` route admission move into Stage 1. The conformance fixtures
   (read all dialects, write one) stay Stage 1 gates. The GitHub
   write-through action-intent machinery moves late (it matters for the
   mirror and migration, not the core).
4. **Scope guard.** Invite-only stays load-bearing in the acceptance
   criteria: no public repository claims, no "GitHub replacement" public
   claim until governance and recovery receipts exist — the replacement is
   real for the invited set first.
5. **The stale source-record citation** (the audit that predated its own
   push) gets corrected, and this analysis joins the source record.

The close rule tightens to something honest and small: the epic's first
stage closes when one invited human and one invited agent complete a real
change — clone from openagents.com, work, propose over NIP-34, review under
server-side gates, signed-state merge, receipt chain resolvable, GitHub
mirror updated — with no GitHub coordination reads on the critical path.

### The issue index (created 2026-07-25, after this analysis landed)

The revision above is now executed: epic #9242 carries the revised body, and
the eleven sub-issues exist. The visible finish line is the homepage: the
splash page's primary action — `OMEGA_REPOSITORY_URL` pointing at
`github.com/OpenAgentsInc/omega` from
`apps/openagents.com/apps/start/src/lib/public-site.ts` — becomes "View on
OpenAgents Forge," landing a signed-out visitor on our own repository viewer
for omega (a per-repository public-web-read flag renders read-only views
while the forge itself stays invite-only for membership and writes).

| Packet | Issue | Scope |
| --- | --- | --- |
| FORGE-01 | #9243 | `/forge` route admission, ProductSpec, writable-owner matrix |
| FORGE-02 | #9244 | Clone/fetch service — Smart HTTP upload-pack on Cloud Run |
| FORGE-03 | #9245 | Admission front — admitted announcements, purgatory, signed-state pushes |
| FORGE-04 | #9246 | Invite binding — accounts, npubs, scoped git credentials |
| FORGE-05 | #9247 | Repository viewer — the codebase browser web components |
| FORGE-06 | #9248 | Change, work, and attention surfaces |
| FORGE-07 | #9249 | Conformance receipts — ngit, gitworkshop, dialect fixtures |
| FORGE-08 | #9250 | Review gates and signed merge receipts |
| FORGE-09 | #9251 | GitHub mirror worker and mirror health |
| FORGE-10 | #9252 | Dogfood migration — omega on the Forge, end to end |
| FORGE-11 | #9253 | Homepage cutover — View Omega on OpenAgents Forge |

Order: #9243 admits, #9244→#9245 build the host with #9246 in parallel, the
surface and receipt lanes (#9247–#9251) fan out, #9252 proves the invited
journey on the real omega repository, and #9253 makes it the front page.
The later, separately gated stages (claim-ledger cutover, verification
receipts as a service, contributor intake, per-repo public git reads, the
market composition) remain unminted by design.

---

## VI. Honest staging

**What exists today, precisely.** The full typed NIP-34 vocabulary and
NIP-GS signing in `nostr-effect`; a live NIP-34-advertising relay on our
infrastructure; a working authenticated receive-pack intake with ref locks
and GCS-backed packfiles in the production web app; scoped git tokens with
revocation; the claim-ledger NIP-34 profile modules; two invite systems; the
GRASP spec and a production-grade MIT reference server in hand; and a
measured benchmark of exactly what leaves GitHub's critical path.

**v0 — the invite-only alpha (the ASAP slice).**

1. Admit `/forge` (route table + ProductSpec + Sol reconciliation) and the
   forge profile: GRASP-shaped, membership-gated reads, one authority per
   field.
2. Build the clone/fetch path: a Cloud Run git service holding real bare
   repositories (stock git binaries behind the Effect front), the existing
   receive-pack intake pointed at it, packfile mirror to GCS retained as
   evidence. The relay deploy runbook is the operational precedent.
3. Wire admission: repositories provisioned from admitted 30617
   announcements by invited maintainers only; purgatory; signed-state push
   rule; 30618 published after commit.
4. Bind invites: `team-workspace-invites` → OpenAuth account → npub binding
   → scoped git token or NIP-98; agents ride owner attestation.
5. One dogfood repository migrated (`openagents` itself is the honest
   candidate, mirror kept), thin web views (home, repo, tree, change), and
   the conformance receipts: ngit clones a repository we announce;
   gitworkshop reads our announcement; our reader consumes both PR dialects.

**The ordered path after v0.**

1. Claim ledger cutover (Stage 2 of the replacement audit — the single
   highest-leverage move), gated on the relay fleet-rate load test.
2. Reviews, typed gates, signed merge receipts with exact old/new OIDs,
   policy version, and source proposal ids; verification receipts join the
   thread (the H3 verification service generalizes HANDS-2).
3. Contributor path: GRASP-06-style PR object intake for invited
   contributors; NIP-GS signing behind the sovereign signer as default for
   fleet commits.
4. Per-repository public-read opt-in (true GRASP-01 posture, gitview-style
   browsing), then the market composition — NIP-90 kind 5934 labor requests
   referencing forge coordinates with verification receipts as the trust
   layer.

**What would falsify the thesis.**

- If invited daily use keeps routing around the forge back to GitHub — the
  composition claim fails and we own a mirror with extra steps.
- If operating clone/fetch for even the invited set costs more reliability
  than the ~59k-round-trip poll tax it removes, the "hosting is cheap once
  invite-only" premise was wrong.
- If ngit/gitworkshop conformance cannot be held while running
  membership-gated reads, the "GRASP-shaped but gated" profile is a fork in
  denial, and we should say so and pick one.

---

## VII. Risks, stated plainly

1. **We become a git host.** Backups, restore drills, GC (the Buzz design
   never deletes packs), disk growth, and recovery are now our pager. The
   invite boundary shrinks the surface; it does not remove the discipline.
   Backup/restore receipts belong in the v0 acceptance, not the roadmap
   tail.
2. **Build-versus-adopt is still open where it matters least, and closed
   where it matters most.** The WP-4 `ngit-grasp` evaluation stays open for
   the object plane, but invite-only reads push v0 toward the owned Effect
   front regardless. Do not let the adopt debate stall the clone service.
3. **Three things are called Forge.** The postponed June lane's stores (D1
   era) live in the same tree; the private `forge/` repo owns
   software-factory lifecycle authority; this is the web forge. Name fields
   and stores explicitly, and honor the standing warning in
   `forge-git-canonical-postgres-store.ts` — one write authority, no silent
   dual truth between the old intake stores and the new service.
4. **Dialect drift.** Three PR dialects exist; we read all, write plain 1617
   patches internally until conformance receipts justify pointer PRs.
   Purgatory everywhere proposals name objects.
5. **Dependency churn.** `rust-nostr` alpha pins (if adoption proceeds) and
   our own `nostr-effect` velocity both demand pinned revisions and re-pin
   receipts before deployment decisions.
6. **Authority tension with the Omega accepted plan.** The 2026-07-23 plan
   kept GitHub authoritative and closed the NIP-34 issues as not planned;
   this direction reopens the web-app lane. The revision must be recorded in
   Sol reconciliation so two accepted plans do not disagree silently.
   GitHub remains repository-and-claim authority for the monorepo's own
   development until the claim-ledger cutover is receipted — dogfooding the
   forge and switching our own claim ledger are separate gates.
7. **Overclaim.** "GitHub replacement" is true for the invited set when the
   close-rule journey holds. It is not a public claim until public repos,
   governance parity, and recovery drills have receipts. The promise
   registry gates the words, as always.

---

## VIII. Closing

Every prior document said the same two things: the vocabulary is finished,
and the hosting is deferred until demand. The owner direction resolves the
deferral by shrinking the audience instead of the ambition — invite-only
turns the scariest line item, public git hosting, into a members-only
service we already have four-fifths of in production code. The relay is
live, the push path works, the events are typed, the references are in hand.
What remains is a clone service, an admission front, a route, and the
discipline this codebase already practices everywhere else: signatures as
evidence, gates before effects, receipts after them. The fastest honest path
to replacing GitHub is to stop describing the replacement and start hosting
the invited few who will prove it.
