# `coax/lib/lean/` — Lean 4 & Riemann Hypothesis Gym Core

Pure Coax modules powering the OpenAgents Desktop Lean 4 Gym (`Gym -> Lean`),
the `lean-run` CLI (`coder/tui/lean-run.coax`), and the interactive Riemann
Hypothesis Proof Workbench (Epic #13943; `docs/lean/`).

| Module | Role |
| --- | --- |
| `policy.coax` | `#authority` kernel verification & anti-cheat verdict gate (`0` surviving mutants) |
| `wire.coax` | Single canonical wire codec for `lean-doctor.v1`, `lean-verify.v1`, `lean-gym-request.v1`, `lean-gym-state.v1`, `rh-index.v1`, `rh-note.v1` |
| `wire-checks.coax` | Golden byte round-trip & malformed-input refusal checker (`proto/coder/lean-gym-v1.json`) |
| `gym.coax` | Lean Gym request planner (`dc_lean_gym_plan`, exported as `oa_dc_lean_gym_plan`) |
| `unicode.coax` | Lean 4 math Unicode backslash-abbreviation expander (`\forall` -> `∀`, `\to` -> `→`, `\C` -> `ℂ`, ...) |
| `verify.coax` | Real `elan`/`lean`/`lake` doctor, SHA-256 digester, subprocess kernel verifier & `OA-SHOW` JPEG proof card renderer |
| `rh-index.coax` | Pure Coax streaming binary indexer & `< 15 ms` query engine for all 175,187 modules of the Nov 2026 RH proof (#13945) |
| `rh-tour.coax` | Guided 20-step `remark.pdf` tour & persistent local note store (`rh-note.v1`, #13946) |
| `rh-math.coax` | Pure Coax `f64` numerical visualizers: Critical Strip / Hardy $Z(t)$ / Dirichlet $L(\chi, s)$, Robin/Lagarias $\sigma(n)$, $\sqrt{X}$ Theta-Lift Chain (#13946) |
