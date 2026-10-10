# File finding: from an issue to the files its fix needs (#11210)

Status: **work in progress** (first usable version). Numbers below are from
the first end-to-end run and will be replaced as the pipeline improves.

## The tool

`scripts/filefind/filefind.py` takes an issue (number or text) and a
repository path and prints the ranked files the change needs, each with the
stages that found it, a confidence and the reasons.

```sh
export OPENROUTER_API_KEY=...        # embeddings; without it the deterministic stages still run
python3 scripts/filefind/filefind.py index --repo .          # once, then after big pulls (about 1-2 min)
python3 scripts/filefind/filefind.py query --repo . --issue 11210 --k 50 --json
python3 scripts/filefind/filefind.py query --repo . --text "title\n\nbody" --k 50 --json
```

`--json` prints `openagents.filefind.v1`: `files[]` with `rank`, `path`,
`confidence` (the scorer's probability that the fix touches the file),
`stages` (which candidate stages found it) and `reasons`, plus `timing_ms`.
It needs Python 3 with numpy, `git`, and `gh` for `--issue`.

The index lives in `~/.cache/openagents/filefind/<repo>/`: the commit
history (paths per commit, issue references), 256-dim
`text-embedding-3-small` vectors per blob (path plus the first 1,200
characters), per commit subject and per closed issue, and a SQLite token
index (identifiers and definitions per blob). It holds no file contents.
`query` refreshes missing blobs on its own.

## Pipeline

Stage 1, candidates (all lookups in the precomputed indexes, except one
embedding call for the issue text):

| Stage | What it adds |
|---|---|
| emb | the 300 files whose path+head embedding is nearest the issue |
| sym | paths, file names, crate names, identifiers and quoted literals the issue names, resolved through the token index (mentions and definitions) |
| co | files that changed together with the seed files (top emb + sym) in history before the fix |
| sim | the fix files of the 12 most similar past closed issues |
| hist | files touched by the 150 past commits whose subject is most similar to the issue |
| pair | tests, `mod`/`lib`/`main`, `Cargo.toml`, README/INVARIANTS of the seeds |
| dir | the other files in a seed's directory |
| recent | the files touched most in the last 400 commits |

Stage 1 scorer: a small gradient-boosted tree model over about 50
features per candidate, trained on 400 past issue -> fix pairs.

Stage 2, propagation: co-change and paired files of the stage-1 top 10,
and how much of the stage-1 mass sits in each file's crate and directory;
a second model re-ranks.

## Bench

`scripts/bench/file-finding-dataset.py` builds the cases: closed issues
fixed by exactly one commit on main whose subject carries `(#N)` and no
other issue, at most 25 hand-written files. 500 cases (#5311..#11164); the
newest 100 are the bench, the 400 older train the scorer. Each case is
replayed at the fix's parent commit: tree, history, similar issues and
commit subjects are all cut there.

Ground truth is the files the fix changed. Lockfiles and generated files
(`Cargo.lock`, `tree.json`, files with a generated header) are **derived**
and reported separately. Hand-written files the fix *added* cannot be found
in the parent tree, so recall is reported over the existing files and over
all hand-written files.

`scripts/bench/file-finding-bench.py prepare|features|train|eval` runs it.

### First results (100 bench cases, 710 existing hand-written files, 179 added)

Stage recall alone and in union (existing files):

| Stage | Recall | Median candidates |
|---|---:|---:|
| emb | 0.496 | 300 |
| sym | 0.352 | 23 |
| co | 0.775 | 361 |
| sim | 0.520 | 92 |
| hist | 0.642 | 122 |
| pair | 0.166 | 23 |
| dir | 0.476 | 186 |
| recent | 0.437 | 150 |
| union | 0.941 | 912 |

Shortlist after stage 2:

| Shortlist | Recall (existing) | Recall (all hand-written) | Precision |
|---|---:|---:|---:|
| top 20 | 0.537 | 0.429 | 0.191 |
| top 50 | 0.738 | 0.589 | 0.105 |
| top 100 | 0.844 | 0.674 | 0.060 |

Latency per query (median): index load 0.1 s, tree 0.06 s, issue embedding
0.5-0.6 s (network), all candidate stages and scoring about 0.25 s.
