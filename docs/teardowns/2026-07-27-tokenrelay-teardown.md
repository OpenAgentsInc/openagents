# TokenRelay Teardown — 2026-07-27

Read-only architecture and product audit of the public
`tcballard/TokenRelay` source tree at an exact commit in the local reference
clone `~/work/projects/repos/TokenRelay`. Nothing tracked was modified.
The audit read the protocol, API, PostgreSQL migrations and stores, control
worker, CLI, web console, Buzz and GitHub bridges, payment boundary, agent
wallets, tests, release gates, and commit history. The audit also compared
TokenRelay with the `exoharness/exo` trusted-substrate and executor split.
[source]

The local check installed the exact lockfile, then ran `pnpm check`,
`pnpm build`, and `git diff --check`. Formatting, lint, type checks, 117
non-PostgreSQL tests, and the production build passed. The suite skipped 27
PostgreSQL tests because Docker was not running. This audit did not start the
API, control worker, web console, market simulator, Buzz relay, GitHub App,
Stripe adapter, or a provider executor. It did not reproduce the checked-in
PostgreSQL evidence. [test] [limitation]

**Pin:** `e423d57b3fe23a34462bd6b4199f5267b6df8411` (2026-07-26,
"Establish TokenRelay's public repository foundation (#33)"). Workspace
version `0.1.0`, Apache-2.0, Node 22 or later, pnpm 10.15.0. [source]

## Summary

TokenRelay makes one useful product bet: trade a bounded accepted software
deliverable, not model access, tokens, allowance, a run, or compute capacity.
An invited buyer reserves a simulation reward. An eligible provider gets a
generation-bound lease and submits a patch. The buyer accepts, rejects, or
enters a dispute. Only acceptance or an accepted dispute can create provider
earnings. [source]

```text
Buzz / GitHub / web / CLI
           |
           v
  Fastify API on :3210
  auth · contracts · leases · decisions
           |
           v
       PostgreSQL
 tasks · proposals · outbox · ledgers · wallets
           |
           v
     control worker
 match · expire · settle · release · publish
           |
       +---+----------------+
       |                    |
 signed Buzz diff      GitHub branch + PR
       |                    |
       +------ buyer review-+
                    |
             accept or reject
```

The strongest part is the coordination core. PostgreSQL is the queue,
transaction boundary, tenant fence, and accounting source. State changes and
outbox events commit together. Control replicas claim work with
`FOR UPDATE SKIP LOCKED`. Generation-bound lease tokens prevent stale workers
from submitting after reassignment. Settlement and release handlers are
idempotent. Ledger postings use integer units and must balance to zero.
[source]

The weakest part is the evidence seam between provider execution and buyer
acceptance. TokenRelay does not run the provider, retain its event log, rerun
declared validation commands, apply the submitted patch in a clean checkout,
or verify the provider proposal signature. The current backend accepts
provider-written validation evidence after it checks the patch digest, size,
syntax, and allowed paths. Buyer acceptance is therefore the real semantic
oracle. [source] [inferred]

The `exoharness/exo` comparison makes this boundary precise. Exoharness keeps
the trusted append-only history, artifacts, sandbox identity, and turn
lifecycle separate from the executor policy. TokenRelay keeps the market
separate from the provider executor, but it has no typed execution attachment.
It receives a patch and a short provider claim, not a durable executor record
or a content-bound verification receipt. The separation is correct. The
missing attachment is the main gap. [source] [inferred]

**Central OpenAgents finding:** TokenRelay is a strong focused reference for
accepted-deliverable state transitions, PostgreSQL outbox coordination,
tenant-bound credentials, integer ledgers, and acceptance-gated settlement.
Do not deploy it as a second OpenAgents market, Buzz authority, wallet, or
control plane. Re-derive its best transactional laws inside the current
OpenAgents authority and receipt model. Add the evidence binding that
TokenRelay lacks. [inferred]

## 1. Repository identity and posture

- The repository has 34 commits at the audited main tip. The first commit is
  dated 2026-07-21. The complete public history is less than one week old.
  [history]
- Git records three author strings across main. All three identify Tom
  Ballard or Tom Armytage and use `armytage.co` or a personal address. This
  appears to be one-maintainer governance. [history] [inferred]
- The repository has no release tag. One archive tag records a recovery point.
  A staging-readiness branch exists, but its commits are not on audited main.
  [history]
