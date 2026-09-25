# `verify.executed` fixtures

`instruction.md` is a synthetic task, not a benchmark task. `untouched/`
is its workspace before any session: a Python package, `tally`, whose
mean is wrong, and no `report.sh`. Each directory under `candidates/`
holds only the files a session changed; the tests in
`crates/coder-one/src/checks/contract/executed_tests.rs` lay it over a
copy of `untouched/` and run the post-session rule on it.

| Candidate | What it changed | Verdicts | Rejected |
| --- | --- | --- | --- |
| `no-regression` | Fixed the mean | `ok` for every command that ran before | No |
| `crash` | Divides by zero | `python3 -m tally data/numbers.txt` regressed | Yes |
| `import-failure` | Imports a module that doesn't exist | `import tally` and the module run regressed | Yes |
| `unrunnable` | Only its notes; still no `report.sh` | `sh report.sh` isn't a regression: it failed before too | No |
| `timeout` | Hangs | `unknown` for the module run; a timeout isn't a regression | No |
