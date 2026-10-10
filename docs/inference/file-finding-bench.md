# File finding: from an issue to the files its fix needs (#11210)

**Goal.** Given an issue and a repository, list the files the fix needs
within about a second, using the repository's full commit history. Most of
the work should be deterministic lookups in indexes built ahead of time. A
model (Jev or a planner) should be called only when that helps.

**Result, in one line.** Over 100 closed issues, replayed at each fix's
parent commit, the finder's top 100 files hold **84%** of the files the fix
edited that already existed. The top 200 hold **91%**. A query takes about
0.8–1.3 s, and 0.45–0.55 s of that is the one embedding call. Adding Jev and
a planning model raises the top-100 figure to 87%. **That is still well
short of the 98% target.** Twenty percent of the hand-written files in a fix
are *new* files, which no finder can retrieve. The last section says what to
build next.

## The tool

`scripts/filefind/filefind.py` (Python 3 with numpy, `git`, and `gh` for
`--issue`; nothing is compiled):

```sh
export OPENROUTER_API_KEY=...                      # embeddings; without it the deterministic stages still run
python3 scripts/filefind/filefind.py index --repo .                         # first run 1-2 min, then seconds
python3 scripts/filefind/filefind.py query --repo . --issue 11210 --k 100 --json
python3 scripts/filefind/filefind.py query --repo . --text "title

body" --k 100 --json
```

`--json` prints `openagents.filefind.v1`. Each entry in `files[]` has:

- `rank` and `path`;
- `confidence`: the scorer's probability that the fix touches the file;
- `stages`: which candidate stages found it;
- `reasons`: for example "issue names `upstream.rs`", "defines `guard`",
  "changes with the seeds (co 1.49)", "fixed with similar #10128",
  "uses `X` defined in `Y`".

