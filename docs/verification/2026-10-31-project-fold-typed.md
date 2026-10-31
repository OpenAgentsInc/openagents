# The project fold reads through the typed readers (#b8cd)

2026-10-31, claude-coax-nostr.

`apps/openagents-web/project-fold.coax` no longer matches tags itself. Which
events make up a project, and which tag of each the fold reads, comes from
the typed readers: `coax/lib/nostr/nip34.coax` (`n34_repo`, `n34_project`,
`n34_order`, `n34_issue`, `n34_fields`, `n34_labels`, `n34_status`) and
`coax/lib/nostr/nip22.coax` (`n22_comment`). The page, the homepage's project
rows (`landing-fold.coax`) and the native adapters all share that one fold.

## How the readers see the relay's bytes

The readers take spans of an event's original JSON. The store now keeps each
event's tags array verbatim in its arena, beside the unescaped copy it
always kept, and records where each element's raw bytes start
(`Store.raw_at`, `raw_n`, `el_raw`). `store_nt(s, e)` is the reader's view of
event e; `span_ref(s, e, span)` finds the unescaped element a reader's span
selects, so the tables still show unescaped text. The arena grew from 4 MiB
to 6 MiB to hold both (the Coax project's live events take 2.76 MB unescaped
plus 1.79 MB raw; Verse 1.78 + 1.25 MB). The project page's `Page` is 28.2 MB
of its 64 MiB Wasm memory.

## The rules, from the NIP text

| Question | Old fold | Now | Why |
| --- | --- | --- | --- |
| Which `d` addresses a 30621, 30623 or 30617 | any `d` tag with the value | the first `d` tag | NIP-01: an addressable event is addressed by "the `d` tag value"; model.ts (`tags.find`) and our relay (`rs_d`) read the first |
| A 30621 with two `d` tags | the project | refused | NIP-MP rule 1 (`d-cardinality`): several `d` tags make the address reader-dependent, so they are rejected |
| Which issue a status (1630-1633) is for | the first `e` tag naming a known issue | the `e` tag marked `root`, else the first `e` | NIP-34 Status: `["e", <issue id>, "", "root"]`; a `reply` `e` names an accepted revision |
| Which issue a comment (1111) is for | the first `E` tag naming a known issue | its NIP-22 root scope, the first of `E`, `A`, `I`, which must be an `E` with `K` 1621 | NIP-22: comments MUST point to the root with uppercase tags and the parent with lowercase ones, and `K` and `k` MUST be present |
| An issue under the project | any `a` tag equal to the coordinate | the same, over the reader's `a` tags | NIP-34 allows several `a` tags; the relay's `#a` filter matches any of them |
| A malformed event (a valueless selected tag, a non-hex `blocked-by` or `parent`, a non-numeric `reward`, an order item without an id and rank, an escaped spelling of an id) | read as far as it went | refused by its reader; it is not part of the project | INVARIANTS.md: projections cannot create authority; a record that doesn't have the NIP's shape is not read |
| A revision whose fields its reader refuses | the newest revision wins | it revises nothing; the newest admitted one stands | as above |
| An order with more than 4096 items | every item | refused (N34_OUTPUT), never cut | the order's ranks are all or none |

The `slug`, `subject`, `size`, `parent`, `by`, `claim`, `commit`, `plan` and
`revision` tags are read as before: the first tag of the name.
`project-live.coax` picks the newest order by the same first-`d` rule
(`addressed`).

## Differential test: the old fold against the new

`coax/web/project/fold-dump.coax` prints everything a fold decides for a
relay transcript: the project, order and repository events; per issue its
id, slug, kind, status and status event, revision, fields event, claim and
claim time, commit, rank, alias, plan and its steps, latest note and every
note with its author; the whole log; and the five tables. It was built twice,
once over the previous `project-fold.coax` (`origin/main` at 0e60da2447) and
once over this one, and both ran over every project's events, paged from
relay.openagents.com at 2026-10-31 10:56 CDT (the page's five filters plus
every 30621, 30623 and 1621 by the key, so events addressed elsewhere are
included), and over the captured fixture `coax_relay.jsonl`.

| Project | Events | Issues | Old vs new | New vs model.ts tables |
| --- | ---: | ---: | --- | --- |
| belt-2200 | 173 | 36 | identical | identical |
| coax | 2697 | 278 | identical | identical |
| coder | 188 | 23 | identical | identical |
| dead-gears | 91 | 17 | identical | identical |
| hold-the-stair | 184 | 32 | identical | identical |
| openagents-desktop | 213 | 31 | identical | identical |
| orbit-league | 103 | 30 | identical | identical |
| planetary-villa | 229 | 50 | identical | identical |
| psionic | 187 | 46 | identical | identical |
| verse | 2023 | 287 | identical | identical |
| fixture coax_relay | 237 | 105 | identical | the transcript check |

Every one of the 278 slugs in `coax/web/project/state/coax.json` folds to
the issue id the publisher recorded. No live event hits any rule in the
table above (no multi-`d` events, every comment carries `E`, `K`, `e`, `k`,
every status a `root` `e`), so the differences are intended and appear only
in the made-up transcripts of the checks below. Native fold time over the
Coax project's 2697 events went from 0.04 s to 0.14 s.

## Where the checks run now

The readers are `#authority` units, and compiling a graph that loads one runs
that graph's checks at compile-time limits (coax/spec.md, "#authority belongs
to a physical source unit"). The page's units held checks that need
production-sized storage, so they moved, unchanged:

- `apps/openagents-web/project-checks.coax` (interpreter,
  `coax test ... --library --steps 2000000000`): the checks of project.coax,
  project-links.coax and project-fold.coax, plus the new fold-rule checks:
  first-`d` addresses and the two-`d` project, the status root marker, the
  NIP-22 root scope (no `K`/`k`, a root kind that isn't an issue, an escaped
  id), a refused issue, an issue under two projects, escaped values read
  unescaped, a refused order and a refused revision, and `span_ref`.
- `apps/openagents-web/project-live-checks.coax` (native suite,
  `--native-link project-capacity.coax`): project-live.coax's five checks,
  each of which holds a whole `Live` (two Stores), now more than the
  interpreter's 33,554,432 aggregate slots.

The test fixtures that signed comments without NIP-22's parent scope
(`live-catchup.test.ts`, `history-fixture.ts`) now write the tags
`publish.ts` writes (`E`, `K`, `P`, `e`, `k`, `p`).

## Results

- `coax/web/tool.sh suites --only apps/openagents-web/project-live-checks.coax,apps/openagents-web/project-checks.coax`: 5 of 5 and 82 of 82 passed.
- `bun test coax/web/project/live-catchup.test.ts`: 14 pass, native and headless WebGPU browser.
- `python3 coax/tests/nostr/project_history.py` and `project_details.py`: native sanitizers and Wasm, checked and release.
- `coax/web/tool.sh suites --since origin/main`: see the commit's record.
