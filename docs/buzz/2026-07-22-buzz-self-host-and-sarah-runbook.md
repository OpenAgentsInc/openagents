# Buzz self-host and Sarah runbook — 2026-07-22

> **Canceled plan — do not execute.**
>
> The owner canceled the separate OpenAgents Buzz installation on 2026-07-23.
> The current
> [`Omega plan`](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)
> replaces this runbook.
> Omega will implement the useful Buzz workroom outcomes as native GPUI panes.
> This document remains historical source evidence.
> Its commands, checklists, Sarah posting path, DNS path, deployment path, and
> fork opportunities are inactive.

## Cancellation and retained deployment inventory

The canceled #9195 lane created a private Google Cloud deployment before the
owner changed direction.
The lane did not complete DNS, public WSS, NIP-42, nonmember rejection, or
independent NIP-29 acceptance.
It did not produce an accepted OpenAgents product surface.

The last public-safe work record identified:

- Buzz source commit `acfbb1bb6af54cb29cb152496ff43b8285dcb8cf`
- GCE instance `buzz-community-1` in `us-central1-a`
- reserved address `buzz-openagents-ip` at `34.135.238.164`
- Artifact Registry image digest
  `sha256:9369c5849027ca266cdbb14581e73d30f4543927ff9d5ca811a8ec3e2eb0c478`
- GCS PostgreSQL backup
  `gs://openagentsgemini-buzz-backups/postgres/buzz-initial-20260723T135223Z.sql.gz`
- stopped-stack snapshot `buzz-community-1-initial-20260723`

The last issue status said that the private stack was healthy and that
`buzz.openagents.com` was not in DNS.
The current planning pass could not refresh the Google Cloud command-line
login.
It therefore does not claim a current runtime state or a completed cloud
retirement.
Issue closure and plan cancellation do not prove resource deletion.
Do not delete the retained backup, snapshot, secrets, address, disk, image, or
instance without a current inventory and a separate verified retirement
action.

This runbook explains how to run our own Buzz instance. It explains how Sarah
can communicate in that instance. It explains how our team and our community
can join and participate. It is grounded in the read-only teardown at
[`../teardowns/2026-07-21-buzz-teardown.md`](../teardowns/2026-07-21-buzz-teardown.md)
and in the actual source at the sibling clone `/Users/christopherdavid/work/buzz`.

The runbook does not contradict the teardown. The teardown decision still holds.
We adopt selected Buzz-compatible protocols. We do not adopt the Buzz relay as
product authority for our chat, receipts, or sessions. Cloud SQL and Khala Sync
stay authoritative. This document adds the operational steps for a bounded,
owned Buzz community as a communication and collaboration surface.

No step here prints or invents a secret. Secret placement points at the
existing pattern only. That pattern is the workspace `~/work/.secrets/`
directory plus Google Cloud Secret Manager in project `openagentsgemini`.

---

## 1. What Buzz is and our fork intent

Buzz is one self-hostable Rust relay. That relay is the workspace. A Buzz
community is one URL that is backed by one Nostr relay. Every message,
reaction, forum post, canvas edit, workflow step, review approval, moderation
action, git patch, and CI status is a signed NIP-01 event in one log. The
event `kind` integer is the single dispatch switch. There are about 81 kind
constants at the audited tip.

Agents are members, not bots. An agent holds its own Nostr keypair. An agent
holds its own channel memberships, its own memory, and its own audit trail. An
agent reaches the workspace through the same protocol that humans use.

### 1.1 Stack

- **Backend**: a Rust workspace of about 27 crates. The entry point is
  `crates/buzz-relay`, an Axum WebSocket plus REST server. The audited tree is
  about 218,000 lines of Rust.
- **Data plane**: PostgreSQL 17 for the event store and the generated full-text
  search index. Redis 7 for pub/sub, presence, and typing. S3-compatible object
  storage (MinIO in dev) for Blossom media and git content-addressed storage.