- The source is Apache-2.0. It has `CONTRIBUTING.md`, `SECURITY.md`, an
  agent guide, issue template, and one CI workflow. It has no CODEOWNERS,
  dependency-update bot, CodeQL workflow, published package, or release
  workflow. [source]
- The monorepo has four applications and ten packages. It contains about
  34,000 lines of TypeScript, TSX, and SQL across the counted source and
  migration files. `packages/database/src/backend-store.ts` alone has about
  4,800 lines. [source]
- TypeScript uses `strict`, `noUncheckedIndexedAccess`,
  `exactOptionalPropertyTypes`, and project references. The production API
  and control worker are separate entry points over shared packages and one
  PostgreSQL database. [source]
- The root package is still named `agent-exchange`. Workspace packages use
  the `@relay/*` namespace. Human text is partly TokenRelay and partly
  AgentExchange. The CLI retains `tokenrelay`, `agentexchange`, and `relay`
  names. This is a deliberate compatibility window, but it leaves product,
  package, and protocol identity split three ways. [source]

The project moved quickly through several market models. The tree still
contains the original test-credit service, a paid-attempt sandbox, a v2
capacity control plane, an experimental market, and the current
accepted-outcome backend. Superseded docs label the old models honestly, but
the old services, schemas, migrations, MCP facade, stores, and tests remain
compiled. [source]

That history is valuable evidence of correction. It is also current
complexity. A new maintainer must distinguish the production
`backend-main.ts` and `backend-app.ts` path from three earlier application
paths that still type-check and test. [inferred]

## 2. Product and domain model

The current product is a private organisation exchange for bounded software
patches. Its primary records are: [source]

```text
organisation
└── exchange
    ├── memberships and scoped tokens
    ├── buyers and workers
    ├── tasks
    │   └── task slots
    │       └── lease
    │           └── deliverable
    │               ├── buyer decision
    │               ├── dispute
    │               └── accepted settlement
    ├── provider proposals and attestations
    ├── ledger transactions and entries
    ├── outbox and audit events
    ├── Buzz identities and links
    └── simulation wallet issuer, treasury, and agent wallets
```

An outcome task contains a public GitHub repository URL, exact 40-character
base commit, outcome text, objective acceptance criteria, allowed path globs,
validation method, optional declared commands, reward, deadline, permitted
execution modes, and policy versions. The protocol limits the patch to one
MiB and the declared file list to 50 items. [source]

Provider supply is a proposal with an increasing sequence, requested reward,
claim count, capabilities, repository policy, execution mode, attestation
version, expiry, and a 64-character `signature` field. Matching applies safety
eligibility before reward ordering. It then uses a pay-as-ask model inside
the task reward. [source]

The provider credential is separate from the lease credential. A lease token
is bound to one opaque lease ID and generation. Only its digest is stored.
Rematching increments the generation. A stale worker cannot submit with the
old token. Repeated submission returns the original result only when request
identity and content agree. [source]

The decision model is better than the usual "patch submitted equals done"
flow. Submission creates `pending_acceptance`. Rejection holds the reservation
during the dispute window. A provider can dispute. An operator resolves the
dispute to accepted or rejected. Only the accepted path enqueues settlement.
The finally rejected path releases the reservation. [source]

## 3. PostgreSQL as the control plane

PostgreSQL is the most mature architectural choice in the repository. There
is no Redis queue and no memory scheduler in the current backend. The API and
control process do not call each other. Both use the database package.
[source]

The current schema is an 18-migration stack. It grew from test credits through
paid attempts, the v2 control plane, the outcome backend, GitHub and Buzz
bridges, organisation exchanges, and agent wallets. Migration 17 backfills all
prior market records into `exc_local`. Migration 18 adds wallet accounting.
[source]

State changes and outbox inserts share a transaction. The control worker
claims up to 20 events, dispatches by event type, marks success, or schedules
a retry. After the configured retry count, it dead-letters the event. The
operations API shows pending depth, oldest age, and dead letters. An operator
can requeue a dead letter after the cause is fixed. [source]

The worker handles five important families: [source]

- match all available slots for a task
- settle an accepted deliverable
- release a finally rejected deliverable
- publish a submitted deliverable to Buzz or GitHub
- run recurring expiry, earning, and maintenance sweeps

Row locks and `SKIP LOCKED` permit several control replicas. Database unique
constraints backstop one active lease per slot, one deliverable request per
lease, one accepted settlement per deliverable, and scoped idempotency keys.
A database trigger rejects a claim that joins a task and proposal from
different exchanges. [source]

