# The retail cloud service

Status: implemented behind the paid-availability gate and tested only with
fake payments, fake providers, and fake task owners. Nothing here is
purchasable until the owner confirms the [retail contract](retail-contract.md)
and the funded qualification passes (`NEEDS_OWNER.md`).

The [`retail-cloud`](../../crates/retail-cloud) crate composes the central
money ledger ([compute balance](compute-balance.md)), the router contract
(`crates/route-contract`), and the receiver wallet's contract into the
retail flow. Each section names the module that implements it.

## Customer transport and resident worker

[`retail-service`](../../crates/retail-service) mounts `POST /v1/retail` as
bounded HTTP JSON (`openagents.cloud.retail-customer.v1`). The binary accepts
`--config /absolute/private/config.json`, binds only loopback, and starts a
resident worker independently of customer connections. REV-15 packages the
external authenticated TLS origin and host deployment; REV-14 adds the selected native customer
controls below. This transport has only synthetic qualification.

Every request sends `Authorization: Bearer ...` and `x-retail-principal`.
The central compute ledger resolves the current principal, credential epoch,
account, and read/spend right. The operator separately configures retail
observation, execution, and disclosure rights for that principal and epoch.
Account ownership, pairing, a balance, and an invoice grant no retail rights.
The service uses no cookies and rejects browser `Origin` requests.

An optional host `commercial` configuration names `canonical_directory`,
`issuer`, and an explicit `commercial-accounts` `native` policy and store set.
It must select the same held retail ledger. Reviewed bindings add customer
and workspace attribution without granting product or financial rights.
Offers freeze that reference in credential-custody terms; confirmation and
new worker execution or disclosure require the same current revision.
Credential, membership, or ownership changes require reviewed lineage and a
new offer. History retains the original reference, while safe cleanup and
money reconciliation continue under the original native records.
Mapped funding freezes attribution before invoice creation. Retries and status
reads retain the original reference after conversion or membership removal;
an interrupted invoice attempt cannot resume under changed attribution.
Legacy funding keeps its original native account without a retroactive mapping.

| `op` | Result or effect |
| --- | --- |
| `account`, `capacity` | Account-scoped exact millisatoshi balance; existing paid-availability gate |
| `top_up`, `top_up_status` | Account-scoped idempotent invoice and receiver-observed status; no customer paid callback |
| `offer`, `confirm` | Frozen supported offer; exact offer, admission, and credential-custody digests plus explicit consent |
| `executions`, `execution`, `progress` | Account-scoped bounded discovery, retained recovery/hold state, and progress cursor |
| `cancel` | Durable stop request; acknowledgment, deletion, final usage, and settlement remain separate |
| `artifact`, `receipt` | Logical retained artifact, retention/deletion, cancellation, and actual settlement records |
| `environment_offer`, `environment_confirm`, `environment`, `environment_delete` | Saved customer environments on the Pro subscription ([contract](retail-environment-contract.md)); "not available" until the configuration's `environments` launch opens |

The listener admits at most 32 in-flight requests before reading bodies;
excess requests receive `429 busy`. The request body limit is 32 KiB.
Idempotency keys contain at most 64 ASCII letters, digits, hyphens, or
underscores and are scoped to the authenticated account. Offers remain
immutable in a private store capped at 4,096 records;
confirmation retries preserve the original execution and credential digest.
Progress returns at most 128 events and 64 KiB of text per page. Artifact
names are logical identifiers; the existing eight-artifact and 8 MiB-per-artifact
bounds and 30-day retention apply.

A confirmation also admits `openagents.cloud.retail-credential-custody.v1`:
the authenticated service may retain the customer's own OpenAI key in a
mode-0600 private vault, deliver it only to the exact admitted sandbox and
model payer, redact it, and remove it. The journal stores only its digest.
Custody ends after acknowledged resource deletion or the quoted task limit
plus 35 minutes, whichever comes first; unfinished cleanup and unknown cost
holds remain recorded. The service never imports a provider login, operator
model key, environment credential, or home-directory fallback. Configuration
and state paths reject symlinks; one process owns each service state directory.

