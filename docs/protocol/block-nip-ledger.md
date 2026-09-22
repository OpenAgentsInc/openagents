# Block NIP ledger

`nostr::block_lane` checks the 15 specification files pinned at Buzz commit
`8342dfcc5890b81a269a8ec3db73a8a56f76ce79`. Each check calls a shipped
validator or client and the test requires the pinned file to contain the
anchor for that behavior.

| Spec | What the check calls |
| --- | --- |
| NIP-AA | `verify_agent_auth_attestation` on a signed kind 22242 auth tag |
| NIP-AE | `validate_block_ingest` for kind 30174 |
| NIP-AM | `agent_turn_metric_owner` for kind 44200 |
| NIP-AO | `agent_observer_route` for a telemetry frame |
| NIP-AP | `validate_block_ingest` for kinds 30175 and 30178 |
| NIP-CW | `render_window` and `accept_window` for one kind 39006 page |
| NIP-DV | `dm_visibility_channel` for kind 41010 |
| NIP-ER | `validate_block_ingest` for kind 30300 |
| NIP-GS | `sign_git_object` and `verify_git_object` |
| NIP-IA | `parse_identity_archive_request` for kind 9035 |
| NIP-MP | `validate_block_ingest` for kind 30621 |
| NIP-OA | `verify_owner_attestation` on a signed auth tag |
| NIP-PL | `accept_lease`, then the fixed APNs reconnect body |
| NIP-RS | addressable class of kind 30078 |
| NIP-WP | `workspace_icon` for kind 9033 |

NIP-CW pagination on `POST /query` and NIP-PL delivery are relay behavior.
They are advertised only when the process is configured for them, as
`docs/protocol/block-nips.md` describes.
