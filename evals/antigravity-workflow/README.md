# Antigravity Workflow Benchmark (`evals/antigravity-workflow/`)

A curated 8-task OpenAgents-specific engineering benchmark mined directly from real Antigravity development chats (`597403ca`, `ba6d40e0`, `fe80b00c`, `099f2a5a`, `a5a91943`, `24d8752e`) between Nov 23–25, 2026.

Unlike generic coding benchmarks, every task tests a recurring workflow specific to OpenAgents development:

| Task ID | Workflow Pillar | Real Chat / Commit | Key Verification Gate |
|---|---|---|---|
| `wf_001_agy_default_effort_high` | Cross-layer Coax + Swift config hunt | `597403ca` / `7d0555b496` | 14 files across Coax + Swift + desktop checks |
| `wf_002_agy_sidebar_icons_12pt` | Desktop UI icon atlas & `--minimal` parity | `24d8752e` / `9a0b2b8aa2` | `nav.coax` + `chats` + `app-v1.coax` minimal checks |
| `wf_003_agy_authority_cache_expiry` | `#authority` Coax + mutation testing | `ba6d40e0` / `a583dd7e53` | **`coax mutate --root-only` (0 surviving mutants)** |
| `wf_004_agy_inky_tool_json_leak` | Cross-language Coax + C Psionic inference | `fe80b00c` / `e7c3a80ae8` | `inky/loop.coax` + `inkling-serve.c` native Metal build |
| `wf_005_agy_stall_watchdog_steer` | Multi-file worker runtime concurrency | `099f2a5a` / `6a1acd6ea6` | `cursor.coax` + `antigravity.coax` + `agyworker.coax` |
| `wf_006_agy_flawed_turn_invariants` | Strict wire-protocol state invariants | `a5a91943` / `4a246e6029` | `flawedworker.coax` structural sanitizer & wire guards |
| `wf_007_agy_shared_core_c_abi` | `#authority` Coax -> C ABI export | `ba6d40e0` / `6d0966f4de` | `client-core.coax` + `clang -Wall -Werror` C bridge |
| `wf_008_agy_desktop_sidebar_polish` | Multi-file Desktop UX & keyboard wiring | `ba6d40e0` / `128d4a64d4` | `nav.coax` + `sidebar` checks + `app-v1.coax` |
