# Autopilot Codebase Audit (2025-12-22)

Scope
- crates/autopilot (core CLI, daemon, metrics, logs, state, planmode)
- crates/issues (issue/session persistence)
- crates/claude-agent-sdk (process transport)
- related helper modules used by the above

Method
- Static review only (no tests run)
- Focused on correctness, safety, and Rust best practices

Summary
- Initial pass: High 3, Medium 6, Low 5
- Extended pass: High 3, Medium 6, Low 2
- Deeper pass: High 1, Medium 6, Low 2
- Deeper pass 2: Medium 5, Low 3
- Deeper pass 3: Medium 3, Low 3
- Deeper pass 4: Medium 3, Low 3
- Key themes: DB schema mismatches, unsafe process cleanup, panic-on-corrupt metrics, silent log loss, identity/secret handling, missing integrations, stubbed unified interfaces, and AgentGit cache/query gaps

See `docs/logs/20251222/audit1/findings.md` for the initial findings, `docs/logs/20251222/audit1/findings-extended.md` plus `docs/logs/20251222/audit1/coverage-extended.md` for the extended review, `docs/logs/20251222/audit1/findings-deeper.md` plus `docs/logs/20251222/audit1/coverage-deeper.md` for the deeper audit, `docs/logs/20251222/audit1/findings-deeper-2.md` plus `docs/logs/20251222/audit1/coverage-deeper-2.md` for pass 2, `docs/logs/20251222/audit1/findings-deeper-3.md` plus `docs/logs/20251222/audit1/coverage-deeper-3.md` for pass 3, and `docs/logs/20251222/audit1/findings-deeper-4.md` plus `docs/logs/20251222/audit1/coverage-deeper-4.md` for the latest pass.