`timing_ms` gives the time per stage. Without `--json` the same list is
printed one line per file. The briefing builder (#11211) consumes this
output.

The index lives in `~/.cache/openagents/filefind/<repo>/`, about 0.8 GB for
this repository with 1,700 replayed revisions. It holds no file contents:

- `history.pkl`: every commit's paths, plus issue references from commit
  subjects.
- `blobs.npz`: one 256-dimensional `text-embedding-3-small` vector per blob,
  made from the path plus the first 1,200 characters. Vectors are keyed by
  blob, so any revision can be scored.
- `commits.npz`: one vector per commit subject.
- `issues.pkl`: one vector per closed issue.
- `tokens.sqlite`: identifiers and defined names per blob.

`query` embeds any blobs and commits it has not seen before, so a pull costs
a few seconds at most.

## Pipeline

**Stage 1: candidates.** Every stage below is a lookup, except the one
embedding call for the issue text, which runs in parallel with the token
lookups.

| Stage | What it adds |
|---|---|
| emb | The 300 files whose path+head vector is nearest the issue. |
| sym | Things the issue names, resolved through the token index: paths, file names, crate names, identifiers (files that define them and files that mention them, weighted by rarity) and quoted literals (checked to occur exactly). |
| co | Files that changed together with the seed files (top emb + sym), in history before the fix. Commits touching more than 40 files are ignored. |
| sim | The fix files of the 12 most similar past closed issues. |
| hist | The files touched by the 150 past commits whose subject is most similar to the issue. |
| pair | For each seed: tests, `mod.rs`/`lib.rs`/`main.rs`, `Cargo.toml`, README/INVARIANTS, the crate's `tests/`. |
| dir | The other files in each seed's directory. |
| recent | The files touched most in the last 400 commits. |

**Stage 1 scorer.** A gradient-boosted tree model (numpy, 200 depth-3
trees) scores every candidate on 57 features. The features are each
source's score, the score relative to the query's best, the rank within the
pool, file kind, churn and recency. It is trained on 1,484 past issue → fix
pairs, all older than the bench.

**Stage 2: propagation.** From the stage-1 top 10, stage 2 adds:

- co-change with those files;
- their paired files;
- the reference graph: files that use a name a top file defines, when
  fewer than 25 files use it;
- each file's share of stage-1 confidence in its crate and in its
  directory.

A second model (70 features) re-ranks the result. Its training inputs come
from two folds cross-fitted on stage 1, so it never sees stage-1 scores the
first model was trained on.

**Optional model stages** (bench only; see the results):

- *Jev* re-judges ranks 30–150: one batched Noul per file, over the issue
  plus the file's first 1,500 characters.
- *Plan*: Claude Sonnet 5.5 reads the issue, the top 60 paths and the file
  lists of the four strongest crates. It writes the change plan as steps,
  each naming the files it edits or creates.

## Bench

`scripts/bench/file-finding-dataset.py` builds the cases. It takes closed
issues whose fix commits are findable on main from the commit subject
(`(#N)`, `fixes #N`).

- **Eval set:** the newest 100 issues fixed by exactly one commit that
  names no other issue, with at most 25 hand-written files. They run from
  #10194 to #11204 (Sept 26 – Oct 9).
- **Training set:** 1,484 older issues, including issues fixed over several
  commits. Their ground truth is the union of those commits' files. Every
  training fix landed before the oldest eval fix.

Each case is replayed at the fix's parent commit. The tree, co-change
history, similar issues and commit subjects are all cut there, so nothing
from the fix or later can leak in.

Ground truth is the set of files the fix changed:

- **Derived files** are counted separately: lockfiles, `tree.json`, OpenAPI
  documents, and files with a generated header.
- **Hand-written files** the fix *created* do not exist at the parent. So
  recall is given two ways: over existing files (what a finder can return)
  and over all hand-written files.

The eval set has 702 existing hand-written files, 172 added hand-written
files and 33 derived files, about 8.5 hand-written files per fix. Run it
with:

```sh
B=scripts/bench/file-finding-bench.py; A="--repo . --dataset $D --work $W"
python3 $B prepare $A; python3 $B features $A; python3 $B train $A; python3 $B eval $A
python3 $B judge $A; python3 $B judge-eval $A; python3 $B plan $A --limit 50; python3 $B check $A
```

### Stage 1 recall, each stage alone and their union (existing hand-written files)

| Stage | Recall | Median candidates per issue |
|---|---:|---:|
| emb | 0.497 | 300 |
| sym | 0.319 | 18 |
| co | 0.775 | 355 |
| sim | 0.500 | 92 |
| hist | 0.637 | 124 |
| pair | 0.164 | 23 |
| dir | 0.476 | 171 |
| recent | 0.426 | 150 |
| stage-2 additions | 0.023 | 74 |
| **union** | **0.960** | **959** |

Co-change is the strongest single source, and the history-based sources
(co, hist, sim) carry most of the recall. The union reaches 96%, so the
remaining loss is mostly in ranking.

### Shortlist recall and precision (100 eval issues)

| Pipeline | @20 | @50 | @100 | @200 | Precision @20 / @50 / @100 |
|---|---:|---:|---:|---:|---|
| stage 1 scorer | 0.504 | 0.725 | 0.815 | 0.879 | 0.177 / 0.102 / 0.057 |
| **stage 1 + stage 2 (shipped)** | **0.531** | **0.741** | **0.842** | **0.912** | 0.186 / 0.104 / 0.059 |
| + Jev on ranks 30–150, every query | 0.531 | 0.793 | 0.860 | 0.915 | 0.186 / 0.111 / 0.060 |
| + Jev only when unsure (63% of queries) | 0.531 | 0.783 | 0.849 | 0.916 | 0.186 / 0.110 / 0.060 |
| plan files first, then finder + Jev (50 issues)* | 0.573 | 0.800 | 0.866 | 0.919 | |

Recall is over existing hand-written files. Over all hand-written files,
including the ones the fix created, the shipped finder scores 0.427 / 0.595
/ 0.676 / 0.732.

\* On those 50 issues the finder alone scores 0.542 / 0.737 / 0.842 / 0.902.
The plan names a median of 28 files, and those 28 alone hold 69% of the
existing files. Of the 107 files the fixes created, it names 25 by their
exact path. Plain union (finder top *k* plus every plan file) gives 0.742 /
0.797 / 0.857 / 0.912.

**When a model is called.** The query is "unsure" when the scorer's own
confidence mass in its top 50 is under 70% of its total mass. That is true
for 63 of 100 queries. Calling Jev on only those queries keeps about 80% of
Jev's gain at 50 files. Jev judges 120 files per query in 0.7 s (five
parallel batches of 25, about 48k input tokens) for about $0.002. The planner takes about 8 s and
about $0.03 per issue (8k tokens in, 1k out). At a threshold of 60%, only
14% of queries call Jev, and the gain mostly disappears.

