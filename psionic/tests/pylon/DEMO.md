# Stage buyer

This is operator/test tooling for the Oct 30 demo, based on `buyer.ts`.
Install its pinned dependencies with Bun, then list the public relay:

```sh
bun install --cwd psionic/tests/pylon --frozen-lockfile
bun psionic/tests/pylon/demo-buyer.ts --list
```

Send one encrypted toy decision to a selected Pylon, or one to every
available toy Pylon in the discovery snapshot:

```sh
bun psionic/tests/pylon/demo-buyer.ts --slug PYLON_SLUG --json
bun psionic/tests/pylon/demo-buyer.ts --all --json
```

`--slug` is repeatable. If a slug is advertised by multiple public keys,
each matching key is a distinct Pylon; output includes its npub and exact
model identity. `--relay URL` defaults to `wss://relay.openagents.com`.
`--discover 4` bounds discovery to four seconds; `--timeout 45` bounds each
answer wait. The relay must acknowledge AUTH and job publication within
15 seconds. `--json` puts machine-readable results on stdout and progress
on stderr. A refusal, stale target, missing target or timeout exits nonzero.
No target flags means list-only.

Every invocation creates a fresh random buyer key in memory. There is no
key argument or persistent buyer identity, no payments and no production
key path. Dependencies independently sign/verify/encrypt the test traffic;
this tool does not replace the Coax engine or its protocol implementation.

The reader verifies event signatures, content hashes, freshness, v1 beacon
fields and matching free decision services. A newer valid announcement
that drops the selected model supersedes its older announcement. The demo
profile supports unowned v1 beacons, `cj-decision`, `free-v1`, and identities
of the form `MODEL@sha256:DIGEST`; owner-auth tags and v2 model groups are
unsupported and excluded. The default model is `psionic-toy`; an explicit
`--model` never silently substitutes another service. The toy question is
not a model-quality test: either valid choice may be returned.

Discovery is a bounded relay snapshot (256 beacons), not a durable fleet
registry. It folds generations within that invocation, checks expiration
again before dispatch, and reports if its relay history limit was reached.
It does not reconnect or retry jobs. Answers are ephemeral: subscribe
before publishing and keep the buyer connected. Accepted answers must
match the provider signature, buyer tag, exact job event, request/attempt,
model identity, and valid choice probabilities. Output reports the elapsed
wall time from dispatch, including relay acknowledgement.

Focused tests use a loopback independent protocol peer, not a fake success
result in the command:

```sh
bun run --cwd psionic/tests/pylon test:demo
```

A real Coax Pylon exchange through the public relay is captured in
`docs/verification/data/oct30-demo-buyer/public-a.json`, alongside the
exact buyer source used for that exchange. The Oct30MVP results log records
its scope; this exchange alone is not the full two-machine rehearsal.