- **Clients**: a Tauri 2 plus React 19 desktop app under `desktop/`. A Flutter
  mobile app under `mobile/`. A small browser repo-viewer under `web/` that the
  relay serves. A tiny operator console under `admin-web/`.
- **Agent surface**: `buzz-cli` is an agent-first tool. It takes JSON in and
  returns JSON out. `buzz-acp` is a harness. It bridges the relay to a pool of
  agent subprocesses over the Agent Client Protocol. It supports Goose, Codex,
  and Claude Code. `buzz-agent` is their own minimal fallback agent.
- **Toolchain**: Rust 1.88 or newer (the Docker image builds with Rust 1.95).
  Node 24 or newer. pnpm 10 or newer. The `just` task runner. Hermit pins the
  full toolchain under `bin/`.
- **License**: Apache-2.0. The origin vendor is Block, Inc. The internal
  codename is Sprout.

### 1.2 Identity and Nostr model

Buzz speaks NIP-29 relay-based groups natively. A third-party Nostr client can
connect straight to the relay over NIP-29 and NIP-42. The former NIP-28
compatibility proxy is removed.

The identity model is a Nostr keypair per actor. A person, an agent, and the
relay itself each have a keypair. The relay derives the community from the
request host into a `TenantContext`. Every query binds the community id as its
first predicate. The wire format never grows a tenant tag.

Buzz implements a wide standard NIP set. That set includes NIP-01, NIP-05,
NIP-09, NIP-10, NIP-11, NIP-16, NIP-17, NIP-25, NIP-29, NIP-34, NIP-42, NIP-43,
NIP-50, NIP-70, NIP-98, and Blossom media. Buzz also authors about 15 custom
NIPs in `docs/nips/`. The most relevant custom NIPs for us are NIP-OA (owner
attestation), NIP-AA (agent authentication), NIP-AP (agent persona), and NIP-AE
(owner-decryptable agent memory).

The channel message kind is 9. It requires an `#h` tag that names the channel.
Reactions are kind 7. Deletions are kind 5. Group management uses kinds 9000 to
9008 and 9022. Relay-signed group state uses kinds 39000, 39001, and 39002.
Direct messages use NIP-17 gift wrap (kind 1059). Membership notifications use
kinds 44100 and 44101, and only the relay key may sign them. Git collaboration
uses the NIP-34 kinds, plus a relay-signed ref pointer event (kind 30618).

### 1.3 Bitcoin and Lightning

Buzz has no required Bitcoin or Lightning dependency. The README states that
Buzz is not blockchain. Signed Nostr events carry the identity and audit value
without any coin. There is no payment path to configure for a self-host. This
removes a whole class of risk from our adoption.

### 1.4 Our fork intent

We run one owned Buzz community on Google Cloud. Sarah is a member of that
community with her own agent keypair. Our team members are relay members. Our
community members join under an explicit access policy. The Buzz community is a
communication and collaboration surface. It is not a replacement for our
canonical stores.

---

## 2. Install, run, and self-host runbook

All commands below run inside the read-only clone
`/Users/christopherdavid/work/buzz`. Do not modify that clone as part of this
task. The steps are the exact source-grounded steps for a future admitted run.

### 2.1 Local developer run

Prerequisites are Docker plus Hermit. Hermit provides the pinned toolchain. As
an alternative you can install Rust 1.88 or newer, Node 24 or newer, pnpm 10 or
newer, and `just` yourself.

```bash
cd /Users/christopherdavid/work/buzz
. ./bin/activate-hermit     # pinned toolchain, tools auto-download on first use
just setup && just build    # copies .env.example to .env, starts Docker, migrates
just dev                    # starts the relay plus the desktop app together
```

The relay listens on `ws://localhost:3000`. For split logs, run `just relay` in
one terminal and `just desktop-dev` in another. The `just setup` step runs
`just bootstrap`. That step copies `.env.example` to `.env` when needed. It
starts the Docker services and runs the migrations.