The worker visits at most 16 active confirmations per pass, resumes accepted
confirmations and holds, reconciles original provisioning/task identities,
and advances material delivery, metered dispatch, deadlines, cancellation,
credential removal, retention, deletion, and evidenced settlement. It reads
fresh ledger revocation/generation and spend rights before side effects;
changing an operator retail grant takes effect on service restart. Cleanup
continues after revocation. Provider/task uncertainty never becomes another
resource or task, zero cost, or a released hold. The Boat adapter syncs the
original create intent before sending; after a process loses the reply and
local index entry, the freshly admitted worker may reconcile only identical
create bytes under the original idempotency key within 10 minutes. A later
restart preserves uncertainty and cleanup obligations without recreating.
`GET /healthz` reports only
process liveness; use authenticated `capacity` and the existing operator
health reader for admission and obligations.

## Selected native client

`retail-client` (`compute-workbench`, feature `client`) uses this actual service
adapter. Select a private `openagents.compute-retail-client.v1` configuration
with `endpoint` (an HTTPS origin ending in `/v1/retail`), `principal`,
`bearer_file`, `state`, `read_only`, and `development_loopback`. The latter
admits numeric loopback HTTP only for isolated development. Files must be
owned, private, unshared, and free of symlinks; existing shared files are
refused without changing permissions. No credential or state comes from the
home directory, environment, paired host, or gateway key.

Use `retail-client --config /absolute/private/client.json` with `account`,
`capacity`, `top-up`, or `top-up-status`. A top-up requests an exact invoice;
pay it with a separately selected wallet. Only receiver evidence credits the
central balance. `quote --idempotency ID --task /absolute/private/task.json
--provider-key /absolute/private/openai.key` displays the supported contract,
all payer/disclosure lines, expiry, and a retained review digest. Confirm it
with `confirm --review DIGEST --provider-key /absolute/private/openai.key
--service-custody`. The key reaches the authenticated service only through
that explicit confirmation. Task input and shell approval grant nothing.
Changed terms, key, account, or current rights require a new review; a lost
confirmation reply retries the identical funded request.

`executions`, `reconnect`, and `progress` observe account-bound execution and
cursor records. `cancel` requests a stop under current execution authority;
`receipt` separates the request, executor acknowledgment, deletion, unknown
usage, actual measured charge, and unused hold release. `artifact` reads a
logical retained result. Customer model expense remains unknown and separate;
a failed check does not erase measured compute charges. A `read_only` client
can observe these records and cannot fund, quote, confirm, or cancel.

Client references replace atomically under an exclusive private lock. The
service pins its receiver node and fences the original state, lock, journal,
and ledger descriptors before effects and after blocking collaborator calls.
The native journal rechecks custody at write boundaries, including recovery's
unknown-hold mutation; losing custody cannot append a conservative observation
through the old writer. Replacing or sharing those paths refuses the old writer;
reconnect preserves
the original ledger and execution rather than admitting a second purchase.
These client controls have synthetic binary/HTTP acceptance only. O3/O4/O8
still gate real funds, TLS deployment, live provider cleanup, and installed
customer acceptance.

## Browser delegation

