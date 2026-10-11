# File finding: from an issue to the files its fix needs (#11210)

**Goal.** Given an issue and a repository, produce the files its fix needs,
in about a second, from the repository's full commit history. Do as much as
possible with lookups in indexes built ahead of time. Call a model (Jev or a
planner) only when that helps.

**Result.**

- **The map reaches 95% at 400 paths.** The deterministic finder's map holds
  **95.2%** of the existing files each fix edited, measured over 100 closed
  issues replayed at each fix's parent commit. The map is the top 400 ranked
  files, grouped by crate. The plain top 100 holds **85%**, and the top 300
  holds 94%.
- **Speed.** A query takes 0.8–1.3 s with the issue text in hand; the
  deterministic part takes 0.6–1.0 s. That leaves out the one embedding
  call, which runs in parallel with the token lookups.
- **Models add nothing at 400 paths.** Jev and a planning model help only
  the short "start here" list: 0.80 instead of 0.75 at 50 files.
- **Newer fixes score lower.** On 8 larger fixes that landed after the
  scorer was trained, the map holds 85%. That is the same with a ranker
  trained on every fix up to Oct 9 (the latest replayable fix before
  them), and the same after adding wire-field fan-out and cross-client
  templates; see "Fixes that landed after training".
- **New files are not counted.** About 20% of the hand-written files in a
  fix are files the fix creates. No finder can return those, so they are
  left out of these figures and reported separately.

## The tool

`scripts/filefind/filefind.py` (Python 3 with numpy, `git`, and `gh` for
`--issue`; nothing to build):

```sh
export OPENROUTER_API_KEY=...                      # embeddings; without it the deterministic stages still run
python3 scripts/filefind/filefind.py index --repo .                    # first run 1-2 min, then seconds
python3 scripts/filefind/filefind.py query --repo . --issue 11210 --json            # top 100 + map of 400
python3 scripts/filefind/filefind.py query --repo . --text "title

body" --k 100 --map 400 --json
```

`--json` prints `openagents.filefind.v1`:

- `files[]` is the top `--k` files (default 100). Each entry has:
  - `rank` and `path`;
  - `confidence`, the scorer's probability that the fix touches the file;
  - `stages`, which stages found it: `emb`, `sym`, `co`, `sim`, `hist`,
    `pair`, `dir`, `recent`, `ref`, `iface`, `rule`, `crate`;
  - `reasons`, for example "issue names `upstream.rs`", "defines `guard`",
    "changes with the seeds (co 1.49)", "fixed with similar #10128",
    "uses `/coder/memory` like `account_memory.rs`", "changes in 9 of 11
    commits that change `X`".
- `map` is the top `--map` files (default 400; 0 turns it off), grouped by
  crate. Groups are ordered by the confidence they hold. Each group has
  `group` and `confidence_mass`, and its `files` are ordered best first,
  each with `rank`, `path`, `confidence` and `stages`.
- `timing_ms` gives the time per stage.

The intended use in a briefing:

1. Give `files` as "start here".
2. Give `map` as "the rest of the change is very likely in these crates and
   files".
3. Add `Cargo.lock` whenever a `Cargo.toml` is in the top 50.