The local `docker-compose.yml` starts Postgres 17, Redis 7, Adminer, Keycloak,
MinIO, a MinIO bucket-init job, and Prometheus. The bucket is `buzz-media`.

### 2.2 What must be configured

The dev defaults in `.env.example` work out of the box. A real deployment must
override the values below. The production template is
`deploy/compose/.env.example`.

| Setting | Purpose | Note |
| --- | --- | --- |
| `DATABASE_URL` | Postgres event store | Canonical data. Back it up. |
| `REDIS_URL` | Redis pub/sub | Required for more than one relay replica. |
| `BUZZ_BIND_ADDR` | Relay bind host and port | Default `0.0.0.0:3000`. |
| `RELAY_URL` | Public WebSocket URL | Used in NIP-42 auth challenges. Use `wss://` in production. |
| `BUZZ_RELAY_PRIVATE_KEY` | Relay signing key | 32-byte hex. Stable. Rotating it makes a new relay identity. |
| `RELAY_OWNER_PUBKEY` | Relay owner identity | 64-char hex Nostr pubkey. Required for closed relay mode. |
| `BUZZ_GIT_HOOK_HMAC_SECRET` | Git push policy HMAC | Random 64 hex. Stable. |
| `BUZZ_S3_ACCESS_KEY` / `BUZZ_S3_SECRET_KEY` / `BUZZ_S3_BUCKET` | Object storage | Media and git packs. |
| `TYPESENSE_API_KEY` | Search key | Present in the production template. |
| `BUZZ_REQUIRE_AUTH_TOKEN` | Require authenticated NIP-42 | Set `true` in production. |
| `BUZZ_REQUIRE_RELAY_MEMBERSHIP` | Enforce the member list | Set `true` in production (closed relay). |
| `BUZZ_ALLOW_NIP_OA_AUTH` | Accept agent owner attestation | Lets an owned agent inherit its owner's membership. |
| `BUZZ_PUBKEY_ALLOWLIST` | Allowlist pubkey-only auth | Optional. Fail-closed on DB error. |
| `BUZZ_AUTO_MIGRATE` | Run migrations at startup | Opt-in. Or run `buzz-admin migrate`. |

### 2.3 Single-node or VPS deployment

The bundle is `deploy/compose/`. It is separate from the local
`docker-compose.yml`. The steps are:

```bash
cd deploy/compose
cp .env.example .env
$EDITOR .env            # replace every CHANGE_ME value
./run.sh config         # render and validate
./run.sh start
curl -fsS "http://127.0.0.1:$(grep -E '^BUZZ_HTTP_PORT=' .env | cut -d= -f2-)/_liveness"
./run.sh status
```

For a public host with automatic Let's Encrypt certificates, set
`BUZZ_COMPOSE_TLS=true` before `./run.sh start`. That path uses a Caddy
compose file. The stack uses Postgres, Redis, MinIO, and a git data volume.

### 2.4 Kubernetes deployment

The Helm chart is `deploy/charts/buzz/`. It has two profiles. The Quickstart
profile brings up Postgres, Redis, and MinIO in-cluster for evaluation only.
The Production profile uses external managed Postgres, Redis, and S3, plus a
pre-created Secret. Production must use `secrets.existingSecret`. The required
inputs are `relayUrl`, `ownerPubkey`, `secrets.existingSecret`, and the
external service URLs. The chart fails the install with a clear message when an
input is missing or malformed.

The relay embeds its SQLx migrations. Migrations run at startup behind a
Postgres advisory lock when `BUZZ_AUTO_MIGRATE` is true. `helm upgrade` is the
whole upgrade procedure. More than one replica hard-requires Redis. Git state
is object-store-backed, so no shared filesystem is needed.

### 2.5 Our target: Google Cloud

Our production infrastructure authority is Google Cloud. A future owned Buzz
instance must run there. The natural mapping is:

- Relay container on Cloud Run or GCE, from the published image or from a build
  in Artifact Registry.
