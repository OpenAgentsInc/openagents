# Classified aggregate source audit

The aggregate checker can now analyze an explicit whole-parameter classification supplied by the caller and bound to the compiled source graph and resolved entry parameters. The default remains **all entry parameters secret**. A classification is an assumption about the actual call boundary, not proof that any input is public; malformed, duplicate, incomplete, reordered, mismatched, or stale declarations refuse. The report includes the source hashes, compiler and type-inventory identities, parameter labels and manifest digest. The direct graph entry is an internal analysis interface and does not authenticate arbitrary IR or caller claims.

In the classified profile, an unknown public condition analyzes both feasible arms. Possible and definite returns are tracked separately and their origins and authored guard obligations are joined; an arm cannot mutate a pre-existing binding or region, including a region reached through a view or callee. The checker permits branch-local mutations of fresh storage. A nonempty loop can have a possible early return only when a bounded syntactic scan finds its body read-only; this subset has no assignments or calls. Other loops retain the previous conservative backedge analysis. Unknown effects, unsupported joins, directly traversed loops inside unknown public arms, zero-trip loops, and secret-dependent branch conditions or indices still refuse. Existing visit, depth, region and cache limits remain in force. Earlier all-secret source entry points retain their original classification.

The independent source review found no new concrete false acceptance in the exact checked policy and bridge recorded in [the evidence](2026-10-29-classified-root-audit.json). It traced binding and region protection, aliases through calls, both-arm return and guard joins, read-only loop returns, and by-value memoization. A memoized call can avoid re-entering the blanket branch-loop refusal, but this review found no external mutation channel in the admitted by-value subset; view arguments and returns are not cached, and nested views are rejected. This is a scoped source review, not a target or production approval.

## Observed real root

For source `smc_multiply`, the tested manifest labels `input: ScScalar` secret and `peer: SgPoint` public. Under those caller assumptions the checked native-source profile reaches 44 procedures in 98,078 visits and records the three secret-derived authored assertions at `secp_field.coax:239` and `secp_field32.coax:70,92`. It returns `guard-obligations`, with `accepted=false`; it does not automatically apply the separate [field guard proof](2026-10-29-secp-field-guard-reproof.md). The same source under the default all-secret classification refuses a secret branch at `secp_mult_const.coax:143`. No result here proves that a caller supplies a public peer, that emitted native or Wasm code has constant-time behavior, that secrets are erased, or that ECDH or production-secret use is authorized.

The classification manifest is JSON with exactly `schema`, `entry`, `sources`, and `parameters`. `sources` must equal the compiler source map's complete `source_hashes` object, including paths. The ordered `parameters` array contains objects with exactly `name`, `type`, and `classification` (`public` or `secret`). Generate the source map in the actual checkout and independently choose labels that match the call boundary. For example, after inspecting that map, a manifest for this specific root can be constructed and audited as follows:

```sh
coax/bin/coax c coax/lib/crypto/secp_mult_const.coax --library --source-map /tmp/smc-map.json -o /tmp/smc.c
python3 - <<'PY'
import json
m = json.load(open('/tmp/smc-map.json'))
manifest = {'schema': 'coax.secret-classification.v1', 'entry': 'smc_multiply',
            'sources': m['source_hashes'], 'parameters': [
                {'name': 'input', 'type': 'ScScalar', 'classification': 'secret'},
                {'name': 'peer', 'type': 'SgPoint', 'classification': 'public'}]}
open('/tmp/smc-classification.json', 'w').write(json.dumps(manifest))
PY
python3 tools/coax-nostr/secret-check/aggregate.py coax/lib/crypto/secp_mult_const.coax --entry smc_multiply --classification /tmp/smc-classification.json --report /tmp/smc-audit.json
```

The last command exits nonzero for the recorded guard obligations. A changed source, type, entry or parameter order requires a new reviewed manifest; it is not silently reclassified. Duplicate JSON keys refuse.

## Checks

The full policy test ran 191/191 checks. Classified integration ran six tests, including independent checked/release runtime witnesses for early return and a hidden unsafe call through a mutable alias; legacy aggregate, scalar and source-policy integrations passed 13, seven and four tests respectively. The complete policy mutation run killed 2,783/2,783 mutants, with 25,501 selective checks and 131,849 checks skipped by the existing scheduler; no mutant was excluded. Earlier full passes had 114, 30 and 10 survivors, and the final zero-survivor result follows added policy checks and a simplified redundant condition, not waivers. The evidence records exact hashes and scheduler statistics. Raw detailed reports and mutation logs were retained under `/tmp` for this run; their hashes are in the evidence but they are not committed.

```sh
coax/bin/coax test tools/coax-nostr/secret-check/aggregate.coax
coax/bin/coax mutate tools/coax-nostr/secret-check/aggregate.coax --mutation-stats
python3 tools/coax-nostr/secret-check/test_classified.py
python3 tools/coax-nostr/secret-check/test_aggregate.py
python3 tools/coax-nostr/secret-check/test_secret.py
python3 tools/coax-nostr/secret-check/test_source_policy.py
```