The outbox is at-least-once, not exactly-once. Internal settlement and release
handlers are guarded for replay. External publication has a sharper edge.
The real GitHub App client creates a branch, commit, pull request, and issue
comment through several API calls. The tests use a fake client. A failure
after branch creation can make a retry collide with the existing ref. The
repository claims PR publication is idempotent, but no live GitHub test or
recovery state machine proves the multi-call sequence at this commit.
[source] [inferred]

## 4. Authentication and organisation isolation

Outside development, the API requires a token hash key and an administrator
bootstrap token. Persisted bearer tokens carry actor roles, optional worker
identity, exchange identity, expiry, and revocation state. The database stores
keyed token digests instead of raw bearer tokens. Development-only static
principals remain available when the application explicitly runs in
development mode. [source]

The exchange comes from the credential. Participant endpoints do not accept
an exchange selector. A platform administrator can provision exchanges. An
exchange administrator can operate only its bound exchange. Workers must have
active exchange membership. Matching, balances, decisions, audit records,
outbox work, Buzz links, and wallets all include the exchange ID. [source]

This is strong application-level logical isolation, but it is not physical
isolation. The shared PostgreSQL schema does not enable row-level security.
Deployment labels such as `dedicated` and `sovereign` are metadata only.
The documentation states this limitation repeatedly. [source]

The exchange migration adds many composite foreign keys and a cross-exchange
claim trigger. That defense-in-depth is worth adapting. However, application
queries still carry most of the isolation burden. A missing predicate in a
future query can expose shared-schema data unless a constraint or test catches
it. The checked-in PostgreSQL suite is therefore load-bearing. [inferred]

## 5. Buzz and GitHub bridges

### 5.1 Buzz

TokenRelay uses Buzz as a collaboration surface, not as market authority.
An invited buyer posts a structured `/bounty`. A workflow calls TokenRelay.
The API then queries the configured relay and verifies the exact signed source
event before it creates a task. A dedicated bot posts the signed deliverable.
The original buyer accepts or rejects with a reaction. The API again queries
the relay and verifies the signed reaction before it changes state. [source]

The workflow secret is explicitly not identity. Channel members can read the
workflow definition. It only filters nuisance traffic. NIP-98 proofs bind the
method, exact URL, body digest, timestamp, and nonce. The service verifies the
Schnorr event and consumes each authentication event once. Participant secret
keys stay local. [source]

Each exchange has one relay URL and one bot key. The bot key comes from the
process environment and is not stored in PostgreSQL. The stored relay binding
keeps only secret references. The API and control process must receive the
same binding configuration. [source]

This boundary aligns with the Buzz teardown: signed relay events are portable
identity and collaboration inputs. They are not assignment or settlement
authority. TokenRelay improves on a webhook-only design because it re-reads
and verifies the signed event. It still depends on a Buzz-specific REST
`/query` and `/events` surface, not only standard relay WebSockets. [source]

### 5.2 GitHub

The GitHub identity path uses OAuth state and PKCE. It accepts only invited
logins. The GitHub user token performs one `/user` lookup and is then
discarded. TokenRelay issues a separate scoped marketplace token. Repository
authority remains a distinct GitHub App installation mapping. [source]

The issue bridge uses webhook HMAC verification and installation-to-buyer
mapping. A submitted patch can become a new branch and pull request in the
buyer's installed public repository. TokenRelay never merges, force-pushes,
deploys, or writes to the default branch. The buyer's merge remains the
application of the work. [source]

The publisher reads base files at the pinned commit, applies the unified diff
in memory, creates Git objects, creates the branch, opens the pull request,
and comments on the source issue. The real installation client has no live
contract test. The project correctly lists a real GitHub staging exercise as
a release gate. [source]

## 6. Patch validation and the evidence gap

The current backend checks important envelope properties. It parses the
unified diff on the server. It rejects NUL bytes, Git binary patches, quoted
paths, path traversal, unsupported path forms, a digest mismatch, excessive
bytes, and paths outside the task globs. It derives paths from patch headers
instead of trusting the provider's `changedFiles` list. [source]

The older generic validation package adds changed-line and changed-file
limits plus a high-confidence secret-pattern scan. The current outcome
backend does not call that package. It has its own patch checks in
`BackendService.submitDeliverable`. The outcome task schema also has no
changed-line limit. [source]

More important, the current backend does not: [source]

