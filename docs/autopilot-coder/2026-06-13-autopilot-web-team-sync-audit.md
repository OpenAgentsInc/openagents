# Autopilot Web UI ↔ Team & Pylon Sync — Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-13
Status: design audit. No code/invariant changes here. Audits the existing
`/autopilot` web surface in the `openagents.com` app and specifies what is needed
to sync it to the newly built Pylon / `autopilot-control-protocol` system so a
**team-scoped, shareable, read-capable dashboard** works end to end. Described
generically (no specific tenant/use-case names).

## 1. The target flow (generic)

A lead user can:
1. **Declare a team** (a named group) and a **project** under it, with the lead
   user as owner.
2. **Invite other members by email**; invited members get **read access** to the
   team's project dashboard (the lead has full access).
3. **Attach a GitHub repository** to the project.
4. **Attach one or more ongoing Pylon sessions** to the project so they appear
   **live** in the web UI.
5. View a **shareable, multi-member-readable web dashboard** of the project and
   its build-out status.

Today this surface exists only partially and is effectively restricted to the
single owner/admin account. This audit maps current state → gaps → a phased plan.

## 2. Where it lives

- **Active web app:** `apps/openagents.com/apps/web` (Foldkit/Effect/Tailwind),
  in the `openagents` repo. (`autopilot-omega` is an older fork missing the
  autopilot work/decisions routes and the protocol deps — not the target.)
- Routes (`apps/openagents.com/apps/web/src/route.ts`): `/autopilot` (chat
  workroom), `/autopilot/work` + `/autopilot/work/:ref` (work-order dashboard),
  `/decisions`, `/admin` (admin-gated operator console), `/share/:shareId`,
  `/clients-preview`.
- It **already depends on** `@openagentsinc/autopilot-control-protocol` and
  `@openagentsinc/autopilot-ui` (`apps/openagents.com/apps/web/package.json`), and
  `/clients-preview` (`src/page/clientsPreview.ts`) already renders
  `SessionList` + `DecisionCard` — but against **fixtures**, not live data.

## 3. Current state

### 3.1 Team / project / membership model — TYPES EXIST, no live CRUD
`apps/openagents.com/apps/web/src/domain/session.ts` defines Effect Schemas:
- `Team { id, name, slug, role, members: TeamMember[], projects?: TeamProject[] }`
- `TeamMember { userId, name, email, avatarUrl, githubUsername, githubId, role,
  status, joinedAt }` — already has **email**, **role** (string: owner/lead/
  member/…), and **status** (active/invited/…), plus GitHub identity.
- `TeamProject { id, teamId, name, slug, description, status, agent? }`
- `TeamProjectAgent { id, name, status, scope, runtime, backend, repository,
  focus }` — already has a **`repository`** field (GitHub repo reference).

So the *shape* of teams, members (by email, with roles + invited status),
projects, and a per-project repo + agent is modeled. What's missing is the
**backing store + commands + UI** to create/mutate them and the **auth** to scope
access by membership.

### 3.2 Access gating — owner/admin only
- `src/product-policy.ts`: `loggedInAdminAccessAllowed = auth.isAdmin &&
  onboardingComplete`; admin identity is an email check
  (`auth.session.email === '<admin>'`). The operator console (`/admin`) hard-gates
  on `auth.isAdmin`. The autopilot surface is, in practice, restricted to the
  owner account — there is **no team-membership-based authorization** that would
  let an invited member read a project dashboard.
- Auth is session-based with GitHub identity (`Session { userId, email, name,
  login, avatarUrl, provider, githubId }`).

### 3.3 Invites / sharing — minimal
- `InviteRoute` is onboarding copy, **not** a team-member invite flow. There is no
  email-invite/accept path that adds a `TeamMember` and grants read access.
- `ShareRoute` = `/share/:shareId` exists (opaque share token, reachable
  logged-out or in) but there is no visibility/projection model wired to teams or
  sessions. `visibility`/`accessMode` strings exist on orders/sites only.

### 3.4 Live Pylon sessions — NOT shown (only stats + fixtures)
- The web app shows `PublicPylonStats` on the homepage and `/autopilot/work`
  work-orders that reference a `RunId`, but it does **not** display live Pylon
  control sessions. `/clients-preview` renders the protocol/UI off **fixtures**.