- Cloud SQL for PostgreSQL as the event store.
- A managed Redis (Memorystore) for pub/sub.
- Cloud Storage through the S3-compatible path, or a self-run MinIO on GCE.
- Google Cloud load balancing for the public `wss://` endpoint.
- Cloudflare stays DNS-only for the hostname, with the record pointing at
  Google Cloud. Do not enable the Cloudflare proxy without an owner decision.

This runbook does not deploy anything. A real deployment needs a new Google
Cloud design and explicit product authority. Do not restore a retired
Cloudflare Workers or Durable Objects path for this.

### 2.6 Where secrets live

Never place a secret in a tracked file or in terminal output. Follow the
existing pattern. The local backup copies live under `~/work/.secrets/`. The
runtime authority is Google Cloud Secret Manager in project `openagentsgemini`.
The secrets a Buzz instance needs are the relay private key, the git hook HMAC
secret, the database password, the Redis password, the search API key, and the
S3 access and secret keys. The relay owner private key stays with the operator
and is never held by the chart.

### 2.7 What we must decide before a build

The teardown and the source both flag freeze points. Before a first packaged or
deployed build we must fix the community domain, the relay identity keypair, the
owner pubkey, the closed-relay policy, the storage backend, and the backup plan.
Section 7 lists the full checklist.

---

## 3. Sarah integration

Sarah is `principal.sarah`. She is the owner's orchestrator. She runs on one
stable owner-private Khala Sync thread inside supported OpenAgents clients. Her
authority is the intersection of `AUTHORITY.md` and
[`../authority/SARAH_AUTHORITY.md`](../authority/SARAH_AUTHORITY.md) revision 5.
Her authority may not amplify itself. Visibility is never mutation authority.
Every action must pass a typed capability broker and emit a receipt.

### 3.1 What exists today

- Sarah has a web-communications broker. It is the `sarah_web_comms` tool in
  `apps/openagents.com/workers/api/src/sarah-runtime-tools.ts`. The tool binds
  to the grant `grant.sarah.web_communications`.
- That tool already has a `nostr` channel path. Today the `nostr` channel
  produces a public-safe draft plus a repository-delivery handoff. It writes a
  draft under `docs/sarah/nostr/`. It emits the receipt
  `sarah.web_comms.nostr_draft_ready`. It does not post to a live relay yet. The
  code comment states that live relay posting is a later lane.
- We own a Nostr signing and relay bridge already. It is
  `apps/openagents.com/workers/api/src/sol-claim-ledger-relay.ts`. It signs
  event templates with the owned `nostr-effect` signer. It serializes NIP-01
  relay frames. It parses frames back and verifies the signature. It keeps the
  signer secret key out of the returned frame, the recovered record, and every
  log. It surfaces only the derived public key and the event id.
- We own `nostr-effect`. It provides the signer, the event builders, NIP-19
  encoding, the standard NIPs, and all 15 Buzz custom NIPs.
- Sarah's web-communications program is `program.sarah_web_communications`. Its
  status is `runtime_pending`. Blog and document drafts land through repository
  delivery now. Outward timeline and animated-spoken publication refuse with a
  receipt until the owner-supplied interfaces and the broker are admitted.

### 3.2 What needs building

To let Sarah post and reply in Buzz, we add a bounded relay-posting lane. The
pieces already exist as parts. The lane joins them under her authority.

1. **A Sarah Nostr identity.** Sarah needs one stable Nostr keypair. The public
   key is her npub in the community. The secret key stays in Secret Manager. She
   never exports a raw secret key. Her signer follows the sovereign-signer
   boundary that the teardown recommends. The signer signs an admitted event
   template. It does not hand out the key.
2. **An owner attestation.** Sarah's agent key carries a NIP-OA `auth` tag from
   the owner key. With `BUZZ_ALLOW_NIP_OA_AUTH=true` on the relay, Sarah inherits
   the owner's membership. This means the operator does not enroll every agent by
   hand. It also means a revoked owner membership disables the agent on the next
   connection.
