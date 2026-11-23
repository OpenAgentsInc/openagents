---
name: coax-pitfalls
description: >-
  Use when diagnosing Coax compiler errors (E0105, E0216, E0225, E0315, E0317,
  E0351, E0366, E0381, E0400, E0404, E0406, E0700, E0702, E0941, E0942),
  configuring coax/web/tool.sh directives, or running #authority mutation checks.
---

# Coax Pitfalls & Fast Troubleshooting

## First 10 Seconds on Any Error
Do **not** guess across 3+ compile-fail turns. Run the compiler's own built-in lookup first:

```sh
./coax/bin/coax explain E0366
./coax/bin/coax vocab <keyword-or-stdlib-name>
```

## Most Frequent Transcript Error Codes

| Code | Cause | Exact Fix |
| --- | --- | --- |
| `E0216` | Closed range (`0..n` or `0..=n`) | Always use half-open `0..<n`. |
| `E0225` | Foreign spelling (`let`, `fn`, `var`, `pub`, `it`, `null`) | Use Coax syntax (`x := ...`, `f :: (...) -> T`, `ifx c then a else b`). |
| `E0315` / `E0351` | Unknown procedure / struct field | `git grep -n "Name ::"` to verify exact spelling & signature before editing. |
| `E0317` / `E0366` | Slice/view/mutability mismatch | Pass `arr[..]` or `arr[0..<n]`; cast `cast(string) buf[0..<n]`; respect `[] const T` vs `[] T`. |
| `E0323` | Ignored return value | Assign explicitly: `_ = call(...);`. |
| `E0400` / `E0404` | Compile-time step or aggregate storage budget exceeded | Add `#steps 100000000` on large `#test` blocks; avoid giant stack arrays inside compile-time tests; add `// tool.sh: run-steps 250000000` for tools. |
| `E0406` | `#assert` failed in `#test` | Isolate single test with `./coax/bin/coax test FILE --library --only "SUBSTR"`. |
| `E0700` / `E0702` | GPU threadgroup array read/write barrier rule | Insert the required threadgroup barrier between writes and cross-thread reads in `coax metal` kernels. |
| `E0941` / `E0942` | `#effects(...)` contract mismatch | Keep pure logic effect-free; only call host effects inside procedures declaring them. |
| `E0105` | Invalid string escape | Verify valid escape sequences (`\n`, `\r`, `\t`, `\"`, `\\`, `\0`). |

## `coax/web/tool.sh` File-Header Directives
Place at the top of a `.coax` tool entry file (parsed via `sed`/`grep` by `coax/web/tool.sh`):
- `// tool.sh: tls` — link TLS native runtime (`https://`, `wss://`).
- `// tool.sh: run-steps N` — raise compile-time evaluation step budget.
- `// tool.sh: release` — emit release C (`--release`, no checked-arithmetic traps).
- `// tool.sh: stack-gib N` — raise `tool_main` stack from 1 GiB to `N` GiB.
- `// tool.sh: objc FILE.m FRAMEWORK...` — link macOS Objective-C helper.

## Fresh Worktree & Mutation Workflow
1. Ensure `./coax/bin/coax` exists (`coax/build.sh` or `cp -c -R /Users/christopherdavid/work/openagents/coax/bin ./coax/bin`).
2. Tight inner loop: `./coax/bin/coax test FILE --library --only "SUBSTR"`.
3. Full file suite: `./coax/bin/coax test FILE --library`.
4. For `#authority;` files: `./coax/bin/coax mutate FILE --changed origin/main` -> zero surviving mutants (mark true equivalents `// mutant: equivalent // why`).
5. Apply the **3-round rule**: after 3 failed rounds on the same error, stop tweaking and re-read the compiler diagnostic / spec (`coax/spec.md`) or change approach.