- clone the repository
- apply the patch to the exact base commit
- run the task's declared validation commands
- verify `validationEvidence.command` or its exit code
- compare the provider's declared file list with the parsed file set
- establish semantic correctness

The service stores the provider's short evidence object and compressed patch.
The buyer can download the diff and decide. This is honest for an
acceptance-driven simulation, but the README phrase "mechanical patch
validation and validation evidence" can sound stronger than the current
outcome path proves. [source] [inferred]

Provider proposal signatures have the same problem. The schema requires 64
hex characters, and the store persists the field. No code verifies it against
a registered proposal key or canonical proposal bytes. The real cryptographic
checks are elsewhere: Nostr events, wallet intents, GitHub webhooks, and mock
certification reports. The proposal signature is currently shaped data, not
authentication evidence. [source]

Worker certification is explicit about its limit. The only accepted profile
is `mock-v1` with the `mock` adapter. Its executor returns deterministic
fixture observations for secret, filesystem, network, resource, cleanup, and
patch assertions. It proves the challenge, signature, invalidation, and
coordinator protocol. It does not start a VM or certify hostile-workload
isolation. [source]

The only runner implementation is `DeterministicMockRunner`. Real providers
execute outside TokenRelay with their own credentials and tools. This is a
reasonable authority boundary. It means TokenRelay cannot claim that a patch
came from the stated executor, model, sandbox, or validation process.
[source] [inferred]

### The exoharness lesson

Exoharness draws the relevant line cleanly. Its trusted substrate owns the
durable event log, artifact bytes, sandbox identity, snapshots, and turn
heads. Its executor owns prompt and model policy. The log is not the prompt,
and executor telemetry is not automatically accounting truth. [source]

TokenRelay should preserve its provider-controlled executor boundary. It
should not import provider credentials or run arbitrary repositories inside
the market service. It should add a typed execution attachment with: [inferred]

- exact executor and sandbox identity
- exact repository and base commit
- content digest for the patch and evidence artifacts
- validation command identity and result
- durable event or receipt references
- attestation class and known loss
- buyer decision bound to those exact digests

An Exoharness-backed provider could attach artifact and event references from
its append-only substrate. A Codex, ACP, or other provider could attach its
own native receipt form. TokenRelay or OpenAgents must verify the common
envelope and declare the evidence class. It must not pretend all providers
share one execution runtime. [inferred]

## 7. Accounting, payments, and agent wallets

### 7.1 Reward ledger

Marketplace rewards use integer USD cents in PostgreSQL and decimal strings
on the wire. Reservations, provider earnings, platform amounts, releases,
refunds, and payment events use append-only balanced postings. Settlement is
separate from submission. This is the core accounting law worth adapting.
[source]

The process configuration and database release record are independent gates.
Simulation is the default. Stripe routes are absent unless a gateway is
configured. Funding and payouts still fail closed until the database posture
moves to `stripe_test` or owner-approved `capped_live`. Reconciliation holds
can pause purchases and payouts. [source]

The Stripe adapter implements Checkout, Connect accounts, onboarding links,
transfers, refunds, webhook signature verification, and normalized event
types. Tests use a fake gateway. The project has not exercised the current
adapter against real Stripe objects. Its docs correctly keep test and live
activation behind separate gates. [source] [limitation]

OpenAgents must not adopt TokenRelay's Stripe path or payment-mode registry as
a second settlement authority. OpenAgents already separates verification,
payable classification, settlement authorization, and settlement receipts.
TokenRelay corroborates those boundaries. It does not replace them.
[inferred]

### 7.2 Agent wallets

Each exchange gets a simulation issuer and treasury. An administrator can
create one Ed25519-controlled agent wallet per controller, fund the treasury,
and delegate a bounded amount. Policies set per-payment limits, UTC daily
limits, optional destination lists, expiry, and freeze state. [source]

Wallet intents bind source, destination, accepted deliverable, asset, exact
atomic amount, nonce, expiry, and memo. The API verifies the Ed25519 signature,
controller identity, wallet state, exchange, balance, policy, nonce, and
accepted-deliverable relationship inside one transaction. A buyer-controlled
wallet can pay only the worker that produced that accepted deliverable.
[source]

The wallet transaction and entry tables are append-only by database trigger.
Unique constraints prevent nonce reuse and repeated payment for one
deliverable, wallet pair, and asset. Amounts use bigint atomic units.
[source]

