# Usage, activity, and the dashboard

`gateway`'s usage surface answers the question every paying or
quota-bound caller eventually asks: what did my workspace do, what did
it cost, and what is still outstanding. `GET /v1/workspaces/{id}/usage`
and its siblings read the workspace's own execution receipts joined to
the money ledger's holds and the quota ledger's reservations, so the
number a caller sees is the number the ledger charged — never a sample,
never an estimate. The dashboard under `/dashboard` renders the same
reads as server-rendered pages a member opens with a session cookie.
The surface mounts only when `gateway.json` names an `accounts`
document, because every read binds a workspace membership; the billing
block is optional — without it the entitlement field is `null` and the
billing page says so.

## Authorization

Every usage read is member-scoped: the caller authenticates as a
session token or an `oak_` key bound to an account, and the account
must hold an active membership in the named workspace. A member of
another workspace, an anonymous session, and a missing credential all
refuse — `unauthenticated`, `no_account`, or the membership refusal —
and no response carries another workspace's data. The disclosure block
counts what the scoping skipped: receipts with no workspace field
(`unattributed`), receipts belonging to other workspaces
(`other_workspace`), and lines that failed digest verification
(`unverifiable`). Scoping is disclosed, never silent.

## Reads

All routes carry the `openagents.usage.v1` schema tag and take the
same query filters.

| Route | Answer |
| --- | --- |
| `GET /v1/workspaces/{id}/usage` | The position: call totals by outcome, quota-derived units, exact cost fields, outstanding holds, breakdowns by model, key, lane, and transport, and the billing entitlement |
| `GET /v1/workspaces/{id}/usage/activity` | The filtered call list, newest first, with a keyset cursor |
| `GET /v1/workspaces/{id}/usage/timeseries` | The same receipts folded into UTC day buckets — calls, outcomes, units, and retail spend per day |
| `GET /v1/workspaces/{id}/usage/receipts/{digest}` | One receipt by digest, joined to its hold and reservation |
| `GET /v1/workspaces/{id}/usage/export` | The filtered receipts as `application/x-ndjson`, verbatim sealed lines |

### Filters

| Parameter | Matches |
| --- | --- |
| `from`, `to` | RFC 3339 bounds on `resolved_at`; a day prefix like `2026-09-22` works. A receipt with no timestamp fails a bounded window |
| `key` | The credential id (`oak_` token's middle segment), never the secret |
| `model` | The served or requested model id |
| `outcome` | `answered`, `refused`, `unavailable`, `unattempted`, `unknown` |
| `lane` | `decision`, `generation`, `executor` |
| `transport` | `http`, `relay`, and so on |
| `job` | The job the attempt ran under |
| `policy` | The admitted policy digest |
| `capacity` | The money lane's capacity, joined through the hold's price — a field the receipt does not carry |
| `limit` | Page size, bounded at 200 for activity and 10000 for export |
| `cursor` | The `cursor` value an activity page returned |

### Pagination

Activity pages newest-first over a `(resolved_at, digest)` keyset
cursor. A page's position names a record, not an offset, so appends
between reads shift nothing a later page reports. A cursor that
decodes but names a record the window no longer holds returns an empty
page with a `null` cursor; a malformed cursor refuses `invalid_cursor`.

### The cost contract

Cost fields are exact ledger reads in fixed-point millionths of the
account currency:

- `cost.retail` sums `retail_charge` over priced calls.
- `cost.provider_cost` and `cost.hosting_cost` are `null` until a
  settlement reports them — an unknown cost is `null`, not zero.
- `outstanding` lists the holds still `held` or `unknown` — spend
  authorized but not settled. Delayed settlement is reported as
  outstanding, never estimated.
- `units` sums the quota ledger's reserved questions and input bytes
  per receipt; `unmeasured` counts receipts with no reservation.

The disclosure block on every answer states the scale, the UTC
timezone and day boundary, the lag model (a receipt writes at
resolution; settlement may arrive later), the retention posture
(append-only logs), and the scan's skipped and truncated counts.

## The dashboard

`/dashboard` serves the same data as HTML pages. A member signs in by
pasting a `sess_` session token; the dashboard validates it through
the same principal path the API uses and stores it in an `HttpOnly`,
`SameSite=Lax` cookie named `oa_session`. Every workspace page re-runs
the membership check — a revoked member's next load refuses.

| Page | Shows |
| --- | --- |
| `GET /dashboard` | The workspace picker, or the sign-in form without a session |
| `GET /dashboard/w/{id}` | Overview: balance, outstanding reservations, today's calls, subscription state |
| `GET /dashboard/w/{id}/usage` | Summary numbers and a per-day call chart with the same filters |
| `GET /dashboard/w/{id}/activity` | The receipt table, newest first, linked to receipt pages |
| `GET /dashboard/w/{id}/receipts/{digest}` | One receipt's fields, cost, and quota units |
| `GET /dashboard/w/{id}/members` | The roster and live invitations, with the caller's row marked |
| `GET /dashboard/w/{id}/keys` | The workspace's credentials — admins see all, members see their own |
| `GET /dashboard/w/{id}/billing` | The subscription and invoices the billing book holds |

`POST /dashboard/session` validates a pasted token and sets the
cookie; `POST /dashboard/sign-out` expires it. Management operations —
member changes, key rotation, checkouts — stay on the JSON API; the
dashboard reads.

Pages render references only: key ids, request ids, digests, amounts.
Raw request state, answer payloads, and key secrets never appear —
the receipt's digests stand in for content, and the pages HTML-escape
every interpolated value.

## Receipts and the workspace field

`ExecutionReceipt.workspace` records the workspace the call was
admitted under. Receipts written before the field existed carry none;
the usage reads exclude them and count them as `unattributed` — no
reader upgrades a missing workspace into a guessed one. Key rotation
changes no attribution: the workspace is on the receipt, not inferred
from the credential.

## Operator notes

- A usage read scans `receipts.jsonl` once per request, bounded at
  100,000 lines; `scan_truncated` in the disclosure reports when the
  bound cut the window. Operators with longer logs should export and
  truncate the file per their own retention policy — the surface
  honors what the log holds.
- Reads take the quota lock and the money lock for the join. They do
  not block settlement: both are short reads of in-memory state.
- Session cookies carry the session token itself. Revoking the session
  (`DELETE /v1/session`) invalidates the cookie; signing out clears it
  without closing the session.
