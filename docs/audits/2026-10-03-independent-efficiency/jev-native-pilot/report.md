# Native pilot results

Panel status: **complete**. Twelve attempts are assigned; unrun or unreadable attempts remain in the denominator.

| Arm | Accepted / assigned | Launched | Native completed | Known cost attempts | Total cost USD | Mean primary seconds |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| bare | 2/4 | 4 | 4 | 4 | 2.082077 | 243.356699 |
| deterministic | 2/4 | 4 | 4 | 4 | 1.480213 | 202.883057 |
| jev | 2/4 | 4 | 4 | 4 | 1.350181 | 184.055801 |

Primary time includes confirmed scratch cleanup. Failed-attempt costs remain included. Detailed task means, four matched pairs, stage timings, accounting bounds, artifact errors, and every win gate are in the JSON report.

## Directional win gates

- `complete_valid_panel`: true
- `jev_accepts_all_four`: false
- `jev_accepts_no_fewer`: true
- `cost_reduction_10pct_vs_bare`: true
- `time_reduction_10pct_vs_bare`: true
- `three_of_four_joint_pairs_vs_bare`: true
- `cost_reduction_10pct_vs_deterministic`: false
- `time_reduction_10pct_vs_deterministic`: false
- `three_of_four_joint_pairs_vs_deterministic`: true

This exposed-task pilot does not establish general coding reliability. Setup and capability costs are separate.
