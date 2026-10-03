# Model and briefing factorial panel

Primary verdict: **cost gate passed**.

The combined policy and Sonnet briefing comparison pass their separate cost gates on this task. This does not establish a population-level interaction.

A is Opus control; B is Opus with a brief; C is Sonnet control; D is Sonnet with the same brief.

| Arm | Final results | Accepted | First-attempt accepted |
| --- | ---: | ---: | ---: |
| A | 4/4 | 4/4 | 3/4 |
| B | 4/4 | 4/4 | 4/4 |
| C | 4/4 | 4/4 | 1/4 |
| D | 4/4 | 4/4 | 0/4 |

| Comparison | Verdict | Median cost ratio | Cheaper blocks | Median recorded endpoint ratio |
| --- | --- | ---: | ---: | ---: |
| D versus A | cost gate passed | 0.3458 | 4/4 | 0.8930 |
| C versus A | cost gate passed | 0.5292 | 4/4 | 0.9416 |
| D versus C | cost gate passed | 0.6534 | 3/4 | 0.9483 |
| B versus A | cost gate not met | 1.1959 | 1/4 | 1.0713 |
| D versus B | cost gate passed | 0.2891 | 4/4 | 0.8335 |

Ratios compare candidate to reference. Each cost uses the final cumulative CLI list-price estimate once, including repair, plus paid briefing preparation. These estimates are not verified charges. Unknown or incomplete costs do not pass a gate.

Recorded endpoint wall time is wall_s + preparation_wall_s + warm_brief_wall_s, with warm briefing preparation added only for B and D. It includes source export, instruction checks, prompt setup, the model session, external checks, and executor-process shutdown. The frozen runner records wall_s before final candidate/artifact capture, scratch-workspace deletion, and final result serialization. Those later steps are excluded, so this metric does not measure the entire harness elapsed time. The registered formula and gate remain unchanged. Shared warmups use the same timing boundary and remain separate from scored runs; cold indexing is reported separately.

Acceptance differences remain explicit. A failed arm’s cost is a fixed-endpoint observation, not a cost through acceptance or an effectiveness win. The five comparisons were planned separately; old Opus runs and unregistered attempts are retained outside their calculations.

All registered rows, sanitized check logs, raw and normalized candidate patches, model/cache counters, and artifact digests are in metrics.json. Raw transcripts, thinking, tool inputs/results, and account metadata are excluded. Redacted or binary-omitting patches are labeled. Four blocks on one task do not establish broad superiority.
