---
trigger: glob
globs: "**/*.coax"
description: Coax syntax, compiler error quick-fixes, and #authority / mutation discipline.
---

# Coax Language & Compiler Guardrails

- **Always invoke `./coax/bin/coax`** (never bare `coax`). In a fresh worktree, install via `coax/build.sh` or APFS-clone `cp -c -R /Users/christopherdavid/work/openagents/coax/bin ./coax/bin`.
- **Look up before guessing:** On any unfamiliar compiler error, run `./coax/bin/coax explain CODE` or `./coax/bin/coax vocab NAME` before retrying edits.
- **Top 10 recurring Coax pitfalls (avoid on first write):**
  1. **Half-open ranges (`E0216`):** Always write `0..<n`, never `0..n` or `0..=n`.
  2. **String/slice views (`E0366`, `E0317`):** Explicit `cast(string) buf[0..<n]`; pass slices as `arr[..]` or `arr[0..<n]`; remember `[] const T` vs mutable `[] T` parameters.
  3. **Unused return values (`E0323`):** Discard explicitly with `_ = expr;`.
  4. **Foreign keywords/spellings (`E0225`):** No `let`, `var`, `fn`, `pub`, `null`, `None`, `it`. Use `x := ...`, `foo :: (...) -> T { ... }`, `ifx c then a else b`.
  5. **Struct fields & procedures (`E0315`, `E0351`):** Check the exact struct definition and signature via `rg` before calling — don't guess field or helper names.
  6. **Compile-time step & storage budgets (`E0400`, `E0404`):** Large `#test` loops need `#steps N`; `tool.sh` tools needing more budget take `// tool.sh: run-steps N` (and `// tool.sh: tls` for TLS, `// tool.sh: stack-gib N` for large stacks). Watch giant stack aggregates inside compile-time tests.
  7. **Effect contracts (`E0941`, `E0942`):** Match declared `#effects(...)` cleanly; keep pure logic separate from host effects.
  8. **GPU/Metal threadgroup barriers (`E0700`, `E0702`):** Any thread loop writing a threadgroup array read across threads requires the barrier contract enforced by `coax metal`.
  9. **Literal escapes (`E0105`):** Check valid Coax string escapes before writing regex/JSON string literals.
  10. **Check-first & `#authority`:** Write `#test` before code; run `./coax/bin/coax test FILE --library` (add `--only SUBSTR` during tight loops). On `#authority` files, run `./coax/bin/coax mutate FILE --changed origin/main` to 0 surviving mutants before pushing.
- **For deep error troubleshooting**, load the `coax-pitfalls` skill.
