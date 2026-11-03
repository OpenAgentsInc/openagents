# Coax relay: advertised parity with relay.openagents.com

2026-11-03, issue #12631 (the parts that need no production access).
Source baseline `edff29c285`. Nothing here contacted relay.openagents.com or
read production data. The Rust relay is read from history only
(`git show 0b401561a0^:crates/nostr-relay/...`).

## What each relay advertises

**Rust relay** (`crates/nostr-relay/src/gateway/config.rs`,
`active_supported_nips`): always 1, 9, 11, 40, 45, 50, 65, 67, 77 and 94;
17, 42, 59 and 70 with a relay URL; 29 with a relay signer; 86 with a
management key; 98 with a management key or the media service. The
production document recorded in coax/nostr.md section 17 lists 1, 9, 11,
17, 29, 40, 42, 45, 50, 59, 65, 67, 70, 77, 94 and 98, so production runs
with a relay URL, a relay signer, a management key and media.

**Coax relay** (`coax/lib/nostr/relay_route.coax`, `RR_NIP11`): 1, 9, 11,
40, 42, 45, 50, 62, 70 and 77; 86 with `--admin` (`RR_NIP11_ADMIN`); 13
with `--pow` above 0 (`rr_nip11_pow`); 29 with `--groups` (`rr_nip11_nip`).
The relay advertises only what it wires: every listed NIP has a code path in
`coax/relay/relay.coax`.

## The table

"Wired" means a code path in `coax/relay/relay.coax` handles it. "Gap" is
behaviour the Rust relay has and the Coax relay doesn't, as a served-set or
protocol difference a shadow period would see.

| NIP | Rust | Coax advertises | Coax wiring | Gap | Decision |
| --- | --- | --- | --- | --- | --- |
| 1 | yes | yes | wired (`relay_server.coax`) | reason texts; per-kind structure checks (R1 owner question 1); multi-filter order and shared limit (R1 owner question 2) | open since 2026-10-30 (NEEDS_OWNER) |
| 9 | yes | yes | wired (`store_delete.coax` through `rd_admit`) | Rust never deletes kind 30350 by an `e` tag (`DELETE_EVENT_TARGET_SQL`); Rust keeps a tombstone after NIP-40 removes the deletion event (`0002_nip_expansion.sql`), Coax rebuilds tombstones only from stored requests; Coax holds at most `RD_TOMBS` = 8,192 tombstones | owner: carry orphan tombstones (below) |
| 11 | yes | yes | wired | extra limitation fields differ (R1 record) | none needed |
| 13 | no | with `--pow` | wired, off by default | Coax extra | none needed (off) |
| 17 | with relay URL | no | **not wired** | Rust serves kinds 1059, 21059 (and 24200, 30622, 44200) only to an authenticated reader who is the event's single `p` recipient; Coax serves them to anyone | **owner, blocks cutover** |
| 29 | with relay signer | with `--groups` | partly wired (`nip29.coax`, `RyWork.groups`) | no relay key, so no 39000 to 39003 group state; no private-group or hidden-group read gating; no group methods over NIP-86 | owner: relay key (NEEDS_OWNER Nostr follow-ups, question 4) |
| 40 | yes | yes | wired | none known | none needed |
| 42 | with relay URL | yes | wired (`nip42.coax`) | Coax requires AUTH for writes (R1 decision) | decided R1 |
| 43 | no | no | not wired | none: neither relay advertises it | owner: relay key (question 4); not a parity requirement |
| 45 | yes | yes | wired (`nip45.coax`) | not re-measured against Rust since R1 | differential refresh |
| 50 | yes | yes | wired (`nip50.coax`) | Rust excludes the private kinds below from search too | follows the private-kind decision |
| 59 | with relay URL | no | **not wired** | as NIP-17 | **owner, blocks cutover** |
| 62 | no | yes | wired | Coax extra. An imported kind 62 that names relay.openagents.com or `ALL_RELAYS` retires its author's earlier events, which the Rust relay still serves | owner: review each kind 62 the import applies |
| 65 | yes | no | stored as replaceable kind 10002 | no relay behaviour beyond storage | owner: advertise or not |
| 67 | yes | no | **not wired** | Rust ends stored results with `["EOSE",sub,["finish"]]` or `["more"]`; Coax sends `["EOSE",sub]` | owner: wire or drop |
| 70 | with relay URL | yes | wired (`nip70.coax`) | none known | none needed |
| 77 | yes | yes | wired (`nip77.coax`, `rr_neg_*`) | none known; step 9 of the runbook uses it against the Rust relay | none needed |
| 86 | with management key | with `--admin` | wired (`nip86.coax`, `FILE.nip86`) | method sets differ: Rust has `creategroup`, `deletegroup`, `listgroups`, `putgroupuser`, `removegroupuser`; Coax has `banevent`, `unbanevent`, `listbannedevents`, `listdisallowedkinds`, `changerelayname`. The Rust policy tables (`relay_blocked_pubkey`, `relay_blocked_kind`, `relay_allowed_*`, `relay_member_pubkey`) are not in the event export | owner: policy migration |
| 94 | yes (media service) | no | **not wired** | no media upload or NIP-94 metadata endpoint | owner: keep media elsewhere, or drop |
| 98 | with management or media | no | used inside NIP-86 only | no separate advertisement | owner: advertise with 86 or not |

### Private and hidden kinds (not a NIP, but the largest served-set gap)

The Rust relay's query (`crates/nostr-relay/src/store/statements.rs`, the
REQ select) never serves kinds 39604 to 39613, 39620, 39650 and 30179. It
serves these only to an authenticated reader who owns them, or who is their
single `p` recipient: 78, 30078, 30300 and 30350 (author); 1059, 21059,
24200, 30622 and 44200 (single `p` recipient); 30174 (author or a `p`);
30175 and 30178 (author, or tagged `["shared","true"]`); 3187, 3188 and
30186 (author or single `p` recipient); 30181 tagged
`oa:cap-policy:private:v1` (author). Search never matches any of them.
Events in private NIP-29 groups go only to members.

The Coax relay serves all of these to anyone who asks. During a shadow
period every REQ that touches them is an unexplained answer, and after a
cutover it would be a privacy regression. Wiring them is a policy port with
an `#authority` unit, but the list itself is OpenAgents product policy that
grew kind by kind, so whether to copy it whole is the owner's decision.

## NIP-77, 86, 43 and 29: what this run wired

Nothing new. NIP-77 and NIP-86 were already wired (chapter 103's table); NIP-29
is wired up to the point that needs a relay key; NIP-43 needs that key
before anything can be wired. The relay key is NEEDS_OWNER's open question 4
(Nostr book follow-ups), so no wiring here is free of an owner decision.
No advertisement changed.

## For the owner (NEEDS_OWNER.md)

1. Which of NIPs 17, 59, 65, 67, 94 and 98 must the Coax relay serve before
   cutover, and which are dropped from the advertisement. The private-kind
   read rules above block cutover until decided either way.
2. The relay key question (29's group state, 43) stands.
3. Carrying the Rust relay's NIP-86 policy tables and orphan tombstones
   (tombstones whose deletion event NIP-40 already removed): neither is in
   an event export, and neither can be re-verified, because there is no
   signature to check.