The web app reaches this transport only through `openagents-web`'s retail
delegation ([README](../../crates/openagents-web/README.md#retail-delegation)):
an operator-provisioned native client per account, workspace, and membership
epoch, called from the server without `Origin`. The browser `Origin` refusal
above is unchanged. The customer's key passes from the site's scoped custody
vault into the same `confirm` request as the native client's.

## Authorities

[`retail_cloud::authority`](../../crates/retail-cloud/src/authority.rs)

A retail admission (`openagents.cloud.retail-admission.v1`) binds the
account, the funded execution, the source and request digest, the computer
and task classes, the retail grant generation, the effects, the material
recipients and disclosed content, the model payer, the price book version
and digest, and the maximum charge. It is a new document bound by digest;
the frozen admission snapshot is unchanged.

Before every side effect, `check` reads the current rights and requires
every authority the step needs:

| Step | Observe | Execute | Disclose | Spend |
| --- | --- | --- | --- | --- |
| Reserve funds | | | | Yes |
| Provision a computer | | Yes | | Yes |
| Dispatch the executor | | Yes | Yes | Yes |
| Read progress, artifacts, or receipts | Yes | | | |
| Cancel or steer | Yes | Yes | | |
| Upload source and the customer's key | | Yes | Yes | |

- **Execute** needs a retail grant for this execution at the admitted
  generation. An operator, pool, auto-start, or pairing grant refuses
  (`not_retail`).
- **Disclose** needs the consent the customer gave for this exact admission
  digest. Consent for an earlier admission does not cover a changed one.
- **Spend** is the principal's spend right, read from the ledger at the
  moment of the step, so a rotated or revoked credential stops spending at
  once.
- **Observe** is a grant for this account and execution.

Pairing, world membership, a balance, and a paid invoice supply none of
these.

A changed recipient, source, computer, effect, payer, or quote is a new
admission: `RetailAdmission::changes` names what moved, and the caller
issues a new offer.

Revocation stops new work and never erases an obligation:

| Stage at revocation | What follows |
| --- | --- |
| Reserved only | Release the hold; no charge |
| Computer provisioned, executor not started | Tear down; no charge |
| Executor started | Request a stop, wait for its acknowledgment, tear down, and settle the measured seconds |
| Unknown | Reconcile before reporting |

## Offers

[`retail_cloud::offer`](../../crates/retail-cloud/src/offer.rs)

`make_offer` quotes one request for one account and returns a
[`route_contract::Offer`](../../crates/route-contract/src/offer.rs) whose
terms carry the request digest, the retail admission's digest, the computer
class, the effects, the material recipients, the maximum charge, and the
source revision, together with the price book's quote and the proposed
admission. The quote lists every charge line with its payer: compute and
coordination from the purchased balance, and the model on the customer's
own OpenAI key at zero. Sponsored hosted inference is a separate field that
is always `off` for a retail run.

No offer is made, and nothing is reserved, for a typed outcome:

| Outcome | When |
| --- | --- |
| `unsupported` | The source is not a public `github.com` repository at a 40-digit commit, the task text is empty or over 16 KiB, there are not 1 to 8 checks of at most 1,024 bytes, or the wall time is not 1 to 3,600 seconds |
| `unavailable` | Four retail sandboxes already run (`retail_limit`), or the operator plan has no Boat starts left (`plan_limit`) |
| `price` | The price book refused: an unknown class, seconds out of bounds, or a maximum above the customer's ceiling |

An offer stays confirmable for 10 minutes. `confirm` accepts only the offer's
own control; shell-command approval and input routing refuse
(`not_an_offer_control`). It re-quotes the request from the current book and
rebuilds the admission, and refuses an expired offer (`expired`), another
digest (`mismatch`), or any changed price, source, recipient, or provider
(`changed`). The first confirmation records exactly one funded request with
its execution identity in the journal; a repeated confirmation, even after a
restart, returns that same funded request and reserves nothing more.

## Provisioning

[`retail_cloud::provision`](../../crates/retail-cloud/src/provision.rs)

`Provider` is the provider seam: create a sandbox from an exact
`CreateSpec`, find one by its provisioning identity, read its state, delete
it, and read its billed seconds. The v1 class pins one Boat `large` sandbox
per task from the daily template `oa-coder-main-<date>`, started with no
account environment (`no_env`), labeled with the customer's account and the
provisioning identity `<execution>#<attempt>`, with a lifetime of the quoted
seconds plus 20 minutes. Tests use `retail_cloud::fake::FakeProvider`. The
live Boat binding, [`retail_cloud::boat`](../../crates/retail-cloud/src/boat.rs)
(feature `boat`), implements this seam and the sandbox, task-owner, stop, and
artifact seams; it runs only in the owner's funded qualification
([qualification](retail-qualification.md), `NEEDS_OWNER.md`).

`advance` moves one funded execution forward by one observation and is safe
to repeat after any crash:

| State | Meaning |
| --- | --- |
| `intent` | Recorded before any provider call |
| `creating` | A create call went out and its answer is not known; the next step looks the sandbox up by its provisioning identity instead of creating another |
| `starting` | The sandbox exists and is not reachable yet |
| `ready` | Reachable; provisioning is done |
| `refused` | The provider refused the start for plan or capacity limits; no charge |
| `unavailable` | Not reachable after the one replacement; no charge |

- Nothing calls the provider without the provision authorities and a live
  hold for the funded request.
- A failed listing is not proof of absence: the state stays `creating`, and
  nothing is created.
- A sandbox that is not reachable within 10 minutes, or whose restore
  failed, is deleted and replaced once under attempt 2; the replacement's
  cost is the operator's. Abandoned sandboxes are kept in the journal for
  teardown.
- Nothing widens the provider or the computer class automatically.

`retail_cloud::retain` records cleanup duties before provider deletion, retains declared bounded task artifacts in the private journal for 30 days, and records missing delivery separately from acknowledged resource deletion; a service worker reconciles all recorded provisioning attempts after client loss, and observing retained artifacts requires the original read grant.

`retail_cloud::cancel` records cancellation or matching revocation before new work is refused, reconciles exact task-owner stop receipts without replaying unknown stops, and exposes executor acknowledgment, deletion, final usage, actual ledger charges, and remaining holds separately. Read-only reconnects observe the same receipt without control or spending authority.

## Source and credentials

[`retail_cloud::material`](../../crates/retail-cloud/src/material.rs)

A retail sandbox receives exactly two things: the admitted public source at
its exact commit, and the customer's own API key for the admitted model
payer. `deliver` reads nothing else to fill a gap: no owner or operator
credential, no Secret Manager entry, and no environment variable.

- Only this execution's own ready sandbox receives its material; another
  customer's execution is refused.
- The clone must check out the admitted commit with a clean tree, or the
  delivery refuses (`source_unverified`).
- The key is written to `/tmp/oa-retail/<execution>/provider.key` with mode
  0600 through standard input. It never appears in a command line, the
  environment, a log, a manifest, or an artifact. `scrub` redacts it from
  any text before it is kept. `CustomerSecret` prints as `redacted` and
  zeroes its bytes when dropped.
- The journal records the sandbox, source, payer, key path, and the key's
  SHA-256 digest, never the key. `remove_credentials` deletes the file at
  teardown, checks that it is gone, and records the removal.

| Refusal | When |
| --- | --- |
| `provider_changed` | The key is for another provider than the admitted payer; a new offer is needed |
| `source_changed` | The source differs from the admitted one, for example a wider repository; a new offer is needed |
| `own_login_export` | The customer offered an engine login; a login never leaves the customer's computer in v1 |
| `paid_provider_unsupported` | OpenAgents-paid model access is not part of the v1 class |
| `source_unverified` | `HEAD` is not the admitted commit, or the tree is dirty |
| `isolation_unsupported` | The sandbox cannot keep a private file |

## Dispatch and observation

[`retail_cloud::dispatch`](../../crates/retail-cloud/src/dispatch.rs)

A funded execution has one task identity, `task_<execution>`, recorded in
the journal before the task owner on the sandbox is contacted. `dispatch`
starts the executor only when the dispatch authorities, a live hold, a ready
sandbox, and delivered material are all in place.

- A transport attempt is not a task. The journal counts attempts separately
  (`intent`, `sent`, `acknowledged`).
- Before each submission, `dispatch` asks the owner whether it already has
  the task; the owner's submission is idempotent on the task identity. A
  lost acknowledgment, an unreachable owner, a service restart, or a client
  retry recovers the same task, and the executor starts once.
- The specification carries the request digest, the checks frozen before
  the candidate exists, the quoted seconds, and the one v1 engine, Codex.

`observe` reads the owner's events after a cursor and the task's status. It
needs only the observe right and changes nothing, so a client that
disconnects reattaches with its cursor without cancelling or redispatching.

`verdict` maps an ended task to the contract's check outcomes: `verified`
when every declared check passed on the exact retained candidate,
`check_failed` when one failed or ran on another candidate, and `unchecked`
when the executor made no change.

The [settlement module](../../crates/retail-cloud/src/settle.rs) binds the
original quote and price book to retained final usage and execution or
cancellation evidence. The central ledger posts the debit, obligations, and
unused hold release in one transaction; a retry seals the same receipt.
Unknown costs stay held. An unused hold release is separate from a payment
refund, and a failed check does not erase measured compute charges.
