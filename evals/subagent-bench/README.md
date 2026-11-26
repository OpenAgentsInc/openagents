# Subagent Parallelization Gym Benchmark (`evals/subagent-bench/`)

An apples-to-apples A/B benchmark suite measuring **single-agent execution (`--no-subagents`, `OA_AGENT_SUBAGENTS=0`)** vs. **parallel subagent delegation (`--subagents`, `OA_AGENT_SUBAGENTS=1`)** on identical models, prompts, fixtures, and deterministic graders.

All task prompts are natural engineering requests with **zero mention of subagents**, so the model decides autonomously when and how to parallelize.

## Tasks (6 Multi-Subsystem Tasks across 2 Tracks)

### Heavy / Decode- & Compute-Bound Track (High Parallelization Payoff)
| Task ID | Modules | Workload |
|---|---|---|
| `sb2_five_plugin_impl` | 5 plugins | Implement 5 independent algorithmic plugins (`csv_norm`, `json_patch`, `log_parser`, `rate_limiter`, `cron_matcher`) from `SPEC.md` + hidden verifier |
| `sb5_four_data_engines` | 4 engines | Implement 4 substantial data/compiler engines (`mini_sql`, `expr_vm`, `diff_engine`, `kv_wal`) from `SPEC.md` + hidden verifier |

### Small / Fast-Test Track (Amdahl's Law & Crossover Control)
| Task ID | Modules | Workload |
|---|---|---|
| `sb1_five_service_bugfix` | 5 packages | Fix 10 subtle bugs across 5 independent service packages (`auth`, `billing`, `inventory`, `notifications`, `analytics`) |
| `sb3_six_service_security_audit` | 6 services | Read-only security audit of 6 microservices producing `AUDIT_REPORT.json` (12 CWE findings) |
| `sb4_five_client_v2_migration` | 5 clients | Migrate 5 API clients + tests from `lib.http_v1` to `lib.http_v2` per `MIGRATION_GUIDE.md` |
| `sb6_five_ci_fuzz_services` | 5 services | Fix 10 bugs across 5 services verified by randomized fuzz/property test suites |

## Usage

```bash
# Run full A/B comparison (Arm A: --no-subagents vs Arm B: --subagents)
python3 evals/subagent-bench/run_ab.py --model gemini-4-neon -j 1 --tag ab

# Run a single task A/B
python3 evals/subagent-bench/run_ab.py --filter sb2_five_plugin_impl --model gemini-4-neon -j 1

# Run directly via tier2/run_tasks.py
python3 evals/openagents-desktop/tier2/run_tasks.py \
  --tasks-dir evals/subagent-bench/tasks \
  --worker oa-worker --model gemini-4-neon -j 1 --subagents
```

See [`docs/verification/2026-11-26-subagent-parallelization-gym-bench.md`](../../docs/verification/2026-11-26-subagent-parallelization-gym-bench.md) for the full round-by-round hillclimb and A/B benchmark report.
