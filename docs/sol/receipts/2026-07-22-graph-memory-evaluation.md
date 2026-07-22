# Graph memory evaluation and owner lifecycle receipt

- Class: evidence
- Date: 2026-07-22
- Issue: [#9166](https://github.com/OpenAgentsInc/openagents/issues/9166)
- Source commit: `3f19dedca13db740d3e0675559daba7c45adade5`
- SDK train: `0.2.1-rc.2`
- Evaluation status: complete
- Quality result: inconclusive

## Final disposition

The evaluation implementation is complete. The evidence is partial because
the graph-assisted holdout has non-complete rows. The quality result is
inconclusive. Owner review is not complete. The feature is not released, and
a public quality or parity claim is not authorized.

Graph memory stays off by default. This receipt does not enable it. It does
not accept the quality result for the owner.

## Pinned comparison

The executable comparison uses the released Desktop history recall and graph
memory paths. It builds the history arm with `runDesktopRlmDeterministicGrep`.
It builds the graph arm with `runDesktopGraphMemoryTurn`, the production
parser, the production limits, and the encrypted SQLite adapter. A separate
smoke test runs `makeDesktopGraphMemoryWorkflow` and requires a cited advisory.

The development split runs before the runner loads the holdout split. The
receipt pins the source commit, clean source state, Node runtime, architecture,
lock digest, eight SDK package integrities, 63 Desktop build artifacts, parser,
runner, oracle, policy, budget, dataset, and quality-policy digests. Each arm
uses the same post-policy source bytes and the same production query projection.
The model state is `NotUsed`, and each arm has exact zero model calls and zero
provider tokens.

The public aggregate is
[`2026-07-22-evaluation.json`](../../../apps/openagents-desktop/benchmarks/graph-memory/2026-07-22-evaluation.json).
Its file SHA-256 digest is
`d8c1ad77677b6b10d4a0efa1f23dd237d7476f815262adb47709893256ef2178`.
The owner-local row detail had digest
`5c23759d41fd65e5352fe0bd806eea1794d02661b9b1c7af5fd743e94fbf76a3`.
The runner removed that detail after it verified the public binding.

## Result

The history arm completed all 7 holdout rows. The graph arm completed 2 rows,
returned 3 partial rows, and failed 2 rows. The failures were one stale-graph
recall and one same-name setup. Both arms had citation validity 1.0. Neither
arm emitted an answer assertion, so both answer-support values were 0. History
retrieval recall was 0.5714. Graph retrieval recall was 0.2857. Both
false-merge and missed-entity metrics are unsupported. The failed same-name
row and the partial extraction row have no inspected graph state. The scorer
does not treat that unavailable state as an empty graph.

The graph arm also recorded the observation-character cap, a partial extraction,
revoked-source exclusion, prompt-injection-as-data handling, one stale-graph
failure, one invalid-source recall
failure, and one store failure. These outcomes make the comparison
inconclusive under the policy that was pinned before the holdout run. A
required metric is also unsupported because graph-state evidence is absent.
The
receipt does not convert incomplete rows into zero values or passing results.

## Owner lifecycle

The owner lifecycle uses the real encrypted SQLite adapter. It starts with two
mentions, two entities, one relation, one vector, one summary, one ranking ref,
and one ranking snapshot. It closes and reopens the store three times.

The proof inspects graph, ranking, provenance, source membership, and pending
operation state. It refuses one incomplete source delete plan without a graph
change. It exports an archive and validates the exact SDK archive digests. It
then forgets the graph, vector, summary, ranking, and internal archive planes.

The caller-held archive stays present through forget. The proof confirms that
the repeated forget returns the same receipt, and then it removes the caller
archive. The final inspection has no current graph and has zero counts in each
plane.

The aggregate is
[`2026-07-22-owner-lifecycle.json`](../../../apps/openagents-desktop/benchmarks/graph-memory/2026-07-22-owner-lifecycle.json).
Its file SHA-256 digest is
`211617dc8a635b1c521433e931f8277b5e6cbef4adf2d94f45aa999f934a7145`.
The custody rung is `standalone_proof_process_wrapping_key`. It is not an
Electron `safeStorage` proof.

## Verification

The following checks passed on the source commit:

```sh
pnpm --dir apps/openagents-desktop run graph-memory:evaluate
pnpm --dir apps/openagents-desktop run graph-memory:owner-lifecycle
pnpm --dir apps/openagents-desktop run typecheck
pnpm --dir apps/openagents-desktop exec vp test --run --root ../.. \
  apps/openagents-desktop/src/desktop-graph-memory-evaluation.test.ts \
  apps/openagents-desktop/src/desktop-graph-memory-owner-lifecycle.test.ts \
  apps/openagents-desktop/src/graph-memory-sdk-conformance.test.ts
```

The consumer conformance test ran 22 SDK laws for graph identity, deletion,
extraction, RLM, ranking, and archive behavior. The focused evaluation and
lifecycle tests also passed. A later signed package, owner review, release,
and public claim need their own authority and evidence.
