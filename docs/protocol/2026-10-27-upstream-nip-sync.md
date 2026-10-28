# Upstream NIP sync, 2026-10-27

The official and external mirrors now match their upstream branch heads at sync
time. This is a source update; it does not implement a relay, change native
HOST/REACH admission, or introduce generated Nostr protobuf messages.

| Source | Previous pin | Current pin | Inventory |
| --- | --- | --- | --- |
| Official | `b82211e96c6dad616ed2ea43034c1c621256b745` | `400d157c53264b8e58c7b3cdbd21ef546fb7a157` | 100 specs plus upstream README |
| External | `781d39510cf23cfe224e8f521ae06a23377e06de` | `24a370606044c0c7be787cc2fd98ec0bfe1d606b` | 20 specs plus two JSON fixtures |

Exact source links are in `nips/manifest.json`. The pre-reset recovery record
is unchanged and describes the historical tree, not the current upstreams.
The 36 OpenAgents-authored NIPs and their shared contracts/schemas are unchanged.

## Official changes reviewed

- **01:** Empty filter arrays explicitly match nothing; they must not be
  treated as an omitted condition. Future filters need missing/empty vectors.
- **24, 92:** Profile picture/banner metadata may use matching `imeta` tags,
  with exact URL matching, fallback handling and retention on profile updates.
- **34:** Corrected subordinate-fork tag spelling in the example.
- **39:** Unified verification text, backwards acceptance of older proofs,
  and added Bluesky/Discord proof forms. Network proof validation remains
  separate from a claimed identity's parsed fields.
- **51:** Favorite follow sets use kind 10021; the old pin named 10011.
  Installed slash commands use kind 10992. A source sync must not silently
  reinterpret old stored records or signatures.
- **A3:** Heading/format corrections, with payment-target meaning retained.
- **CD (new):** Kind 31992 slash-command definitions, scoped plain-text
  invocations, typed arguments, optional client support and kind 10992 lists.
  Presence of a command definition is not permission to execute a tool.
- **README:** Index and kind registry follow these changes.

## External changes reviewed

- **AR (new):** Kind 45010 channel artifacts, stable UUID identity,
  complete-snapshot revisions, atomic `prev` checks, replay idempotence,
  channel-scoped access, atomic moves, delete/restore and current-state query
  rules. Generic replaceable-event storage does not implement this contract.
- **CL (new):** Channel type/label/creator identity, creation timestamps,
  deterministic channel selection, explicit system-channel discovery,
  membership notice filtering and named-only unlisted access. Labels grant
  no authority; filters apply before pagination and counts.
- **FA (new):** Enterprise identity adapter login/exchange/session/assertion
  boundaries. Includes handoff binding, exact session expiry verification,
  key proof, redirect refusal, bounded retries and owner-authorized agent grants.
- **AP:** Teams and agent/team configuration now compose AR artifacts and CL
  system channels, with explicit launch precedence, history, conflict handling,
  invalid/deleted/unavailable states and constrained offline startup. Agent
  type must match the settings; share projections remain deliberate snapshots.
- **AE:** Core-as-index and optional source/date metadata conventions. These
  are memory organization conventions, not verified attestations.
- **FI:** Distinct agent assertion type, verified owner binding, owner-aware
  deny/disconnect checks, and explicit public-HTTPS versus operator-protected
  mesh JWKS transport. No implicit transport or token-class fallback.
- **OA, GS:** Corrected `created_at<t` condition spelling; signature preimages
  still preserve exact original condition bytes/order. OA example reformatting
  is copied verbatim.
- **PL:** The fixed APNs body now includes `"sound":"default"`; delivery still
  depends on system/user notification settings.

The two MP JSON fixture files already existed upstream but were omitted by the
old Markdown-only sync. Both are now included unchanged and syntax checked.

## Verification and consumer boundary

`sync-nips.sh` reads complete Git trees, not a list of known NIP numbers.
`check-nips-pin.sh --latest` verifies every upstream byte and the live branch
heads. `--offline` checks hashes, inventory, spec counts and JSON syntax.
The checker rejects missing/extra/changed files; only the locally owned external
index is exempt. Tool regressions cover fixture/nested-file inclusion, byte
preservation, stale-file removal, local/upstream README precedence and symlink
refusal. Protocol compatibility tests remain a separate gate.

Before implementing these contracts in Coax, select the actual consumer,
preserve signed JSON and schema presence, model its authority and revision
rules, and add behavioral/negative fixtures. Neither vendoring these drafts nor
parsing the MP fixtures constitutes runtime conformance. No new supported-NIP
advertisements or authority migrations are made by this sync.
