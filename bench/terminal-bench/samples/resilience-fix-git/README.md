# Sample: failure demonstrations on fix-git

Sanitized evidence from the resilience trials of 2026-09-22 on the x86_64
NixOS host: one directory per failure case, each a real Harbor trial over
`archive/fix-git` or a run the harness refused before Harbor started.
[Terminal-Bench resilience](../../../../docs/terminal-bench/resilience.md)
explains each case, what the harness did, and the terminal status.

Every trial ran a contract probe from `tests/fixtures/` (or the `oracle`
control) and spent no inference. Job names start with `resilience--`.

## Contents

- `<case>/attempt.json` and `<case>/manifest.json`: the attempt record and
  episode manifest the harness wrote.
- `<case>/evidence/`: Harbor's trial output and the collected episode
  bundle.
- `<case>/refusals/`: the refusal record of a run refused before Harbor
  started.
- `<case>/transcript/`: the shell session that drove the case, including
  the leftover checks.
- `resume/trials/` and `resume/interrupted/`: the three trials of the
  resumed job and the interrupted trial the harness preserved.
- `before-fix/`: what the same cases did before the harness changes.
- `image-state.txt`: the trial-log evidence behind a cold and a warm
  image state.
- `compare.txt`: `tbench compare --arm coder-v05 --arm oracle` over these
  jobs.
- `leftovers-after-all-cases.txt`: no container, network, or process from
  any resilience trial remained.

Local paths are rewritten to `<jobs-dir>`, `<upstream>`, `<package>`,
`<site-packages>`, `<python>`, `<scratch>`, and `<tmp>` placeholders. The
directory was scanned for every credential on the host before commit.
