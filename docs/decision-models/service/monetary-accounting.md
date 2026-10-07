# Monetary accounting

`gateway`'s monetary admission is the request-time half of
[`tenancy::money`](monetary-ledger.md): it charges an authenticated workspace
for dispatched work. The mode is an explicit operator opt-in — no part of it
runs, and no workspace is charged, unless `gateway.json` names a `money`
document. This document covers the admission path, the settlement rules, and
the reconciliation responsibilities an operator takes on by enabling it. It
does not choose launch prices, collect payments, or grant credit; the fixtures
in the test suite are synthetic and carry no commercial meaning.

## Configuration

```json
{
  "v": "openagents.gateway.v1",
  "registry": "/srv/registry",
  "require_workspace_membership": true,
  "money": {
    "ledger": "/srv/protected/money.jsonl",
    "doors": {
      "acme-kev": {
        "price": {
          "version": "synthetic-fixture-v1",
          "currency": "USD",
          "model": "kev-0.6b",
          "capacity": "dedicated",
          "policy": "observed-usage-v1",
          "rates": {
            "input-tokens": {"millionths": 10, "per_units": 1},
            "output-tokens": {"millionths": 40, "per_units": 1}
          }
        },
        "maximum_usage": {"input-tokens": 1000, "output-tokens": 100}
      }
    }
  }
}
```

- `money.ledger` is the `tenancy::money` log's path. Keep it in a protected
  directory outside every executor write grant. The gateway opens it
  exclusively at startup and holds the lock for its lifetime; a ledger that
  cannot open fails the process, not the first call.
- `money.doors` maps a door name to the price it charges under. A configured
  backend door missing from this map refuses every call as `unpriced` — the
  gateway never invents a price to keep a door serving. An entry naming a door
  with no configured backend is refused at load.
- `price` is the versioned `tenancy::money` schedule. Its `model` must equal
  the binding's artifact id, its `capacity` the binding's lane, its `policy`
  the settlement policy this build implements (`observed-usage-v1`), and its
  `currency` the workspace account's. A price that disagrees with the binding
  refuses each call as `price_invalid` before dispatch.
- `maximum_usage` is the explicitly bounded worst case one attempt may be
  billed for, and must name exactly the priced resources. The reservation holds
  the price's quote of this map; a config whose price cannot quote its own
  bound is refused at load.
- Monetary admission requires `require_workspace_membership`: a charge binds
  an authenticated workspace, never an anonymous or bearer-only call. The
  config check refuses `money` without it.

Absent `money`, the gateway behaves exactly as before: no ledger opens, no
workspace is charged, no balance route exists, and no `x-settlement` header
appears.

### Selected native decision offer

The first supported offer is native `kev-0.6b` on CPU with `f32` model and
head tensors, through the packed `POST /v1/systemone` route. Add an
`offer` to that door's existing `money.doors` entry with schema
`openagents.decision-offer.v1`, an explicit `id` and `version`, the exact
`identity`, and `requests_per_minute`. Configure `accounts`, workspace
membership, and a dedicated registry binding with one concurrent call and
the same request-rate limit. The
[isolated offer fixture](../../../crates/gateway/tests/decision_offer.rs)
contains the complete configuration shape; its rates, balance, and digest
are synthetic and authorize no commercial launch.
The gateway forwards the selected native artifact ID as `model`, preserving
the original request digest and admitted registry snapshot; a customer door
name needs no additional native alias.

Use the retained
[`kev-0.6b` artifact lock](../../../crates/kev/fixtures/variants/kev-0.6b/artifact-lock.json)
to identify the supported artifact family. Pin the loaded content digest
that the [native loader](../../../crates/kev/src/artifacts.rs) computes,
rather than the artifact lock's file digest. The offer and registry must
match that digest and all nine execution settings: `backend`, `dtype`,
`head_dtype`, `attention`, `bucket_size`, `lora_merge`, `option_isolation`,
`max_state`, and `max_branch`. This offer has no separate adapter identity.

The native card declares `openagents.decision-metering.v1` with the
`openagents.kev.packed-input.v1` adapter. It prices only `input_tokens`:
the number of encoded token IDs in the packed state and question sequence.
Native token admission bounds the padded forward before inference, so it
also bounds this counter. The native ceiling and `maximum_usage` must match
exactly. The gateway checks the counter's token units, basis, empty overlap
list, caller-loop execution, and enforced ceiling before dispatch.
`output_tokens` counts serialized answers and remains unpriced; this offer
reports no cached-input counter. Decision access supplies no hosted text
generation or execution authority.

