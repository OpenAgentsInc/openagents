# Revocation

**Status:** built. Proved in tests end to end, and the first three steps run
live against a door on this machine. The schema is
`crates/lev/src/policy.rs`, the published snapshot is
[`crates/lev/policy/`](../../crates/lev/policy/README.md), and the proof is
`crates/lev/tests/revocation.rs`.

## What was missing

`lev-serve` pins a release at startup. It reads the manifest, checks the
artifact's digest against the package on disk, checks the base model
signature against the device, and exits when either disagrees, because a
signature mismatch is a deployment error and the door should not start.

That is the load-time half, and it protects a door that restarts. It does
nothing for a door already running, and nothing at all for a door on a
machine you cannot reach.

The failure it leaves open is dated rather than hypothetical. The base model
ships with the operating system. An update replaces it, the signature moves,
and **every adapter and every calibration map fitted against the old
signature becomes invalid at once**. That is a revocation event, and until
now there was no mechanism to act on one:
[`manifest.md`](manifest.md) could name the field a revocation would use and
could not do anything about it.

## The mechanism

A *policy snapshot* is the document a canonical service publishes:

```json
{
  "schema": "openagents.lev.policy_snapshot.v1",
  "issued": "Sat, 19 Sep 2026 23:16:22 GMT",
  "freshnessWindowSeconds": 86400,
  "revoked": []
}
```

Every release names one, in the manifest's `policySnapshot` block:

```json
"policySnapshot": {
  "source": "../policy/current.json",
  "cache": "~/.lev/policy/current.json",
  "freshnessWindowSeconds": 86400
}
```

A door started from a manifest fetches the snapshot, caches it, and **reads
the cache on every question**. Reading it once at startup would put this
check back where the pinning already is.

Three rules decide what a door serves.

**A revocation stops what it names.** An entry is aimed at a release
(`lev-adapted@1`), at a base model signature, or at both, and it stops either
the whole release or the families it lists. A caller gets `revoked` with the
reason the publisher wrote, and HTTP 410, because a revoked release does not
come back.

**A stale snapshot stops everything.** A door whose cached snapshot is older
than the window in force stops serving its managed release outright, whether
or not it has heard of any revocation. The caller gets `policy_stale` and
HTTP 503, because a door that fetches a current snapshot serves again.

**An absent snapshot is a stale one.** A cache that is missing, unreadable,
or not a snapshot refuses exactly as an expired one does.

## The guarantee

> **The maximum enforcement delay is the freshness window.** With the window
> this repository publishes, that is 24 hours.

The reasoning is one line. A door holds a snapshot issued at `T`. Any
revocation is published at some `P >= T`, since the service cannot revoke
something before it issued the snapshot the door is holding. The door stops
at `T + window`, which is at or before `P + window`. So every door either
sees the revocation or stops, and no door can be running a revoked release
more than one window after it was revoked.

This is what makes the design a revocation mechanism rather than a check. A
check protects the doors you can reach. The window covers the ones you
cannot, including a laptop that is asleep, offline, or on somebody else's
network.

Two things the window is not. It is not a heartbeat: nothing needs to reach
the door, and the service being down does not extend it. And it is not a
retry policy: a failed fetch leaves the cached snapshot alone, so a door that
cannot refresh goes stale exactly on schedule.

The window in force is the smaller of the snapshot's and the release's, so a
snapshot cannot make a release immortal by claiming a century.

## The complement

**Deleting the cache must not turn a managed artifact into an unmanaged local
one.** It does not, because an absent snapshot is an expired one. The
manifest is what makes a release managed — it is the document that grants a
family — and nothing a client deletes is the manifest. Three ways of trying
to get out from under the policy all end in the same refusal:

- Delete the cache. The door refuses `policy_stale` and says so in the
  message.
- Overwrite the cache with something else. A document that is not a snapshot
  cannot be dated, so its freshness cannot be decided, so the door refuses.
- Leave `policySnapshot` out of the manifest. `Manifest::load` refuses the
  document, naming the blank field.

The one way out is not a hole in the mechanism but its boundary: a door
started with `--adapter` and no manifest is not a released model. It has no
`evalRef`, so it admits nothing, serves no probability, and refuses
`require_calibration`. There is nothing for a revocation to take away.

## Running it

Publish a snapshot to the canonical service:

```text
cargo run -p lev --bin lev-policy -- publish --window 24h \
    > crates/lev/policy/current.json
```

Revoke a release, or every release fitted against a base:

```text
cargo run -p lev --bin lev-policy -- publish --window 24h \
    --revoke 'lev-adapted@1' --families routing \
    --reason 'the routing map was refitted and lost its gate' \
    > crates/lev/policy/current.json

cargo run -p lev --bin lev-policy -- publish --window 24h \
    --revoke-base 9799725 \
    --reason '25E246 replaced the base model' \
    > crates/lev/policy/current.json
```

