# Domain Fixture Corpus

These fixtures test the pinned specifications under `nips/`. Each directory
names the NIP that owns the behavior. The corpus was extracted from the
public Immortal repository (CC0); the market, provider, laboratory, and
OpenAgents-lane fixtures are not part of this repository.

## Provenance

- `nip01/events.json` contains the signed `hello world` event and canonical
  ID vector from `scsibug/nostr-rs-relay`, commit
  `b5c1f642e4f4c3b9c54f5d18d66f4c53642076b4`, `src/event.rs`, MIT license.
  Its `tags: null` compatibility input was normalized to the NIP-01 wire form
  `tags: []`; the canonical bytes, ID, and signature are unchanged.
- `nip01/filters.json` adapts the filter-matching cases from the same commit's
  `src/subscription.rs`. The old prefix cases assert exact matching because
  the pinned NIP-01 no longer allows prefix matching.
- `nip01/replacement.json`, `nip09/deletion.json`, and
  `nip40/expiration.json` were written for Immortal directly from the pinned
  NIP-01, NIP-09, and NIP-40 text.
- `nip01/gateway_messages.json`, `nip11/document.json`, and
  `nip42/auth.json` were written for Immortal directly from the pinned NIP-01,
  NIP-11, and NIP-42 texts. They pin gateway message shape, relay information,
  limit metadata, and canonical authentication acceptance boundaries. The
  live contract separately checks NIP-11 CORS behavior.
- `nip17/routing.json`, `nip29/groups.json`, `nip45/count.json`,
  `nip50/search.json`, `nip65/relay-list.json`, `nip70/protected.json`,
  `nip86/management.json`, `nip94/metadata.json`, `nip98/http-auth.json`,
  and `nipb7/servers.json`
  were written for Immortal from the corresponding pinned official texts.
  They pin validation, routing, group action, COUNT, search, protected
  publishing, and HTTP-authentication boundaries. The live Postgres gateway
  contract checks the associated storage, access-control, signing, sweep,
  management, media metadata/server-list, and wire behavior.
- `nip50/search-equivalence.json` was written for OpenAgents from the pinned
  NIP-50 text and the relay search contract in
  `docs/protocol/nip-expansion.md`. It is the one oracle both
  `crates/nostr/tests/search_equivalence.rs` and the relay's
  `store_postgres` suite read, so the Rust matcher and the replay SQL are
  judged on the same pairs; `docs/nostr/crypto-primitives.md` describes it.
- `nip19/keys.json` carries the `npub` and `nsec` vectors from the pinned
  NIP-19 text plus refusal cases written for Immortal: mixed case, a wrong
  checksum, a prefix swap, and wrong lengths.
- `nipoa/attestation.json`, `nipaa/auth.json`, `nipao/observer.json`, and
  `nipam/turn-metrics.json` pin the Block agent ownership, agent-authentication,
  ephemeral observer, and private turn-metric envelopes against the Block
  commit in `nips/manifest.json`, `8342dfcc5890b81a269a8ec3db73a8a56f76ce79`.
- `nipae/`, `nipap/`, `niper/`, `nipmp/`, `nippl/`, `nipia/`, `nipdv/`,
  `nipwp/`, `nipcw/`, `niprs/`, and `nipgs/` each contain a committed server
  contract derived from the corresponding pinned Block text. They cover
  private-data ACLs, validators, relay commands and snapshots, channel-window
  degradation on WebSocket `REQ`, race-free standard relay semantics, the
  client-side NIP-GS git signature, and NIP-PL's refusal when no executor is
  configured. Configured `POST /query` and push delivery are covered by the
  Rust checks in `docs/protocol/block-nip-ledger.md`.
- `migration/relay-shadow-v1.json` and `migration/signed-event-import-v1.json`
  pin the shadow-relay comparison workload and the signed JSONL import lane.

Fixture data is committed rather than generated so a specification or
implementation change produces a reviewable diff.

Run the complete fixture layer manually with
`cargo test --locked --all-targets`.
GitHub workflows and GitHub-billed automation are prohibited.