**Derived files.** `Cargo.lock` changed in 20 fixes. The rule "a
`Cargo.toml` is in the top 50" predicts all 20. Generated files changed in
13 fixes; in 11 of them the file's directory is among the top-50 files'
directories.

### Which kinds of file were missed (existing hand-written files, shipped finder)

| Kind | Files | Missed @20 | Missed @50 | Missed @100 |
|---|---:|---:|---:|---:|
| Rust source | 366 | 163 | 95 | 58 |
| docs (.md) | 82 | 32 | 19 | 16 |
| Rust test | 81 | 48 | 26 | 14 |
| `lib.rs` / `main.rs` / `mod.rs` | 80 | 30 | 14 | 5 |
| README / INVARIANTS / AGENTS | 30 | 15 | 11 | 6 |
| `Cargo.toml` | 27 | 16 | 8 | 5 |
| script | 9 | 6 | 2 | 2 |
| Swift / Kotlin | 9 | 9 | 3 | 2 |
| web / shader | 6 | 4 | 2 | 2 |
| fixture / data | 5 | 5 | 2 | 1 |
| other | 7 | 1 | 0 | 0 |

The 110 existing files missed at 100 break down by crate:

- 30 sit in one of the three crates that dominate the top 20;
- 51 sit in a crate that has some other file in the top 100;
- 29 sit in a crate the list never touches.

The typical miss is a second implementation of a changed interface (another
provider backend, the phone and desktop clients of a changed route) or a
sibling crate swept up by a refactor. Of the 172 files fixes *created*, 116
went into a directory that already existed, and 102 of those directories
appear among the top-50 files' directories.

### Latency per stage (live `query`, this Mac, median)

| Stage | ms |
|---|---:|
| load indexes | 120 |
| `git ls-tree` | 75 |
| token stages (sym) | 200–430 |
| issue embedding (overlaps sym) | 450–550 |
| emb, co, sim, hist, pair, recent | about 45 together |
| stage 2 (incl. reference graph) | 140–210 |
| two scorers | 170–240 |
| **total** (issue text in hand) | **770–1,250** |

Fetching the issue with `gh` adds 0.4–0.6 s.

### Fixes that landed after the model was trained

Eight issues still open on project 22 already have fixes on main, landed
after the model's training data. Running the shipped finder at each fix's
parent gives an out-of-sample check. These fixes are larger than the bench
median: 4–19 hand-written files, several spread over multiple commits.

| Issue | Existing files | @20 | @50 | @100 | @200 |
|---|---:|---:|---:|---:|---:|
| #11156 one error/list shape | 15 | 4 | 5 | 8 | 10 |
| #11158 app routes under /v1 | 17 | 6 | 9 | 12 | 13 |
| #11159 owned routes | 14 | 5 | 7 | 10 | 13 |
| #11160 key scopes | 6 | 5 | 5 | 6 | 6 |
| #11177 background work | 14 | 6 | 9 | 11 | 13 |
| #11182 account memory (5 commits) | 16 | 5 | 7 | 9 | 10 |
| #11132 provider fallback note | 15 | 2 | 3 | 5 | 8 |
| #11134 account export | 3 | 2 | 2 | 2 | 3 |
| **all** | **100** | **0.35** | **0.47** | **0.63** | **0.76** |

Two examples of misses:

- **#11132** missed the CLI's output paths (`coder/src/turn.rs`,
  `headless.rs`, `relay.rs`) and the NIP-CJ files. The issue names neither.
- **#11182** missed the phone app's account link and the chat router. The
  issue says "web chat reads them" and "apps".

## Open issues on project 22: is the list comprehensive?

For ten open issues, a separate agent read the code and wrote down the files
each complete change needs. It did this without seeing the finder's list.
The finder's list was then taken at current main, and the table counts how
many of those reference files it holds. Where part of an issue has already
landed, the reference covers only the remaining work.

