# Git-forensics oracles on the production hop

Date: 2026-08-28. Lane: proxy. Model: `glm-5.3-flash`.
Plain task list `git-leak-recovery` + `sanitize-git-repo` (not a pinned
score suite). Gym run `44a64c03-d0d2-452c-a26e-cfefaf9b88d3` finalized
**200 graded 1/2**. Harbor job `/tmp/gym-jobs/git-oracles/2026-08-28__08-52-29`,
9m 47s, **zero exceptions**.

## Outcomes

- `git-leak-recovery` reward 1.0, 59217 billed tokens. Hop answered.
- `sanitize-git-repo` reward 0.0, 189735 billed tokens. Hop answered; the
  verifier rejected the work.

This is the first proxy pair where both git-forensics trials completed a
turn. T2 `--stat`/`-p` is not visible in the captured `coder.txt` tails
(tool argv is not printed there). Do not promote or refute T2 from this
pair. #120 plugin A/B still needs a with/without-plugin repeat on these
oracles.

Not appended to `bench-results/tb2-cross-section.jsonl` (no suite pin).
