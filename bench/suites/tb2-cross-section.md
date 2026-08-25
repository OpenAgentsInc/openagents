# Terminal-Bench 2.0 cross-section suite

Date: 2026-08-24. Companion to
`docs/terminalbench/2026-08-24-fix-git-run-analysis.md` (openagents.com repo)
and the Gym plan. Purpose: test whether the tool-declaration efficiency
levers (batching, quiet flags, `--stat`-before-`-p`, lane-aware verbosity)
generalize beyond `fix-git`, across distinct skill families and across
round-count and output-volume regimes.

Source of truth for task metadata: `task.toml` in each task directory of
`laude-institute/terminal-bench-2` (the repo Harbor's `terminal-bench@2.0`
registry entry points at). All 89 tasks are amd64, `gpus = 0`,
`allow_internet = true`; none use docker-compose or multi-container setups.

## The twelve tasks

| # | Task | Skill family | What it tests | Difficulty | Expected shape | Why this slot |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `git-leak-recovery` | version control / forensics | Recover a secret purged by history rewrite (reflog, dangling objects, packfiles), then scrub it without touching unrelated history | medium | many rounds, high output volume (history dumps) | Direct oracle for the `git_lost_work` plugin delta (OpenAgentsInc/openagents#37); the recon phase is exactly what the plugin collapses |
| 2 | `sanitize-git-repo` | version control / security | Find and replace all API keys across a real repo's full history with consistent placeholders, leaving clean files untouched | medium | many rounds, high output volume (grep/log over history) | Second plugin-delta candidate; punishes full-patch dumps — the `--stat`-before-`-p` lever is load-bearing here |
| 3 | `merge-diff-arc-agi-task` | version control + codegen | Fetch two git bundles into branches, merge with conflict resolution, and write an `algo.py` that generalizes an ARC-style mapping | medium | medium rounds, low volume | Multi-skill in one task: git mechanics plus a reasoning payload; the closest sibling to `fix-git` without repeating it |
| 4 | `build-cython-ext` | build / compile | Clone pyknotid 0.5.3, fix NumPy 2.x incompatibilities, compile three Cython extensions, install system-wide | medium | MANY rounds (compile-fix loop), high volume (compiler errors) | The canonical iterate-on-compiler-output loop; round count here is where batching either pays or doesn't |
| 5 | `sqlite-with-gcov` | build / system | Build SQLite from a pre-vendored fossil tarball with gcov instrumentation and put it on PATH | medium | FEW rounds, very high output volume (configure/make) | Offline and deterministic — the pure output-volume probe: a good agent quiets make, a bad one replays it every round |
| 6 | `fix-code-vulnerability` | debugging / security | Identify the CWE-classified vulnerability in a pinned `bottle.py` checkout and report it as JSONL | hard | FEW rounds, low volume, reasoning-heavy | Quick-shaped but hard: discriminates on success, not cost — anchors the suite so efficiency wins aren't confounded with capability losses |
| 7 | `regex-log` | data processing | Author one regex (last date on IPv4-bearing lines, validity rules) into `/app/regex.txt` | medium | FEW rounds (quick task) | The suite's fast lane: near-zero tool surface, measures whether the agent can just answer |
| 8 | `count-dataset-tokens` | data processing / ML | Pull a 1k-sample HF dataset, read its README for usage, tokenize the science domain with the Qwen2.5-1.5B tokenizer, write one integer | medium | medium rounds, network-bound | Real-world dataset plumbing (pip + HF hub at agent time); tokenizer-only download, no model weights |
| 9 | `password-recovery` | security / forensics | Carve a fragmented zip out of a raw disk image (dd/strings/foremost/sleuthkit) to recover a deleted password | hard | MANY rounds, medium volume | File-forensics: the non-git half of the future forensics-plugin story; forensic tools are pre-installed so it is iteration, not setup |
| 10 | `openssl-selfsigned-cert` | security / sysadmin | Execute a fully-specified checklist: key with 0600 perms, self-signed cert with exact subject fields, verification artifacts | medium | FEW rounds if batched, MANY if not | The purest batching discriminator in the set: every command is known upfront; round count is entirely a tool-habit measurement |
| 11 | `nginx-request-logging` | networking / sysadmin | Install and configure nginx: custom log format, rate limiting zone, 404 page, exact file locations; verifier drives HTTP traffic | medium | medium-many rounds (config-test loop) | The service-configuration family; edit-reload-curl loops reward tight rounds and punish re-reading configs |
| 12 | `schemelike-metacircular-eval` | long-horizon SWE | Write a metacircular evaluator in a scheme-like language that runs all test programs *and itself* | medium (300-min expert) | MANY rounds, large artifacts, long wall time | The long-horizon anchor: sustained write-test-debug over one growing file; context growth over many rounds is exactly the quadratic-replay cost under test |