The index lives in `~/.cache/openagents/filefind/<repo>-<identity>/`. The
identity is the normalized remote URL, the root commit(s) and the workspace
(`--workspace`, else `FILEFIND_WORKSPACE` / `OPENAGENTS_WORKSPACE`, else
`local`), stamped in `identity.json`. Every worktree of one repository shares
the cache; two repositories with the same name or the same issue numbers never
do, and a cache stamped for another identity is refused (#11232, DATA-04). A
cache from before identities is adopted once by the repository its history was
built from; its old feedback is set aside as `feedback.unbound.jsonl` and never
read. It is about
1.9 GB for this repository with 1,700 replayed revisions, and it holds no
file contents:

| File | Holds |
|---|---|
| `history.pkl` | Paths per commit, issue references from commit subjects, and per-directory commit lists. |
| `blobs.npz` | One 256-dim `text-embedding-3-small` vector per blob (path plus the first 1,200 characters). |
| `commits.npz` | One vector per commit subject. |
| `issues.pkl` | One vector per closed issue. |
| `tokens.sqlite` | Identifiers and defined names per blob. |
| `iface.sqlite` | Interface strings per blob, with each string's document frequency. |

**Freshness.** Nothing is ever stale:

- `query` appends any commits that are newer than the history index,
  using `git log <indexed head>..HEAD`; a rewritten history triggers a
  full rebuild. It embeds the new commit subjects, and it indexes,
  embeds and tokenizes any blob it has not seen. The first query after a
  big pull pays 1–3 s for this; later queries pay nothing.
- `scripts/filefind/post-merge-hook.sh` does the same in the background
  after every pull, and also refreshes recently closed issues and the
  run feedback. To install it, link it as `.git/hooks/post-merge`.
  `filefind.py index` is incremental: about 5 s after a day of commits.
- `scripts/filefind/retrain.sh` trains a **candidate** ranker; it never
  writes `model.json` (#11231, LEARN-01). It rebuilds the issue → fix
  dataset from main, replays every parent, trains with cross-fitting on the
  train cases only, and copies the model to an immutable
  `candidates/model-<digest>.json` in its work directory. Then
  `file-finding-bench.py compare` measures the active model and the
  candidate on the same held-out eval cases under the frozen plan in
  `scripts/filefind/ranker_gate.py` (recall@50 gain of at least 2 SE; no
  loss beyond 2 SE at recall@20 or @100; no rise beyond 2 SE in the Brier
  score of the top 100) and writes a receipt. Only
  `ranker_gate.py promote --candidate C --receipt R` changes the active
  model, atomically, and only when the receipt passed, names that exact
  candidate, and compared against the model active now; the receipt is
  kept beside it as `model.receipt.json`. `compare` and `eval` refuse a
  model trained on any eval case (`train --all` marks its model so);
  `eval --allow-overlap` labels such numbers as development. The shipped
  model predates the gate: it was trained on every replayable fix up to
  Oct 9 (1,692 issues), so its numbers on those issues are optimistic.

**Late files from agent runs.** `filefind.py feedback` collects these
into `feedback.jsonl` in the index. Every row is bound to the cache's
identity and carries an authority (#11231, LEARN-04):

- **label**: a file a verify-replayed trace (`admitted.jsonl`, read by
  default) changed, when its replayed checks passed, its base commit is in
  this repository, and its issue group is in the corpus's `training`
  partition (#11215 map).
- **observation**: everything else: the files a `coder issue-run` (#11214)
  changed (recorded checks, not replayed), A/B trial results (#11211,
  `--ab <results dir>`), files opened outside a briefing, and failed or
  held-out traces.

Only labels change the ranking: the similar-issue stage treats them as fix
files of that issue, and a query on the same issue pins them into the list.
Observations are listed after the ranking (`observations` in `--json`) for
exploration, and never scored. A run whose base commit is not in this
repository is skipped before its patch is read.

The bench never uses feedback, because every run is newer than the
replayed fix.

## Pipeline

**Stage 1: candidates.** These are all lookups, except the one embedding
call for the issue text.

| Stage | Adds |
|---|---|
| emb | The 300 files whose path+head vector is nearest the issue. |
| sym | Paths, file names, crate names, identifiers (definitions and mentions, weighted by rarity) and quoted literals the issue names, resolved through the token index. Literals are checked to occur exactly. |
| co | Files that changed together with the seed files (top emb + sym) before the fix. Commits touching more than 40 files are ignored. |
| sim | The fix files of the 12 most similar past closed issues. |
| hist | Files touched by the 150 past commits whose subject is most similar to the issue. |
| pair | Tests, `mod`/`lib`/`main`, `Cargo.toml`, README/INVARIANTS and the crate's `tests/`, for each seed. |
| dir | The other files in each seed's directory. |
| recent | The files touched most in the last 400 commits. |

**Stage-1 scorer.** A gradient-boosted tree model (numpy, 200 depth-3
trees) scores the candidates on 57 features: each source's score, that
score relative to the query's best, its rank within the pool, the file's
kind, churn and recency. It is trained on 1,484 past issue → fix pairs, all
older than the bench.

**Stage 2: propagation.** Stage 2 starts from the stage-1 ranking:

| Stage | Adds |
|---|---|
| co2 / pair2 | Co-change and paired files of the stage-1 top 10. |
| ref | Files that use a Rust or other name that one of the top 6 files defines, when fewer than 25 files use it. |
| iface | Files that use an *interface string* of one of the top 8 files, when at most 30 files use it. Interface strings are routes and every prefix of them, method and event names (`background.list`), kebab and snake ids, CSS classes, and **wire-type field names**. Field names are the fields (and `serde` renames) of Rust structs that derive `Serialize`/`Deserialize`, and JSON keys written or read anywhere (`"name":`, `.get("name")`, `["name"]`). Rust readers of a rare field (`.name`) are found through the token index. Strings are taken from quoted strings in every language, so a route in an axum router reaches the Swift/Kotlin bridge, the JS and the CLI client that call it. |
| rule | "X changes ⇒ Y changes" mined from history before the fix: P(Y \| X) for the stage-1 top 20 files, and P(Y \| something in directory D changes) for the six directories that hold the most confidence. Kept when the support is at least 2 or 3 commits. |
| tmpl | Cross-client templates: in the last 3,000 commits that touched one of the strongest crates *and* at least two other crates (a feature landing in several clients), how often each file of the other crates changed. |
| crate | Every file of the four crates that hold the most stage-1 confidence, if a crate has at most 500 files. This lets the ranker place the whole crate, and it is what the map draws on. |

A second model (83 features, adding stage-1 score, crate rank, crate and
directory mass, and the above) re-ranks the pool. Its training inputs are
cross-fitted on two folds of stage 1.

**Optional model stages** (bench only):

- **Jev** re-judges ranks 30–150 with one batched Noul per file, reading
  the issue plus the file's first 1,500 characters.
- **Plan:** Claude Sonnet 5.5 reads the issue, the top 60 paths and the
  file lists of the four strongest crates. It writes the change plan as
  steps, each naming the files it edits or creates.

## Bench

`scripts/bench/file-finding-dataset.py` builds the cases from closed issues
whose fix commits on main can be found from the commit subject (`(#N)`,
`fixes #N`).

- **Eval set.** The newest 100 issues that were fixed by exactly one commit
  naming no other issue, with at most 25 hand-written files: #10194–#11204,
  Sept 26 – Oct 9.
- **Training set.** 1,484 older issues, including fixes made over several
  commits (the union of their files). Every training fix landed before the
  oldest eval fix.
- **Replay.** Each case runs at the fix's parent commit. The tree, the
  co-change history, the similar issues and the commit subjects are all cut
  there.

Ground truth is the files the fix changed. Lockfiles, `tree.json`, OpenAPI
documents and files with a generated header count as **derived** and are
reported separately. Hand-written files the fix *created* do not exist at
the parent, so recall is given over existing files (what a finder can
return) and over all hand-written files. The eval set has 702 existing
hand-written files, 172 added and 33 derived.

```sh
B=scripts/bench/file-finding-bench.py; A="--repo . --dataset $D --work $W"
python3 $B prepare $A; python3 $B features $A; python3 $B train $A; python3 $B eval $A
python3 $B judge $A; python3 $B judge-eval $A; python3 $B plan $A --limit 50; python3 $B check $A
```

### Recall of each candidate stage alone, and of their union (existing hand-written files)

| Stage | Recall | Median candidates per issue |
|---|---:|---:|
| emb | 0.497 | 300 |
| sym | 0.319 | 18 |
| co | 0.775 | 355 |
| sim | 0.497 | 89 |
| hist | 0.637 | 124 |
| pair | 0.164 | 23 |
| dir | 0.476 | 171 |
| recent | 0.426 | 150 |
| ref (stage 2) | 0.517 | 132 |
| iface (stage 2, with wire fields) | 0.585 | 238 |
| rule (stage 2) | 0.470 | 28 |
| crate (stage 2) | 0.714 | 367 |
| **union** | **0.973** | **1,260** |

Wire fields raised the interface stage from 0.429 to 0.585, and the union
from 0.967 to 0.973.

The `rule` stage stands out for precision: 28 candidates per issue that
hold 47% of the files.

### Ranked list and map

All figures are recall of existing hand-written files.

| Pipeline | @20 | @50 | @100 | @200 | @300 | **@400 (map)** |
|---|---:|---:|---:|---:|---:|---:|
| stage-1 scorer only | 0.504 | 0.725 | 0.815 | 0.879 | | |
| finder before wire fields and templates | 0.514 | 0.745 | 0.850 | 0.913 | 0.942 | 0.952 |
| **finder, stages 1 + 2 (shipped)** | **0.524** | **0.752** | **0.853** | **0.905** | **0.943** | **0.952** |
| … without the template feature (ablation) | 0.527 | 0.744 | 0.855 | 0.907 | 0.943 | 0.956 |
| precision | 0.184 | 0.106 | 0.060 | 0.032 | 0.022 | 0.017 |
| recall over all hand-written files, added included | 0.421 | 0.604 | 0.685 | 0.727 | 0.757 | 0.764 |
| issues with every existing file found | 24 | 47 | 56 | 66 | 77 | 80 |
| + Jev on ranks 30–150 (every query) | 0.514 | 0.788 | 0.865 | 0.917 | 0.942 | 0.952 |
| + Jev only when unsure (72% of queries) | 0.514 | 0.782 | 0.849 | 0.910 | 0.932 | 0.950 |
| plan first, then finder (50 issues)* | 0.482 | 0.804 | 0.866 | 0.919 | 0.931 | 0.947 |

The Jev and plan rows were measured with the ranker before wire fields
and templates.

\* On those 50 issues the finder alone scores 0.530 / 0.752 / 0.857 / 0.909
/ 0.936 / 0.945. The plan names a median of 27 files, and those 27 alone
hold 71% of the existing files. It names 24 of the 107 files the fixes
created by exact path.

Recall at larger sizes:

| Paths | 500 | 700 | 1,000 |
|---|---:|---:|---:|
| Recall | 0.954 | 0.963 | 0.964 |

That is the ceiling of the candidate pool.

**Map shape compared.** Instead of the ranked top 400, the map could be the
top *k* files plus the whole file list of the top *c* crates. That shape is
worse at every size. The top 100 plus three full crates holds 0.890 in a
median of 337 paths (p90 543). The top 150 plus six crates holds 0.927 in
519 (p90 786). The ranked list beats it because the scorer already pulls in
the right files of those crates (the `crate` stage) and leaves out the
rest. So the shipped map is the ranked top 400, grouped by crate for
reading.

**When a model is called.** A query counts as unsure when the scorer's
confidence mass in its top 50 is under 70% of its total mass; that is 72 of
100 queries. Jev helps only the 50–100 range (+4 points at 50) and costs
about $0.002 and 0.7 s per query. Above 200 files it does nothing, and with
a strong weight it hurts. The planner takes about 8 s and $0.03 per issue.
It is the only stage that names new files, but it does not raise recall at
the map size. **Recommendation:** run the deterministic map always. Spend
Jev on the "start here" list for unsure queries. Run the planner once per
briefing only to propose new files.

**Derived files.** `Cargo.lock` changed in 20 fixes, and the rule "a
`Cargo.toml` is in the top 50" predicts all 20. Of 13 generated files
(`tree.json`, OpenAPI documents), 11 are in the top 100 and all 13 are in
the top 400.

### Which kinds of file are missed

Existing hand-written files missed by the shipped finder:

| Kind | Files | Missed @20 | Missed @50 | Missed @100 | Missed @400 |
|---|---:|---:|---:|---:|---:|
| Rust source | 366 | 167 | 90 | 55 | 17 |
| docs (.md) | 82 | 36 | 19 | 16 | 6 |
| Rust test | 81 | 47 | 24 | 10 | 2 |
| `lib.rs` / `main.rs` / `mod.rs` | 80 | 32 | 16 | 8 | 3 |
| README / INVARIANTS / AGENTS | 30 | 17 | 10 | 5 | 2 |
| `Cargo.toml` | 27 | 17 | 8 | 4 | 3 |
| script | 9 | 6 | 3 | 2 | 0 |
| Swift / Kotlin | 9 | 8 | 3 | 2 | 0 |
| web / shader | 6 | 3 | 2 | 2 | 0 |
| fixture / data | 5 | 5 | 4 | 1 | 1 |
| other | 7 | 3 | 0 | 0 | 0 |

**Where the 34 files missed at 400 come from:**

- 23 never become candidates.
- 11 are ranked past 400.
- 26 are in a crate the map does cover. The other 8 are in a crate the map
  never touches.

Most of them come from a few sweeping refactors: #11032 alone accounts for
7, and #11058 and #11002 for 5 more. In those refactors, other
implementations of a changed trait sit in sibling crates the issue never
mentions.

**Added files.** Of the 172 files that fixes created, 116 were placed in a
directory that already existed, and 103 of those directories appear among
the top-50 files' directories.

### Latency per stage

Median ms on this Mac, under a load average of 12–17 from other agents'
builds:

| Stage | Bench median / p90 | Live query |
|---|---:|---:|
| load indexes | | 140 |
| `git ls-tree` | 167 / 176 | 70 |
| sym (token index, literal check) | 190 / 556 | 50–570 |
| issue embedding, overlapping sym (network) | | 450–1,050 (wait 0–1,050) |
| emb, co, sim, hist, pair, recent | about 40 together | about 40 |
| history refresh (no new commits) | | 0 |
| stage 2, total | 271 / 403 | 200–325 |
| … of which iface (with wire fields) | 114 / 244 | 50–170 |
| … of which rules | 3 / 5 | 3–6 |
| … of which templates | 9 / 11 | 9–15 |
| two scorers | | 225–345 |
| **total, issue text in hand** | | **785–1,210**; deterministic part 0.6–1.1 s |

Fetching the issue with `gh` adds 0.45 s.

### Fixes that landed after the scorer was trained

Eight issues still open on project 22 already have fixes on main that
landed after the bench data. Running the finder at each fix's parent gives
an out-of-sample check. These fixes are larger than the bench median, with
4–19 hand-written files each, some over several commits. Results with the
shipped ranker, which was trained on every replayable fix up to Oct 9 (the
newest training fix before these):

| Issue | Existing files | @20 | @50 | @100 | @200 | @300 | @400 |
|---|---:|---:|---:|---:|---:|---:|---:|
| #11156 one error and list shape | 15 | 4 | 6 | 8 | 10 | 11 | 12 |
| #11158 app routes under /v1 | 17 | 8 | 10 | 13 | 13 | 14 | 15 |
| #11159 owned routes | 14 | 5 | 7 | 11 | 14 | 14 | 14 |
| #11160 key scopes | 6 | 4 | 6 | 6 | 6 | 6 | 6 |
| #11177 background work | 14 | 6 | 9 | 11 | 12 | 13 | 14 |
| #11182 account memory (5 commits) | 16 | 5 | 7 | 8 | 10 | 12 | 12 |
| #11132 provider fallback note | 15 | 2 | 3 | 5 | 7 | 9 | 9 |
| #11134 account export | 3 | 2 | 2 | 2 | 3 | 3 | 3 |
| **all** | **100** | **0.36** | **0.50** | **0.64** | **0.75** | **0.82** | **0.85** |

The newer-fix row after each change:

| Ranker and stages | @100 | @200 | @300 | @400 | @600 | @1,000 |
|---|---:|---:|---:|---:|---:|---:|
| first version (two stages, no fan-out) | 0.63 | 0.76 | | | | |
| + iface, rules, crate expansion; ranker cut at Sept 26 | 0.60 | 0.74 | 0.81 | 0.85 | | |
| same stages, ranker trained through Oct 9 | 0.62 | 0.78 | 0.84 | 0.85 | | |
| + wire fields and templates, ranker through Oct 9 (shipped) | 0.64 | 0.75–0.76 | 0.82–0.83 | 0.85 | 0.89 | 0.93 |
| same, without the template feature | 0.64 | 0.76 | 0.82 | 0.85 | 0.89 | 0.92 |

The candidate pool now holds 94 of the 100 files. The loss is in ranking.
Fifteen files are missed at 400, and ten of them come from two issues:

- **#11132** (6 files): the coder CLI's event output (`turn.rs`,
  `headless.rs`, `relay.rs`, `main.rs`, `tests/relay_job.rs`) and
  `NIP-CJ.md`, ranked 441–1,036. The fix *adds* the wire field
  (`switched`), so at the parent there is no field to follow. These
  files are linked to the change only by being the consumers of the
  result type.
- **#11182** (4 files): the two chat routers and the phone's
  `link_tests.rs`. The feature is new, so they share no string yet.

On the bench, fixes of 10 or more files score 0.939 at 400 and fixes of
4–9 files score 0.985. This 8-issue set is small and weighted toward two
cross-client features, so the gap is mostly those two issues.

### Late files from agent runs

Six A/B issues have both a replay case and recorded misses. Across them,
briefed agents (with a small top-k briefing) opened 25 distinct files
outside their briefing. The shipped finder holds 17 of them in its top
100 and **24 in its map of 400**. The one it misses is a debug file the
agent wrote itself. So in those A/B runs the agents' misses came from
the briefing's size, not from what the finder can see.

## Open issues on project 22: is the map comprehensive?

For ten open issues, a separate agent read the code and listed the files
each complete change needs, without seeing the finder's list. The finder
ran at current main. Where part of an issue has landed already, the
reference covers only the work that remains.

| Issue | Reference files | @20 | @50 | @100 | @200 | @400 | Assessment |
|---|---:|---:|---:|---:|---:|---:|---|
| #11193 split the router decision for Clef | 19 | 9 | 17 | 19 | 19 | 19 | **Comprehensive by 100.** |
| #11159 own `/coder/*` | 9 | 5 | 7 | 9 | 9 | 9 | **Comprehensive by 100.** `upstream.rs` is #1. |
| #11134 account export, CLI part | 12 | 5 | 8 | 9 | 10 | 12 | **Comprehensive at 400.** The OpenAPI document and its generator now come in through history rules. |
| #11132 pinned-model failure | 14 | 2 | 5 | 11 | 11 | 13 | **Nearly.** Only `cj_conversation/tests.rs` is missing. |
| #11174 attachments everywhere | 43 | 9 | 16 | 23 | 34 | 41 | **Nearly.** JS, iOS and Android bridges, release-acceptance files and the chat worker are all in the map. The two `NativeChat` files (Swift, Kotlin) are missing. |
| #11158 remaining clients of the old paths | 12 | 2 | 3 | 7 | 10 | 10 | **Partial.** Missing: `coder-sync/src/memory.rs` and the phone's `link_tests.rs`. |
| #11177 schedules on web and apps | 24 | 5 | 7 | 10 | 14 | 18 | **Partial.** The web, background, coder-new and host side are there. Missing: the desktop Background pane, the phone's computers screen and account link, `coder-computers`, `phone_api.rs` and `route_owners_tests.rs`. These are clients of a feature that does not exist yet, so no interface string links them. |
| #11156, #11160, #11182 | | | | | | | Already fixed, and measured above at their fix's parent. |

Across the seven, the map holds 122 of 133 reference files (92%), against
96 of 133 (72%) in the top 100. The map is comprehensive when the issue sits
in one subsystem or names what it changes. When an issue adds a *new*
feature to every client, the clients that do not yet share a string with
the server are what the map misses: the desktop pane and the phone screen
for #11177. Those need the plan stage, or the agent's own exploration.

## Last round: two experiments, neither kept

**Planner re-rank of the top 1,000.** Claude Sonnet reads the issue and the
finder's top 1,000 paths, each with a one-line summary (the first doc line
of the file). It returns up to 150 file numbers, most likely first. Its
picks go to the front of the list, and the rest keep the finder's order.
Run with `file-finding-bench.py rerank`.

| Ranking | Bench @100 | Bench @400 | Newer fixes @100 | Newer fixes @400 |
|---|---:|---:|---:|---:|
| finder (shipped) | 0.853 | 0.952 | 0.66 | 0.85 |
| + planner re-rank on unsure queries (72 of 100) | 0.863 | 0.956 | | |
| + planner re-rank on every query | 0.872 | 0.956 | 0.68 | 0.86 |

Cost and latency:

- **Latency:** a median of 6–8 s per query (p90 8.6 s).
- **Cost:** $0.09 per query through OpenRouter's Sonnet 5.5, about 25k
  input tokens. Claude Code's own estimate for the same prompt was
  $0.34.
- **Exception:** in two newer-fix cases the base order was rebuilt from
  an earlier run, because the embeddings account was out of credit.

The bar was +5 points on newer fixes at 400. The re-rank adds 1–2 points
at 100 and 0–1 at 400, so **it is not part of the finder**. The planner
mostly reorders files the finder already has. The files it would need to
pull up, such as #11132's CLI output paths ranked 441–1,036, are files it
also does not see as related.

**Consumers of a changed result type.** I followed the module of each of
the top 8 Rust files (`generate::` and similar) to every file in the same
crate that names it.

- **What it reached:** only files the finder already had. On the
  newer-fix set it hit 1–3 files per issue.
- **What it missed:** none of #11132's misses, because `turn.rs`,
  `headless.rs` and `relay.rs` use the types without naming the
  module.
- **What it cost:** up to 3.2 s per query on large crates.

I checked type names too. `Usage`, `Meta` and `Door` occur in 86–301
files, so following them is noise. **Not kept.**

## Conclusion

The finder gives a coding agent, in about a second:

- a "start here" list of 100 files;
- a 400-file map grouped by crate.

The map holds **95%** of the existing files a fix edits on the bench, and
**85%** on the newer, larger fixes. The 98% target in this issue is
**re-scoped to "map + verify"**, for two reasons:

- **The misses are files that are not yet related to anything.** About
  20% of a fix's hand-written files are created by the fix itself. Most
  of the remaining misses are consumers of a field, route or feature that
  does not exist yet at the parent commit: #11132's new `switched` field,
  and #11182's new memory routes. No index of the parent commit can link
  them; neither the planner nor the type-consumer pass found them.
- **The agent's own checks catch them.** The briefed agent's `verify` tool
  compiles and tests the change (#11211). A missing consumer shows up as
  a compile error or a failing test, and the agent then opens the file.
  `filefind.py feedback` records it as an observation; once a replayed,
  training-partition trace changes it, it becomes a label.

How to keep it fresh and learning:

- **Every query refreshes history.** `filefind.py query` appends new
  commits, embeds them and indexes unseen file versions.
- **A post-merge hook refreshes after pulls.** `.githooks/post-merge` (the
  repository's hooks path) runs `scripts/filefind/post-merge-hook.sh` in
  the background after every pull. One refresh runs at a time, and it
  never fails the merge. It refreshes history, recently closed issues,
  embeddings and the token indexes, then collects run feedback.
- **Retrain weekly** with `scripts/filefind/retrain.sh`, which trains and
  compares a candidate; promote it only when its receipt passes.
- **Embeddings are optional.** Without an embeddings key, or with an
  account out of credit (as the shared OpenRouter account is at the time
  of writing), every stage except emb/sim/hist still runs. The finder
  degrades rather than fails, and new blobs are embedded once the key
  works again.

## The decision corpus built from this bench: file-relevance-v1 (#11215)

Roadmap item N8 of the [training-system audit](../audits/2026-10-10-training-system-audit/roadmap.md)
turns this bench's issue → fix cases into a labelled corpus in the
`tenancy::training` format, for calibrating and training the file-relevance
decision (Clef, #11216; the head-only ranker, #11217). Evidence class:
`measured`.

- **Items.** One item per (issue, file): the state is
  `ISSUE #N: title\n\nbody` (body cut to 2,500 characters), then
  `FILE: path` and the file's first 2,048 bytes at the fix's parent commit.
  The question is the noul "Is this file relevant to solving the issue?".
- **Labels come from outcomes.** `true` when a commit that fixed the issue
  changed the file, read from git; `label_source: measurement`. A judge's
  answer (Jev) has its own `teacher` field on the item and is never the
  label. None is recorded yet.
- **Candidates per issue**, with a seed derived from the issue number: the
  fix's existing hand-written files (at most 8), 3 siblings from their
  directories, 2 files of their crates, 1 recently busy file and 1 random
  text file. Items are about 45% `true`, so this corpus measures and fits
  probabilities at that rate, not at the finder's pool rate.
- **Partitions by time.** Issues are ordered by their last fix commit:
  training (450 issues, 5,419 items) < calibration (150, 1,952) <
  development (196, 2,520: this bench's window, from its oldest fix
  `9f5f8ad756` to the ranker's cutoff `63a5197dfb`) < locked (24, 343:
  fixes after the cutoff). Excluded everywhere: the 10 issues of the
  [Clef/Jev relevance bench](clef-jev-relevance-bench.md), which tuned its
  thresholds, and the 8 newer-fix issues above. Three later-partition items
  that near-duplicated an earlier one (token Jaccard ≥ 0.8) were dropped,
  and so were four issues left with only one label.
- **In git:** `crates/gym/suites/file-relevance-v1/` holds `items.tsv.gz`
  (issue, partition, path, blob, label, candidate kind, text digest),
  `issues.tsv` (fix and parent commits, issue-text digest) and
  `manifest.json` (rules, counts, digests). The corpus itself holds file
  contents and issue text and stays out of git.
- **Rebuild:** `scripts/bench/file-relevance-corpus.sh OUT_DIR` reads git
  at the pinned commits and the issue text from GitHub, and checks every
  digest. `--select` also re-runs the selection from git, which must
  reproduce the committed manifest. Both runs reproduced the corpus byte for
  byte (`sha256:6a8a34f0…2619`).
- **`tenant-train check` passes:** provenance and labels on every item, no
  group across partitions, and no exact or near duplicate across partitions.
  Tenant digest `sha256:4385be4e…69c4`. The locked partition was read
  once, by the X1 calibration check (#11216).

The leakage check compared every pair of items across partitions, which
for 10,234 items is about 52 million token-set comparisons. It now builds
each token set once and compares only the pairs that can reach the
threshold (sizes within the ratio, and a shared token among each set's
rarest). A test checks that its verdict is the all-pairs verdict. The check
on this corpus takes 3 min 43 s in a debug build.

## A head-only ranker on the frozen finder (#11217, roadmap X2a)

This follows the Tassadar W3 rule. The finder (`scripts/filefind`) stays
frozen as the exact core: its candidates and its 86 per-candidate features
are inputs and are never trained. Only a small head learns on top of them.
Evidence class: `measured`. The target is the one this bench scores:
existing hand-written files the fix changed.

**What runs.**
- **Trainer.** `psionic-decision-train` in `crates/psionic` (psionic-train,
  `src/decision_train.rs`). The head is an MLP: signed-log, standardized
  finder features, optionally Clef's noul logit, and optionally a learned
  16-wide projection of Clef's pooled head rows; 32 GELU units; one logit.
  - Loss: weighted BCE. Optimizer: AdamW, with early stopping on the last
    10% of the training issues.
  - Each run writes a `measured` receipt: recipe, data and model digests,
    seed, loss series and wall time.
  - On the CPU, a full-pool run (1.3M rows) takes about 5 s, and a run with
    hidden rows takes 2–3 min.
  - Gradient check: the analytic gradient matches central differences to
    1.5e-7 over 5 seeds.
- **Hidden rows.** `psionic-openai-server --decision-export-rows DIR` wrote
  them: f16 `last`, `question`, `option:true` and `option:false` rows per
  question, joined to candidates by request sha256. psionic-train does not
  depend on psionic-serve.
- **Data.** `scripts/bench/file-relevance-ranker.py export|fresh|join|compare`.
  Clef ran on coderos-4080 on these candidates:
  - the numpy ranker's top 100 for the 100 bench issues (9,948 rows);
  - its top 400 for the 8 newer-fix issues (3,188 rows);
  - its top 50 for 220 of the corpus's training-partition issues (12,191
    rows). The run was cut short to free the 4080 for production decisions.
- **Comparison.** Recall over every existing hand-written fix file, on the
  finder's natural candidate pool. The standard error of the difference is
  clustered by issue. The numpy ranker is the bench's held-out model (trained
  on the 1,484 older cases) on the bench, and the shipped model on the
  newer-fix set; each head trains on the same cases as its baseline.
- **Newer-fix caveat.** The newer-fix set was replayed without the issue
  embedding for 5 of 8 issues, because OpenRouter is out of credit. Its
  baseline is therefore 0.81 at 400, not the 0.85 recorded with embeddings.

Mean of 3 seeds; in brackets, the mean difference from the numpy ranker and
the mean SE:

| Bench (100 issues) | @20 | @50 | @100 | @400 |
|---|---:|---:|---:|---:|
| numpy ranker | 0.524 | 0.752 | 0.853 | 0.952 |
| head, finder features only, full pool | 0.518 (−0.006, 0.014) | 0.716 (−0.036, 0.013) | 0.837 (−0.016, 0.010) | 0.953 (+0.001, 0.004) |
| re-rank top 100: features only | 0.465 (−0.059, 0.016) | 0.687 (−0.066, 0.017) | = | = |
| re-rank top 100: + Clef logit | 0.481 (−0.043, 0.020) | 0.700 (−0.052, 0.017) | = | = |
| re-rank top 100: + Clef logit + hidden rows | 0.470 (−0.055, 0.022) | 0.682 (−0.070, 0.018) | = | = |
| *no training:* numpy logit + Clef logit, top 100 | **0.593 (+0.068, 0.014)** | **0.786 (+0.034, 0.011)** | = | = |

| Newer-fix set (8 issues, 100 files) | @50 | @100 | @200 | @400 |
|---|---:|---:|---:|---:|
| numpy ranker | 0.400 | 0.550 | 0.680 | 0.810 |
| head, finder features only | 0.407 (+0.007, 0.041) | 0.513 (−0.037, 0.031) | 0.663 (−0.017, 0.023) | 0.797 (−0.013, 0.020) |
| re-rank top 400: + Clef logit + hidden rows | 0.500 (+0.100, 0.089) | 0.613 (+0.063, 0.043) | 0.750 (+0.070, 0.052) | = |
| *no training:* numpy logit + Clef logit, top 400 | 0.520 (+0.120, 0.086) | **0.660 (+0.110, 0.041)** | **0.780 (+0.100, 0.043)** | = |

"=" means unchanged by construction: a re-rank of the top K leaves recall
at K and beyond where it was.

**Verdict: the trained head does not ship.**
- **Bench.** No trained variant beats the numpy ranker by 2 standard errors
  at any cut. Every re-ranker trained on the 220 issues loses at @20 and @50.
  The early-stopped epoch is 1–2, so the hidden-row projection (265k
  parameters) overfits 11k rows at once.
- **Calibration.** The full-pool head's calibration is no worse on the bench
  (ECE 0.0067 against 0.0085). On the newer-fix set it is slightly worse
  (ECE 0.0064 against 0.0038).
- **What does help Clef: a frozen fusion.** Adding Clef's logit to the numpy
  ranker's logit inside its top K, with no learned parameter, wins by more
  than 2 SE:
  - bench @20, +0.068 ± 0.014;
  - bench @50, +0.034 ± 0.011;
  - newer-fix @100, +0.110 ± 0.041;
  - newer-fix @200, +0.100 ± 0.043.
  
  The trained heads lose because they cannot use the numpy ranker's logit:
  on its own training cases that logit is in-sample.
- **@400 (the map).** Clef has to score past rank 400 to change @400, at
  about 0.18 s a file on the 4080.
- **Open.** The fusion still needs a frozen evaluation plan and a gated
  activation. That gate belongs to #11231 (`scripts/filefind/ranker_gate.py`),
  and nothing here activates it.

Receipts, comparison reports and data metadata are in
`crates/psionic/fixtures/decision-train/file-relevance-v1/`. They chain the
corpus manifest, the Clef artifact and head digests, the recipe digest, the
data digest and the model digest.

## Two candidates through the ranker gate (#11220, #11231, 2026-10-10)

Both ran through `file-finding-bench.py compare` under the gate's frozen
plan (`ranker_gate.py`, plan `sha256:d835c0d2f1ea…`) on the same 100
held-out eval cases (`sha256:65f862d38a6c…`), against the active
`scripts/filefind/model.json` (`sha256:b11f9b6b1b74…`). Every feature came
from Vertex AI `text-embedding-005` vectors (the finder's first door since
`960ba59cf9`). The active model was trained on 93 of the 100 eval cases, so
its numbers are optimistic and the gate is harder to pass. Receipts and the
fusion card are in
`crates/psionic/fixtures/decision-train/file-relevance-v1/gate-2026-10-10/`.

| Candidate | recall@50 (primary) | recall@20 | recall@100 | Brier top 100 | Verdict |
|---|---|---|---|---|---|
| Active model | 0.8075 | 0.6531 | 0.9117 | 0.0717 | stays |
| Retrained on Vertex vectors (`model-9e44a34bb536`, 1,509 train cases) | 0.8029 (−0.0046, SE 0.0060) | 0.6428 (−0.0102, SE 0.0119) | 0.9036 (−0.0081, SE 0.0055) | 0.0586 (−0.0131) | fail: no gain |
| That model + #11217's frozen Clef fusion, top 100 (`model-c8ad7220c0bc`) | 0.8454 (+0.0379, SE 0.0190) | 0.6961 (+0.0430, SE 0.0177) | 0.9036 (−0.0081, SE 0.0055) | 0.0479 (−0.0238) | fail: +1.99 SE, the plan needs 2.0 |

Neither was promoted. What this shows:

- **The active model holds up on Vertex vectors.** It was trained on OpenAI
  similarity values, but retraining on Vertex's does not beat it.
- **The fusion helps.** It is the strongest candidate at @20 and @50 and the
  best calibrated, and it misses the primary bar (2 SE = 0.0380) by 0.0001 of
  recall. Its base is the retrained model, the only numpy base
  with no overlap with the eval cases. The plan is frozen and was not
  loosened. Fusing over a base that beats the active model at @100, or a
  larger held-out set (the SE is what fails), is the next try.
- **Cost.** The fusion asked Clef 9,900 times through
  `https://openagents.com/api/v1/systemone` (answered by
  `pylon:coderos-4080-clef`, artifact `sha256:fd3e9060…`, head
  `sha256:6e469970…`), one request at a time so production decisions kept
  the GPU. It took 2 h at 0.6–2.5 s an answer. A Vertex answer during a
  pylon bench was refused by the card's digest check, and the case was asked
  again. At query time a fused card re-ranks the top 100 in about 15–60 s
  on that pylon, and falls back to the numpy order past
  `FILEFIND_CLEF_BUDGET_S` (default 20 s) or on any miss.
- **A fix along the way.** The feature stage crashed (segfault, then
  "database disk image is malformed") because its worker threads shared one
  SQLite connection; each thread now opens its own (`900d756e39`).

## File cards at index time and a distilled tower (#11249, 2026-10-10)

**Goal.** Move the model work out of the query. Per-file Clef decisions cost
15–60 s a query. Instead, every file version gets a card when it is indexed,
and a query does one issue embedding, one batched tag decision for the issue,
and lookups.

**Verdict: not promoted.** The candidate fails the gate on quality and on
latency. The active model is unchanged. The card stage is built, tested and
off by default: it runs only for a model card that carries a `cards` block,
or with `FILEFIND_CARDS=1`.

### What was built

`scripts/filefind/cards.py`, wired into `filefind.py index`, `query` and
the bench:

- **Vocabulary.** A closed vocabulary is mined once per repository
  (`cards.py vocab --rev R`):
  - 130 areas and 159 entities, from one Gemini 2.5 Flash call over the
    repository's parts and their descriptions, its docs directories and its
    commit subjects;
  - 13 layers (route/handler, store, protocol/wire type, UI, CLI verb, test,
    doc, registry, generated, core logic, config/build, script, fixture);
  - 5 clients (web, phone, desktop, CLI, worker);
  - 186 routes: quoted URL paths that occur in at least two files.

  The bench vocabulary was mined at `41e0b9ce4a`, the parent of the oldest
  eval fix, so no eval-era text shaped it. Changing the vocabulary means
  re-carding every blob.
- **Cards.** There is one card per blob: a summary of 40 words at most, plus
  areas, layers, clients and entities. Each card is one structured call to
  Vertex AI `gemini-2.5-flash-lite` with a response schema, and ids outside
  the vocabulary are dropped. Routes are read from the text, and only routes
  in the vocabulary are kept. Data, binaries, `bench/`, `third_party/` and
  lockfiles get a card with no model call. Cards are keyed by blob and
  vocabulary digest in the per-repository cache (`cards/cards.jsonl`), and
  the compiled tag matrix is in `cards/tags.npz`.
- **Card vectors.** The card text plus the file's defined identifiers are
  embedded with Vertex `text-embedding-005` (256 dims).
- **Issue profile.** One Jev decision per issue, through TypeSafe directly:
  a choice over the areas, a choice over the entities, and a noul for each
  layer and each client. Over the 1,735 replayed issues it took a median of
  0.33 s (p95 0.51 s). It runs in parallel with the issue embedding, and in
  40 live queries the ranker never waited for it.
- **Two-tower distillation.** The score is
  `a · (card cos, path+head cos, tag overlap) + (q Wq)·(d Wd) + b`, with
  k = 32. The file side is projected once per blob and cached as
  `cards/tower-<digest>.npz`, so the query pays one dot product per file.
  - **Training data:** the file-relevance-v1 **training partition** only (449
    issues, 19,676 pairs). These are the corpus items, every existing fix
    file, and 30 other files per issue.
  - **Labels** are outcomes. **Clef's answer** is a separate teacher field,
    fitted with a squared-error term (weight 0.5) on the 5,409 items it
    answered.
  - **Teacher run:** `file-finding-bench.py teacher` asked Clef about all
    5,419 items through the fusion card's door (digest-checked). It took
    26 min. Clef's AUC against the outcomes is 0.79.
  - **Cross-fitting:** a training-partition issue is scored by the fold
    tower that did not train on it. Every other issue is scored by the tower
    trained on all of them.
- **Ranker features and candidates.** Each candidate gets these features:
  - `card_cos`;
  - `card_area`, `card_entity`, `card_layer` and `card_client`, each the
    issue's probability mass on the file's tags;
  - `card_route`;
  - `card_tag`;
  - `tt`, the distilled score;
  - for `card_cos`, `card_tag` and `tt`, the relative value and rank.

  A new `card` stage adds three candidate sets: the top 150 by card cosine,
  the top 150 by `tt` and the top 100 by tag score. On the eval cases that
  stage alone holds 0.625 of the fix files with 338 candidates. Embedding
  similarity holds 0.473 with 300, and the union is 0.967.

### Gate (frozen plan `sha256:d835c0d2f1ea…`, the same 100 eval cases)

The candidate is `model-08dcb51ecf82`, trained with card features on the
same 1,509 train cases as the Vertex retrain. The receipt and the candidate
are in
`crates/psionic/fixtures/decision-train/file-relevance-v1/gate-2026-10-10/cards-11249/`.

| | recall@50 (primary) | recall@20 | recall@100 | Brier top 100 |
|---|---|---|---|---|
| Active model (on the card-widened pool) | 0.8092 | 0.6753 | 0.9242 | 0.0692 |
| Card candidate | 0.8188 (+0.0097, SE 0.0122) | 0.6645 (−0.0108, SE 0.0119) | 0.9083 (**−0.0159, SE 0.0059**) | 0.0598 (−0.0094) |

Verdict: **FAIL.** The @50 gain is 0.8 SE, and @100 fell 2.7 SE.

Diagnostics. These were not gated, and nothing was tuned on them.

- **Against the held-out Vertex retrain without cards** (same train cases,
  same pool), the card features give +0.0186 at @50 (SE 0.0155) and −0.0099
  at @100 (SE 0.0067). The ranker uses them: `card_client`, `card_layer`,
  `tt` and `card_cos` are among its most frequent split features. But they
  do not move the top 50 by 2 SE.
- **The tower does not carry Clef's judgment.** On corpus partitions the
  tower never trained on, its AUC is close to plain cosine:
  - calibration: tower 0.738, card cosine 0.711, path+head cosine 0.724;
  - development: tower 0.763, card cosine 0.766, path+head cosine 0.770.

  Clef's own AUC is 0.79. A frozen fusion (ranker logit + `tt` in the top
  100, the #11217 form) gives +0.005 at @50 over the Vertex retrain and
  −0.003 over the card candidate. Clef's fusion gave +0.038. What Clef adds
  comes from reading the file's text against the issue. A 32-wide projection
  of 256-dim vectors, distilled from 449 issues, does not reproduce it.

### Latency

40 queries (issues #11216–#11255, text in hand, one process per query, this
Mac under a load average of 8–10):

| | p50 | p95 | max |
|---|---:|---:|---:|
| Active model, no cards | 1,025 ms | 1,614 ms | 2,463 ms |
| Card candidate (cards, tower, Jev profile) | 1,073 ms | **1,888 ms** | 3,653 ms |
| … of which loading cards + the card stage | 48 + 40 ms | 53 + 47 ms | |
| … waiting for the Jev profile | 0 | 0 | |

The card path itself costs about 90 ms. The p95 target of 1.5 s is missed
with or without cards. The tail is in the existing stages, `sym` (up to
2.4 s on long issue bodies with many literals) and stage 2. The card
candidates make stage 2's pool larger, which adds about 200 ms at p95.

### Cost

| Run | Blobs | Model calls | Time | $ |
|---|---:|---:|---:|---:|
| Vocabulary (one Flash call plus an additions pass) | | 2 | 2 min | 0.08 |
| Every blob of 1,559 replayed trees | 106,517 | 74,614 (flash-lite; 67% of input tokens cached) | 26.9 min, 64 workers | 19.69 |
| … card vectors | 106,557 | | 3.8 min | 1.33 |
| A fresh index of HEAD (39,423 files, 10,452 carded by a model; at the measured rates) | 39,423 | 10,452 | about 5 min | about 3.0 |
| Incremental: 35 commits, about 2 h of main (measured, post-merge path) | 102 | 91 | 13.9 s | 0.039 |
| A day of commits (Oct 4–10: 535–3,385 new model-carded blobs, median about 850) | | about 850 | about 1–2 min | about 0.35 |
| Clef teacher (5,419 items, pylon) | | 5,419 | 26 min | 0 (our pylon) |
| Jev profiles (1,735 issues; one per query live) | | 1,735 | 51 s, 12 workers | TypeSafe usage, not metered here |

### What to try next

1. **Keep cards as candidates, not as features.** The card stage reaches 62%
   of fix files on its own and widens the pool. The loss at @100 suggests
   the 2-stage ranker spends capacity on the new features. Two things to
   try: a candidate that keeps the old feature set and only adds the card
   candidates, and one with only `card_cos` and `card_client`.
2. **A stronger teacher target.** Collect Clef on the stage-1 top 30 of every
   training-partition issue, not only the corpus items, and give the
   tower's correction term the file's identifiers. Its correction should
   learn something the cosines lack.
3. **Latency.** To make p95 ≤ 1.5 s: cap `sym` (literal checks on long
   bodies), precompute iface users per blob, and keep the card stage's
   contribution to the stage-2 pool to the top 150.

`retrain.sh` with `FILEFIND_CARDS=1` reproduces the candidate. It runs the
`cards`, `profiles` and `tower` bench steps, then `features --cards` and
`train --cards`. Set `FILEFIND_CLEF_TEACHER` to the JSONL that
`file-finding-bench.py teacher` writes. Any promotion still goes through
`ranker_gate.py`.

## The active finder under 1.5 s at p95, results unchanged (#11269, 2026-10-10)

**Goal.** Bring the active finder's per-query p95 under 1.5 s without
changing a single ranked file, map entry, stage or reason. The ranker, its
features, its calibration and the candidate pools are untouched. Nothing is
pruned.

**Where the time went.** The bench was 50 queries (issues #11213–#11262,
`scripts/filefind/fixtures/latency-queries.json`, with each issue's text and
query embedding in hand). Each query ran in its own process at `1d16475293`,
on one frozen clone of the cache. The tail was one thing: SQLite postings.

- `sym` looks up every identifier and every literal's tokens with
  `SELECT b FROM tok WHERE t = ?`. That reads one table row per posting, about
  1.5 µs each warm. A literal's common tokens (`source`, `null`, `base`) cost
  up to 270k rows, about 0.4–1.0 s warm and 3–6 s cold.
- Stage 2's reference graph (`postings`) and interface users
  (`SELECT … FROM s WHERE t IN …`) read the same way.

**What changed** (`scripts/filefind/filefind.py`):

- **Packed postings.** `tokpack` sits in `tokens.sqlite` and `spack` in
  `iface.sqlite`. Each holds, per string, its blob ids as int32 chunks,
  clustered by string, so one string is one range read. Chunks are appended in
  source rowid order, which is the order the indexed SQL returns. Every caller
  therefore sees the same ids in the same order, and tie-breaks are unchanged.
- **Building and refreshing.** `filefind.py index`, which the post-merge hook
  runs, builds and refreshes the packs. It takes 31 s once on this cache and
  adds about 300 MB. After that each refresh is incremental.
- **The watermark.** `packmark` records the last source rowid a pack covers.
  An older filefind on the same cache only appends source rows, so a pack is
  current exactly when its mark reaches the last rowid.
  - A query catches the pack up when at most 1M rows are missing.
  - Otherwise, or when the database is busy, that query reads the old SQL
    path, with the same results.
- **Path scans.** Exact-name matches go through a file-name index, and
  "under this directory" matches through a bisect over the sorted paths. Both
  keep path order.
- **`relative()`.** It reads its numpy arrays once as Python floats, giving
  the same float64 values.

**Identical output.** One finding matters for any "identical" claim: the
active finder's output depends on `PYTHONHASHSEED`. Tied rank features
(`*_rk`) and `most_common` cut-offs follow set order, and set order follows
the per-process string hash. Two runs of the same version on the same cache
differ unless the seed is fixed. So results are compared per seed:

- Under seeds 0 and 1, all 50 queries came out byte-identical to the previous
  version, warm and cold. That covers the ranked top 100, the 400-file map,
  stages and reasons.
- `scripts/filefind/test_latency.py` runs:
  - always: synthetic checks that packed postings equal the SQL in content and
    order (full build, chunked build, an older writer's rows, catch-up,
    compaction, users over the 500-string chunks), and that the path scans and
    `relative()` match the old code. Reversing the pack order fails 3 of these
    tests.
  - with `FILEFIND_IDENTITY=1`: the bench queries through this version and
    through `1d16475293` on one cache clone, digest for digest. Passed: 50
    queries in 114 s.
- `scripts/filefind/latency_bench.py` reruns the measurements: `run --mode
  warm|cold`, `stats` and `same`.

### Latency

Times are in ms, counted from "issue text in hand" (`total`), one process per
query, `nice -n 19` and sequential. The Mac was shared with a Codex session
(load average 6–23). Cold evicts every cache file from the page cache
(`msync(MS_INVALIDATE)`) before each query; Git's packs stay warm.

| 50 queries | p50 | p95 | p99 | max |
|---|---:|---:|---:|---:|
| #11249's measurement (40 queries, live embedding, load 8–10) | 1,025 | 1,614 | | 2,463 |
| Before, warm (embedding in hand) | 1,102 | 1,732 | 1,894 | 1,992 |
| **After, warm** | **524** | **689** | **714** | 729 |
| Before, warm, a quieter minute (seed 1) | 781 | 1,308 | 1,435 | 1,515 |
| **After, warm (seed 1)** | **489** | **680** | **813** | 865 |
| Before, cold | 5,030 | 7,880 | 8,587 | 8,939 |
| **After, cold** | **790** | **1,186** | **1,284** | 1,370 |
| Before, live CLI (`query --issue`, real Vertex embedding) | 1,313 | 2,295 | 3,644 | 4,469 |
| **After, live CLI** | **910** | **1,076** | **1,143** | 1,192 |

Per stage, p95, before → after:

| Stage | Warm | Cold |
|---|---|---|
| `sym` | 883 → 121 ms | 6,193 → 390 ms |
| interface users | 278 → 52 ms | 2,600 → 146 ms |
| stage 2 | 535 → 167 ms | 3,565 → 296 ms |

What is left warm is mostly fixed cost: loading the indexes (about 120–180 ms),
`git ls-tree` (about 60 ms), scoring (about 100 ms) and the embedding
round trip. That round trip is now the longest wait on the live path (p95
about 380 ms).

The live cache gets its packs on the first `filefind.py index` after this
lands, which the post-merge hook runs. Until then queries take the SQL path,
with the same results.
