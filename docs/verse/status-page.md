# The status page

`https://openagents.com/status` shows how a change reaches production and
where things stand: main's newest commits, the commit production runs, the
release path, and the green-main pipeline
([green-main-pipeline.md](green-main-pipeline.md), #13102). It is a Coax page
drawn on the homepage's Grid. It adds no JavaScript.

## What it shows

| Part | Content |
| --- | --- |
| Four facts | main's head and its age; production's commit and how many commits behind main it is; `main-green`, or NOT SET; commits in 24 hours and 7 days, and how many name their agent |
| The rail | main's commits as rings, newest on the right, with production's commit and `main-green` marked. Point at a ring to read its subject; click it to open the commit |
| Live now | The path a change takes today: builders, checks, push main, `release.sh`, staging, production. A dash travels each arrow |
| Built, not switched on | The green-main pipeline's stages, drawn dim: submit, queue, integrator, main, full check, `main-green`, release. The queue, the red list and the branches sent back show their counts |
| Recent commits | Hash, age, agent and subject for the newest 14. A row opens its commit |
| Build hosts | Hosts, job slots, and the jobs and billed cost of the 24 hours before the release |

Green marks what passed, red what failed and amber what isn't switched on.
Everything else keeps the site's greys.

## Where the data comes from

| Data | Source | Fresh |
| --- | --- | --- |
| main's commits | GitHub's public API, read by the visitor's browser when the page opens | Live |
| Production's commit and release time | The commit `release.sh` builds | Each release |
| `main-green`, queue, branches sent back | `git ls-remote origin` at build time | Each release |
| Red list | `~/.openagents/pipeline/releases/after/red.json` | Each release |
| Commit counts | `git log` at build time | Each release |
| Build hosts | `coax/remote/remote.sh`'s defaults and `~/.openagents/remote/ledger.tsv` | Each release |

If GitHub doesn't answer, the page keeps the release's commit list and says
so.

## How it's built

| File | Role |
| --- | --- |
| `apps/openagents-web/status.coax` | The page: reads the two texts, lays out and draws. Loads `newland.coax` for the Grid, text, frames and buttons |
| `apps/openagents-web/status-face.c` | The Wasm exports. Scalars only |
| `coax/web/status-data.coax` | A Coax tool that writes the snapshot into `status/index.html` and `status/index.md`. `build.sh` runs it on a release build |
| `apps/openagents-web/status/index.html` | The page shell. `data-text` holds the snapshot, `data-lines` names GitHub's commit list, `data-wgsl` names the homepage's shader |

The host already delivers `data-text` as message 2 and the text at
`data-lines` as the message it names (61 here), so the page needs no host
change.

```sh
coax/bin/coax test apps/openagents-web/status.coax      # the page's checks
coax/bin/coax test coax/web/status-data.coax            # the tool's checks
coax/web/tool.sh coax/web/status-data.coax --write      # rewrite the snapshot
```

## Limits

- Only the commit list is live. The pipeline's counts change when a release
  ships. When the pipeline's watchers run (the cutover in #13102), they
  should publish the snapshot after every round so the queue and red list
  are live too.
- GitHub allows 60 unauthenticated requests an hour from one address. Past
  that, the page shows the release's list.
- The page lists 30 commits. When production is further back than that, it
  says so without a count.
