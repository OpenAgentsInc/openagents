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
  scorer was trained, the map holds 85%.
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

The index lives in `~/.cache/openagents/filefind/<repo>/`. It is about
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

`query` indexes any blob or commit it has not seen, so a pull costs a few
seconds at most.

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
| iface | Files that use an *interface string* of one of the top 8 files, when at most 30 files use it. Interface strings are routes and every prefix of them, method and event names (`background.list`), kebab and snake ids, and CSS classes. They are extracted from quoted strings in every language, so a route in an axum router reaches the Swift/Kotlin bridge, the JS and the CLI client that call it. |
| rule | "X changes ⇒ Y changes" mined from history before the fix: P(Y \| X) for the stage-1 top 20 files, and P(Y \| something in directory D changes) for the six directories that hold the most confidence. Kept when the support is at least 2 or 3 commits. |
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
| ref (stage 2) | 0.516 | 133 |
| iface (stage 2) | 0.429 | 172 |
| rule (stage 2) | 0.474 | 28 |
| crate (stage 2) | 0.714 | 361 |
| **union** | **0.967** | **1,218** |

The `rule` stage stands out for precision: 28 candidates per issue that
hold 47% of the files.

### Ranked list and map

All figures are recall of existing hand-written files.

| Pipeline | @20 | @50 | @100 | @200 | @300 | **@400 (map)** |
|---|---:|---:|---:|---:|---:|---:|
| stage-1 scorer only | 0.504 | 0.725 | 0.815 | 0.879 | | |
| **finder, stages 1 + 2 (shipped)** | **0.514** | **0.745** | **0.850** | **0.913** | **0.942** | **0.952** |
| precision | 0.180 | 0.105 | 0.060 | 0.032 | 0.022 | 0.017 |
| recall over all hand-written files, added included | 0.413 | 0.598 | 0.683 | 0.733 | 0.756 | 0.764 |
| issues with every existing file found | 23 | 46 | 58 | 67 | 78 | 82 |
| + Jev on ranks 30–150 (every query) | 0.514 | 0.788 | 0.865 | 0.917 | 0.942 | 0.952 |
| + Jev only when unsure (72% of queries) | 0.514 | 0.782 | 0.849 | 0.910 | 0.932 | 0.950 |
| plan first, then finder (50 issues)* | 0.482 | 0.804 | 0.866 | 0.919 | 0.931 | 0.947 |

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
| stage 2, total | 146 / 241 | 190–290 |
| … of which iface | 25 / 70 | 25–62 |
| … of which rules | 3 / 5 | 3–6 |
| two scorers | | 225–345 |
| **total, issue text in hand** | | **815–1,250**; deterministic part 0.6–1.0 s |

Fetching the issue with `gh` adds 0.45 s.

### Fixes that landed after the scorer was trained

Eight issues still open on project 22 already have fixes on main that
landed after the training cut. Running the finder at each fix's parent gives
an out-of-sample check. These fixes are larger than the bench median, with
4–19 hand-written files each, some over several commits.

| Issue | Existing files | @20 | @50 | @100 | @200 | @300 | @400 |
|---|---:|---:|---:|---:|---:|---:|---:|
| #11156 one error and list shape | 15 | 4 | 5 | 8 | 10 | 11 | 12 |
| #11158 app routes under /v1 | 17 | 5 | 9 | 12 | 13 | 15 | 15 |
| #11159 owned routes | 14 | 5 | 7 | 9 | 13 | 14 | 14 |
| #11160 key scopes | 6 | 5 | 6 | 6 | 6 | 6 | 6 |
| #11177 background work | 14 | 7 | 10 | 10 | 12 | 14 | 14 |
| #11182 account memory (5 commits) | 16 | 6 | 6 | 8 | 9 | 10 | 13 |
| #11132 provider fallback note | 15 | 2 | 2 | 5 | 8 | 8 | 8 |
| #11134 account export | 3 | 2 | 2 | 2 | 3 | 3 | 3 |
| **all** | **100** | **0.36** | **0.47** | **0.60** | **0.74** | **0.81** | **0.85** |

Before the iface, rule and crate stages, this set scored 0.63 at 100 and
0.76 at 200.

Misses at 400:

- **#11132** misses the coder CLI's event output (`turn.rs`, `headless.rs`,
  `relay.rs`, `main.rs`) and NIP-CJ (`nostr/src/cj_conversation.rs`,
  `NIP-CJ.md`). The link to them is a JSON field name (`switched`), a plain
  word the interface index does not keep.
- **#11182** misses the two chat routers and `docs/api/design.md`.

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

## Next steps

1. **Teach the interface index JSON field names that sit next to a NIP or
   wire type.** The `switched` field in #11132 is the example. Today a
   plain word is too common to index, so tie such words to the files that
   define the wire type.
2. **Per-client templates for "add X to every client".** Mine from history
   which files a cross-client feature touches (web page + desktop pane +
   phone screen + bridge), and propose those slots when the scorer's top
   files sit in one client.
3. **Feed late files back.** Record every file the agent adds after the
   briefing as training data. Retrain with `train` every few hundred fixes.
   The model is `scripts/filefind/model.json`, about 250 KB.
