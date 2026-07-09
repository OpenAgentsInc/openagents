# CND-050: SHC Codex Terminal-Bench Smoke

> **Historical bootstrap note (#8591).** Kept for archaeology and ops memory.
> Active Cloud implementation is in the public monorepo (`crates/*`,
> `docs/cloud/`). Deprecated authority names: **Vortex** → Worker/Khala Sync;
> **Treasury product** → Worker credits + MDK/Nexus payout bridge only;
> **Nexus-as-registry** → Worker/Khala Sync (CLI may still say `nexus`).
> Do not treat this note as current product-authority ownership.

Status: measured single-task smoke
Last updated: 2026-06-01
Owner: OpenAgents

This report records the first actual Terminal-Bench 2.0 run on
`oa-shc-katy-01` using the SHC Docker substrate and Harbor's built-in Codex
agent. It is an internal smoke result, not a public leaderboard claim.

Sources:

- Harbor Terminal-Bench run guide:
  <https://www.harborframework.com/docs/tutorials/running-terminal-bench>
- Harbor agent integration docs:
  <https://www.harborframework.com/docs/agents>
- Terminal-Bench 2.0 registry:
  <https://www.harborframework.com/registry/terminal-bench/2.0>

## Host

| Field | Value |
| --- | --- |
| Host | `oa-shc-katy-01` |
| IP | `23.182.128.195` |
| OS | Ubuntu 24.04 |
| Shape | 16 vCPU, 64 GB RAM, 256 GB NVMe class VPS |
| Docker | `Docker version 29.1.3` |
| Docker Compose | `Docker Compose version 2.40.3` |
| Harbor launch path | `uvx --from harbor harbor run ...` |
| Codex package inside task | `@openai/codex@0.135.0` |
| Codex model | `gpt-5.5` |
| Auth mode | Temporary ChatGPT Codex `auth.json` injected through `CODEX_AUTH_JSON_PATH` |

The temporary host-side Codex auth file was removed after the run. Harbor also
removed `/tmp/codex-secrets` and the task-local `CODEX_HOME` in the container.
No auth material should be copied into docs, issue comments, or committed
artifacts.

## Setup Performed

The SHC host was reachable by SSH but did not have the benchmark harness
prerequisites. These were installed before the run:

```bash
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
  docker.io docker-compose-v2 python3-venv python3-pip jq git curl ca-certificates
sudo systemctl enable --now docker
sudo usermod -aG docker ubuntu
curl -LsSf https://astral.sh/uv/install.sh | sh
```

`docker compose` is required. Plain `docker.io` without
`docker-compose-v2` failed before any agent work started.

## Successful Run

Command shape, with the auth path redacted:

```bash
uvx --from harbor harbor run \
  --dataset terminal-bench@2.0 \
  --task terminal-bench/openssl-selfsigned-cert \
  --agent codex \
  --model gpt-5.5 \
  --agent-kwarg version=0.135.0 \
  --agent-env CODEX_AUTH_JSON_PATH=<session-auth-json> \
  --n-concurrent 1 \
  --n-attempts 1 \
  --jobs-dir /home/ubuntu/oa-bench-runs \
  --job-name shc-codex-tb2-openssl-gpt55-20260601 \
  --yes \
  --debug
```

Artifact root on SHC:

```text
/home/ubuntu/oa-bench-runs/shc-codex-tb2-openssl-gpt55-20260601/
```

## Score

| Metric | Value |
| --- | --- |
| Dataset | `terminal-bench@2.0` |
| Task | `terminal-bench/openssl-selfsigned-cert` |
| Task checksum | `2b70d5535b5873f644fad37b76dbef86a1e42162e018c7bc06316e5e2521929a` |
| Trials | 1 |
| Exceptions | 0 |
| Mean reward | `1.000` |
| Trial reward | `1.0` |
| Verifier tests | 6 passed, 0 failed |
| Total runtime | 1 minute 12 seconds |
| Environment setup | 5.07 seconds |
| Agent setup | 17.25 seconds |
| Agent execution | 30.65 seconds |
| Verifier runtime | 4.85 seconds |
| Input tokens | 95,009 |
| Cached input tokens | 85,504 |
| Output tokens | 1,013 |
| Reported cost | `$0.120667` |

Verifier tests that passed:

- `test_outputs.py::test_directory_structure`
- `test_outputs.py::test_key_file`
- `test_outputs.py::test_certificate_file`
- `test_outputs.py::test_combined_pem_file`
- `test_outputs.py::test_verification_file`
- `test_outputs.py::test_python_verification_script`

Codex created the requested `/app/ssl/server.key`, `/app/ssl/server.crt`,
`/app/ssl/server.pem`, `/app/ssl/verification.txt`, and `/app/check_cert.py`.
The verifier accepted the result.

## Failed Attempts Before The Passing Run

These are useful operational lessons, not benchmark scores.

| Attempt | Result | Cause |
| --- | --- | --- |
| Bare task slug `--task openssl-selfsigned-cert` | No trial started | Harbor v0.13 expects registry task refs in `org/name` form. Use `terminal-bench/openssl-selfsigned-cert`. |
| Docker without Compose v2 | No trial completed | Harbor invoked `docker compose`; `docker.io` alone did not provide the plugin. |
| `--model gpt-5-codex` with ChatGPT auth | Trial errored, reward 0 | Codex returned `400`: that model was not supported with the ChatGPT account auth mode used for this run. |

## Interpretation

This proves the SHC node can run the real Harbor Terminal-Bench 2.0 Docker
path with Codex auth injected into the task container. It does not prove full
Terminal-Bench performance, provider economics, or public benchmark standing.

The meaningful current claim is narrow:

```text
oa-shc-katy-01 can run one official Terminal-Bench 2.0 task through Harbor's
Codex agent, execute the task inside Docker, run the verifier, capture a
trajectory, and produce a passing internal score.
```

## Next Steps

- Pin task refs or record task checksums for every future run.
- Add a Cloud wrapper command that launches this exact Harbor path from
  `oa-codex-control` or a Benchmark Cloud control service instead of manual SSH.
- Copy the Harbor result JSON and verifier artifacts into the normalized
  `BenchmarkResult` and proof-bundle schema used by `runners/py-bench-runner`.
- Run a 3-task smoke set before any customer-facing claim:
  `openssl-selfsigned-cert`, one git task, and one data-processing task.
- Keep public projection disabled until Worker/Khala Sync has proof-bundle redaction,
  dataset/version disclosure, retry policy, and artifact retention checks.
