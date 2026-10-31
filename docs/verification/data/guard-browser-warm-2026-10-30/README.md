# Existing guard consumer after the first warm fleet rollout

These two measurements used the first warm Elephant revision,
`oa-mesh-elephant-m0warm-20261031a`, after the
[documented rollout](../../fleet-warm-rollout-2026-10-30.md). They precede the
subsequent Nix-compliant packaging replacement and do not qualify that later
revision. Capture timestamps are 2026-10-31 UTC, still October 30 locally.

The browser stayed on `https://openagents.com`. Its page, Wasm, host and assets
came from the current local checkout through the existing test's resource
interception; normal login and decision requests went to the untouched public
endpoints. Other game APIs were suppressed. This measures the actual existing
shadow consumer, not the isolated Directions candidate and not a deployed new
production page.

## Results

| Scenario | Observed result |
| --- | --- |
| [Admitted](admitted.json) | HTTP 200; network 671 ms, Coax callback 676 ms, frame queue 86 ms, total shadow admission 762 ms. One start, reply, terminal and admission; selected quiet. |
| [Pending blur](pending-blur.json) | Browser fetch cancelled at 150 ms. One revoked terminal (reason 7), no callback or admission through the subsequent 7.5-second observation. |

Both sessions used the task's same owned demo identity under normal quotas.
The session cookie was present, Secure and HttpOnly; the test also checked that
page JavaScript could not read it. Decision requests sent no Authorization
header. Both sessions logged out, and their muted headless browser profiles
closed. The retained key was left for the coordinating agent's final rollout
qualification. No identity rotation, quota exemption or further model call was
performed by this measurement task.

Cancellation proves local fetch/ticket revocation and no local application in
the observation window. It does not prove upstream Pylon/GPU cancellation.
These are one admitted and one revoked sample, not a production latency
distribution or availability SLO. Faster quiet does not establish added model
utility over local quiet.

## Exact input and artifact scope

The [earlier admitted pilot](../guard-browser-2026-10-30/admitted.json) and this
admitted pilot have byte-identical request bodies. The offline Coax
[extractor](extract-request.coax) decoded each record's request into
[before](request-before.json) and [after](request-after.json). `cmp` passed;
both [SHA-256 values](request-pair.sha256) are
`0d7d397b1b3dbfa46edd698665c10b86eead5aca9f85238cc90ddbfa5c6d4e17`.
That is the frozen v1 afternoon/rain context, not a rewritten benchmark prompt.

The response's quiet choice, confidence, complete probability distribution,
provider, service door and original checkpoint also match the earlier record.
Only its reported backend duration changed from 141 to 140 ms. The network
measurement changed from 5714 to 671 ms, and total shadow admission from 5724
to 762 ms. The [fleet rollout record](../../fleet-warm-rollout-2026-10-30.md)
contains the separately controlled service comparison.

The freshly rebuilt module has SHA-256
`0b31088f0677a0c137744f95cc10ba1879aa47e5fcae744d1342f878dc8db49b`, recorded
in [wasm.sha256](wasm.sha256). It matches the previously qualified final local
artifact. The earlier live admitted pilot used a different recorded module,
`4eb03e9a98afc8f6cf89e42e621813f53df4baa610bc78c260b64a9bd809baa7`;
[the M3 record](../../pylon-guard-shadow-2026-10-30.md) describes the intervening
metadata/unused-entry cleanup. The comparison therefore pins exact request
bytes and reports both module identities rather than claiming an identical
binary experiment.

[source-head.txt](source-head.txt) records the starting repository HEAD.
[source-files.sha256](source-files.sha256) pins the measured direct page,
guard, host, build and compiler components; those hashes were unchanged after
the build and both pilots. This is a declared component scope, not a complete
transitive source inventory. The compiler reports
[coax0 0.24.0](compiler-version.txt).

## Focused execution

Only the two named existing live cases ran: each passed, with seven unrelated
cases filtered out. [Admitted test output](admitted-test.txt) records eight
assertions; [pending-blur output](pending-blur-test.txt) records seven. No new
TypeScript, offline/full suite rerun, production page deploy or default-mode
change was introduced.

The fresh build used:

```shell
coax/web/wasm.sh apps/openagents-web/everglade.coax apps/openagents-web/everglade-face.c /tmp/guard-browser-warm-2026-10-30/everglade.wasm verse/player.coax verse/grid.coax verse/everglade/river.coax verse/everglade/guard-decision.coax
```

The opt-in cases require explicitly retained owned-key and signer paths. This
run reused the existing task identity; the commands do not create identities.

```shell
export EVERGLADE_GUARD_WASM=/tmp/guard-browser-warm-2026-10-30/everglade.wasm
export EVERGLADE_GUARD_LIVE=1
export EVERGLADE_GUARD_LIVE_KEY="$OWNED_DEMO_KEY"
export OA_MESH_SIGNER="$OWNED_SIGNER"
EVERGLADE_GUARD_LIVE_RECORD=/tmp/warm-admitted.json bun test coax/web/everglade-guard.test.ts --test-name-pattern 'owned session reaches a live Pylon from the actual guard page'
EVERGLADE_GUARD_LIVE_REVOKE=1 EVERGLADE_GUARD_LIVE_RECORD=/tmp/warm-pending-blur.json bun test coax/web/everglade-guard.test.ts --test-name-pattern 'owned session revokes a pending live Pylon request on blur'
```

Records retain synthetic game observations, model output, timing and boolean
session/header facts. Cookies, headers, auth events, signer output and raw CDP
messages are excluded.
