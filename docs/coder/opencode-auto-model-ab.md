# OpenCode auto model: Cat Jam vs Big Pickle (#11547)

Coder's automatic OpenCode turn runs a free OpenCode Zen model
(`OPENCODE_AUTO_MODEL` in `crates/coder-delegate/src/delegate.rs`). On
2026-10-13 it moved from `opencode/big-pickle` to `opencode/cat-jam-free`,
because of the A/B below.

## Method

- OpenCode 1.18.26 on this Mac, `opencode run --auto -m opencode/<model>`.
  Both models are free.
- Seven small Python bug-fix tasks. Each is a scratch git repo with
  `app.py`, which has a real bug, and `test_app.py`, which fails: slugify
  edges, median of an even list, 1-based pagination, LRU recency, a
  duration parser, interval merging, and word wrap.
- One prompt for every run: run `python3 -m unittest -q`, fix `app.py`
  without changing the tests, and run them again.
- Two runs per task per model, 28 runs in all. Each run gets a fresh copy
  of the repo, the model that goes first alternates by task, and a run times
  out at 300 s.
- Success means the tests pass afterwards and `test_app.py` is unchanged.
  The time is the wall time of the whole `opencode run`.

## Results

| task | Cat Jam r1 | Cat Jam r2 | Big Pickle r1 | Big Pickle r2 |
|---|---|---|---|---|
| slugify | pass 21.5 s | pass 25.6 s | pass 25.4 s | pass 33.8 s |
| median | pass 25.7 s | FAIL 18.3 s | FAIL 18.0 s | pass 196.1 s |
| paginate | pass 26.7 s | pass 16.1 s | pass 33.1 s | pass 26.6 s |
| lru | pass 29.1 s | pass 18.2 s | FAIL 29.0 s | pass 20.9 s |
| parse_duration | pass 37.0 s | pass 18.1 s | pass 36.0 s | pass 22.8 s |
| merge_intervals | FAIL 21.0 s | pass 19.6 s | pass 28.1 s | FAIL 300.1 s (timeout) |
| wordwrap | pass 30.7 s | pass 23.4 s | pass 33.3 s | pass 26.5 s |

| model | passed | median, all runs | mean, passing runs | total time |
|---|---|---|---|---|
| `opencode/cat-jam-free` | 12 / 14 | 22.5 s | 24.3 s | 331 s |
| `opencode/big-pickle` | 11 / 14 | 28.6 s | 43.9 s | 830 s |

The failures:

- **Cat Jam (2):** both times it read the files and then ended its turn
  (exit 0) without editing anything. It never made a wrong fix.
- **Big Pickle (3):** twice the provider stopped the run with
  `Bad Request: {"model":"big-pickle"}`, and once it hung until the 300 s
  timeout. On one more task it passed, but took 196 s. A third
  `Bad Request` came after a run that had already fixed slugify.

## Decision

Cat Jam is at least as accurate and faster, so it is the auto model.
Cat Jam is a stealth model and could be withdrawn, so Big Pickle is the
fallback:

- **Before a turn:** `opencode_auto_model` reads OpenCode's own model
  catalog (`~/.cache/opencode/models.json`, the list OpenCode checks a
  model against before it runs one). If the catalog no longer lists
  `cat-jam-free`, the turn runs Big Pickle.
- **During a turn:** if Coder's automatic OpenCode turn fails before any
  answer with OpenCode's "model gone" error, it runs again on Big Pickle
  once (`opencode_model_gone`). That error is `ProviderModelNotFoundError`,
  or `model_unavailable` / "Model access is disabled" from the provider.
- **On the rail:** the model shows as `OpenCode (Cat Jam)`, or
  `OpenCode (Big Pickle)`.

This is a small sample: seven toy tasks, measured on one day, from one
machine. Run the A/B again if Cat Jam's behaviour changes, or before
choosing between two non-stealth models.