3. **A posting broker path.** Extend the `nostr` channel of `sarah_web_comms`
   past the draft. The new path signs a kind 9 event with the `#h` channel tag
   through the `nostr-effect` signer. It publishes the NIP-01 frame to the owned
   Buzz relay over WebSocket. It records a target receipt with the event id. This
   reuses the `sol-claim-ledger-relay.ts` signing and framing shape. It does not
   invent a second signer.
4. **A read-and-reply path.** Sarah reads a Buzz thread with a NIP-01 REQ that
   filters kind 9 by the channel `#h` tag. She replies with a kind 9 event that
   carries a NIP-10 `["e", "<root>", "", "reply"]` tag. The relay creates the
   thread metadata atomically. This is the same wire path the CLI and desktop
   use.
5. **An autonomous community-update tick.** Sarah's autonomous loop can post a
   bounded community update to a fixed channel. The update is public-safe,
   redacted, freshness-labelled, and cited. It runs under the web-communications
   grant. It emits an authority receipt and a target receipt. It never claims an
   action ran without the target receipt.

### 3.3 Authority guardrails for the Sarah lane

- The relay-posting lane needs the web-communications program to advance from
  `runtime_pending` to active, or a separate admitted grant for Nostr posting.
  Do not treat the existing draft path as posting authority.
- The community channel is an open channel. The teardown treats open Nostr
  channels as public. So Sarah's posts there are public claims. Public claims
  need the redaction and product-promise gates. She must not post owner-private
  business context to an open channel.
- A signed event proves who signed. It does not prove permission, execution,
  acceptance, release, or payment. Keep those gates separate. A Sarah Buzz post
  is a communication. It is not a settlement or a release proof.
- The signer secret key never appears in a draft, a receipt, a log, or a public
  projection. Only the npub and the event id are surfaced.

---

## 4. Team and community participation

Buzz has an explicit membership and access model. We configure it to let our
team in first, then a wider community.

### 4.1 Identities

Every participant has a Nostr keypair. A human uses a Buzz desktop or mobile
client, or a third-party NIP-29 client. An agent uses `buzz-cli` or the ACP
harness with its own key in `BUZZ_PRIVATE_KEY`. An owned agent can inherit its
owner's membership through a NIP-OA attestation.

### 4.2 Closed relay membership (NIP-43)

Set `BUZZ_REQUIRE_RELAY_MEMBERSHIP=true` for a closed community. The relay then
checks each authenticated connection against the `relay_members` table. The
relay bootstraps the owner from `RELAY_OWNER_PUBKEY` on startup. Manage members
with the `buzz-admin` CLI or the `run.sh` wrapper:

```bash
./run.sh add-member npub1abc...            # or a 64-char hex pubkey
./run.sh add-member npub1abc... --role admin
./run.sh remove-member npub1abc...
./run.sh list-members
```

Member management needs `DATABASE_URL`, `REDIS_URL`, and
`BUZZ_RELAY_PRIVATE_KEY`. The relay signs a kind 13534 membership list event
after each change. An owner or admin can also manage members over WebSocket with
NIP-43 admin events. Those are kind 9030 (add), 9031 (remove), and 9032 (change
role).

### 4.3 Pubkey allowlist

Set `BUZZ_PUBKEY_ALLOWLIST=true` to gate pubkey-only NIP-42 connections against
the `pubkey_allowlist` table. Users with a valid API token bypass the allowlist.
The allowlist is fail-closed. A database lookup failure denies the connection.
There is no CLI for the allowlist yet, so it is managed with direct SQL.

### 4.4 Channels and roles

A channel is a NIP-29 group. Anyone admitted can create a channel with kind
9007. A channel can be open or private. The channel owner and admins manage
membership. An open channel accepts a join request (kind 9021). A private
channel rejects a join request at ingest. Group state is published as relay-
signed kinds 39000, 39001, and 39002. Adding an agent to a channel is the same
action as adding a person.

### 4.5 Moderation

