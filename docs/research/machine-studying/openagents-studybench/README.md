# OpenAgents StudyBench

Date: 2026-06-17
Status: public-retained MVP fixtures and boundary docs

This folder holds the OpenAgents-owned StudyBench-style repo benchmark material.
Rows use the `openagents.studybench_task.v0` contract and are grounded in a
pinned public `openagents` source snapshot.

## Splits

- `public-retained/`: committed examples and regression fixtures. Gold answers,
  rubrics, and evidence spans are public.
- Private validation and private holdout rows are not committed here. Public
  docs may carry split refs and checksums only.
- `private/`: ignored local authoring path for private rows and local scorer
  artifacts. Only `private/.gitignore` is committed.
- `runs/`: public-safe run summaries and refs-only aggregate comparison
  reports. These may include private validation refs and checksums, but not
  private row bodies, hidden rubrics, hidden gold answers, or private evidence
  spans.

See `private-boundary.md` for evaluator access, leak response, GEPA/study-packet
limits, and refs-only public projection rules.

## Public-Retained Package

- `public-retained/openagents-launch-v0.jsonl`

This initial package contains 10 launch-focused rows covering:

- launch claim boundaries;
- Tassadar public projection truth;
- settlement and wallet truth;
- Customer #1 evidence;
- Forge Autopilot Coder study-packet projection;
- Blueprint, Probe, and GEPA authority boundaries;
- Pylon and launch priority boundaries;
- StudyBench schema adaptation;
- answer-mode versus patch-mode evaluation;
- product-promise and marketplace gates.

The rows are public retained fixtures. They are useful for examples, regression,
loader tests, and evaluator calibration. They are not hidden benchmark evidence.