- The web server has **no path to a Pylon node**: Pylon's control server binds
  loopback (`127.0.0.1:4716`, bearer token) — a hosted web app cannot reach it.
  Off-machine access is exactly what the **bridge** (below) is for.

## 4. The new system surface (what the web UI can consume)

### 4.1 Protocol (`packages/autopilot-control-protocol/src`)
- `SessionSummary { sessionRef, adapter, state, objectiveRef?, workspaceRef?,
  accountRefHash, lastProgressRef?, updatedAt }` — the list row.
- `SessionEvent { sessionRef, eventId, sequence, phase, projectionLevel,
  observedAt, detailRef? }` — ordered, **cursor-resumable** stream.
- `DecisionRecord` (pending/resolved/cancelled/expired) — approvals.
- `cursor.ts` (`acceptEvent`/`needsResnapshot`) — dedup + resume.
- **`bridge.ts`** — the remote surface: verbs (`session.list/subscribe/snapshot/
  history`, `turn.steer/interrupt`, `session.pause/resume/cancel`,
  `decision.resolve`, `artifact.read`, `capability.list`, `bridge.pair.exchange/
  revoke/clients.list`); `Capability` set (`observe_public`, `observe_private`,
  `answer_decision`, `send_instruction`, `cancel`, `pause_resume`,
  `read_artifact`); and **`PairingCredentialClaims { pairingRef, clientId,
  deviceClass, issuer, audience, expiresAt, jti, projectionLevel, capabilities }`**.
- **`ProjectionLevel` = `public_safe | team | private`** — already the exact
  primitive for "team members see the team projection; secrets stay private."

### 4.2 UI (`packages/autopilot-ui`)
Reusable Foldkit components the web dashboard can render directly: `SessionList`,
`SessionDetail`, `EventTimeline`, `DecisionCard`/`DecisionActions`, `AccountList`,
`NodeStatusBadge`/`ProviderStatusList`, `VerifyStatus`, `ArtifactList`/
`ReceiptList`, `AssignmentList`, `EarningsPanel`, `SteerControls`, dark `tokens`.
(Same components the desktop/mobile clients use → identical look.)

### 4.3 Pylon control + bridge (`apps/pylon`)
- Control server (`src/node/control-server.ts`): `GET /health`, `POST /command`
  (`session.list/spawn/cancel/events`), `GET /sessions/:ref/events` (SSE,
  cursor-resumable), `GET /events`. Loopback today.
- Bridge (`src/node/bridge-*.ts`): pairing (bootstrap secret → scoped
  `PairingCredentialClaims`), sequenced/replayable streams
  (`EventSequencer`/`ReplayBuffer`, lossless/best-effort tiers), capability +
  projection enforcement. This is the **partially-built M2 transport** that lets
  an off-machine client (a hosted web server) observe sessions.
- Intent intake (`src/node/intent-intake.ts`) + coordinator
  (`src/coordinator/*`): submit work + track `received→…→shipped` status — the
  source of "build-out status" for a project.

## 5. Gaps to close (the core of this audit)

| # | Capability | Current | Needed |
|---|---|---|---|
| G1 | **Team/project store + CRUD** | types only | persistence (e.g. D1) + commands `CreateTeam`/`CreateProject`/`LoadTeam`; lead user set as `owner` on create |
| G2 | **Membership by email + invite/accept** | `TeamMember` type w/ email+status | invite flow: add member `status:'invited'` by email → accept → `'active'`; backed by the auth provider; lead can manage members |
| G3 | **Team-scoped authorization** | admin-email gate only | authorize the project dashboard by **team membership + role**, not `isAdmin`; lead = full, member = read-only |
| G4 | **Attach GitHub repo** | `TeamProjectAgent.repository` field | UI/command to set + persist the repo on a project (optionally validate via GitHub identity already on the session) |
| G5 | **Attach Pylon sessions (live)** | only stats/fixtures; loopback-only | a **project↔session link** + the **bridge** so the hosted web server holds a scoped, **read-capability**, `team`-projection pairing credential and subscribes to `session.list`/`subscribe`/`snapshot`/`history`; render live with `autopilot-ui` (replace the `/clients-preview` fixtures) |
| G6 | **Shareable read-only dashboard** | `ShareRoute` exists, unused for this | a per-project dashboard route authorized by membership (and/or a `/share/:shareId` link) that is **read-only** for members (capabilities limited to `observe_*`; no `send_instruction`/`cancel` unless lead) |
| G7 | **Build-out status surface** | work-orders only | project dashboard aggregates: attached sessions' states + timelines, the coordinator/intent status (`received…shipped`), repo, and member list |
| G8 | **Projection/secret safety** | n/a for sessions | members receive the **`team`** projection (refs-only, no tokens/secrets); `private` detail never leaves Pylon; browser never holds raw credentials |