The asset names are `USD`, `USDC`, and `BTC`, but all three are simulation
units. There are no blockchain addresses, deposits, withdrawals, custody,
redemption, conversion, or broadcast transactions. The docs state this
clearly. Even so, financial asset labels create product and compliance
confusion without a settlement adapter. OpenAgents should adapt the policy
and signature shape, not copy the simulated asset registry. [source]
[inferred]

## 8. Web, CLI, and operations

The API serves the Vite React console from the same origin. The console has
buyer, provider, market, and operations workspaces. The API sets no-store,
no-sniff, frame-deny, and no-referrer headers. It redacts bearer and webhook
headers from structured request logs. [source]

The buyer surface creates tasks and reviews diffs. The provider surface gets
leases, acknowledges work, sends heartbeats, submits patches, and opens
disputes. The operations surface shows matching gates, exposure, unmatched
slots, eligible workers, breakers, outbox health, dead letters, and disputes.
[source]

The market-depth display refreshes three times per second. It aggregates one
cent price levels around a deterministic simulation midpoint. It is an
indicative read model. The PostgreSQL transaction is the actual matcher.
[source]

The CLI is a large JSON-first command surface. It stores the server and token
in a local home, supports separate buyer and provider homes, and sends machine
output to stdout. It covers authentication, tasks, provider work, decisions,
disputes, exchange administration, payment modes, and wallets. [source]

The production Dockerfile builds one image whose default command starts only
the API. The control worker needs a second command or service definition. The
checked-in Compose file provides only PostgreSQL. Audited main has no hosted
deployment definition, backup automation, restore automation, or secret
provisioning. Those are still roadmap gates. [source]

## 9. Quality and claims

The CI workflow runs on pull requests and pushes to main. It starts PostgreSQL
17, installs the frozen lockfile, runs formatting, lint, type checks, all
tests, the production build, and `git diff --check`. Actions are current major
tags, not commit SHA pins. Permissions are read-only. [source]

At the audited commit, Vitest discovers 144 tests in 36 files. The local run
passed 117 and skipped 27 PostgreSQL-dependent tests. CI supplies the database
URL and is designed to run all 144. The checked-in status document reports a
green macOS run with all 144 tests. This audit did not independently reproduce
that database run. [test] [source] [limitation]

The tests are strongest around deterministic stores, protocol parsing,
payments fakes, Buzz authentication and relay re-query, wallet signatures,
GitHub patch planning, and the full PostgreSQL vertical slice. The real
GitHub App, real Buzz relay, real Stripe, hosted multi-exchange topology,
backup and restore, and hostile executor remain untested in this audit.
[test] [limitation]

The documentation is unusually candid for a young project. It says that
simulation is not money, logical isolation is not physical isolation, a mock
certification is not a sandbox proof, a workflow secret is not identity, a
patch is untrusted, and configuration does not cross a release gate.
[source]

The main documentation weakness is version layering. Current guidance sits
beside superseded v1 and v2 market designs and live code for those older
designs. The warnings reduce semantic risk, but they do not reduce the
compiled and migration surface. [source] [inferred]

## 10. Comparison with OpenAgents

| Dimension | TokenRelay | OpenAgents |
| --- | --- | --- |
| Product unit | Private exchange for accepted patches | Omega workroom and Khala orchestration over typed work and receipts |
| Execution | Provider-owned, outside the service | Disclosed native and external harness lanes with typed runtime events |
| Durable authority | PostgreSQL market state and ledgers | Product contracts, Cloud SQL authorities, engine state, receipts, and signed projections |
| Collaboration input | Buzz signed events and GitHub events | Selected Nostr profile, Forum, GitHub, Sync, and native workrooms |
| Evidence | Patch digest, provider claim, buyer decision | Typed verification and receipt classes with explicit loss |
| Settlement | Simulation ledger and gated Stripe adapter | Existing owner-gated settlement and public receipt ladders |
| Tenancy | Shared schema with exchange predicates and constraints | Owner, work, visibility, and service-specific authority boundaries |
| Runtime relation | No real runner, only mock certification | Native and attached runtimes, including Exoharness as a disclosed harness |

TokenRelay is narrower and more internally coherent than the old broad
OpenAgents labor-market documents. Its accepted-deliverable primitive matches
the current OpenAgents rule that cost per accepted outcome matters more than
raw model usage. Its buyer, provider, and operator separation also matches the
rule that the worker cannot verify or settle its own work. [inferred]

