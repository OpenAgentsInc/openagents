# Sample: v0.5 contract probe on fix-git

A sanitized copy of one real trial's retained evidence: the `coder-v05`
arm running a *contract probe* artifact over `archive/fix-git`, on
2026-09-22 on an arm64 Mac.

The artifact here is `tests/fixtures/coder-v05-probe` — a shell
executable that implements the `openagents.coder.episode.v1` command
surface, writes an honest bundle, and exits nonzero. It is not Coder
v0.5 and claims nothing about v0.5 capability. What it proves is the
integration itself: a pinned artifact installed, verified by sha256
inside the task environment, doctored, run, and collected — with its
honest failure retained end-to-end.

## What the trial shows

- Install: the artifact's sha256 was checked on the host and again
  inside the container (`sha256sum -c`); `--version` and
  `episode doctor` passed before any episode ran.
- Execution: `episode run` wrote `manifest.json`,
  `trajectory.atif.json`, and `evaluation/usage.json`, then exited 7.
- Terminal status: `agent_error` with `NonZeroAgentExitCodeError` — the
  episode's exit is the evidence, not a fabricated result.
- The independent verifier still ran and scored the task 0.0, which is
  the correct outcome: a failed episode earns no reward.
- The probe's ATIF trajectory was validated against Harbor's Pydantic
  model and converted; `counts` recorded 1 ATIF step, 0 calls.

## Contents

- `attempt.json` — the `openagents.tbench.attempt.v1` record.
- `manifest.json` — the episode manifest with resolvable sha256s.
- `job-config.json` — the materialized job config, artifact pin included.
- `evidence/` — Harbor's trial outputs plus the collected episode bundle:
  `agent/episode/manifest.json`, `agent/episode/trajectory.atif.json`,
  `agent/episode/evaluation/usage.json`, `agent/trajectory.json`,
  `exception.txt`, `verifier/reward.txt`, `verifier/ctrf.json`.

Local paths in tracebacks are rewritten to `<package>` and `<python>`
placeholders; job paths to `<jobs-dir>` and `<upstream>`.

## Reproduce

```sh
SHA=$(shasum -a 256 tests/fixtures/coder-v05-probe | cut -d' ' -f1)
OPENAGENTS_API_KEY=probe OPENAGENTS_DOOR_URL=http://localhost:1 \
  uv run tbench run --profile smoke --agent coder-v05 --task fix-git \
    --agent-kwarg artifact_path=$PWD/tests/fixtures/coder-v05-probe \
    --agent-kwarg artifact_sha256=$SHA
```
