# Cycle review: T2 after Phoenix→rust hop, tb2-quick proxy Flash

Date: 2026-08-28. Lane: proxy. Model: `glm-5.3-flash` (Harbor `-m zai/glm-5.3-flash`).
Suite: `tb2-quick`. CLI linux-x86_64 digest
`sha256:834fdb5edd5f4a40b76ce63dfac681311033cea4728bc4ed0c27f27950d07cdf`
from worktree `dc06fb409d`. Gym run id `31507ff7-6355-4738-a38f-da04c4658b61`
(ingest PATCH 422 then the runner marked the row abandoned; Harbor grades
still stand). Store row appended as `receipt:fde443beec3ce65e8f2049754a56298284cf74a5ede2f15fd369a731e6aebc3c`.

Harbor job `/tmp/gym-jobs/tb2-quick-after-hop/2026-08-28__03-04-49`
(`9e53a0f5-c182-4697-adf6-33bd28cb65df`): 2 trials, mean 0.500, 10m 29s.

## Outcomes

- `openssl-selfsigned-cert` reward 1.0. `coder.txt` billed 58847 prompt +
  6255 completion tokens on `glm-5.3-flash`. The production hop answered.
  This is the first Flash `tb2-quick` trial after T2 that completed a turn
  (`docs/coder/reviews/2026-08-28-t2-stat-before-p.md` required that before
  T1/T3).
- `regex-log` reward 0.0, `NonZeroAgentExitCodeError`. `coder.txt` ends
  `https://openagents.com/api/inference/proxy could not be reached`. No
  turn. Same container-to-proxy miss as the 03:06/03:18 rows, on the second
  trial only.

Success rate 1 of 2, gate floor 0.5, `cost_unknown` (scorer did not bind
Harbor cost). T2 remains unmeasured on git-forensics oracles. Do not refute
T2. Do not treat regex-log's miss as a T2 delta (`ledger:M5`).

## Refs

- Harbor job `9e53a0f5-c182-4697-adf6-33bd28cb65df`
- `row` receipt `receipt:fde443beec3ce65e8f2049754a56298284cf74a5ede2f15fd369a731e6aebc3c`
- `ledger:T2`, `ledger:M3`, `ledger:M5`
- Production hop SHA Phoenix `436d37f`, rust coder-api `2638a49b94`
