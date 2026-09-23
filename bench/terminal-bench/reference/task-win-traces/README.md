# Public traces for the task-win review

These seven bundles support the
[September 23 trace analysis](../../../../docs/terminal-bench/2026-09-23-task-win-analysis.md).
They are public reference trials, separate from local Coder runs and
excluded from the local suite population.

For each highlighted zero-score task in the retained Astra max or Opus 5
max source job, the review selected the lexicographically first trial UUID
before inspecting its trajectory. Each `provenance.json` records that
choice, all five available trial IDs, the source job, model and agent
version, downloaded archive digest, and retained file digests. One sampled
failure does not establish the cause of every failure in that row.

Each bundle contains the normalized trajectory, native executor output,
verifier output and reward, a trimmed Harbor result, and collected text
artifacts in `produced-files.json`. Source and generated files are stored
as evidence data, not added as another product implementation. Environment
configuration, session settings, binary artifacts, and generated package
lockfiles are excluded. Known-credential and token-pattern scans found no
matches in the retained files.

The audit compares the public task instructions with the corresponding
local instructions, ignoring HTML comments and outer whitespace. They
match for all seven tasks; this does not prove identical images, tests,
or dependency versions.
