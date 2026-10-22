# Kiln workstation: spec

Status: spec, 2026-10-21. Owner direction relayed by the coordinator.
Phases 1 and 2 are built (#12432, #12437). Phase 3's agent identities,
the repository announcement, and runner admission (#12449, #12451), phase
4's maintainer apply (#12452), phase 5's GitHub bridge and pull requests
(#12453, #12455), and a lasting runner on `oa-kiln` (#12454) are built;
see [maintainers.md](maintainers.md). Phase 6, Kiln at the Grid terminal
(#12456), is built for site admins; see [Phase 6 as built](#phase-6-as-built).
Phase 7, runners on Pylons (#12457), is built with free runs; see
[rust-tests.md](rust-tests.md#many-runners-chosen-like-pylons). NIP-07 and
NIP-46 signing for players and paid runner time are not.

The [Grid Foreman standing-objective plan](../foreman/grid-foreman-plan.md#14-standing-project-objectives-and-kiln)
puts Kiln under Foreman's task-selection loop. Foreman chooses a bounded
task from an issue or an ongoing project objective; Kiln edits and
proposes, a runner checks, and the bridge opens a draft PR. A persistent
native Kiln host and Psionic workspace/device runner profiles are still
needed for that unattended path. The browser's `start_headless` entry
point is a presentation option, not that native host.

## The vision

The computer terminal that stands in front of the Pylon Bay in the Verse
becomes a **Kiln workstation on the OpenAgents codebase**, connected to
Pylons:

1. **Its own filesystem.** Each visitor has a persistent Kiln workspace in
   OPFS: the repository, or slices of it, over a shared read-only base at a
   pinned commit.
2. **Access control.** Anyone can browse and propose. Runner time, spend,
   and anything that reaches the real repository are gated by the player's
   Nostr identity: the owner-signed mandate from
   [the browser-agent research](../research/browser-agent/README.md#proposal-a-tag-plus-a-mandate-enforced-in-three-places)
   for spawned agents, and allowlists and roles for maintainers.
3. **Purpose.** A player, or an agent the player spawned, edits code, runs
   `cargo test` through the sandboxed remote runner, and **proposes the
   change as a Nostr pull request** (NIP-34).
4. **Inference** comes from Pylons, with no vendor key, as today.

## What exists (phase 1)

- The Kiln page: WASI tools in workers, an OPFS workspace, the agent loop,
  and a NIP-CJ buyer for Pylons ([README](README.md)).
- A crate of this repository loaded into the tab at a commit, and a remote
  `cargo test` on `kiln-runner` inside `coder-boundary`
  ([rust-tests.md](rust-tests.md)).
- `nostr::domain::git` already reads every NIP-34 kind:
  `open_repository` (30617), `open_repository_state` (30618), `open_patch`
  (1617), `open_pull_request` (1618 and 1619), `open_issue` (1621),
  `open_patch_status` (1630 to 1633), and `applied_commits`. It builds for
  `wasm32`, so the page and a viewer use the same reader.

## NIP-34 for this repository

The pinned spec is [`nips/official/34.md`](../../nips/official/34.md).

| Kind | What we publish | Signed by |
| --- | --- | --- |
| `30617` repository announcement | `d` = `openagents`; `name`, `description`; `web` and `clone` = `https://github.com/OpenAgentsInc/openagents`; `relays` = `wss://relay.openagents.com`; `r` = the earliest unique commit, marked `euc`; `maintainers` = the maintainer keys; `t` = `rust`. | A repository key the owner holds (not a page key). Published once and on changes to maintainers or relays. |
| `30618` repository state | `refs/heads/main` = the current `main` commit; `HEAD` = `ref: refs/heads/main`. | A state publisher listed in `maintainers`, run beside the land queue, so Kiln knows the base commit to load. |
| `1617` patch | `git format-patch` text of a Kiln change (under 60 KB); `a` = `30617:<repo key>:openagents`; `r` = euc; `p` = repository key; `t` = `root`. | The player's key, or their agent's key with an `auth` tag and a mandate that allows `git.propose`. |
| `1618` pull request | For changes past 60 KB: `subject`, `c` = tip commit, `clone` = where that commit can be fetched, `branch-name`, `merge-base` = the base commit. | As `1617`. Needs a git host for the commit; see phase 5. |
| `1619` pull-request update | A new tip after review. | The PR's author. |
| `1621` issue | An agent or player reporting a problem found while working. | Any key. |
| `1111` comment (NIP-22) | Review comments, and the runner's check result on a patch. | Anyone; maintainers' and listed runners' comments get badges. |
| `1630` to `1633` status | Open, applied or merged, closed, draft. | The author or a maintainer; the newest valid one wins, per NIP-34. |

### How a Kiln change becomes a patch

1. **Diff in the tab.** The page already keeps the untouched copy of the
   slice. A new `kiln::patch` module builds a unified diff for each changed
   file (a pure-Rust line diff, which builds for `wasm32`) and wraps it in
   `git format-patch` form: the `From <id> Mon Sep 17 00:00:00 2001`
   header, `From:` (the display name and `<npub>@nostr`), `Date:`,
   `Subject: [PATCH] <summary>`, the body, the diffstat, and one
   `diff --git` section per file with `index <old blob>..<new blob>` lines.
   Blob ids are git's SHA-1 of `blob <len>\0<content>`, so `git am -3` can
   fall back to a three-way merge. The page builds no commit object, so the
   optional `commit` and `parent-commit` tags are left out.
2. **Green first.** The publish button is enabled only after a
   `cargo test` on the runner passed for exactly these files at this base.
   The runner signs a NIP-22 comment on the patch after it is published:
   the base commit, the SHA-256 of the patch text it applied, the command,
   the summary, and the time. Reviewers trust runner keys the repository
   lists (a `kiln-runner` tag in `30617`, or a maintainer-signed list).
3. **Sign.** The page never holds the player's long-term secret:
   - **The player** signs through a NIP-07 extension, a NIP-46 remote
     signer, or the phone or desktop app, as `verse_key.rs` does today.
   - **An agent** signs with its own key; its patch carries the NIP-OA
     `auth` tag and the mandate's digest, and the mandate's action set
     includes `git.propose` (kinds 1617, 1618, 1619, 1111, and 1621). An
     agent's mandate never includes status kinds, so it can't mark anything
     merged.
   - **An unlinked page key** (today's demo) may publish, but readers label
     the patch "unverified author" and the bridge ignores it.
4. **Publish** to `wss://relay.openagents.com`, the relay the announcement
   names.

### Review

A **viewer** lists patches and PRs by the repository's `a` tag: author
(with "authorized by" for agents), subject, the diff with syntax
highlighting, the runner's check comment, NIP-22 threads, and the newest
valid status. In phase 2 it's a page in the Kiln app; later it's a Rust
Native card the Grid terminal shows and the desktop app reuses.
Maintainers reply with NIP-22 comments and publish status events. Status
from a key outside `maintainers` (other than the author's own close or
draft) is shown but not counted.

### From a merged Nostr PR to `main`

Two paths, not exclusive:

- **A. Maintainers apply (phase 4, recommended first).** An
  `openagents git apply EVENT` command fetches the patch from the relay,
  checks its signature, its runner check, and its base, runs `git am -3` in
  a worktree, runs the crate's tests, and lands it through the normal land
  queue with the author credited (`Co-Authored-By` with the npub, and the
  event id in the trailer). It then publishes `1631` with
  `applied-as-commits`. No GitHub token is involved anywhere new.
- **B. A bridge (phase 5).** A service with a GitHub token watches the
  relay for patches and PRs that a maintainer marked `1630` (open), or
  that come from allowlisted keys. It pushes a branch `nostr/<event id>` to
  a fork, opens or updates the GitHub PR with a link back to the event,
  mirrors comments both ways, and publishes `1631` with `merge-commit` when
  GitHub merges it. It never acts on unreviewed events, so spam can't
  reach GitHub. For `1618` PRs, the same service is the git host its
  `clone` tag names.

**Who can merge:** only maintainers, through GitHub or the land queue,
exactly as today. A Nostr status event records a merge; it never causes
one. No agent key, runner, or bridge can merge on its own.

## Access control

| Action | Who | Gate |
| --- | --- | --- |
| Browse the base, read patches | Anyone | None. The base is public. |
| Edit in your own workspace | Anyone | Local to the visitor's OPFS; nothing leaves the tab. |
| Ask Pylons for free inference | Anyone | Pylon admission and rate limits, as today. |
| Run `cargo test` on a runner | Anyone, within a quota | Per-key quota on the runner: a small anonymous quota (with NIP-13 proof of work on the job), a larger one for keys with an owner attestation, and none for maintainers. Agents draw from their owner's quota under the mandate. |
| Paid inference or paid runner time | Linked players and their agents | The mandate's spend cap, enforced by the Pylon or runner and by a sub-wallet holding only the cap. |
| Publish a patch, PR, issue, or comment | Anyone | Relay rate limits per key and per owner; unlinked keys labelled. |
| Mark merged or closed | Maintainers (and the author for close or draft) | `maintainers` in `30617`; readers ignore other status. |
| Land on `main` | Maintainers | GitHub review or the land queue. |

Roles live in signed events, not in the page: maintainers in the
announcement, runner keys and quota tiers in a maintainer-signed list.

## Phases

| Phase | What ships | Done when |
| --- | --- | --- |
| **1. Rust tests** (built) | Load a crate at a commit; remote `cargo test` in `coder-boundary`; the agent edits Rust until green. | The agent turns a red test green on `memory-stream` from the tab. |
| **2. Publish and view** (next demo) | `kiln::patch` (format-patch text); **Publish patch** after a green run; the runner's check comment; a simple viewer page that reads `1617`, the check, comments, and status back from the relay. Signed by the page key, labelled unverified. | From the Kiln page, an agent makes a small change, gets green `cargo test`, publishes a `1617`, and the viewer shows it as a reviewable patch. |
| **3. Identity** | NIP-07 and NIP-46 signing for players; agent keys with NIP-OA `auth` and a mandate allowing `git.propose`; the repository's `30617` and `30618` published by the owner's keys; runner quotas by identity. | A patch shows "by `<player>`" or "agent, authorized by `<player>`". |
| **4. Maintainer apply** | `openagents git apply EVENT`; `1631` with `applied-as-commits`; author credit. | A Kiln patch lands on `main` through the land queue. |
| **5. Bridge and PRs** | The GitHub bridge for maintainer-opened events; `1618` PRs with a git host; `1619` updates; comment mirroring. | A Nostr PR opens a GitHub PR, and the merge comes back as `1631`. |
| **6. The Grid terminal** | The workstation as the terminal in front of the Pylon Bay: the Kiln page's views as Rust Native cards in the Grid; per-visitor workspaces over a shared base cached by blob id; more of the tree (sparse gitoxide reads). Coordinated with the agents who own `crates/openagents-web/src/pages/grid_home.rs` and the lander chat. | A player walks up to the terminal and does phase 2 there. |
| **7. Runners on Pylons** | Runner capacity as a Pylon service with its own beacon lane, paid under NIP-X402, checked like other Pylon work. | Many runners, chosen like Pylons. |

## Phase 6 as built

A site admin walks up to the terminal in front of the Pylon Bay on `/`.
The terminal's chat opens, with **Kiln** beside the model choice. Kiln
opens in a panel the GPUI chat draws over the world; nothing in it is HTML
(#12459):

- **Cards.** Task, Crate, Patch, and Files are Rust Native views from
  `crates/kiln-view`, drawn by the shared GPUI adapter
  (`openagents_chat_view::rust_native_gpui`), so the desktop app can mount
  the same cards. The task and directory fields are the chat's own text
  inputs, supplied through the adapter's field hook.
- **The run.** Kiln's transcript, `openagents-web-kiln-transcript`, is
  embedded in the panel: thinking blocks, tool cards with diffs, and the
  test card.
- **The engine.** The page loads Kiln's module from `/admin/kiln/pkg/`
  when the panel first opens and starts it without markup
  (`start_headless("/admin/kiln/")`). Kiln keeps its OPFS workspace, tool
  worker, Pylon client, and runner jobs, publishes its state as JSON
  (`kilnState()`, the `kiln_view::State` shape), and takes the cards'
  presses through `kilnAgent`, `kilnStop`, `kilnLoad`, and `kilnPublish`.
  It keeps working with the panel closed.
- **In the world.** While Kiln works, the terminal's screen in the world
  shows "KILN AT WORK:", the crate, and "PROPOSING A CHANGE" for that
  visitor (`openagentsKilnScreen`, `deploy_board::screen::kiln_lines`).
- **The page's policy.** `/` allows `worker-src 'self'` for Kiln's tool
  worker and `connect-src` to `api.github.com` and
  `raw.githubusercontent.com` for public crate reads.

Checked locally on 2026-10-22 against the synthetic-account preview
(`browser_preview` with `OA_GAME_PREVIEW_ADMIN=1` and
`OA_GAME_PREVIEW_KILN=DIR`): walk up, open Kiln, load
`crates/memory-stream`, and run the default task. DeepSeek V4.1 Flash
edited the crate, the local runner's `cargo test -p memory-stream` passed
(6 tests), and Kiln proposed patch `f13cbaebfb14` with the runner's
passing check, 627 s after Run. Captures are in
[captures/2026-10-22-grid](captures/2026-10-22-grid/): the terminal with
Kiln beside the model choice (`terminal.png`), the cards and the run while
it works (`working.png`), the terminal's screen in the world
(`screen.png`), the proposed patch (`proposed.png`), and the panel at phone
width (`phone.png`).

Since #12488, signed-in players get it too when the site has its Kiln key:
the site vouches for each player's browser key for a day, and runners list
the site key with `each=N`, so every player's key has its own quota
([kiln-admin.md](../deployment/kiln-admin.md#kiln-for-players)). Files
load through this browser's blob cache, keyed and checked by git blob ID,
and a path outside the loaded crate that `ls`, `view`, or `grep` names is
read from the repository at the loaded commit the first time. Checked
locally the same way as above with a player who isn't an admin: the site
vouched for the browser's key, the runner admitted it as "agent of a listed
key, counted alone", and Kiln proposed patch `7fb989310556`
(`player-proposed.png`).

## Risks

- **Spam and abuse.**
  - Patches are cheap to publish. The viewer shows runner-checked
    patches first and hides unlinked ones by default.
  - The relay rate-limits per key and per owner.
  - The bridge acts only after a maintainer opens an event, so GitHub never
    sees spam.
  - Runner jobs cost real CPU: per-key quotas, proof of work for anonymous
    keys, one job at a time per runner, and the crate allowlist bound it.
- **Secrets.**
  - The page holds no token: GitHub reads are public and anonymous, and
    inference is on Pylons.
  - The runner clears the environment, can't reach the network, and can
    read only the worktree, the Cargo caches, the toolchains, and Cargo's
    configuration files (never `credentials.toml`), so a test can't read a
    key and print it into its log.
  - Before publishing, the page scans the patch for key-shaped strings
    (`nsec1`, `sk-`, `ghp_`, PEM blocks), and the viewer repeats the scan.
  - The bridge's token lives only on the bridge host.
- **Runner sandboxing.**
  - Untrusted code runs at build time (`build.rs`, procedural macros) and
    test time. All of it runs inside the boundary with no network,
    confined reads and writes, a 600-second cap, and a 32 MB log cap.
  - Jobs can't change `Cargo.toml` or `build.rs`, and only touch files
    inside an allowlisted crate.
  - Still open: no memory or disk cap on the target directory
    (`supervise` has a memory limit to wire in), and the runner reuses one
    target directory, so one job's build output can influence the next.
    Phase 7 needs a fresh target per tenant or a content-checked cache.
  - The runner should run on a machine that holds nothing else of value;
    on the owner's Mac it's a demo only.
- **Who can merge.**
  - Only maintainers, outside Nostr.
  - Status events are advisory; readers count only maintainer status.
  - Agent mandates exclude status kinds.
  - The land queue reruns the tests rather than trusting the runner's
    comment.
- **Base drift.** A patch names its base; `git am -3` and the maintainer's
  rerun catch conflicts. The viewer marks a patch whose base is far behind
  `main`.
- **Model limits.** The DeepSeek V4.1 group takes at most 2,872 prompt
  tokens today, so the agent sees a small window of the code and reads it
  in ranges. Larger tasks need a group with a longer prompt limit.

## Experiment: `rustc` in the tab

Not run. Rubrc, the one working `rustc`-as-WebAssembly in a browser, has
no license, so we can't ship or adapt it; it also handles no external
crates or procedural macros. The Rust project doesn't publish a `rustc`
for `wasm32-wasip1`, and building one means building LLVM for WASI. A
type-check-only path for a dependency-free file would still need the whole
front end. The remote runner covers the need today; a licensed in-tab
checker stays a research item.
