# Read deployment verdicts from stored passes

The `deployment_from_store` example builds the `Deployment` profiles that
`Gate::judge_deployment` reads. It makes no model calls and changes no records.
Use it to reproduce deployment comparisons from receipt-verified result stores.

Supply a JSON config with a suite path, gate path, workload budget, and doors.
The first door is the regression baseline. Paths are relative to the working
directory. Each selected door must have exactly one complete pass over the
suite's calibration and development partitions in its named store. Permutation
probe rows are excluded. Repeated passes, missing items, untimed rows, mixed
model identities, and mismatched question digests fail the read instead of
changing the denominator.

For example, this config reuses the router budget stated in the original
[deployment ranking record](measurements/2026-09-19-deployment-ranking.md):

```json
{
  "suite": "crates/gym/suites/support-v2-three-way.json",
  "gate": "crates/gym/gates/deployment-v2.json",
  "budget": {
    "workload": "router",
    "max_latency_ms": 300.0,
    "max_cost_per_decision_usd": 0.0001,
    "max_refusal_rate": 0.02,
    "source": "docs/gym/measurements/2026-09-19-deployment-ranking.md"
  },
  "doors": [
    {
      "name": "kev-0.5b",
      "store": "crates/gym/results/support-v2-three-way-quiet.jsonl",
      "cost": { "lane": "unmetered_local_lane" }
    },
    {
      "name": "jev (hosted)",
      "store": "crates/gym/results/support-v2-three-way-quiet.jsonl",
      "cost": null
    }
  ]
}
```

Save it as `/tmp/deployment-comparison.json`, then run from the repository root:

```sh
cargo run -p gym --example deployment_from_store -- /tmp/deployment-comparison.json
```

The output is one JSON object per door with its source store, row count, model
identity, suite and question digests, profile, budget, gate digest, and actual
gate outcome. Refusals remain timed calls and contribute to the refusal rate.
All rows must have finite, nonnegative latency values.

Set local costs to `{ "lane": "unmetered_local_lane" }`. Use `null` for a price that has not
been established; the example does not infer a current hosted price from an old
measurement or record an unmetered lane as zero. The output records the exact
cost supplied by the config.

Receipt verification proves the stored bytes, not quiet measurement conditions.
Keep the host, load, run commands, and block-spread evidence beside the output.
A verdict under `deployment-v2` still uses that gate's existing noise floor and
pending-measurement limits. This reader does not establish that the floor
transfers to a new host or slower model, or turn a comparison across hosts into a
controlled comparison.
