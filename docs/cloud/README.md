# Cloud

Running Coder and agent work on Google Cloud machines (project
`openagentsgemini`) in parallel with, and in place of, the owner's own
computers.

Nothing in this directory is implemented on `main` yet. The earlier Cloud
crates and documents (`oa-codex-control`, `oa-node`, `oa-workroomd`, the old
`docs/cloud/`) were removed in commit `dabc08102f` on 2026-09-18; read them at
[`8f84d05896`](https://github.com/OpenAgentsInc/openagents/tree/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/cloud).

| Document | What it covers |
| --- | --- |
| [Cloud parallel execution audit, 2026-10-02](2026-10-02-cloud-parallel-execution-audit.md) | What existed before the reset (GCE and Firecracker lanes, the Coder run pool, Factory Droid Computers and Amp orbs), what runs in Google Cloud today, the gaps, a recommended design, and the issues to open |
