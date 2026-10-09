# Saved customer environments: contract v1 (proposed)

Status: proposed 2026-10-09 for owner review
([#11006](https://github.com/OpenAgentsInc/openagents/issues/11006), ENV-10).
Nothing here can be bought yet. The code path exists and is closed. It opens only
when the owner has reviewed this page, published a reviewed price book, and
recorded a funded qualification (see [Availability](#availability)). A change
after that review is a new contract version, not an edit to this one.

The [retail contract v1](retail-contract.md) sells one task on one sandbox and
then deletes the sandbox. That contract cannot keep anything. This contract
is the separate class that the [environment onboarding contract](example-cursor-cloud-agent-onboarding/environment-onboarding.md)
requires before customers can keep a machine image: a customer pays to set up,
build, check, and save one repository environment. The saved image is then kept
for the days they paid for.

## The classes

| Identity | Value |
| --- | --- |
| Contract | `openagents.cloud.retail-environment.v1` |
| Computer class | `retail-env-boat-large-v1` |
| Task class | `retail-environment-setup-v1` |
| Price book schema | `openagents.cloud.environment-price-book.v1` |
| Ledger resource | `openagents.cloud.retail-environment.v1` |

### What is bought

- **Machines.** One setup machine, one clean builder, and one independent
  verifier with its idempotence fork: at most 4 Boat `large` machines. They
  are the ENV-03 to ENV-05 owners' dedicated machines, never a shared pool
  host and never a customer's chat computer.
- **Wall time.** At most 2 hours for setup, build, and check together. The
  customer may set less.
- **Source.** A public HTTPS repository on `github.com` at an exact
  40-character commit, as in retail v1. No private repositories, uploads, or
  submodules.
- **Request.** An objective of at most 8 KiB, a qualification profile name,
  and 1 to 8 behavior checks, each at most 1,024 bytes. These are frozen by
  digest before any build.
- **Saved image.** When the checked version is saved, its image is kept
  for the retention days the customer chose: 1 to 90 days, prepaid. Images
  larger than the class's limit (50 GB in the proposed book) are not saved.

### Credentials and authority

- **One credential.** The setup agent runs on the customer's own OpenAI API
  key (`customer:openai`), delivered when the setup machine starts through
  the same private custody and per-boot delivery as retail v1 (#10712). It
  is never written into a recipe, image, log, or receipt, and the builder's
  sanitization step removes sign-ins before capture. No owner or operator
  credential reaches any machine.
- **Selection.** A saved version belongs to the account that bought it.
  Only that account's tasks can select it (`selectable_by`), and only while
  its retention is paid. A balance does not grant execution, and pairing
  does not grant spending, as in retail v1.
- **No machine access.** The customer gets no shell, SSH, or terminal on
  any machine. They see the setup conversation, the recipe, the check
  results, and the evidence.
- **No publication.** No push, pull request, or issue comment.

## Prices

Prices are in sats. One credit is one sat. The proposed book
(`crates/retail-cloud/fixtures/environment-price-book.json`,
`retail-env-2026-10-09.0`, status `proposed`) is a starting point for owner
review, not a decision:

| Line | Proposed rate | Basis |
| --- | --- | --- |
| Compute | 40 msat per machine-second | Measured seconds summed over every machine, capped at 4 machines for the whole wall time |
| Coordination | 200 sats | Once per purchase whose first machine started |
| Storage | 3 sats per GB-day | The saved image's real size, rounded up to a whole GB, for the paid retention days |
| Model | 0 | The customer's own OpenAI key, billed by OpenAI |

The offer quotes and holds the maximum: every machine for the whole wall
time, coordination, and the largest image for every retention day. For
example, two hours and 30 days hold 1,152 + 200 + 4,500 sats.

| Ending | Charge |
| --- | --- |
| No machine started, or none reachable | Nothing; the hold is released |
| Machines ran; no version saved, or cancelled | Measured compute plus coordination |
| Version saved | Measured compute, coordination, and storage for the real image size |
| Not known yet | The whole hold stays held until the ending is known |

Settlement posts the charge once and releases the rest of the hold. A
release is not a refund. More retention is a renewal: the customer pays
`size × days × rate` at once, and the days are added. A retry with the same
renewal identity is the same renewal.

## Retention

When the paid days run out, the version lapses. It can no longer be
selected or renewed, and its image is due for deletion by the provider
owner (`environment::lapse` returns it). A lapsed image is not kept "just in
case". Tasks that already started on that version keep the version they
started with. The purchase record and its receipt stay in the retail
journal.

## Funding and recovery

Each step has exactly one ledger effect. The journal records the step
before or after that effect, so a restart never repeats it:

- **Confirm** holds the quoted maximum under `env:<purchase>`. A retry
  returns the same hold. An insufficient balance holds nothing, and the
  purchase stays an offer. If the book changed since the offer, the
  confirmation is refused. A shared retail balance cannot hold for an
  environment.
- **End** records the ending and the measured usage once. An unknown ending
  marks the hold unknown. A later known ending replaces it, and a known
  ending never changes.
- **Settle** charges once, under the environment resource.
- **Recover** (the retail worker, every tick) confirms an offer whose hold
  landed before the journal recorded it. It also settles known endings once,
  keeps unknown ones held, and lapses retention that ran out while the
  service was down.

The fake-funding and recovery checks are `crates/retail-cloud/tests/environment.rs`
and `saved_environments_use_the_retail_transport_and_stay_closed_by_default`
in `crates/retail-service`.

## Transport

Customers use the authenticated retail customer transport (#10956, #10970):
`/v1/retail` with the same principal, grant, and spend checks as tasks.

| Operation | Needs |
| --- | --- |
| `environment_offer` | spend, execute, and disclose |
| `environment_confirm` | spend, execute, and disclose |
| `environment` | read; own account only |
| `environment_renew` | spend |

The native client (`compute-workbench`, feature `client`) has
`environment_offer`, `environment_confirm`, `environment`, and
`environment_renew`.

## Availability

`environments` in the retail service configuration holds the owner's launch:
the published book and its gate. The gate opens only when all of these hold:

1. `contract_reviewed` is true. The owner has reviewed this page.
2. The book's status is `published`, and the gate names its exact digest.
3. A funded qualification receipt is recorded. That receipt comes from a
   real setup, build, check, save, and renewal paid from a real balance on
   the deployed service, with the machines' teardown acknowledged.

Without `environments`, every environment operation answers "not available".
The checked-in book is `proposed`, so it can never open the gate. A fixture
never activates this lane.

## Not in v1

GCE placement (the ENV-09 adapter is operator-only), private repositories,
customer shells, publication, continuation on a kept machine, sharing a
saved environment with another account, and model usage paid by OpenAgents.