OpenAgents is stronger where TokenRelay is weakest. It already has typed
runtime event forms, verification classes, receipt references, settlement
visibility laws, and a current product decision that keeps Buzz and Nostr as
bounded interoperability inputs. TokenRelay should not become a parallel
authority for those domains. [inferred]

The Exoharness integration adds a useful composition:

```text
Exoharness or another disclosed provider runtime
  durable events + artifacts + sandbox identity
                    |
                    v
      content-bound execution attachment
                    |
                    v
   OpenAgents work and verification receipt
                    |
                    v
     accepted-outcome economic decision
                    |
                    v
 existing owner-gated settlement authority
```

The market never receives provider credentials. The runtime never decides
settlement. The buyer decision never rewrites execution evidence. The
settlement receipt never claims more than the verified evidence supports.
That is the composition worth building if a current product packet admits it.
[inferred]

## 11. Recommendation

**Study and re-derive. Do not deploy or integrate TokenRelay now.**

The repository is a strong six-day prototype of a focused market state
machine. Its best laws are already compatible with OpenAgents. Its service,
wallet, payment, and Buzz authority would duplicate current OpenAgents
boundaries. Its evidence seam is not strong enough for accepted work to become
portable proof without additional typed attachments and independent
verification. [inferred]

### Adapt

- Accepted deliverables as the traded unit. Never trade provider allowance,
  tokens, sessions, or runs. [source]
- Explicit submission, acceptance, rejection, dispute, resolution, and
  settlement states. [source]
- Separate buyer, provider, verifier, operator, and settlement authority.
  [inferred]
- PostgreSQL transactional outbox with row-locked claims, dead letters, and
  operator-visible recovery. [source]
- Generation-bound lease tokens and idempotent submission. [source]
- Integer accounting with balanced append-only postings and no implicit
  conversion. [source]
- Credential-derived tenant scope plus database constraints that backstop
  application predicates. [source]
- Buzz re-query and signature verification. Do not trust workflow webhook
  headers as identity. [source]
- Separate identity, repository, worker, lease, wallet, and settlement
  credentials. [source]
- Policy-bound signed payment intents as an authority pattern, without the
  simulated asset registry. [source]
- Exoharness-style artifact and event references in a common execution
  attachment. [inferred]

### Reject

- TokenRelay as a second market, wallet, payment, Buzz, Nostr, or settlement
  service inside OpenAgents. [inferred]
- Buyer acceptance without a content-bound verification and evidence class
  for work that claims independent proof. [inferred]
- The unverified provider proposal `signature` field as evidence. [source]
- Provider-written validation text as proof that a command ran or a patch
  applied. [source]
- Mock certification as executor containment or hostile-workload proof.
  [source]
- Shared-schema exchange predicates as sufficient isolation for confidential
  or regulated workloads. [source]
- `dedicated` or `sovereign` metadata as an infrastructure claim. [source]
- Simulation `USDC` or `BTC` labels as money, custody, payment, or settlement.
  [source]
- The compiled stack of superseded market generations as a model for a new
  OpenAgents package. [source]
- Railway or another new production authority. Google Cloud remains the sole
  OpenAgents production infrastructure authority. [inferred]

## 12. Watch items

1. **Hosted organisation proof.** Watch whether two exchanges complete a
   real relay, backup, restore, rotation, and negative-isolation exercise.
   [vision]
2. **Execution evidence.** Watch for a real runner-neutral receipt schema,
   patch-apply verification, and independent validation command execution.
   [vision]
3. **Proposal authentication.** Watch whether provider proposals gain
   canonical signing bytes and verified registered keys. [source]
4. **GitHub publication recovery.** Watch for live App tests and idempotent
   resume after partial branch, pull-request, or comment creation. [source]
5. **Schema cleanup.** Watch whether the v0, v1, v2, experimental, and outcome
   services converge on one current protocol and migration story. [source]
6. **Tenant hardening.** Watch for PostgreSQL row-level security or a proven
   dedicated-storage mode before confidential work. [vision]
7. **Payment posture.** Watch for real Stripe test reconciliation without
   inferring that test mode admits live money. [vision]
8. **Governance.** One-maintainer, no release tags, and one week of history
   make exact commit pins mandatory for any future reuse. [history]
9. **Exoharness attachment.** An Exoharness provider is a useful test case
   because its event log and artifact model expose exactly what TokenRelay's
   current provider boundary omits. [inferred]