| Issue | Reference files | @20 | @50 | @100 | @200 | Assessment |
|---|---:|---:|---:|---:|---:|---|
| #11193 split the router decision for Clef | 19 | 10 | 16 | 19 | 19 | **Comprehensive by 100.** Everything in `coder/src/router/*`, the worker, goldens, evals, the Clef docs and INVARIANTS is there. |
| #11159 own `/coder/*` | 9 | 6 | 8 | 9 | 9 | **Comprehensive by 100.** `upstream.rs` is #1. `route_owners_tests.rs`, `coder_sync.rs`, `account_memory.rs`, design.md and the deployment doc are all present. |
| #11134 account export, CLI part | 12 | 5 | 8 | 10 | 10 | **Nearly.** It misses the first-party OpenAPI document and its generator script (`docs/api/openapi.first-party.json`, `scripts/api/first_party_openapi.py`). That is a registry rule the finder lacks. |
| #11132 pinned-model failure | 14 | 2 | 8 | 9 | 11 | **Partial.** It misses the coder CLI's output paths (`turn.rs`, `headless.rs`) and `cj_conversation/tests.rs`. |
| #11177 schedules on web and apps | 24 | 4 | 7 | 13 | 16 | **Partial.** It finds the web, background and coder-new side. It misses the desktop Background pane, the phone's computers screen and account link, `coder-computers`, and `route_owners_tests.rs`: the *other clients* of the new feature. |
| #11158 remaining clients of old paths | 12 | 3 | 4 | 6 | 8 | **Partial.** It misses the phone (`account_link.rs`, `link_tests.rs`), `coder-sync/src/memory.rs` and the website's GitHub session client. Those are the callers of the routes being moved. |
| #11174 attachments everywhere | 43 | 8 | 14 | 25 | 34 | **Not comprehensive for the cross-platform part.** The web side is covered: chat, composer, chat_store, chat tests, desktop `chat.rs`/`shell.rs`, mobile app, `attachments.rs`, INVARIANTS. Missed at 200: the JS (`static/chat.js`, `shell.js`), the chat worker, the iOS/Android `NativeChat` files and the release-acceptance script and doc. |
| #11156, #11160, #11182 | — | | | | | Fixed already; measured above at their fix's parent. |

**Assessment.** Some issues are about one subsystem and name its files,
such as #11193 and #11159. For those, the top 50–100 is comprehensive.
Other issues fan out to every client of an interface: web plus desktop plus
phone, or a route plus all its callers. For those the finder reliably finds
the centre and misses part of the fan-out. A reference-graph pass already
exists in stage 2, but it only follows names defined by the top 6 files. A
route string or a protocol method is not a Rust name, so the callers of
`/coder/memory` or of a NIP-HOST method are not found that way.

## Recommendation: what to build next

1. **Treat the briefing as a map, not a list.** Recall at 200 paths is 91%
   on the bench. A briefing can afford 200 paths, about 3k tokens. Group
   them by crate, rank the crates, and list the full file tree of the top
   three crates. That gives 89% of existing files in about 380 paths, with
   the 100-path list as the "start here" section. This is cheap and needs no
   new model.
2. **Interface fan-out as its own stage.** Changed routes, protocol methods,
   NIP kinds, settings keys and feature flags are literals, not Rust names.
   Index string literals per blob the way identifiers are indexed. Then, from
   the top files, follow each route or method string to every file that uses
   it (clients in other crates, Swift/Kotlin bridges, docs). This targets the
   largest miss class in the open-issue check, and the "other crate" misses
   on the bench.
3. **Registry and regeneration rules from history.** Mine "when *X*
   changes, *Y* changes in the same commit ≥ 80% of the time" pairs, for
   example a route in `openagents-web` with `docs/api/openapi.first-party.json`
   and its generator script. Emit them as deterministic rules with their
   support count. This is co-change restricted to high-confidence pairs, and
   it covers the derived and registry files.
4. **Keep Jev for the 30–150 band when unsure.** It adds 4–5 points at 50
   files for about $0.002 and 0.7 s. The planner adds 2–6 points and is the
   only stage that names new files (25 of 107), but it costs about 8 s, so
   use it once per issue when the briefing is built, not per query.
5. **Execution is the last stage (issue step 5).** Even a perfect finder
   cannot list the 20% of hand-written files a fix creates. Compiler errors,
   failing tests and a grep for the old names are what surface the rest.
   Record each file the agent adds after the briefing as a "late file", and
   feed it back as training data for the scorer.

Retrain with `train` whenever the history grows by a few hundred fixes. The
model file is `scripts/filefind/model.json`, about 250 KB.
