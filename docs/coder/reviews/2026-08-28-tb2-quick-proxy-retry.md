# Cycle review: tb2-quick proxy retry after ingest fix

Date: 2026-08-28. Lane: proxy. Model: `glm-5.3-flash`.
CLI linux-x86_64 `sha256:2b7d4c5e851f5187aeaab2eb4613ee4982331e4df92cb68813b1ce5cb71bbea8`
from `2a60adb26a`. Gym run `d5296fb5-06bd-4efc-94d8-6b8e02145a7a` finalized
**200 graded 1/2** (the ingest 422 is gone).

Harbor job `/tmp/gym-jobs/tb2-quick-retry/2026-08-28__08-33-36`, 16m 28s.

## Outcomes

- `openssl-selfsigned-cert` reward 1.0, 80525 billed tokens, hop answered.
- `regex-log` still `could not be reached` for
  `https://openagents.com/api/inference/proxy` after one adapter retry.
  1200-byte log, no turn. Same signature as the first hop run.

Two proxy `tb2-quick` runs now share the ceiling **1 of 2**, with openssl
always completing and regex-log never leaving the container. That is the
regex-log Harbor image's egress, not T2. Do not expect 2/2 on this suite
on the proxy lane. T2 still needs git-forensics oracles
(`tb2-cross-section` tasks 1–2).

Store: `receipt:f4ceba54e2506964caaf830783b7f064852f7163bc262410a20480c5cf78f554`.
