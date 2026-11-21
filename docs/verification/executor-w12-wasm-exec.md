# Executor W1.2 Verification Record (`hull-v2`, `wasm-exec-v1`, & Real Benchmarks)

- **Issue:** GitHub `#13915` (Executor W1.2)
- **Master Spec:** [executor-network-master-spec.md](file:///Users/christopherdavid/work/openagents/psionic/docs/executor-network-master-spec.md)
- **Date:** 2026-11-21
- **Profile / Artifact:**
  - `profile_id`: `wasm-core-i32-v1`
  - `compiler_hash` (`wm_compiler_digest`): `9958f03019b7c6c03548c2a2be26326911fe45b1bc2d852d9145b2ac0576b87c`
  - `executor_artifact_hash` (F64 safetensors SHA-256): `1342be5ffc18ddb4af121c797489770d21a24899b2cb63c2294696d9f7bfe579`
  - Model dimensions: `width = 149`, `n_layers = 93` (3 Attention waves / 13 heads + 90 ReGLU FFN layers), `n_weights = 784,485`, `kv_slots = 55`, `mem_words = 512` (2,048 bytes), `323` F64 tensors, `6,302,816` bytes serialized safetensors.

## 1. Deliverables Implemented

| File | Role | Authority / Mutation |
| --- | --- | --- |
| `psionic/lib/hull-kv.coax` | Balanced AVL-tree `O(log t)` parabolic hull memory (`hull-v2`), segmented wide keys (`±2^30`), and `O(log K)` KV-cache forward engine (`ev_hull_forward_token`) for any loaded `exec-vm-v1` model | `#authority`, 689 mutants, 666 killed, **0 survived**, 23 marked equivalent |
| `psionic/lib/wasm-isa.coax` | Percepta-aligned 38-opcode deterministic `i32` WASM ISA (`wasm-core-i32-v1`): opcodes, typed traps, module validator & ROM resolver, deterministic CPU reference interpreter (`wm_run_ref`), pinned receipts, and extended Ch 234 replay verifier (`wm_replay_check`) | `#authority`, 1,073 mutants, 1,066 killed, **0 survived**, 7 marked equivalent |
| `psionic/lib/wasm-transformer.coax` | Unified `exec-vm-v1` dense F64 weight compiler (`wm_build_graph`, `wm_compile_vm`) for all 38 opcodes + traps, autoregressive dense-weight runners (`wm_run_dense_hull` on `hull-v2` + `wm_run_dense_scan` on `O(t)` scan cross-check), safetensors SHA-256 round-trip, and Ch 230 differential fuzzing | `#authority`, 1,116 mutants, 1,076 killed, **0 survived**, 40 marked equivalent |
| `psionic/lib/wasm-transformer-benchmarks.coax` | Real non-toy benchmark suite: 9×9 Sudoku MRV bitmask backtracker (4 puzzles, 45–53 blanks, up to 77 real backtracks), 10×10 Kuhn–Munkres Hungarian solver with LP strong-duality certificates (4 held-out matrices), and real Coder Ch 8 `PgProgram` AST -> `wasm-core-i32-v1` lowerer (4 programs) | Verified debug & release |
| `psionic/examples/wasm-exec.coax` | End-to-end runnable showcase on default 8 MiB macOS stack: 10,000-op `hull-v2` vs `O(t)` linear scan parity, 1,048,576-op `hull-v2` wide-key benchmark, and all 12 real workloads with side-by-side timings | Runs via `coax/bin/coax run psionic/examples/wasm-exec.coax --release` |
| `psionic/tools/wasm-exec-artifact.coax` | Coax tool (`// tool.sh: release`, `stack-gib 1`): benchmarks `1,048,576` **distinct** keys (sequential & wide `±2^30` random), serializes F64 safetensors to disk, reloads with SHA-256 verification, and executes all 38 opcodes + 12 real workloads through disk-loaded weights | Passes `tools-check` |
| `psionic/tests/wasm_exec_oracle.py` | Independent Python oracle (`# Firm reason: 2 (independent oracle)`): loads the actual F64 safetensors file off disk, runs pure dense matmul + parabolic attention + ReGLU (zero interpreter fallback) on all 38 opcodes, 10 typed traps, and 4 Coder Ch 8 programs, and independently verifies all 4 9×9 Sudoku solutions, all 4 10×10 Hungarian optima, and bit-exact trace digests | Passes `python3 -I psionic/tests/wasm_exec_oracle.py` & `tools-check` |

Total `#authority` mutation score across all three W1.2 authority modules: **2,878 mutants, 2,808 killed, 0 survived, 70 explicitly proven mathematical equivalents**.

## 2. Balanced-Tree `O(log t)` Parabolic Hull (`hull-v2`) Scaling

### Parity vs. `O(t)` Linear Scan (`exact-memory.coax`)
Measured via `coax/bin/coax run psionic/examples/wasm-exec.coax --release`:
- **10,000 randomized ops** (`2,048` writes/overwrites, `2,069` hits, `5,883` misses): **0 divergences**, AVL height = `13`, `hull-v2` compares = `101,582` vs. linear scan compares = `9,995,264` (**98.4× reduction**).

### 1,048,576-Key Scaling Benchmarks
Measured via `coax/web/tool.sh psionic/tools/wasm-exec-artifact.coax` (1,048,576 **distinct** nodes simultaneously resident in the AVL tree, zero hidden heap allocation) and `psionic/examples/wasm-exec.coax --release`:

| Workload | Distinct Keys | Total Writes / Reads | AVL Height | Write Time | Write Avg Cmp | Rotations | Read Time | Read Avg / Max Cmp |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Sequential distinct (`[-524288..+524287]`, worst-case BST input) | 1,048,576 | 1,048,576 / 1,048,576 | **21** | 65 ms (16.13M/s) | 19 | 1,048,555 | 378 ms (2.77M/s) | **19 / 20** |
| Randomized wide segmented (`±2^30`, stride 1999) | 1,048,576 | 1,048,576 / 1,048,576 | **23** | 112 ms (9.36M/s) | 18 | 319,975 | 199 ms (5.27M/s) | **19 / 23** |
| Default-stack 131k active wide segmented (`±2^30`) | 131,072 | 1,048,576 / 1,048,576 | **18** | 48 ms (21.82M/s) | 16 | 131,054 | 108 ms (9.65M/s) | **16 / 18** |

## 3. Real Non-Toy Benchmark Suite — Honest Side-by-Side Timings

All 12 workloads compiled to `wasm-core-i32-v1` and executed **100% through the loaded dense F64 weights + `hull-v2` parabolic attention** (zero fallback to the reference interpreter), verified bit-for-bit against the Coax CPU reference interpreter (`wm_run_ref`), extended Ch 234 replay (`wm_replay_check`), AND the independent Python oracle (`wasm_exec_oracle.py`).

Measured on idle Apple Silicon macOS via `coax/bin/coax run psionic/examples/wasm-exec.coax --release`:

| Workload | Details / Optimality Proof | WASM Steps | CPU Ref (`steps/s`) | Loaded Dense F64 + `hull-v2` (`steps/s`) | `hull-v2` Compares | `O(t)` Linear Scan Compares | Attention Reduction | Trace Digest Match & Replay |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `sudoku-0-wiki-51blanks` | 9×9, 51 blanks, unique solution verified | 60,715 | 2,474,325 | 4,984 | 4,140,334 | 24,224,252,845 | **5,850×** | 100% bit-exact (`exact`) |
| `sudoku-1-med-45blanks` | 9×9, 45 blanks, unique solution verified | 31,815 | 2,458,275 | 4,993 | 2,222,571 | 6,719,677,965 | **3,023×** | 100% bit-exact (`exact`) |
| `sudoku-2-bt10-51blanks` | 9×9, 51 blanks, **10 real backtracks** | 72,042 | 2,511,084 | 5,001 | 4,973,209 | 34,047,661,557 | **6,846×** | 100% bit-exact (`exact`) |
| `sudoku-3-bt77-53blanks` | 9×9, 53 blanks, **77 real backtracks** | 159,886 | 2,445,832 | 4,211 | 11,366,817 | 166,851,993,191 | **14,678×** | 100% bit-exact (`exact`) |
| `hungarian-0-pos-dense` | 10×10, `opt_cost = 195`, LP dual verified | 24,114 | 1,985,802 | 4,070 | 1,311,329 | 3,887,960,505 | **2,964×** | 100% bit-exact (`exact`) |
| `hungarian-1-signed-mixed` | 10×10, `opt_cost = -431`, LP dual verified | 18,236 | 2,083,542 | 3,998 | 990,884 | 2,243,493,018 | **2,264×** | 100% bit-exact (`exact`) |
| `hungarian-2-ties-bottleneck` | 10×10, `opt_cost = 78`, LP dual verified | 13,485 | 1,805,801 | 4,190 | 731,386 | 1,242,561,840 | **1,698×** | 100% bit-exact (`exact`) |
| `hungarian-3-wide-skewed` | 10×10, `opt_cost = -18797`, LP dual verified | 18,025 | 1,973,655 | 3,968 | 979,259 | 2,192,813,350 | **2,239×** | 100% bit-exact (`exact`) |
| `coder-0-pg-one-den-on` | `PG_ONE` (`den=1`), `defined=7`, `world=[0,1,0]` | 110 | 1,693,349 | 4,284 | — | — | — | 100% bit-exact (`exact`) |
| `coder-1-pg-one-den-off` | `PG_ONE` (`den=0`), `defined=3`, `world=[1,0,0]` | 97 | 1,671,261 | 4,292 | — | — | — | 100% bit-exact (`exact`) |
| `coder-2-pg-two-room-toggle` | `PG_TWO`, `defined=112`, `world=[1,1,0]` | 41 | 1,489,825 | 4,224 | — | — | — | 100% bit-exact (`exact`) |
| `coder-3-loop-branch-arith` | Loop + div + set + get + if, `defined=31` | 156 | 1,784,080 | 4,353 | — | — | — | 100% bit-exact (`exact`) |
| **Aggregate (12 workloads)** | **0 divergences, 100% Ch 234 replay exact** | **398,722** | **2,355,552** | **4,466** | **26,734,789** | **241,411,136,512** | **9,029×** | **12 / 12 exact** |

### Honest Speed Interpretation
- On a single CPU thread, the native CPU WASM interpreter (`wm_run_ref`) runs at **~2.36M steps/s**, while executing the exact same 38-opcode VM autoregressively through the **784,485 dense F64 transformer weights** (93 layers, width 149) with `hull-v2` parabolic attention runs at **~4,466 steps/s** (~527× slower than a native CPU interpreter, as expected for single-threaded dense F64 CPU matmuls before GPU batching/quantization in W1.4).
- Crucially, **`hull-v2` eliminates the `O(t^2)` KV-cache wall**: on the 159,886-step `sudoku-3-bt77-53blanks` puzzle (77 recursive backtracks), `hull-v2` stays flat at **4,211 steps/s** (`11.37M` attention comparisons) instead of collapsing under `166.85B` `O(t)` linear-scan comparisons (**14,678× fewer comparisons**).

## 4. Independent Python Oracle Verification

```text
python3 -I psionic/tests/wasm_exec_oracle.py
wasm_exec_oracle.py PASS: artifact=1342be5ffc18ddb4... (6302816 bytes, 323 F64 tensors, width=149, layers=93); hull-v2 1,048,576 distinct keys seq(h=21) & wide +/-2^30 rnd(h=23); pure-Python dense F64 weights verified 38 opcodes + 10 traps + 4 Coder Ch 8 programs; independent solvers & trace digests verified 4x 9x9 Sudoku & 4x 10x10 Hungarian (0 divergences).
```