Use one positive `input-tokens` rate in the existing `tenancy::money::Price`.
`millionths` and `per_units` form an exact rational rate; currency amounts
have scale `1_000_000`, and each attempt rounds its charge up to a currency
millionth. For example, the synthetic rate `7` per `2` tokens with a ceiling
of `128` holds `448` millionths; observed usage of `3` tokens charges `11`.
Missing rates, incompatible resources, a zero denominator, overflow, and
changed retained terms under the same price version refuse. Use a new price
version when rates change: the existing ledger retains the full original
price, usage ceiling, and settlement for each attempt.

Authenticated `GET /v1/models` adds `decision_offer` terms to the selected
card. These terms bind the configured offer, exact price, quote ceiling,
current customer/workspace/payer references, and settlement policy through
the existing purchase context. Balance fields require the key's `balance`
scope; restricted reads retain unknown funding admission. The terms state
`configured`, rather than claiming backend health, production qualification,
purchased funds, or physically reserved hardware. Provider and hosting
expenses remain explicitly unknown and separate from the retail charge.
Missing or conflicting response model identity and unpriceable usage retain
the full outstanding hold. Same-request/attempt retries never redispatch.

Production activation requires [owner step O5](../../../NEEDS_OWNER.md#native-decision-offer-o5-rev-16-10823):
accepted commercial terms, actual artifact and capacity qualification,
restricted deployment credentials, real funding, and a matching typed
result, charge, and receipt. The isolated tests establish admission and
accounting behavior without model weights, real funds, or a host deployment.

## Hierarchical budgets

Set `money.hierarchical_budgets` to `true` to enforce reviewed workspace,
team, and person caps on the native decision route. This profile requires
accounts and supported native offers for every enabled monetary door. It does
not span retail, plugin, or other product ledgers. Missing policy or an
unreviewed person refuses before dispatch. Classification and durable-job
submission explicitly refuse as `budget_route_disabled` in this profile.

The current workspace owner installs a policy through authenticated
`PUT /v1/workspaces/{workspace}/budgets`. The request carries a bounded
`request` ID, `expected_policy` (null for first installation), and a
`openagents.money.budgets.v1` policy: increasing integer `version`, explicit
`currency`, `scale: 1000000`, `route: gateway-monetary-v1`, `effective_from`,
workspace `cap` and `alert_at`, team limits, and native Accounts person IDs
with their reviewed team and limit. Thresholds use the same millionth unit as
the cap. The effective bound must not be in the future; the response also
records the journal's actual `activated_at`. Roster entries must be current
workspace members and include the owner. A policy grants no membership,
funding, or execution authority.

Caps are cumulative. Increasing the version, changing a credential, or
restarting the process never resets usage. Every reservation pins its original
native person, team, policy digest, and sealed membership revision. Lowering a
cap preserves those pins and blocks new work when retained exposure exceeds
it. A team counts its originally admitted work and the historical work of its
current reviewed members, so moving or renaming a roster cannot restart its
capacity. Held work, including dispatched work, and unknown work retain the
full reservation. Verified settlement counts its net charge; only an
authoritative release or refund restores capacity.

First activation retains earlier workspace obligations. Records without
trustworthy native person/team evidence remain unattributed and conservatively
count against each child cap too. They are never assigned to a guessed person.
Policy and reservation mutations share the existing money journal and writer
lock. The Gateway rechecks the original credential and current native
membership under the Accounts writer boundary immediately before appending
the reservation.

Authenticated `GET /v1/workspaces/{workspace}/budgets?requested=AMOUNT` reports
remaining capacity, reserved and unknown liabilities, net settled usage,
owner-declared threshold alerts, and the first bound that blocks the requested
integer amount. It includes the policy digest, journal head, and observation
time. Members see their own person, their team's aggregate, and the payer cap;
admins and owners see the reviewed workspace roster. Alerts explain
enforcement and cannot grant credit or release a hold. The
[isolated fixtures](../../../crates/gateway/tests/decision_offer_budgets.rs)
exercise the actual HTTP route and killed Gateway recovery with synthetic
members, prices, and funds. After a killed writer, the existing native quota
PID marker requires confirmed-dead-writer recovery before restart; the
fixture removes only that exact waited child's marker and retains both
journals. Monetary recovery keeps the full unknown liability. Production
qualification remains an
[owner step](../../../NEEDS_OWNER.md#native-team-budgets-rev-40-10847).

## Admission

A call under monetary admission runs the existing sequence — authenticate,
authorize the door, bound capacity, reserve quota — and then, before any
backend dispatch, reserves the worst-case spend:

1. The caller authenticates a bearer key and exactly one `X-Workspace-Id`
   header; the membership store must confirm the key's account is a current
   member of a workspace bound to the key's tenant. Balance belongs to the
   workspace, so keys in the same workspace share it, and rotating a key
   changes nothing the account holds — the new credential authenticates
   through its own account membership.
2. Quota reserves its own units first, unchanged. Monetary admission is a
   separate control on top of it, never a replacement.
3. The money reservation is atomic inside the ledger's writer lock. It binds
   the attempt `{idempotency-key}#{x-attempt}`, the request digest, the price
   terms, and `maximum_usage`. Concurrent calls serialize on the ledger — a
   workspace cannot overspend. An existing `(request, attempt)` refuses with
   `idempotency_conflict` before dispatch and preserves the original hold
   (`400` on the decision route under NIP-DEC; `409` on classify).
   Concurrent retries cannot execute twice against one reservation.
4. Only then does the backend's published identity get verified and the
   request forwarded.

A refusal at step 3 never reaches the backend. An existing attempt preserves
its original quota reservation; other monetary refusals release a fresh quota
reservation:

| Refusal | Status | Cause |
| --- | --- | --- |
| `workspace_required` | 400 | No `X-Workspace-Id`, or membership mode off |
| `unpriced` | 503 | The door has no configured price |
| `price_invalid` | 503 | The price's identity or terms disagree with the binding |
| `insufficient_funds` | 402 | No provisioned account, or the hold exceeds available balance or remaining spend |
| `ledger_unavailable` | 500 | The ledger refused or failed a write |

A failed identity check after reservation — `door_unavailable` or
`identity_mismatch` — is work that never dispatched: the gateway releases the
hold and reports `settlement: released`. That is the only release the gateway
writes; every other unresolved hold is operator business.

## Settlement

Under `observed-usage-v1` a dispatched attempt is charged exactly the usage
the serving process reported, quoted under the hold's pinned price — nothing
estimated, nothing invented.

- `POST /v1/systemone` settles the response's `usage` object when it reports
  every priced resource as a count.
- `POST /v1/classify` holds one reservation for the aggregate call and settles
  the sum across dispatched items — but only when every dispatched item
  reported every priced resource. One silent item leaves the whole hold
  outstanding rather than partially priced.
- A call whose fan-out dispatched nothing releases its hold.
- A missing, partial, malformed, or over-bound usage report — and any backend
  failure after dispatch — marks the attempt `unknown`: the full reservation
  stays outstanding, never zero. The gateway writes no silent release and no
  silent retry after uncertain forwarding.

The response carries the resolution in `x-settlement` — `settled`,
`outstanding`, or `released` — and gateway-generated refusals carry a
`settlement` field when a hold was involved. The receipt's `usage` field names
the same `{request}#{attempt}` reference the ledger's hold binds, so one name
joins the receipt, the quota entry, and the monetary record.

### Priced and unpriced resources

The priceable resource set is the ledger's: `input_tokens`,
`cached_input_tokens`, `output_tokens`, `reasoning_tokens`, and
`compute_milliseconds`, read from a backend response's `usage` object under
those exact names. A provider counter outside that set is unpriced — a
response carrying it still settles if every *priced* resource is reported.
A priced resource the backend never reports makes every completion
unpriceable, so a price should cover only counters its door actually emits.

## Balance

`GET /v1/balance` exists only under monetary admission. It authenticates the
same bearer key plus `X-Workspace-Id` membership as the decision path — a
caller can only ever read a workspace it belongs to — and answers the
account's exact position plus the configured doors' price versions:

```json
{
  "workspace": "ws_...",
  "balance": {
    "currency": "USD",
    "credited": 28000,
    "reserved": 14000,
    "settled": 70,
    "refunded": 0,
    "available": 13930,
    "spend_remaining": 9223372036854751807
  },
  "prices": {"acme-kev": {"version": "synthetic-fixture-v1", "policy": "observed-usage-v1", "currency": "USD"}}
}
```

A workspace with no provisioned account answers 404 `account_missing`. There
is no top-up, payment, or credit mutation over HTTP — `POST`, `PUT`, and
`DELETE` on the route are refused, and funding is the operator's own ledger
act. Under a configuration without `money` the route does not exist at all.

## Recovery and reconciliation

Restarting the gateway reopens the ledger, which turns every hold still held
into `unknown` without releasing it — the outstanding liability survives a
crash exactly as it survives a timeout. Reconciliation is the operator's job:

- An `outstanding` hold means the attempt may have consumed real work. Settle
  it with evidence of the usage — the ledger accepts a known settlement on an
  unknown hold — or release it only with evidence no charge is due. A timeout
  alone is not that evidence.
- `insufficient_funds` and `price_invalid` refusals are configuration or
  provisioning gaps: credit the account or fix the price; the ledger stays
  authoritative either way.
- A poisoned ledger (a write failure) refuses every subsequent mutation until
  an operator reconciles the file; the gateway reports `ledger_unavailable`
  rather than spending against uncertain durable state.
- The ledger is a bounded single-host log — 16 MiB — and refuses at that
  bound. Archival and compaction are operational work outside the gateway.