Moderation is a workflow, not an admission filter. Reports are private
structural state. They never enter the event log. Actions are signed commands
that the relay validates against the roster. Enforcement bites at the identity
seam. Removals leave honest tombstones. Both the author and the reporter hear
the outcome. This is a strong reference for our Forum moderation when that
surface grows.

### 4.6 What we must configure to let members in

- Turn on closed relay mode with membership and auth-token requirements.
- Enroll the team pubkeys as members, and the owner or leads as admins.
- Decide the community onboarding path for outside members. The options are a
  manual `add-member`, the allowlist table, or NIP-OA inheritance for owned
  agents.
- Turn on `BUZZ_ALLOW_NIP_OA_AUTH` so an owned agent joins under its owner.
- Set a rate-limit profile for humans and agents. The `.env.example` exposes the
  per-minute and per-second limits.

---

## 5. Synergy with our later work

The owned Buzz community lines up with three efforts we already run.

### 5.1 Off-GitHub coordination on an owned relay (issue #9185)

Issue #9185 moves cross-session coordination off GitHub and onto an owned Nostr
relay. The Sol claim ledger already projects the coordination record to and from
NIP-34 event templates. The signing and relay bridge already exists in
`sol-claim-ledger-relay.ts`. Buzz proves the same shape at product scale. A ref
pointer event, a signed claim, and a status event are all just kinds in one log.
The lesson is direct. The signing and framing path we built for the claim ledger
is the same path a Sarah Buzz lane uses. We reuse one signer and one frame
format across both.

### 5.2 Sarah's communications program

Sarah's web-communications program already treats Nostr as an open channel. A
Buzz community gives that channel a real home. Sarah drafts today. With the
posting lane she reads, replies, and posts community updates. The community
becomes the place where Sarah keeps the team and the public informed while the
owner is away. Every post is signed, redacted, cited, and receipted.

### 5.3 The owned relay and the community feedback loop

We own `nostr-effect` and its relay. We can run a Buzz-compatible protocol
profile without importing Buzz code. The community is a growth and feedback
loop. Users, agents, and team members coordinate in signed rooms. The Forum,
product promises, and NIP-90 work requests already treat Nostr as a transport
rail. A Buzz community is the natural next public surface for that rail. It
keeps the canonical authority in Cloud SQL and Khala Sync while it adds a
portable, signed, agent-native room.

---

## 6. Fork opportunities

These are candidate changes to our fork. They are ranked by impact over effort.
None of them is dispatch authority. Each needs the normal admission path.

1. **Sarah relay-posting adapter (high impact, low effort).** Wire the
   `nostr-effect` signer and the `sol-claim-ledger-relay.ts` frame path into the
   `sarah_web_comms` nostr channel so Sarah posts and replies in a Buzz channel.
   Most parts exist. The work is the broker path, the authority advance, and the
   receipt.
2. **Owned-relay deployment profile on Google Cloud (high impact, medium
   effort).** Add a deploy profile that targets Cloud Run or GCE, Cloud SQL,
   Memorystore, and Cloud Storage. Replace the Block image assumptions with our
   own image and Secret Manager wiring. Keep the closed-relay defaults.
3. **NIP-OA owner-attestation onboarding for owned agents (high impact, medium
   effort).** Use NIP-OA and NIP-AA so every owned agent inherits the owner's
   membership. This removes per-agent enrollment. It ties agent access to owner
   membership by construction.
4. **Effect Schema boundary for Buzz events (medium impact, medium effort).**
   Wrap the Buzz kinds we use in Effect Schema through `nostr-effect`. Do not
   accept unvalidated tag arrays at a product boundary. This keeps our mandatory
   schema discipline.
5. **Team-first branding and defaults (medium impact, low effort).** Set the
   community name, the workspace icon (kind 9033 / NIP-WP), the default
   channels, and the rate-limit profile for our team. This is configuration, not
   deep code.
6. **Signed projection outbox pilot (medium impact, high effort).** Follow the
   teardown's signed-projection-bus posture. Produce deterministic signed events
   from a Cloud SQL outbox after a canonical write. A relay failure delays the
   projection but never reverses the canonical write.
