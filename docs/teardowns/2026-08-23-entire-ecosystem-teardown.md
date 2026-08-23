# Entire ecosystem teardown — 2026-08-23

Read-only architecture and product audit of Entire, the Git-native agent-session
platform from [`entireio`](https://github.com/entireio) (public GitHub origin)
and the regional Git mirror network at [entire.io](https://entire.io). Local
clones live in `~/work/projects/entire/repos/` (31 public repos: 30
`entireio`, 1 `entirehq/.github`). Nothing in those trees was modified, built,
or executed. No Entire account was created. Clone and HTTP probes against the
Entire Git network were unauthenticated. [source] [limitation]

Episode calibration: `openagents.com` `docs/episode-triage.md` (episodes
199-275). Entire overlaps the series at session-as-evidence, "why code
exists," public traces, FastFollow, forge Git traffic, and MirrorWatch.
It does not replace PostgreSQL receipts, the forge WAL, or Omega.

Evidence labels (per [README](./README.md)):

- **`[source]`** — observed in a commit-pinned source snapshot
- **`[schema]`** — encoded in a typed wire, storage, or config contract
- **`[docs]`** — stated by checked-in documentation
- **`[test]`** — encoded in a checked-in test or CI surface
- **`[history]`** — supported by Git history at or before the pin
- **`[public]`** — corroborated by a named public source, fetched 2026-08-23
- **`[runtime]`** — observed in live HTTP or CLI behavior on this machine
- **`[vision]`** — stated as intended direction, not necessarily present
- **`[inferred]`** — concluded from several observations
- **`[limitation]`** — a boundary on what this source-and-probe audit can prove

## TL;DR

Entire is **Git as the store for "why the code changed."** A CLI hooks Claude
Code, Codex, Gemini, OpenCode, Cursor, Copilot, Factory Droid, and Pi. Each
agent turn writes ephemeral state onto a shadow branch. Each user commit
condenses that state into a checkpoint and stamps the commit with an
`Entire-Checkpoint:` trailer. Transcripts, prompts, tool calls, token counts,
and attribution percentages travel with the repository, not in a product
database. [source] [docs]

Beside the CLI sits a **regional Git mirror network**. GitHub stays origin.
`entire://aws-us-east-2.entire.io/gh/<owner>/<repo>` is a smart-HTTP remote
served from EntireDB. The `git-remote-entire` helper (a dedicated binary, not
the cobra CLI) authenticates with a login JWT. Unauthenticated HTTPS to that
host returns HTTP 401 `Basic realm="Entire Git Server"`. Empty Basic
credentials return `Invalid or expired token`. This machine had no Entire
login, so the lane cloned GitHub origin. [runtime] [docs] [source]

```text
  coding agent (Claude / Codex / Cursor / …)
        |  agent hooks + git hooks
        v
  Entire CLI  ---- ephemeral ----  entire/<sha7>-<wt6>  (worktree snapshot)
        |                          .git/entire-sessions/<id>.json
        |  post-commit condense
        v
  persistent checkpoints           git-branch: entire/checkpoints/v1
                                   git-refs:   refs/entire/checkpoints/<shard>/<id>
        |  trailer on user commit
        v
  your branch                      Entire-Checkpoint: <id>
                                   Entire-Attribution: 73% agent (146/200 lines)

  GitHub origin  <---- git-sync relay ---->  EntireDB regional cell
                                             entire://cluster/gh/owner/repo
                                             git-remote-entire + login JWT
```

The five most important findings:

1. **The join key is a commit trailer, not a foreign key in a product
   database.** `Entire-Checkpoint` on the user commit points at a checkpoint
   tree (or per-checkpoint ref). Reverse lookup scans trailers. That is a
   portable, git-native receipt join. OpenAgents already has a stronger
   durable authority (PostgreSQL turn receipts and the forge WAL). Harvest the
   *join shape*, not the store. [source] [docs]
2. **Persistent checkpoints used to serialize on one branch tip. They are
   moving to one git ref per checkpoint.** The `git-refs` backend
   (`refs/entire/checkpoints/<shard>/<id>`) writes, pushes, and fetches
   independently, fast-forward only, rebase-on-divergence, never force.
   That is the same race Entire's own docs name: "every condensation rewrites
   its tip, every push races on one ref." The forge WAL already refuses that
   shape. Study the per-ref queue and on-demand fetch. [source] [docs]
3. **`git-sync` is the closest public analog of MirrorWatch.** It mirrors refs
   from source remote to target remote without a local checkout: `info/refs`,
   `upload-pack` with target tips as `have`, `receive-pack` with a streamed
   pack. Relay-first, plan-before-push, typed JSON. Entire's GitHub→EntireDB
   mirror is the product use. OpenAgents' forge→GitHub export is the inverse
   direction with the same physics. [source] [docs]
4. **Agent context on a public Git remote is a visibility incident waiting
   to happen.** Docs are honest: if the repository is public, transcripts are
   on the internet; shadow-branch code snapshots are **unredacted** (gitignore
   is the only filter); redaction is fail-closed for scanners but PII and OPF
   are opt-in and OPF runs only at push. OpenAgents visibility policy and
   server-side receipts are strictly stronger. Do not store prompts on a
   GitHub-visible branch. [docs]
5. **The mirror network is not a second git authority for OpenAgents.** Entire
   keeps GitHub as origin and asks agents to clone from a regional cell.
   OpenAgents keeps the **forge** as origin and GitHub as a read-only mirror
   (REPOSITORY-002). Do not point `origin` at `entire://`. Do not treat Entire
   as a forge replacement. Harvest load-test tooling (`forgemark`) and
   remote-helper admission, not the hosting product. [public] [runtime]

## 1. Identification and scope

### 1.1 Exact source identity

| Field | CLI | git-sync | skills |
| --- | --- | --- | --- |
| Repository | `entireio/cli` | `entireio/git-sync` | `entireio/skills` |
| Audited commit | `b1e08625312571a5a4131d23db22e0f816c4515e` | `2a61c8c79d686f7ad0b8ab833343acd8fcd7b67a` | `2f9a8758e9c5220685ed049f396c826768957030` |
| Commit time | 2026-08-22 17:52 +0200 | 2026-08-21 15:18 +0200 | 2026-08-13 11:25 -0700 |
| Subject | Merge PR #1914 PII phone NANP scope | Merge PR #112 security-release-provenance | Merge PR #40 search skill v0.10 |
| License | MIT | MIT | MIT |
| Release | CLI 0.10.2 (`entire_darwin_arm64.tar.gz` ships `entire` + `git-remote-entire`) | separate cask `git-sync` | `npx skills add` / per-agent discovery |

Companion pins in the same lane: `entire-graph` `0c0f3f005`, `forgemark`
`47f57bf61`, `pgr` `2e702e652`, `auth-go` `bb62b5de0`, `external-agents`
`83eb58122`. [source] [history]

### 1.2 What was probed, what was not

Probed: GitHub org APIs (`entireio` 30 public, `entirehq` 1 public);
unauthenticated smart-HTTP to `https://aws-us-east-2.entire.io/gh/entireio/cli`
(401); `git-remote-entire` without login (`no auth context for cluster`);
`entire repo clone /gh/entireio/cli` without login (`not logged in`);
`GET https://entire.io/api/v1/repositories` (401 `Not authenticated`);
commit-pinned READMEs and architecture docs in the clones. [runtime] [source]

Not probed: logged-in Entire clone, EntireDB internals, entire.io web app
behind auth, `entirehq/entire-api` (CLI examples name it; GitHub 404),
checkpoint signing, live agent hooks, plugin install, ForgeMark against any
host. [limitation]

### 1.3 Full table of contents (31 public GitHub repos)

Live GitHub listing on 2026-08-23: 30 public repos under `entireio`, 1 under
`entirehq`. No public forks or archived repos. Local clones are in
`~/work/projects/entire/repos/<checkout>/`. Two `.github` profile repos
collide on checkout name; the lane uses `entireio-github` and
`entirehq-github`. Pins are `HEAD` at clone time. [source] [public]

`entirehq/entire` and `entirehq/entire-api` are named in product docs and CLI
examples. They are not public on GitHub. Unauthenticated Entire APIs do not
list them. Treat them as private or Entire-native. [public] [limitation]

#### Core product

| GitHub | Local checkout | Pin | Role |
| --- | --- | --- | --- |
| [`entireio/cli`](https://github.com/entireio/cli) | `cli` | `b1e086253` | Entire CLI: agent/git hooks, session capture, checkpoint stores, `git-remote-entire`, plugin dispatcher |
| [`entireio/git-sync`](https://github.com/entireio/git-sync) | `git-sync` | `2a61c8c79` | Remote-to-remote ref mirror without a local checkout; pack relay over smart HTTP |
| [`entireio/skills`](https://github.com/entireio/skills) | `skills` | `2f9a8758e` | Cross-agent skills: search, explain, what-happened, review, session-handoff, session-crosslink |

#### Plugins and agent extensions

| GitHub | Local checkout | Pin | Role |
| --- | --- | --- | --- |
| [`entireio/plugin-index`](https://github.com/entireio/plugin-index) | `plugin-index` | `96df95437` | Git-hosted plugin catalog (`index.json`). Official entries: `graph`, `run`, `upgrade` |
| [`entireio/entire-graph`](https://github.com/entireio/entire-graph) | `entire-graph` | `0c0f3f005` | Local tree-sitter code-map plugin: ranked search, defs, callers, types, routes, change impact |
| [`entireio/entire-run`](https://github.com/entireio/entire-run) | `entire-run` | `2558ec09d` | Plugin that launches an Entire-enabled coding agent in the current repository |
| [`entireio/entire-upgrade`](https://github.com/entireio/entire-upgrade) | `entire-upgrade` | `9b1a6fe6c` | Plugin that upgrades the system-installed Entire binary |
| [`entireio/entire-judge`](https://github.com/entireio/entire-judge) | `entire-judge` | `4c789d734` | Hackathon-jury plugin: authenticity/effort plus LLM-judged prompting and idea execution |
| [`entireio/external-agents`](https://github.com/entireio/external-agents) | `external-agents` | `83eb58122` | Standalone binaries that add extra coding agents to the CLI (Kiro, Pi, and similar) |
| [`entireio/external-agents-tests`](https://github.com/entireio/external-agents-tests) | `external-agents-tests` | `3220ca8cc` | GitHub Actions E2E tests for those external-agent binaries |
| [`entireio/roger-roger`](https://github.com/entireio/roger-roger) | `roger-roger` | `947bf4cf7` | Sample agent plugin that shows how to integrate with the Entire CLI |
| [`entireio/claude-plugins`](https://github.com/entireio/claude-plugins) | `claude-plugins` | `432bfa590` | Deprecated Claude Code plugin marketplace. Development moved to `entireio/skills` |
| [`entireio/herdr-trails`](https://github.com/entireio/herdr-trails) | `herdr-trails` | `c72f0e73b` | Herdr plugin: open an Entire trail URL into a worktree with an agent already running |

#### Git network, search, and measurement

| GitHub | Local checkout | Pin | Role |
| --- | --- | --- | --- |
| [`entireio/forgemark`](https://github.com/entireio/forgemark) | `forgemark` | `47f57bf61` | Concurrent git push/clone throughput benchmark for any smart-HTTP forge, including Entire |
| [`entireio/pgr`](https://github.com/entireio/pgr) | `pgr` | `2e702e652` | Experimental stateless MCP code-search server; research artifact, not a product |
| [`entireio/entire-search-bench`](https://github.com/entireio/entire-search-bench) | `entire-search-bench` | `f998d30ad` | Benchmark harness: clone+grep versus Entire checkpoint search |
| [`entireio/large-ref-test`](https://github.com/entireio/large-ref-test) | `large-ref-test` | `04141af7e` | Generates many lightweight `refs/entire/checkpoints/<shard>/<ULID>` refs to load-test forges |

#### Auth, CI, and distribution

| GitHub | Local checkout | Pin | Role |
| --- | --- | --- | --- |
| [`entireio/auth-go`](https://github.com/entireio/auth-go) | `auth-go` | `bb62b5de0` | Shareable Go OAuth 2.0 client: device flow, token exchange, token storage |
| [`entireio/go-nuts`](https://github.com/entireio/go-nuts) | `go-nuts` | `b16b75fc9` | NATS connection-lifecycle and JetStream consumer helpers |
| [`entireio/entire-core-auth-buildkite-plugin`](https://github.com/entireio/entire-core-auth-buildkite-plugin) | `entire-core-auth-buildkite-plugin` | `4d54eba7c` | Buildkite plugin: mint `ENTIRE_TOKEN`, install `git-remote-entire` |
| [`entireio/homebrew-tap`](https://github.com/entireio/homebrew-tap) | `homebrew-tap` | `2e558c46b` | Homebrew tap for the `entire` and `git-sync` casks |
| [`entireio/scoop-bucket`](https://github.com/entireio/scoop-bucket) | `scoop-bucket` | `1d147c724` | Scoop bucket for the Entire CLI binary |
| [`entireio/devcontainer-features`](https://github.com/entireio/devcontainer-features) | `devcontainer-features` | `c6fc3cc68` | Dev container feature that installs the Entire CLI |

#### Fixtures, demos, and org profiles

| GitHub | Local checkout | Pin | Role |
| --- | --- | --- | --- |
| [`entireio/cli-checkpoints`](https://github.com/entireio/cli-checkpoints) | `cli-checkpoints` | `0204a02aa` | Companion checkpoint object store for the CLI (sharded hex dirs; no README) |
| [`entireio/test-repo`](https://github.com/entireio/test-repo) | `test-repo` | `e55b4cf44` | Minimal sandbox for validating CLI checkpoint flows |
| [`entireio/public-private-validation`](https://github.com/entireio/public-private-validation) | `public-private-validation` | `40e307491` | Dummy public Go CLI used to test source-public / checkpoints-private split storage |
| [`entireio/hackathon-demo`](https://github.com/entireio/hackathon-demo) | `hackathon-demo` | `ff4d5109b` | Entire hackathon demo site |
| [`entireio/summer-of-ai`](https://github.com/entireio/summer-of-ai) | `summer-of-ai` | `9ed222ee3` | Placeholder for an MLH hackathon |
| [`entireio/shared`](https://github.com/entireio/shared) | `shared` | `6e5ecde24` | Empty tree except `.github`; no README |
| [`entireio/.github`](https://github.com/entireio/.github) | `entireio-github` | `4e57765ab` | `entireio` org profile, Code of Conduct, security policy |
| [`entirehq/.github`](https://github.com/entirehq/.github) | `entirehq-github` | `910e9c222` | `entirehq` org profile |

#### Master index (all 31, alphabetical)

| GitHub | Checkout | Pin |
| --- | --- | --- |
| `entirehq/.github` | `entirehq-github` | `910e9c222` |
| `entireio/.github` | `entireio-github` | `4e57765ab` |
| `entireio/auth-go` | `auth-go` | `bb62b5de0` |
| `entireio/claude-plugins` | `claude-plugins` | `432bfa590` |
| `entireio/cli` | `cli` | `b1e086253` |
| `entireio/cli-checkpoints` | `cli-checkpoints` | `0204a02aa` |
| `entireio/devcontainer-features` | `devcontainer-features` | `c6fc3cc68` |
| `entireio/entire-core-auth-buildkite-plugin` | `entire-core-auth-buildkite-plugin` | `4d54eba7c` |
| `entireio/entire-graph` | `entire-graph` | `0c0f3f005` |
| `entireio/entire-judge` | `entire-judge` | `4c789d734` |
| `entireio/entire-run` | `entire-run` | `2558ec09d` |
| `entireio/entire-search-bench` | `entire-search-bench` | `f998d30ad` |
| `entireio/entire-upgrade` | `entire-upgrade` | `9b1a6fe6c` |
| `entireio/external-agents` | `external-agents` | `83eb58122` |
| `entireio/external-agents-tests` | `external-agents-tests` | `3220ca8cc` |
| `entireio/forgemark` | `forgemark` | `47f57bf61` |
| `entireio/git-sync` | `git-sync` | `2a61c8c79` |
| `entireio/go-nuts` | `go-nuts` | `b16b75fc9` |
| `entireio/hackathon-demo` | `hackathon-demo` | `ff4d5109b` |
| `entireio/herdr-trails` | `herdr-trails` | `c72f0e73b` |
| `entireio/homebrew-tap` | `homebrew-tap` | `2e558c46b` |
| `entireio/large-ref-test` | `large-ref-test` | `04141af7e` |
| `entireio/pgr` | `pgr` | `2e702e652` |
| `entireio/plugin-index` | `plugin-index` | `96df95437` |
| `entireio/public-private-validation` | `public-private-validation` | `40e307491` |
| `entireio/roger-roger` | `roger-roger` | `947bf4cf7` |
| `entireio/scoop-bucket` | `scoop-bucket` | `1d147c724` |
| `entireio/shared` | `shared` | `6e5ecde24` |
| `entireio/skills` | `skills` | `2f9a8758e` |
| `entireio/summer-of-ai` | `summer-of-ai` | `9ed222ee3` |
| `entireio/test-repo` | `test-repo` | `e55b4cf44` |

## 2. Product thesis

Checked-in CLI README: Git shows what changed. Entire shows why — prompts,
transcripts, files touched, token usage, tool calls — indexed alongside
commits. Resume from a checkpoint. Keep the working branch clean by storing
agent context on a separate branch or ref namespace. Two-step setup (`entire
enable`). [docs]

The July 2026 hosting post restates Git's original decentralization claim and
positions Entire as a **distributed Git-compatible network** so agent fleets
can clone without GitHub rate limits. GitHub remains source of truth. Regional
cells (US, EU, Australia named) absorb concurrent read traffic. Branches
prefixed `entire/unmirrored/` stay in-region and are not forwarded to GitHub.
[public]

Thomas Dohmke (ex-GitHub CEO) is the public face. Plugin commits in
`entire-graph` carry his name. [history] [public]

This is the same problem episode 259 names as verifiable software and episode
252 names as "when an agent says it finished, how do you know?" Entire's
answer is **git-native session capture**. OpenAgents' answer is **server-side
receipts plus a forge WAL**. They rhyme. They must not be merged into one
store. [inferred]

## 3. CLI: sessions, checkpoints, trailers

### 3.1 Domain

A **session** is one agent interaction (`YYYY-MM-DD-<UUID>`). A **checkpoint**
is a save point inside it (12-hex under `git-branch`, 26-char ULID under
`git-refs`). Low-level types: **ephemeral** (full worktree snapshot on a
shadow branch) versus **persistent** (metadata plus commit reference after
condensation). [schema] [docs]
`docs/architecture/sessions-and-checkpoints.md`

Active session state lives in `.git/entire-sessions/<id>.json` (git common
dir, shared across worktrees). A background zombie sweep finalizes sessions
whose agent process has exited. A 7-day purge bounds the window. `entire
session adopt` moves a live session across worktrees. [docs]

### 3.2 Ephemeral versus persistent

| Kind | Location | Contents |
| --- | --- | --- |
| Session state | `.git/entire-sessions/<id>.json` | Active tracking, last branch, worktree |
| Ephemeral | branch `entire/<commit[:7]>-<worktreeHash[:6]>` | Full worktree + `.entire/metadata/<session>/` |
| Persistent (`git-branch`) | `entire/checkpoints/v1`, tree `<id[:2]>/<id[2:]>/` | `metadata.json`, transcripts, prompts |
| Persistent (`git-refs`) | `refs/entire/checkpoints/<shard>/<id>` | Same subtree as the commit tree root |

Shadow branches are **not pushed**. Docs warn: do not push them; unredacted
source blobs would land on the remote. Gitignored files are filtered from
snapshots; everything else in the worktree is raw. [docs]

Multiple concurrent sessions in one directory share one shadow branch and
interleave. Post-commit condenses **all ACTIVE sessions**, including ones that
did not contribute — spurious sparse checkpoints rather than data loss. The
workaround is separate worktrees. [docs]
`docs/KNOWN_LIMITATIONS.md`

### 3.3 The trailer join

`prepare-commit-msg` appends:

```text
Entire-Checkpoint: a3b2c4d5e6f7
Entire-Attribution: 73% agent (146/200 lines)
```

`post-commit` condenses the shadow branch into the persistent store. Lookup
is bidirectional: commit message → checkpoint tree; checkpoint ID → `git log`
trailer scan. Users can delete the trailer. `git commit --amend -m` can drop
it; session state `LastCheckpointID` usually restores it. [docs] [source]

Attribution is **inferred from diffs and hook timing**, not keystrokes. Docs
state the interleaving problem honestly and classify human_modified as
`min(add, remove)` without per-hunk pairing. Do not treat the percentage as
proof of authorship. [docs]
`docs/architecture/attribution.md`

### 3.4 git-refs backend

Why it exists: the v1 branch is a serialization point. git-refs keeps one ref
per checkpoint, sharded on the **last two characters** of the ID (hex and
ULID). Writes create an orphan commit on first save, then parent the previous
tip. A flock-protected JSONL push queue in the git common dir
(`entire-checkpoint-push-queue.jsonl`) tracks dirty refs. Pre-push
batch-pushes, falls back per-ref, **never force-pushes**. Non-fast-forward
recovery fetches the remote ref and replays local-only commits on top. Reads
fetch a missing ref on demand. `List` can ls-remote names only
(`WithRemoteListDiscovery`) so a second machine discovers ULIDs without
transferring objects; branch views still require the trailer on a pulled
commit. [docs] [schema]
`docs/architecture/ref-checkpoint-backend.md`

Primary plus optional write-only mirrors. Only git-backed backends can be
primary. Kind routing: ULID → refs only; hex with refs primary → refs then
branch fallback. Creates go to the configured primary. [docs]

This is the interesting Git-as-log engineering. It is still Git. It is not a
receipt. [inferred]

### 3.5 Hooks, agents, plugins

`entire enable` installs git hooks and prompts for agent hooks. Agent coverage
claimed: Claude Code, Codex, Gemini CLI, OpenCode, Cursor, Factory Droid,
Copilot CLI, Pi, plus `external-agents` binaries. External **commands** are
kubectl-style: `entire <name>` execs `entire-<name>` on `PATH`. Names
beginning `agent-` are reserved for the agent protocol. Plugin install is
forge-agnostic (`git ls-remote --tags`), verifies `checksums.txt`, writes
`binary_sha256` before linking `bin/`, and validates repo URLs against
`--upload-pack` / `ext::` / `file://` attacks. [docs] [source]
`docs/architecture/external-commands.md`

Checkpoint commit signing is **best-effort**: GPG or SSH when
`commit.gpgsign` is on; failure logs and continues unsigned. [docs]
`docs/architecture/checkpoint-signing.md`

### 3.6 Redaction

Always-on secret passes (entropy, Betterleaks and/or goredact, provider
prefixes, credentialed URIs, DSNs, bounded credential values) run before
persistent writes. Scanner selection is committed in `.entire/settings.json`
and ignored from `settings.local.json` so one developer cannot silently
weaken history. Disabling both scanners is a load error. PII is opt-in.
OpenAI Privacy Filter (`opf`) is opt-in and runs **only at push**, not per
turn. Public repos still publish whatever survived those filters. [docs]
`docs/security-and-privacy.md`

## 4. Mirror network and git-remote-entire

### 4.1 Clone path

Documented:

```sh
entire repo clone /gh/entireio/cli
git clone entire://aws-us-east-2.entire.io/gh/OWNER/REPO
```

`git-remote-entire` is a small dedicated binary. Git execs it as
`git-remote-entire <remote-name> <url>`. Stdout is the pkt-line protocol
only; diagnostics go to stderr. Auth resolves cluster cores from
`cluster_cores.json` or `/.well-known`, then uses the login JWT (or
`ENTIRE_TOKEN` in CI) as the git-transport bearer. User-Agent and git
`agent=` capability share one version string. [source]
`cmd/git-remote-entire/main.go`

Live probe 2026-08-23:

| Probe | Result |
| --- | --- |
| `https://aws-us-east-2.entire.io/gh/entireio/cli/info/refs?service=git-upload-pack` | HTTP 401, `www-authenticate: Basic realm="Entire Git Server"`, body `Missing Authorization header`, node header `x-entire-node-name` |
| Same URL with empty Basic | HTTP 401, `Invalid or expired token` |
| `git clone entire://…` with helper on `PATH`, not logged in | `fatal: no auth context for cluster aws-us-east-2.entire.io` |
| `entire repo clone /gh/entireio/cli` not logged in | `not logged in; run entire login` |
| `GET https://entire.io/api/v1/repositories` | HTTP 401 `{"error":"Not authenticated"}` |

There is no anonymous Entire clone. [runtime]

### 4.2 Mirror semantics

`entire repo mirror create github.com/OWNER/REPO [cluster-host]` registers a
read-optimized copy. Default cluster `aws-us-east-2.entire.io`. Idempotent per
repo and region. Pushes through the mirror still authenticate to GitHub.
`entire/unmirrored/*` branches stay in-region, are not backed up, and are not
on GitHub. Access follows GitHub collaborators, synced hourly. LFS is
unsupported; workaround is `git config lfs.url` back to GitHub. [docs]
`docs.entire.io/guides/repositories/`

OpenAgents taxonomy collision to keep straight:

| Entire word | Entire meaning | OpenAgents word |
| --- | --- | --- |
| origin | GitHub | forge |
| mirror | EntireDB regional cell | GitHub (`MirrorWatch` export) |
| `entire://` remote | authenticated smart-HTTP to a cell | not a product remote |
| unmirrored branch | in-region only, no GitHub forward | do not invent an analog that bypasses the WAL |

A push on OpenAgents never promotes itself. A push on Entire through a mirror
is still a GitHub push. Different planes. [inferred]

## 5. git-sync and forgemark

### 5.1 git-sync

`git-sync sync` / `replicate` / `plan`. In-memory go-git object store. Smart
HTTP only. Bootstrap of an empty target and incremental fast-forward both
prefer **pack relay** (source `upload-pack` streamed into target
`receive-pack` without materializing the graph). Force, prune, deletes, tag
retargets fall back to a materialized path bounded by
`--materialized-max-objects`. `replicate` is source-authoritative, relay-only,
fails rather than materialize. Typed JSON results. Library module
`entire.io/entire/git-sync`. Release provenance via `gh attestation verify`.
[docs] [source]

This is the pattern donor for **forge → GitHub MirrorWatch** and any future
cell-to-cell Git movement. Keep planning, relay eligibility, and typed
results. Keep OpenAgents' WAL as the admission log; git-sync must not become
a second writer. [inferred]

### 5.2 forgemark

Concurrent real git push/clone load against any smart-HTTP host, including
Entire and github.com. go-git packfile + ref update. Local GUI
`forgemark serve` on loopback. Tokens are never typed into the browser; the
server reads `gh` / `glab` / `entire` CLI logins. Demo targets
`demo://fast?p50=60ms`. Push-race preset: 16 concurrent agents, checkpoint-
sized commits. [docs]

Use against **owned** forge infrastructure only, as the README warns. This is
the right tool for "agent fleet versus git plane" once the forge is the thing
under load. Do not run it against github.com or Entire production. [docs]

## 6. Skills, graph, search

`entireio/skills` packages agent-invokable workflows: `search`, `explain`,
`what-happened` (blame + checkpoint), `review` (intent-aware findings with
severity, compatible with `entire review --fix`), `session-handoff`,
`session-crosslink`, `using-entire` as router. Install via skills.sh, Claude
marketplace, `~/.agents/skills`, Cursor, Copilot, Gemini extensions, OpenCode.
Useful only when checkpoints exist. Remote search needs `entire login` and
pushed, indexed checkpoints. [docs]

Episode 259's "investigate why code exists" and episode 255's FastFollow
thread fabric land here as productized skills, not as OpenAgents contracts.
Harvest the **workflow names and the rule "read checkpoint history before
inferring from code."** Implement them against forge receipts and issue
timelines, not Entire's branch. [inferred]

`entire-graph` is a CLI plugin: local tree-sitter index, no model calls at
index time, ranked search plus definitions, callers, types, routes, change
impact with `file:line`. Claims first place on a LoCoMo comparison (94.74,
0 index-time tokens, measured 2026-08-14). Installing the plugin is the
networked step. [docs] [public]

`pgr` is explicitly a research MCP server wrapping `rg` with agent-shaped
ranking. OpenAgents already de-emphasizes MCP as a control plane. Treat pgr
as a ranking paper, not an integration. [docs]

## 7. Mapping onto current OpenAgents work

From episode triage dispositions, Entire touches these live and unfinished
threads:

| OpenAgents thread | Entire overlap | Disposition |
| --- | --- | --- |
| Turn receipts, tool steps, TURN-001..005 | Session + checkpoint + transcript.jsonl | **Live** on OpenAgents as PostgreSQL. Entire is a git-shaped peer, not a store to adopt. |
| Forge WAL / REPOSITORY-002 | git-refs append-only, never force; git-sync relay | **Harvest** queue, ff-only, relay. Keep forge as authority. |
| MirrorWatch (forge → GitHub) | git-sync, Entire GitHub → cell (inverse direction) | **Dust off** as an implementation reference for the export path. |
| "When an agent says it finished" (252, 259) | trailer + checkpoint tree as the claim | **Reimagine** onto issue-linked receipts. Do not use git trailers as the only proof. |
| FastFollow / Amp thread fabric (255) | skills search/explain/handoff, checkpoint resume | **Dust off** as FastFollow source. Typed gap items, not a vendored CLI. |
| Public traces as RL data (228, 237) | checkpoints on public GitHub | **Retired as default-public.** Entire's honesty about internet-visible transcripts is the cautionary tale. Opt-in trace markets (NIP-DS dust-off) stay the revival path. |
| Product Promises registry (234) | `entire review`, attribution trailers | Weak overlap. Promises stay forge projects with LIVE/GATED/WITHDRAWN. |
| Feature-request recorder (211) | none | No Entire analog. |
| Guidance Module (206) | none | Entire captures; it does not steer between turns. |
| GetAfter / NIP-34 GitHub alternative (243) | Entire hosting | **Retired.** Forge already shipped. Entire hosting is a GitHub mirror, not a GitHub replacement. |
| Agent-fleet git load (implicit in SCV/work jobs) | forgemark | **Dust off** as a test harness against the forge git plane. |

Pivot watch: Entire still treats **GitHub as origin**. OpenAgents does not.
Any sentence that says "the remote" in an Entire-inspired design must name
forge or GitHub. [inferred]

## 8. What to harvest, what to reject

Harvest:

- Commit-to-evidence **join key** (trailer or equivalent header on the forge
  push receipt) so `why this SHA` is an index lookup, not a grep.
- Split **ephemeral intra-session snapshot** from **persistent condensed
  record**. Do not confuse live session JSON with the receipt.
- **One-object-per-checkpoint** (or per-turn) identity with ff-only update
  and rebase-on-divergence. No force, no silent clobber.
- **Push-discovery queue** (common-dir, flock, JSONL, remove only after
  confirmed push).
- **git-sync** relay-first remote-to-remote mirroring as the MirrorWatch
  implementation sketch.
- **git-remote helper** as the admission pattern if OpenAgents ever exposes a
  custom git URL scheme. Stdout is the protocol; banners are corruption.
- **Plugin URL allowlist** (`validatePluginRepoURL`, reject `ext::`,
  `--upload-pack`, `file://` in production).
- **Fail-closed redaction policy** as a team-visible committed setting, not a
  local override.
- **Skills that refuse to explain code without history.** Port the workflows
  onto receipts.
- **forgemark** against owned forge cells.

Reject:

- Git (GitHub or EntireDB) as the durable store for prompts and transcripts.
- Default-public checkpoints.
- Unredacted worktree snapshots on any branch that can be pushed.
- Best-effort signing of evidence commits.
- `entire://` as `origin` for OpenAgents repositories.
- Entire unmirrored branches as a way around GitHub rate limits **and** around
  the WAL.
- Attribution percentages as authorship proof.
- MCP search (`pgr`) as a control-plane integration.
- Entire's web app or control plane as a substitute for openagents.com.
- Cloning or vendoring Entire source into product authority (MIT is copyable;
  the catalog rule still prefers pattern over code unless a typed contract
  admits a pin).

## 9. FastFollow note

Add Entire to the FastFollow catalog as a **session-evidence and git-mirror
peer**, not an IDE and not a forge. Ordered lessons:

1. Trailer (or receipt) join from commit SHA to agent context.
2. Ephemeral versus persistent checkpoint stores.
3. Per-ref (per-turn) logs that never force-push.
4. Remote-to-remote pack relay without a warm local mirror.
5. Custom git URL scheme admitted only through a dedicated remote helper.
6. Agent-fleet smart-HTTP load tests on owned infrastructure.

Do not FastFollow Entire hosting, the entire.io dashboard, or GitHub-as-origin
semantics.

## 10. Limitations of this audit

Source-and-probe only. EntireDB, logged-in clone performance, collaborator
sync, unmirrored-branch behavior, and the private `entirehq/*` API were not
exercised. Agent-hook correctness across Claude/Codex/Cursor was not run.
LoCoMo numbers for `entire-graph` are vendor-reported with a linked methods
doc; they were not reproduced here. [limitation]
