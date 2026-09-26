# Block NIP fixture ledger

The [manifest](../../nips/manifest.json) now pins **17 Block specifications**
at Buzz commit `781d39510cf23cfe224e8f521ae06a23377e06de`. The
[2026-09-26 sync assessment](2026-09-26-upstream-nip-sync.md) reviews the four
changed specifications and two additions against current code. Retained
source, tested behavior, and complete implementation are separate claims.

## Source inventory and behavior evidence

[`nostr::block_lane`](../../crates/nostr/src/block_lane.rs) now declares the
current `BLOCK_COMMIT` and all 17 source files. Its
`source_inventory_matches_the_current_manifest_independently_of_behavior`
test checks the manifest, exact file set, and source anchors.
`BASELINE_FIXTURE_COMMIT` separately retains
`8342dfcc5890b81a269a8ec3db73a8a56f76ce79`, and
`baseline_subset_fixtures_still_hold` reruns those earlier scoped checks.
Updating inventory does not turn FI, PMA, or expanded AP/CW/PL/RS roles into
complete implementations.

The pure `nostr` suite passes 290 tests. Strict all-target Clippy and the live
PostgreSQL acceptance suite cover the new relay behavior. RS acceptance uses
actual HTTP and a fresh writer database, including restart and retained
expiration. The [verification record](verification/2026-09-26-nips/README.md)
names the tested code and distinguishes pure, integration, and omitted checks.

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
| NIP-AP | `validate_block_ingest` for kinds `30175` and `30178`. | Retained envelope fixture; current client projection/adoption helpers have separate tests below. |
| NIP-CW | `render_window` and `accept_window` for one kind `39006` page. | Retained channel-page fixture; thread helpers are separate from missing thread server/recovery roles. |
| NIP-DV | `dm_visibility_channel` for kind `41010`. | Command parsing. |
| NIP-ER | `validate_block_ingest` for kind `30300`. | Reminder envelope validation. |
| NIP-GS | `sign_git_object` and `verify_git_object`. | Git-object client primitives. |
| NIP-IA | `parse_identity_archive_request` for kind `9035`. | Archival command parsing. |
| NIP-MP | `validate_block_ingest` for kind `30621`. | Project envelope validation. |
| NIP-OA | `verify_owner_attestation` on a signed auth tag. | Attestation verification. |
| NIP-PL | `accept_lease`, then the fixed APNs reconnect body. | Lease parser and body constant; no full executor/public gateway lifecycle. |
| NIP-RS | Addressable class of kind `30078`. | Original storage fixture only; the new atomic snapshot has separate evidence below. |
| NIP-WP | `workspace_icon` for kind `9033`. | Workspace-icon command parsing. |

## Current-source additions and refusals

| Spec | Owning implementation and evidence | Boundary |
| --- | --- | --- |
| NIP-AP | [`agent_persona`](../../crates/nostr/src/agent_persona.rs): typed known content, portable catalog projection, authenticated foreign adoption, own custom-command preservation, restart decision | No ACP launcher, executable discovery, or catalog UI |
| NIP-CW | [`thread_window`](../../crates/nostr/src/thread_window.rs): strict batches, exact request binding, signed `39007` bounds, budget accounting | No thread-mode database service, complete reconstruction, or deletion recovery; no `nip-cw` advertisement |
| NIP-RS | [`read_state_snapshot`](../../crates/nostr/src/read_state_snapshot.rs): exact raw filters/envelopes, identities, signatures, digest, hard bounds | A client verifier authenticates a supplied cut; server completeness depends on the writer adapter |
| NIP-RS | [`read_state_snapshot_postgres`](../../crates/nostr-relay/tests/read_state_snapshot_postgres.rs): actual HTTP, Host discovery, NIP-98, writer cut, reader isolation, replay/restart, malformed requests, replacement/deletion, corruption, counts/bytes, membership | **Passed** on a fresh disposable database; no RS application merge client or ordinary-EOSE full-state barrier |
| NIP-FI | [`federated_identity`](../../crates/nostr/src/federated_identity.rs): token/policy/session/deny-set tests with an injected verifier | No JWT crypto implementation, JWKS client, live authentication/session enforcement, or FI advertisement |
| NIP-PMA | [`domain/block.rs`](../../crates/nostr/src/domain/block.rs), gateway write rejection and private read guards | Kind `30179` refused; no staged managed-agent runtime or PMA advertisement |
| NIP-PL | [`gateway/config.rs`](../../crates/nostr-relay/src/gateway/config.rs): push executor configuration refuses; NIP-11 omits PL | Existing parser/body fixtures remain; actual delivery is disabled |

Public admission also rejects relay-owned thread bounds kind `39007`.
Unknown thread HTTP modes refuse explicitly instead of falling back to ordinary
query semantics. The existing channel path handles missing/inaccessible groups
without bounds and refreshes access after its read; its bounded summary slice
still does not prove complete CW semantics.

RS requires `NOSTR_RELAY_READ_STATE_COMMUNITY` and the configured relay origin
for snapshot discovery. The relay base `nip-rs` declaration names the existing
storage role; the descriptor names the optional atomic mode. New snapshots
never use the ordinary history cap as a completeness limit.

PL still needs transactional lease/generation authority, current access checks,
durable authenticated dispatch, and public gateway enrollment/recovery before
it can be re-enabled. FI and PMA similarly need complete deployment roles
before advertisement. See [implementation status](block-nips.md) for the exact
runtime boundaries and [`scripts/test-postgres.sh`](../../scripts/test-postgres.sh)
for the isolated live test sequence.