7. **Drop the shells we do not use (low impact, low effort).** We reject the
   Tauri desktop and the Flutter mobile. For our fork we can stop building them
   and keep only the relay, the CLI, and the agent surface. This shrinks the
   build and the attack surface.
8. **Moderation reference for Forum (low impact, high effort).** Port the Buzz
   moderation design ideas into our Forum when that surface grows. Reports as
   private state, signals never triggers, enforcement at the identity seam, and
   honest tombstones.

---

## 7. Decision and configuration checklist

Each item below is an owner decision or an operator configuration. Flag the
owner decisions before any deploy.

### 7.1 Owner decisions

- [ ] Approve running an owned Buzz community as a communication surface. This
      is a new public surface. It needs product authority.
- [ ] Approve the community domain or subdomain. Example `buzz.openagents.com`.
      Cloudflare stays DNS-only.
- [ ] Approve Sarah as a member with her own Nostr identity and posting lane.
- [ ] Advance `program.sarah_web_communications` past `runtime_pending`, or
      admit a separate Nostr posting grant, before Sarah posts live.
- [ ] Approve the community onboarding policy for outside members.
- [ ] Approve the hosting shape on Google Cloud (Cloud Run or GCE, Cloud SQL,
      Memorystore, Cloud Storage or MinIO).

### 7.2 Operator configuration

- [ ] Choose the relay identity keypair. Generate `BUZZ_RELAY_PRIVATE_KEY` once
      and keep it stable. Store it in Secret Manager.
- [ ] Set `RELAY_OWNER_PUBKEY` to the owner's 64-char hex pubkey.
- [ ] Generate `BUZZ_GIT_HOOK_HMAC_SECRET`, the database password, the Redis
      password, the search key, and the S3 keys. Store all of them in Secret
      Manager and back up copies under `~/work/.secrets/`.
- [ ] Set `RELAY_URL` to the public `wss://` URL.
- [ ] Turn on `BUZZ_REQUIRE_AUTH_TOKEN` and `BUZZ_REQUIRE_RELAY_MEMBERSHIP`.
- [ ] Turn on `BUZZ_ALLOW_NIP_OA_AUTH` for owned agents.
- [ ] Enroll the team pubkeys as members and set the admin roles.
- [ ] Set the rate-limit profile for humans and agents.
- [ ] Decide the storage backend and set the S3 endpoint, bucket, and keys.
- [ ] Turn on `BUZZ_AUTO_MIGRATE` or run `buzz-admin migrate` before start.
- [ ] Set the backup plan for the relay key, Postgres, the S3 bucket, and the
      git volume.

### 7.3 Sarah lane configuration

- [ ] Mint Sarah's Nostr keypair. Keep the secret in Secret Manager. Never
      export the raw key.
- [ ] Issue the owner NIP-OA attestation for Sarah's agent key.
- [ ] Add the relay-posting path to the `sarah_web_comms` nostr channel.
- [ ] Reuse the `sol-claim-ledger-relay.ts` signer and frame path.
- [ ] Confirm every Sarah post is public-safe, redacted, cited, and receipted.

---

## 8. Boundaries and non-goals

- This runbook is a plan. It is not dispatch authority and not a product
  decision.
- The relay-as-workspace substrate stays rejected. Cloud SQL and Khala Sync stay
  authoritative for chat, receipts, and sessions.
- The Tauri desktop shell and the Flutter mobile shell stay rejected. They
  conflict with the Electron plus Effect Native and Expo plus Effect Native
  mandates.
- The Buzz custom NIPs are not implicit product policy. Each adopted NIP needs an
  OpenAgents profile, an Effect Schema, a version, and an authority statement.
- Do not run the Buzz relay as a dependency for a single wanted feature. Reuse
  the wire formats through `nostr-effect` first.
- No secret is printed or committed in this work. Secret placement points at the
  existing `~/work/.secrets/` plus Google Cloud Secret Manager pattern only.
