# Block NIP fixture ledger

The [manifest](../../nips/manifest.json) now pins **17 Block specifications**
at Buzz commit `781d39510cf23cfe224e8f521ae06a23377e06de`. The
[2026-09-26 sync assessment](2026-09-26-upstream-nip-sync.md) reviews the four
changed specifications and two additions against current code. Retained
source, tested behavior, and complete implementation are separate claims.

## Known stale inventory guard

[`nostr::block_lane`](../../crates/nostr/src/block_lane.rs) still declares
`BLOCK_COMMIT = 8342dfcc5890b81a269a8ec3db73a8a56f76ce79` and a 15-file
`FILES` list. Its test compares that old commit and count with the current
manifest. The focused September 26 run **failed at that old-pin assertion**,
as recorded in the [verification result](2026-09-26-upstream-nip-sync.md#verification-and-limits).
No Rust constant or test was changed by the documentation update.

The eventual correction must distinguish the 17-file source inventory from
role-specific evidence. Merely replacing the pin and count cannot establish
FI, PMA, or the new AP/CW/PL/RS behaviors. Keep explicit unsupported and
reserved-kind cases as part of that evidence.

## Retained checks from the earlier 15-specification pin

Each existing check calls a shipped validator or client primitive and
requires the pinned file to contain an anchor for that behavior. An anchor
check establishes neither full specification coverage nor a passing run
against the new pin.

| Spec | Existing check | Evidence boundary |
| --- | --- | --- |
| NIP-AA | `verify_agent_auth_attestation` on a signed kind `22242` auth tag. | Owner-attested authentication primitive. |
| NIP-AE | `validate_block_ingest` for kind `30174`. | Engram envelope validation. |
| NIP-AM | `agent_turn_metric_owner` for kind `44200`. | Metric owner extraction/validation. |
| NIP-AO | `agent_observer_route` for a telemetry frame. | Ephemeral route validation. |
| NIP-AP | `validate_block_ingest` for kinds `30175` and `30178`. | Envelopes only; no new ACP projection/adoption behavior. |
| NIP-CW | `render_window` and `accept_window` for one kind `39006` page. | Channel page only; no thread `39007`, batch budgets, or deletion recovery. |
| NIP-DV | `dm_visibility_channel` for kind `41010`. | Command parsing. |
| NIP-ER | `validate_block_ingest` for kind `30300`. | Reminder envelope validation. |
| NIP-GS | `sign_git_object` and `verify_git_object`. | Git-object client primitives. |
| NIP-IA | `parse_identity_archive_request` for kind `9035`. | Archival command parsing. |
| NIP-MP | `validate_block_ingest` for kind `30621`. | Project envelope validation. |
| NIP-OA | `verify_owner_attestation` on a signed auth tag. | Attestation verification. |
| NIP-PL | `accept_lease`, then the fixed APNs reconnect body. | Lease parser and body constant; no full executor/public gateway lifecycle. |
| NIP-RS | Addressable class of kind `30078`. | Storage class only; no RS merge, complete-load barrier, or atomic snapshot. |
| NIP-WP | `workspace_icon` for kind `9033`. | Workspace-icon command parsing. |

## New targets without ledger coverage

| Spec | Required work | Current status |
| --- | --- | --- |
| NIP-FI | Issuer/community policy, JWT/JWKS and Nostr possession checks, session deadlines, protected HTTP admission, issuer disconnect. | No implementation, fixtures, or advertisement. |
| NIP-PMA | First reject reserved private kind `30179`; later implement the staged privacy, transactional update, backup, revocation, and migration contract. | No implementation or required rejection; generic admission/read paths remain unsafe for this private kind. |

The changed CW contract also requires client-publication rejection for
relay-signed kind `39007`; the existing relay-only guard covers `39005` and
`39006` but omits it. The optional RS atomic snapshot is absent. PL has a
configured local HTTP stub path, with preexisting transaction, read-authorization,
and durable-delivery gaps. Its fixture is not public APNs gateway evidence.

See [implementation status](block-nips.md) for existing Postgres fixtures,
current NIP-11 advertisement behavior, and the limits of those claims. This
sync and documentation update ran only the two source-ledger guards, both
of which failed; it ran no full Rust gate or live relay probe.