Coverage check against the selection criteria:

- Version control without repeating `fix-git`: tasks 1–3.
- Build/compile: 4, 5. Debugging/code-fix: 6 (and 4's fix loop).
- Data processing: 7, 8. Security/forensics: 1, 2, 9, 10.
- Networking/sysadmin: 10, 11. Long-horizon: 12 (plus 4, 9).
- Round-count spread: quick — 6, 7, 10 (if batched); many-round — 1, 2, 4,
  9, 11, 12. Output-volume spread: 5 (dominant), 1, 2, 4.
- Plugin-delta candidates (#37): 1 and 2 (git forensics), 9 (file
  forensics).

Near-misses, recorded so the next round doesn't re-derive them:
`extract-elf` (write a JS ELF parser — good, lost its slot to
`openssl-selfsigned-cert` because this round is about efficiency levers),
`build-pmars` (Debian source archaeology, overlaps 4/5),
`cancel-async-tasks` (subtle asyncio, overlaps 6's shape),
`sqlite-db-truncate` and `db-wal-recovery` (overlap 9),
`cobol-modernization` (easy; a fine future quick slot).

## Excluded with cause

- `custom-memory-heap-crash`: the Dockerfile builds a custom gcc/libstdc++
  from source at image build time — hours under Rosetta.
- `qemu-startup`, `qemu-alpine-ssh`, `install-windows-3.11`: nested
  virtualization inside an emulated amd64 container; the run-analysis doc's
  qemu fragility note applies squarely.
- `hf-model-inference`, `mteb-*`, `caffe-cifar-10`, `sam-cell-seg`,
  `train-fasttext`: model/dataset downloads in the hundreds of MB to GB, or
  ML workloads that are emulation-hostile.
- `build-pov-ray` (12,000 s timeouts), `compile-compcert` (Coq/OCaml
  toolchain build): compile times balloon under Rosetta.
- `crack-7z-hash`: builds John the Ripper at image build and then
  brute-forces — CPU-bound cracking under emulation is a wall-time gamble.
- `extract-moves-from-video`, `video-processing`, `code-from-image`,
  `financial-document-processor`: video/OCR workloads, heavy and slow
  under emulation.

## Per-task runnability notes and risks

| Task | Image base | Agent-time network needs | Risks |
| --- | --- | --- | --- |
| git-leak-recovery | ubuntu:24.04 + git | none | none notable |
| sanitize-git-repo | python:3.13-slim + git | none (repo cloned at image build, pinned commit) | image build clones a GitHub repo — needs network at build; verifier bundles a 1.4 MB fixture |
| merge-diff-arc-agi-task | ubuntu:24.04 (bare) | apt-get install git (image ships **without git**) | first rounds are apt-driven; apt mirror slowness inflates wall time, not rounds |
| build-cython-ext | python:3.13-slim + build-essential, numpy 2.3.0 | git clone pyknotid + pip installs | Cython compile under Rosetta is minutes, bounded; network clone at agent time |
| sqlite-with-gcov | ubuntu:24.04 + toolchain, vendored tarball | none | configure/make of SQLite under Rosetta ≈ several minutes; verifier runs gcov checks |
| fix-code-vulnerability | python:3.11-slim + pinned bottle checkout | none | none notable |
| regex-log | ubuntu:24.04 (bare) | none | none notable |
| count-dataset-tokens | python:3.13-slim (bare) | pip (datasets/transformers ≈ 100–300 MB wheels) + HF hub (dataset ~1k rows, tokenizer files only) | heaviest network dependency in the suite; still well under the 2 GB line; HF availability is a flake source |
| password-recovery | ubuntu:24.04 + forensic tools (extundelete, sleuthkit, foremost) | none | 4 MB image is carved with userland tools; no loop mounts needed |
| openssl-selfsigned-cert | (light) | none | none notable |
| nginx-request-logging | python:3.13-slim + curl | apt-get install nginx | verifier makes live HTTP requests incl. rate-limit probes — mildly timing-sensitive under emulation; watch for flaky rate-limit assertions |
| schemelike-metacircular-eval | python:3.13-slim + test corpus | none | 2,400 s agent/verifier timeouts; the self-interpretation test tower is slow — budget the long verifier |

Verifier shape: every task's `tests/test.sh` follows the standard template
(install `uv` 0.9.5, `uvx pytest` with CTRF output, reward.txt), except
`build-cython-ext`, `fix-code-vulnerability`, `hf-model-inference`,
`headless-terminal`, `kv-store-grpc`, and `largest-eigenval`, which
`pip install pytest` directly. Both shapes run fine under Rosetta (the qemu
`uv` segfault is the already-documented failure mode that Rosetta fixes —
see `bench/README.md`).

## Running the suite

`harbor run --help` confirms `-i` / `--include-task-name` is a repeatable
`list[str]` option (and accepts globs), so the whole suite is one
invocation. Follow `bench/README.md` for the working-tree CLI tarball,
dev-forge token, and Rosetta prerequisites.

Proxy lane (one command per catalog model; shown for gpt-5.6-luna —
substitute `-m openai/<model>` per lane, e.g. `gemini-3.7-flash`):

```sh
PYTHONPATH=bench OPENAGENTS_TOKEN=... harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path adapters.openagents_coder:OpenAgentsCoder \
  -m openai/gpt-5.6-luna \
  -i git-leak-recovery -i sanitize-git-repo -i merge-diff-arc-agi-task \
  -i build-cython-ext -i sqlite-with-gcov -i fix-code-vulnerability \
  -i regex-log -i count-dataset-tokens -i password-recovery \
  -i openssl-selfsigned-cert -i nginx-request-logging \
  -i schemelike-metacircular-eval \
  --n-concurrent 2 \
  --jobs-dir /tmp/gym-jobs/tb2-cross-section-luna
```

Local lane (Ollama through the same adapter, as in the fix-git qwen run):

```sh
PYTHONPATH=bench OPENAGENTS_TOKEN=... harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path adapters.openagents_coder:OpenAgentsCoder \
  -m ollama/qwen3.8:27b-mtp-q8_0 \
  -i git-leak-recovery -i sanitize-git-repo -i merge-diff-arc-agi-task \
  -i build-cython-ext -i sqlite-with-gcov -i fix-code-vulnerability \
  -i regex-log -i count-dataset-tokens -i password-recovery \
  -i openssl-selfsigned-cert -i nginx-request-logging \
  -i schemelike-metacircular-eval \
  --n-concurrent 1 \
  --jobs-dir /tmp/gym-jobs/tb2-cross-section-qwen
```

`--n-concurrent 2` is the suggested ceiling on the Rosetta rig for the
proxy lanes (two emulated amd64 builds plus a compile task saturate the
machine); the local lane should stay at 1 because the model itself owns
the cores. Jobs-dir convention:
`/tmp/gym-jobs/tb2-cross-section-<lane>`, one directory per lane, so
`bench/post_gym_run.py` can post each lane's job separately.

Loop form, for scripts (equivalent per-task jobs driven from the
machine-readable list; also the fallback if a future Harbor version
changes `-i` semantics):

```sh
grep -v '^#' bench/suites/tb2-cross-section.txt | while read -r task; do
  [ -z "$task" ] && continue
  PYTHONPATH=bench OPENAGENTS_TOKEN=... harbor run \
    --dataset terminal-bench@2.0 \
    --agent-import-path adapters.openagents_coder:OpenAgentsCoder \
    -m openai/gpt-5.6-luna \
    -i "$task" --n-concurrent 1 \
    --jobs-dir "/tmp/gym-jobs/tb2-cross-section-luna/$task"
done
```

Timeout note: `schemelike-metacircular-eval` carries 2,400 s budgets and
Rosetta compile tasks run slow; if a run brushes limits, prefer
`--timeout-multiplier 2.0` over trimming the suite — a timeout-shaped
failure is not an efficiency signal.
