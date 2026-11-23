---
trigger: always_on
description: Core anti-friction execution, verification, worktree, and context hygiene for OpenAgents.
---

# Core Execution Hygiene

- **Canonical ground truth first:** Before editing or auditing, run `git fetch origin main` and inspect canonical files via `origin/main` (never trust a lagging local main checkout). Never edit `/Users/christopherdavid/work/openagents`; work in a fresh worktree (`git worktree add ../oa-<topic> origin/main`) and remove it when done.
- **No polling, ever:** Never loop on `manage_task(status)` or `sleep N`. Let background commands wake you reactively, or do parallel work while they run.
- **Hard timeouts + watchdogs:** Every live test, server probe, or subprocess MUST carry an explicit OS/script timeout (`timeout 60` or `select`/deadline). Never yield on an unbounded background test without a `schedule` watchdog timer.
- **Bounded output & targeted reads:** Pipe verbose shell commands (`| head -n 80`, `--stat`); prefer `rg`/`git grep` over dumping whole files; never re-read unchanged line ranges in the same context window.
- **Real E2E verification before claiming done:** Compile checks, unit tests, and static PNGs are necessary, not sufficient. Verify the actual installed artifact end-to-end (real UI clicks/inputs, live RPCs against real user state, cold `req#1` streaming). Never hide user-requested core flows behind feature flags unless explicitly asked.
- **Safe probes:** Always set `SSH_CONNECTION=1` when running diagnostic `agy` / OAuth commands so macOS `open` never hijacks Chrome.
- **Clarify upfront, ship completely:** Use structured `ask_question` upfront for genuine ambiguity instead of guessing or appending trailing `"Would you like me to...?"` offers. Include `Co-Authored-By:` and `Agent: <id>` on every commit; verify `git show --stat` before pushing.
- **Long-session hygiene:** Keep a compact scratch ledger (`*_log.md`/`.json`) so compactions lose zero state. After ~4–6 compactions or a shipped milestone, emit a concise handoff summary and recommend a fresh chat.