## 6. Proposed phasing

- **Phase 1 — Team & access spine.** Add the team/project/member store + CRUD
  commands; set the lead user as owner on create; implement email invite/accept
  (member `invited→active`); replace admin-only gating with **team-membership
  authorization** (lead full / member read-only). De-gates the surface from a
  single account to a group.
- **Phase 2 — Repo attach.** Command + UI to attach a GitHub repository to a
  project (persisted on `TeamProject`/`TeamProjectAgent.repository`).
- **Phase 3 — Live Pylon attach via the bridge.** Stand up the bridge path
  (pairing → scoped credential with `projectionLevel: 'team'` + `observe_*`
  capabilities) so the web server can subscribe to attached sessions; persist a
  **project↔sessionRef** link; render live `SessionList` + `SessionDetail` +
  `EventTimeline` + `DecisionCard` from `autopilot-ui` (swap out the fixtures).
  This depends on finishing the M2 bridge (CL-8…CL-14 in the clients roadmap).
- **Phase 4 — Shareable dashboard + status.** A per-project dashboard route
  (membership-authorized, read-only for members; optional `/share/:shareId`),
  aggregating attached-session states/timelines + coordinator/intent build-out
  status + repo + members.

## 7. Security / invariants

- **Read-only by default for members.** Members get `observe_*` capabilities +
  the `team` projection; only the lead/owner gets effectful verbs
  (`send_instruction`, `cancel`, `pause_resume`, `decision.resolve`). Enforce in
  the bridge (`verbAllowedByCapabilities`) AND in the web authorization layer.
- **Refs-only to the browser.** No raw tokens/secrets/credentials reach the
  client; the scoped pairing credential lives **server-side** (the hosted web
  server is the bridge client), and only refs/digests + the `team` projection are
  sent to the browser.
- **Membership is the authorization boundary**, not `isAdmin`. The lead account
  is simply the `owner` member of its team.

## 8. Open questions

1. **Auth for invited members** — reuse the existing session/GitHub identity, or
   add WorkOS-style email auth for non-GitHub members? (Invites are by email; the
   `TeamMember` type already carries email.)
2. **Where the bridge terminates for a hosted web app** — the web server holds
   the scoped pairing credential and proxies projected, refs-only data to
   browsers over its existing transport. Confirm that server-side bridge-client
   placement.
3. **Persistence store** for teams/projects/members/session-links (D1 alongside
   the existing web backend is the likely home).
4. **Reachability of the Pylon node** the sessions run on — the bridge requires
   the node be reachable (Tailnet/relay) from the web server; same prerequisite
   as CL-8 (Tailnet-reachable control binding).

## 9. References

Web app: `apps/openagents.com/apps/web/{route.ts, product-policy.ts,
domain/session.ts, page/loggedIn/page/autopilot-work.ts, page/clientsPreview.ts}`.
New system: `packages/autopilot-control-protocol/src/{control,bridge,cursor,
decision}.ts`, `packages/autopilot-ui/src/index.ts`, `apps/pylon/src/node/
{control-server,bridge-pairing,bridge-stream,intent-intake}.ts`,
`apps/pylon/src/coordinator/*`. Sequencing: `docs/autopilot-coder/
2026-06-13-autopilot-clients-roadmap.md` (M2 bridge = CL-8…CL-14).
