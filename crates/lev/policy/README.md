# The policy snapshot

`current.json` is the published `policy_snapshot.v1` every release in
[`../manifests/`](../manifests/README.md) points at. It says when it was
issued, how long a door may keep serving on it, and what it stops.

A door fetches it, caches it outside this repository, and reads the cache on
every question. Nothing here is a door's cache: a cache is a client's copy of
somebody else's document, and a copy that is committed is a copy that stops
being fetched.

Read [`../../docs/lev/revocation.md`](../../docs/lev/revocation.md) first.
The mechanism is short, and the part that matters is not the revocation list
but the window beside it.

## The committed snapshot reads as stale, and that is the mechanism

`current.json` was issued once, with a 24-hour window. A day after it was
written it is stale, so a door fetching it refuses to serve its managed
release and says why. That is the guarantee working rather than a defect in
this file: a door that cannot confirm its policy does not go on serving.

Publish a fresh one before running a released door:

```text
cargo run -p lev --bin lev-policy -- publish --window 24h > crates/lev/policy/current.json
cargo run -p lev --bin lev-policy -- fetch --manifest crates/lev/manifests/lev-base-v1.json
cargo run -p lev --bin lev-policy -- show  --manifest crates/lev/manifests/lev-base-v1.json
```

## Revoking

A revocation is aimed at a release, at a base model signature, or at both,
and it stops either the whole release or the families it names. It carries
the day it takes effect and the reason a caller reads on the wire, and an
entry that names neither a release nor a signature is refused rather than
published, because it would stop every door that read it.

```text
cargo run -p lev --bin lev-policy -- publish --window 24h \
    --revoke-base 9799725 \
    --reason '25E246 replaced the base model, and every map fitted against it is invalid' \
    > crates/lev/policy/current.json
```

The base-signature aim is the one the operating system treadmill needs.
Nobody lists the releases fitted against the old base, because the update did
not either.
