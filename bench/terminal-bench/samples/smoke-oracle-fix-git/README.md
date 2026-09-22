# Sample: oracle control on fix-git

A sanitized copy of one real trial's retained evidence: the `oracle`
control arm over `archive/fix-git`, run 2026-09-22 on an arm64 Mac with
the task's `linux/amd64` image under emulation. The oracle applied the
task's own solution and the protected verifier scored it 1.0.

This is a control trial, not an agent result. It proves the harness, the
task environment, and the verifier end-to-end; it says nothing about any
agent's capability.

## Provenance

- Job: `smoke--oracle` under `~/.openagents/terminal-bench/jobs/`.
- Task: `archive/fix-git` at upstream commit
  `3b5caaa4863d64dda7f0957bf4fc2d4f019202d4`, checksum
  `e89f54f2fc73ac576f976c1cd09758e4c9ccf152e53c2dcd91da0c178b89abc7`.
- Files copied verbatim from the trial directory, with absolute paths
  rewritten to `<jobs-dir>` and `<upstream>` placeholders. File contents
  are otherwise unmodified.

## Contents

- `attempt.json` — the `openagents.tbench.attempt.v1` record: reward
  separate from terminal status, per-phase timing, unknown usage and
  cost marked unknown rather than zero.
- `manifest.json` — the `openagents.tbench.episode-manifest.v1`
  manifest: every retained file with its sha256. Digests in it were
  computed on the originals; the evidence files here are byte-identical
  copies, so `sha256sum` of each resolves the reference.
- `job-config.json` — the materialized Harbor job config the run used.
- `evidence/` — Harbor's own outputs: `trial-result.json`,
  `trial-config.json`, `trial.log`, `agent/oracle.txt`,
  `verifier/reward.txt`, `verifier/ctrf.json`,
  `verifier/test-stdout.txt`.

The oracle arm writes no ATIF trajectory; the manifest records that
honestly as unresolved rather than fabricating one.