Fetch it into a door's cache, and ask where a release stands:

```text
cargo run -p lev --bin lev-policy -- fetch \
    --manifest crates/lev/manifests/lev-base-v1.json
cargo run -p lev --bin lev-policy -- show \
    --manifest crates/lev/manifests/lev-base-v1.json
```

`show` exits 1 when the release does not serve, so a scheduled check can act
on it. `lev-adapter-check <manifest.json>` reports the same standing beside
the rest of the release's claims.

A door refetches on its own every 15 minutes, or on whatever
`--policy-refresh` says; `off` leaves fetching to something else, and the
window still holds. A stale or revoked policy does not stop the door from
starting: it starts, says so on stderr, and refuses every question, because
refusing while running is the case a startup check cannot cover.

`GET /v1/models` publishes the standing, the window, how long the cached
snapshot has left, its digest, every revocation in force, and `serving` —
the families the door will answer for right now, which is the calibrated set
less whatever the policy has taken away.

## What was proved, and how

`cargo test -p lev --features serve --test revocation` runs the four steps
of issue #9390 against a real router bound to a real port and asked over
HTTP:

1. A released door under a current snapshot serves, and its card names the
   snapshot it is serving under.
2. A revocation is published.
3. The **running** door stops serving that family — no restart, and no
   change to any file the door owns. A family the revocation did not name
   still answers, which is what family scope means.
4. The service is deleted, so the door never reaches it again. The door
   serves to the last second inside the window and refuses one second past
   it, for every family and for a request naming none.

Plus the complement, plus the treadmill aimed the way it arrives: one entry
naming a base signature stops the whole release, without anybody listing the
releases fitted against it.

Every door in that suite runs over an empty helper pool, so **nothing here
touches Apple's runtime and nothing here measures it**. That keeps the suite
runnable on any machine, and it proves the stronger property: the policy is
asked before the runtime is, so a revoked release refuses for the reason it
was revoked rather than for whatever the device happened to say. A question
the policy allows through reaches the empty pool and comes back
`model_unavailable`, and that refusal is how the tests read "the policy let
this one past".

The freshness window is tested against a clock the door is given rather than
the machine's, because the guarantee is about a day passing and a test that
waited a day would not be run.

Steps 1 to 3 were also run live on 2026-09-19, against a real `lev-serve`
door started from `lev-base-v1.json` on this machine with `--helpers 1` and
`--policy-refresh 5s`. Under a current snapshot the card read
`"state": "current"` and `"serving": ["routing"]`. A base revocation was then
published to `crates/lev/policy/current.json` and nothing else was touched.
The door refetched on its own, logged `policy current -> revoked`, and the
same process answered:

```json
{"error": {"code": "revoked", "message": "lev-base@1 is revoked as of ..."}}
```

with HTTP 410, `"serving": []`, and `"calibrated_families": ["routing"]`
unchanged — the map is still fitted and still matches the door, and it is the
policy that stopped it. No question reached the runtime, so no number here is
a measurement of anything.

Step 4 is the one that cannot be run live in a session: it is a day of wall
clock. That is why the door reads a clock it can be given, and why the
assertion for the boundary is in the test suite rather than in a runbook.

## What is deliberately absent

**Signatures and key material.** The chain is transport to a canonical
service plus a digest checked before anything is written where a door can
load it. Nothing is written into the cache until the bytes digest to what was
asked for, parse, and check out, so a cache is never briefly authoritative
and wrong. The reference design this follows has no key material either, and
for one owner and one machine that is the right amount of trust machinery.
Adding a signing key here would be building the second-hardest part of a
system whose easiest part did not exist a week ago.

**A catalog.** No listing, no rollback pointer, no scopes, no namespace. One
release is revoked by naming it. See
[`../decision-models/research/2026-09-19-capability-sockets.md`](../decision-models/research/2026-09-19-capability-sockets.md)
for the review that reached that conclusion.

**Transport.** `source` is a path today, and a canonical service reached over
TLS would be the same fetch into the same `store` with the same digest check.
The client half is what revocation needs and the client half is complete: the
window is enforced by the door, on the door's own clock, whatever the
transport is or is not doing.

## Related

- [`manifest.md`](manifest.md) — the document a release is, which now names
  the policy that governs it.
- [`calibration.md`](calibration.md) — the rule a family has to pass before
  a door serves a probability for it at all.
- [`../decision-models/research/2026-09-19-capability-sockets.md`](../decision-models/research/2026-09-19-capability-sockets.md)
  — where the bounded enforcement delay comes from, and the measured reasons
  not to carry the rest of that toolchain.
