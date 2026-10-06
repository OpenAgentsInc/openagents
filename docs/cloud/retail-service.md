# The retail cloud service

Status: implemented behind the paid-availability gate and tested only with
fake payments, fake providers, and fake task owners. Nothing here is
purchasable until the owner confirms the [retail contract](retail-contract.md)
and the funded qualification passes (`NEEDS_OWNER.md`).

The [`retail-cloud`](../../crates/retail-cloud) crate composes the central
money ledger ([compute balance](compute-balance.md)), the router contract
(`crates/route-contract`), and the receiver wallet's contract into the
retail flow. Each section names the module that implements it.

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
seconds plus 20 minutes. Tests use `retail_cloud::fake::FakeProvider`; a live
Boat binding runs only in the owner's funded qualification
(`NEEDS_OWNER.md`).

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
