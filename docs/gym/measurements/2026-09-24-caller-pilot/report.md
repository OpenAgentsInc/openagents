# `caller-acme-returns-v1` — the measured record

Store `results/caller-acme-returns-v1.jsonl` holds 64 rows; the receipt chain verifies to head `receipt:cd98d470c3b8e1c281fc7cfde73b24a624d5742ae0aa58669d8fe91bc50d203e`.
Recorded 2026-09-22T14:23:55Z through 2026-09-22T14:43:47Z.

A row the harness never wrote is not here: items lost to timeouts or dead doors leave no row, and where the selection is declared they are named as missing — never scored as wrong answers. A refused item counts against the door that refused it.

Suite `caller-acme-returns-v1` — digest `bcba0151f2a6dba5d0dd2b7e95e42790a048105ffacd2c67ed362732bd96f6dc`.
Question set `caller-acme-returns-v1` — digest `b81980f77d70814a55d56a3579788d378563e2bd21f313a266f35bcbaebce2b7`.
Gate `probability-v2` — digest `gate:5bdfd1423c6c2d305a12d8f9f16e7e83a594f8548f1b605de0f77c1ef639e758`.

**Coverage: complete** — every expected item of every expected door recorded exactly once.

Declared selection: 32 items, doors `kev-0.6b`, `constant`.

## `kev-0.6b`

Identity: base `da87bfb6`, checkpoint `sha256:21bcea1838ad78ff54088b9e06eaf62b1f0702b61c5a38cfc1a84acc5e2cc679`, execution `{"attention": "eager-block-causal-v1", "backend": "cpu", "bucket_size": "0", "dtype": "f32", "head_dtype": "f32", "lora_merge": "fp32-before-cast-v1", "max_branch": "8192", "max_state": "8192", "option_isolation": "false"}`.
Trials: estimator `unreported`.
Coverage: 32 of 32 expected items recorded — 32 answered, 0 refused; 0 missing or unattempted.

32 items recorded: 32 scored, 0 refused by the door.
Median latency 30972 ms.

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| --- | --- | --- | --- | --- | --- | --- |
| calibration | 0.88 | 0.242 | 0.132 | 0.414 | 0 | 16 |
| development | 0.69 | 0.135 | 0.199 | 0.600 | 1 | 16 |

| Family | Accuracy | ECE | Brier | NLL | Confident errors | Items | Ceiling |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `consumer-returns` | 0.81 | 0.232 | 0.178 | 0.551 | 1 | 16 | 0.92 |
| `pro-returns` | 0.75 | 0.206 | 0.153 | 0.463 | 0 | 16 | 0.88 |

| Family | Expected | Recorded | Answered | Refused | Missing |
| --- | --- | --- | --- | --- | --- |
| `consumer-returns` | 16 | 16 | 16 | 0 | 0 |
| `pro-returns` | 16 | 16 | 16 | 0 | 0 |

Label evidence:

- `consumer-returns` — acme — rule "the returns desk's disposition at close"
- `pro-returns` — acme — rule "the returns desk's disposition at close"

## `constant`

Identity: base ``, checkpoint `sha256:1111111111111111111111111111111111111111111111111111111111111111`, execution `{"lane": "unmetered-local"}`.
Trials: estimator `unreported`.
Coverage: 32 of 32 expected items recorded — 32 answered, 0 refused; 0 missing or unattempted.

32 items recorded: 32 scored, 0 refused by the door.
Median latency 1 ms.

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| --- | --- | --- | --- | --- | --- | --- |
| calibration | 0.38 | 0.625 | 0.625 | 17.269 | 10 | 16 |
| development | 0.25 | 0.750 | 0.750 | 20.723 | 12 | 16 |

| Family | Accuracy | ECE | Brier | NLL | Confident errors | Items | Ceiling |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `consumer-returns` | 0.31 | 0.688 | 0.688 | 18.996 | 11 | 16 | 0.92 |
| `pro-returns` | 0.31 | 0.688 | 0.688 | 18.996 | 11 | 16 | 0.88 |

| Family | Expected | Recorded | Answered | Refused | Missing |
| --- | --- | --- | --- | --- | --- |
| `consumer-returns` | 16 | 16 | 16 | 0 | 0 |
| `pro-returns` | 16 | 16 | 16 | 0 | 0 |

Label evidence:

- `consumer-returns` — acme — rule "the returns desk's disposition at close"
- `pro-returns` — acme — rule "the returns desk's disposition at close"

## Checking this record

Every row pins the suite digest, the question-set digest, and the gate digest, and carries a receipt over its contents chained to the row before it. Every store-reading command walks that chain and refuses a broken one; `gym verify --store` walks it without rendering the tables. The suite digests itself on load, and the named gate lives in `crates/gym/gates/`.

A verified chain proves these rows were not edited or resequenced inside this file; it does not prove the file is whole or that this is the only store. Completeness comes from the declared selection above, and permanence comes from the commitment written beside this record — check a later copy of the store against it with `gym verify --store … --commitment …`.
