# Live native engine offers v1alpha1

`DescribeTaskOffer` reads an enrolled computer account's actual SDK model list
before a routed task pins its immutable Binding. It uses the existing signed
HOST observation right and explicitly enabled environment grant. It sends no
task prompt, runs no tool, creates no managed task and changes no selected
provider profile. Existing native Agent/SDK owners perform initialization and
account authentication; a curated or static catalog cannot produce a ready
offer. The account must be authenticated and a complete native catalog observed.

The [schema](../../proto/openagents/environment/execution/v1alpha1/offer.proto)
is separate from published environment and Task contracts. All JSON fields are
required and objects are closed. Numbers are exact integers through 2^53−1.

| Request field | Meaning |
| --- | --- |
| `schema` | `openagents.environment.execution.offer-request.v1alpha1` |
| `method` | `DescribeTaskOffer` |
| `probe_id` | 32 lowercase hexadecimal bytes; one observational attempt |
| `engine_id` | `claude` or `codex` |
| `account_id` | `system` or the exact 32-byte lowercase hexadecimal Claude profile; Codex currently accepts `system` |

| Offer field | Meaning |
| --- | --- |
| `schema` | `openagents.environment.execution.offer.v1alpha1` |
| `host_pubkey` | Actual 64-byte lowercase hexadecimal host key |
| `workspace_id` | `default`, the explicitly enrolled workspace |
| `probe_id`, `engine_id`, `account_id` | Exact originating request identities |
| `state` | `warming`, `ready` or `failed` |
| `error_code` | Empty for warming/ready; typed native refusal for failed |
| `observed_at_ms` | Time of this recorded state; positive, no later than expiry |
| `expires_at_ms` | Fixed 30-second attempt lifetime, not extended by polling |
| `models` | Empty until ready; 1–32 exact native models in the SDK's catalog order |

Each model has `model_id` (1–128 opaque UTF-8 bytes without C0 controls),
`display_name` (1–128 UTF-8 bytes without C0 controls) and `effort_ids` (0–8
distinct supported IDs, 1–16 letters/digits/dot/hyphen each). Empty efforts
mean the model accepts no effort field. Duplicate models or a catalog that
cannot be represented within these bounds refuse explicitly; nothing is
silently clipped or filled from static choices. Catalog order is retained;
this contract makes no separate claim that its first entry is the provider's
default or strongest model. Native IDs such as a long-context variant retain
their original bytes. The published Binding's narrower identifier profile still
governs what that execution version can select; listing a model does not widen it.

Native warm-ups share the resident's existing four SDK slots. Completed
observations release their SDK owner and slot. At most 16 attempt records are
retained globally; unexpired records are not evicted. Every request rechecks
current enrollment, and revocation closes an active warm-up. A matching poll
observes the same attempt; changed engine/account reuse conflicts. Expired
attempts are stale and require a fresh probe ID. Restart can lose an observation;
repeating a warm-up has no task effect.

A ready offer is a short-lived observation, never execution authority. The
task owner still authenticates the selected account and rechecks its live
catalog, scope, current report and model/effort before sending an admitted task.
Server dispatch must record the resolved exact model and effort in both final
Binding and route. It cannot turn an unknown model into a guessed current ID.

## Account API and CLI

`GET /api/chat/task/models/v1alpha1?host=HOST&probe=PROBE&engine=ENGINE&provider_account=ACCOUNT`
returns the primary EngineOffer. POST is refused. The account bridge requires a
verified person and an exclusive connection for this host/grant pair. It checks
the native reply's complete request identity and live expiry. No conversation
or Project membership grants this access; this is observation of an explicitly
enrolled computer before a task exists. Responses use private no-store caching.

`openagents tasks models HOST [--engine claude|codex] [--provider-account ID]`
waits for warming to settle and prints the primary ready/failed observation.
It uses the normal account profile and NIP-98 transport; it holds no broker
credential. `--operation-id PROBE` lets a script reuse an exact probe. Engine and
provider flags are explicit script controls, not new mobile pickers.

The strict Codex SDK profile follows native `nextCursor` values through the
shared client and model-field reader. It retains an unpublished candidate until
the final page, preserving native order, exact labels and effort IDs. It allows
at most 32 pages, 1,024 decoded UTF-8 bytes per opaque cursor and 16 KiB of owned
model strings, within the existing 32-model offer bound. Hidden or empty pages
may continue; a final catalog must contain a visible model. Repeated decoded
cursors, malformed replies, duplicate models, unknown efforts, failed RPCs and
exhausted bounds refuse the whole observation. No prefix becomes ready. The
resident resets the complete SDK owner between probes and tasks; an earlier
catalog cannot authorize a later owner.
Native task settings send the exact offered effort, including `minimal`.
Native Claude offers omit the engine's internal effort aliases and retain
opaque model IDs, including long-context variants, without guessing a snapshot.

Status: source implementation for #13620. This contract alone does not
qualify account dispatch, mobile execution or a provider task on staging.
The [catalog qualification](../verification/2026-11-15-native-codex-catalogs.md)
records bounded policy, engine and signed resident/SDK fixture observations.
