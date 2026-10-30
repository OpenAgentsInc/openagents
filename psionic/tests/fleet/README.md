# Fleet black-box checks

The harness sends real HTTP calls to the compiled Chapter20 fleet. One
path uses the actual Coax relay, native Pylon and toy engine. An independent
Bun relay peer signs and encrypts adversarial events with Noble and
nostr-effect; none of that implementation ships in the product. All network
listeners bind loopback. Test identities are fresh or public throwaway
fixtures. No public relay, payment or production identity is involved.

```sh
bun install --cwd psionic/tests/pylon --frozen-lockfile
COAX="$PWD/coax/bin/coax" bun psionic/tests/fleet/fleet_e2e.ts
```

By default this builds a sanitized fleet, relay and Pylon plus the release
toy engine. Each build is bounded to 14 minutes. `OUT_DIR` selects a fresh
artifact directory; its logs and `results.json` explain failures. Never
reuse a directory containing an old relay store or stop file. The temporary
label stays in a private file, never argv or the result record.

Qualified binaries can be reused explicitly instead of rebuilding unchanged
dependencies:

```sh
FLEET=/path/psionic-fleet RELAY_BIN=/path/relay PYLON=/path/pylon \
SERVE=/path/psionic-serve OUT_DIR=/tmp/fleet-proof \
bun psionic/tests/fleet/fleet_e2e.ts
```

`FLEET_SCOPE=native` runs only the actual Coax relay/Pylon/engine cases;
`FLEET_SCOPE=peers` runs only the independent adversarial peer cases. This
supports focused reruns while fixing one side. `FLEET_SOURCE_COMMIT` records
the qualified runtime revision. `FLEET_FROM` starts at an exact check name;
`FLEET_ONLY` selects one exact check (normally with `FLEET_SCOPE=peers`);
`FLEET_MATCH` selects names containing a substring. For example,
`FLEET_SCOPE=peers FLEET_MATCH=numeric-` runs only the numeric boundary
regressions. These cover confidence in `[0,1]`, score in the question's
`[0, level_count-1]`, inclusive exact bounds, and valid exact `0.000001`
serialization tolerance. Negative nanounits are sent in decimal notation,
not exponent notation, so the wire tests exercise the intended boundary.
The current chapter 16 profile admits 2..10 score levels, so valid scores
cannot exceed 9; the numeric decoder's one-digit integer profile covers that
admitted range.
The peer harness waits for AUTH and both subscriptions before sending HTTP;
the listener being open does not imply relay readiness. Results also record the native binary
SHA-256 identities and scan all native logs for ASan/UBSan failures.

The cases cover native choice/noul/score remote success and late-answer deadline refusal;
real same-identity local fallback; unavailable/stale/draining/full/model
and generation supersession; negative publication/subscription replies;
an unanswered job; provider failover and a late answer from a prior attempt;
answer-before-ACK ordering with both positive and negative acknowledgements;
forged signatures and wrong provider,
job tag, recipient, request, attempt, model, identity, door, question or
distribution; wrong-identity local refusal; malformed/ambiguous local HTTP
status and body framing; pre-dispatch model admission;
and a slow local endpoint bounded by the total deadline. This is executable
interoperability evidence, not a signer side-channel audit or a production
approval. No source proof or mutation claim is inferred from these tests.
