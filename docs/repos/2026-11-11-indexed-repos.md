# Indexed repos: a codebase a chat can read and change without cloning it (spec)

Status: accepted, 2026-11-11, and revised the same evening (see
[What changed in review](#what-changed-in-review)). Phase 1 is tracked in
[#13326](https://github.com/OpenAgentsInc/openagents/issues/13326). Code was first read at `0942bd418b`.

## Summary

An **indexed repo** is a repository that one OpenAgents service keeps a copy
of and answers questions about. A chat's agent reads files, searches, looks
at history and proposes changes through a small set of tools. The agent
never clones the repository, holds no git credentials and needs no disk.

Three things make it up:

- **A mirror.** One service on our Google Cloud project holds a bare git
  copy of each repository and keeps it current.
- **An index.** Per file version, the service keeps what makes questions
  fast: line counts, symbols and, later, embeddings.
- **An API.** Read operations against one exact commit, and write
  operations that build a change on the server and end in the land queue,
  checked like any other change.

The first users are the web chat at `/chat` and Microcoder, which today can
only work inside a directory on the machine it runs on.

## Why

What exists today, read in the code:

- **`/chat`'s Gemini models have no tools.** The rebuilt page
  (`apps/openagents-web/chat-web.coax`) asks one model with the conversation
  so far. It can't read a file.
- **Its Claude models read one baked-in checkout.** A Claude turn, and
  every turn of `/chat-gpu`, runs the Claude CLI on the Cloud Run runner
  (`coax/web/agent/chat-runner.c`). Measured on a developer's Mac on
  2026-11-11: the first turn spent 29 seconds making that copy, and it took
  2.1 GB of disk. The
  runner's image holds a checkout of the public repository, and each turn
  fetches what `main` changed. The image build fails over 1,200 MB, the
  runner keeps 4 GiB of memory for it, and a second repository needs a
  second checkout in the image (`docs/chat.md`, "Folders").
- **That agent can't change anything.** Edit, Write and Bash are refused.
- **The project agent clones per turn.** It makes a shallow clone into a
  temporary directory, then commits and pushes to `main` with a deploy key
  (`docs/project-agent.md`).
- **Microcoder needs a working directory.** Its tools are shell commands,
  file views and exact edits under one directory
  (`coax/lib/microcoder/tools.coax`, `action.coax`).

So every hosted agent today either carries the repository with it or can't
touch one. A second repository, a private repository, or a chat that starts
in under a second all run into that.

## Goals

- A chat on any surface can read a repository it has access to, at an exact
  commit, with answers it can cite.
- The same chat can propose a change, which lands through the same checks
  as a person's.
- An agent run needs no clone, no git, no disk and no repository credential.
- Microcoder works against an indexed repo with its loop and action schema
  unchanged.
- Adding a repository is configuration, not a new image.

Not goals, for now:

- Running arbitrary commands against the repository. Checks are a named,
  allowlisted set, run on our build hosts (see [Checks](#checks)).
- Pushing to `main` directly. A change is submitted to the land queue
  (`docs/verse/green-main-pipeline.md`), which checks it on the head it
  lands on. A person approves a queued change from an agent before it
  lands.
- Replacing a local checkout for Coder on your own machine.

## Terms

- **Repo.** A repository the service mirrors, named by a short slug such
  as `openagents`.
- **Snapshot.** One commit of a repo. Every read names a snapshot by its
  full commit id. A branch name is accepted and answered with the commit it
  resolved to.
- **Change.** A set of edits on top of a snapshot, kept on the server
  until it is discarded or committed.
- **Grant.** What one session may do: which repo, and `read` or `propose`.

## What a chat can do

Read operations. Each takes a repo and a snapshot (or a change, to read
through its edits):

| Operation | What it returns |
| --- | --- |
| `resolve` | The commit a branch or tag points at, its time, and which index parts are ready for it |
| `tree` | The entries under a path, to a depth: path, kind, size, line count |
| `read` | A file's text with line numbers, whole or a line range. Several files or ranges may be asked for in one call, so an agent that wants ten files pays one round trip |
| `find` | Paths that match a glob |
| `search` | Lines that match a literal or a regular expression, optionally under given paths, with context lines |
| `log` | Commits, optionally for one path |
| `show` | One commit: its message, the files it changed and its diff |
| `diff` | The difference between two snapshots, optionally for one path |
| `blame` | Who last changed each line of a range, and in which commit |
| `symbols` | Where a name is defined and used, or the outline of one file (phase 3) |
| `similar` | Passages close in meaning to a text (phase 4) |
| `ask` | A written answer to a question about the repo, with the passages it rests on (phase 4, if it is needed) |

Write operations. They need a `propose` grant:

| Operation | What it does |
| --- | --- |
| `change_open` | Starts a change on a snapshot |
| `change_edit` | Replaces one exact passage in a file. The passage must appear once. An empty passage writes the whole file. This is Microcoder's edit |
| `change_delete`, `change_move` | Removes or renames a file |
| `change_diff` | The change so far, as a diff |
| `change_check` | Runs one named check on the change and streams its output (phase 3) |
| `change_commit` | Makes the change one commit, with its message and trailers |
| `change_propose` | Submits that commit to the land queue, where it waits for a person's approval and then for its checks |
| `change_discard` | Drops the change |

`ask` is the one operation that runs a model. It starts a read-only
Microcoder run against the same repo and snapshot and returns its answer.
A chat model with a small context can then question a large codebase
without holding its files. It is last, and conditional: a model inside a
tool costs a second run, adds its latency to the turn, and shows the person
its conclusion, not its evidence. It is built only if a small-context model
is measured failing at questions the read operations could answer.

### What every answer obeys

- **It names its snapshot.** Each answer carries the commit id it read, so a
  conversation's record says exactly what the agent saw, and an answer can
  cite `path#L10-L42` at that commit.
- **It is bounded.** A read returns at most 64 KB, the limit Microcoder's
  view already uses. A search returns at most a set number of matches. An
  answer that was cut says so and says how to get the rest.
- **It is the same every time.** Results are in a fixed order, so the same
  question at the same snapshot gives the same bytes. That lets the service
  cache answers, and keeps an agent's prompt stable for the model's cache.
- **Binary and large files are named, not returned.** `read` on one gives
  its kind and size.

## Changes: writing without a clone

A change is a real git commit on a hidden ref in the service's mirror
(`refs/oa/changes/ID`), with the snapshot as its parent.

- **An edit** writes the new file contents as a git object and moves the
  ref to a new commit. The service builds the tree with git's plumbing
  commands; there is never a working tree.
- **Reading through a change** is reading at that ref's commit, so every
  read operation works on a change with no extra code.
- **`change_diff`** is the difference between the snapshot and the ref.
- **`change_commit`** writes one commit with the given message on a ref of
  its own, `refs/oa/proposed/SESSION-ID`. The author is the person whose
  session it is. The message ends with the model's `Co-Authored-By` line
  and the session's `Agent:` line, as AGENTS.md requires.
- **`change_propose`** submits that commit to the land queue. The
  integrator checks it on the head it would land on, with the same
  pre-push rules a person's push meets (the `Agent:` line, `tools-check`,
  `wire-check`, the suites it touches).
- **Nothing reaches `main`** until a person has approved the queued change
  and its checks have passed.

Why the queue and not a pull request: this repository's checks run in the
pre-push hook and the integrator, not on GitHub, and its rule is work on
`main`. A branch and a pull request would land a change that none of those
checks had seen. A repo that isn't ours, and has no queue, still gets a
branch and a pull request; that is phase 4's concern.

Limits: a change holds at most a set number of files and bytes, and a
session holds at most a set number of open changes. A change left idle is
discarded after a set time. The record of who opened a change, for which
session, and what became of it lives in oa-api's database, which already
owns everything durable for hosted agents.

If `main` moves while a change is open, the integrator checks the change
on the new head and says so if it no longer applies. A `change_rebase`
operation can come later if that proves common.

## Checks

Microcoder is good at coding because it runs the task's checks. A chat
with no checkout still needs that, and it must not get a shell.

`change_check` runs one check from a list that each repo's configuration
names, for example `coax test PATH` for a path inside the repo. The run
happens on our build hosts, not in the repo service:

1. The service pushes the change's commit to a scratch ref.
2. A build host checks that commit out and runs the check, through the
   path `coax/remote/remote.sh run` already uses.
3. Output streams back, cut to a head and a tail as Microcoder's command
   output is today.

A check has a time limit of 15 minutes, the limit AGENTS.md sets for any
command. The repository's code runs only on the build host, in its own
checkout, never in the repo service.

**A check runs code the model wrote.** With `propose`, the change under
test is the model's, and text in a repository can steer a model. So
`change_check` is running untrusted code, and a list of allowed check names
doesn't change that: `coax test PATH` runs whatever `#test` and `#run` blocks
the change put at that path. Before phase 3 ships, a check runs where it can
hurt nothing:

- on a host, or in a sandbox on one, that holds no credentials: no cloud
  token, no write access to the shared build cache or the release bucket,
  no deploy key;
- with no network beyond what the check is declared to need;
- with its own scratch disk, thrown away after the run.

The build hosts as they are today don't meet this (they write the shared
cache and can publish builds), so phase 3 includes that isolation and is
not done until it is shown.

## The index

"Indexed" means the service has done the slow work ahead of the question.
Each part is keyed by the id of the file's contents, not by commit. A new
commit then costs only the files it changed, and two commits that share a
file share its index.

| Part | What it holds | When |
| --- | --- | --- |
| Manifest | Per file: size, line count, text or binary | Phase 1 |
| Text search | Nothing at first: `git grep` on the commit | Phase 1 |
| Symbols | Per file: the names it defines and uses, with lines | Phase 3 |
| Embeddings | Per passage: a vector for search by meaning | Phase 4 |

- **Text search starts with no index at all.** Measured on coderos-4080 at
  `0942bd418b`: a literal search of the whole repository at a commit, with
  no working tree, took 0.8 s, and a regular expression over the `.coax`
  files took 0.4 s. That is fast enough to start. A trigram index is added
  only if search on the service's own machine is measured too slow.
- **Symbols come from each language's own parser where we have one.** For
  Coax that is the compiler. For C, Swift, Python and Markdown, a small
  table of patterns finds definitions and headings.
- **Embeddings reuse what exists.** Microcoder's worker already asks
  Vertex for embeddings (`coder/tui/mcworker.coax`, `embed`). They are
  built only if search and symbols leave a measured gap.
- **Most of the repository is not text.** Of 1,939 MB in 35,191 files,
  compressed archives, weights and images are about 1,400 MB. Coax source is
  106 MB in 7,137 files, Markdown 40 MB and C 25 MB. Only text files are
  indexed.

A snapshot newer than the index still works. `read`, `tree`, `search` and
history need no index. `symbols` and `similar` answer that the snapshot is
not indexed yet, and `resolve` says which parts are ready.

### Keeping it current

- GitHub calls a webhook on every push. oa-api checks its signature and
  tells the repo service to fetch.
- A scheduled job fetches every 10 minutes in case a webhook is lost,
  through the Cloud Scheduler jobs oa-api already runs
  (`docs/coaxial-services.md`).
- `resolve` on a branch fetches first when the last fetch is older than a
  set number of seconds.

## The service

One new service, `oa-repo`, on the `openagentsgemini` project.

- **Where.** One GCE VM with two dedicated cores and a persistent SSD, in
  `us-central1` beside the runner and oa-api. Not a shared-core machine:
  the search times above were measured on a much faster machine, and a
  shared core would be several times slower again. Git wants a real disk: a bare mirror is a
  few large pack files read at random, which a mounted bucket serves slowly,
  and Cloud Run has no persistent disk. We already run one service this way
  (`docs/deployment/room-vm.md`).
- **What is on the disk.** `mirrors/SLUG.git`, the index, and nothing else.
  A mirror can be fetched again, so the only thing to back up is open
  changes. A daily disk snapshot covers them.
- **Who can call it.** Only oa-api and the agent runner, with an identity
  token, as the runner is called today. It has no public address.
- **What it is written in.** Coax, with a C host for sockets and for
  running `git`, as the runner does now.
- **Repos are configuration.** Each entry has a slug, a git URL, a
  credential's secret name, the checks it allows and who may read it. A new
  repo is one entry and a first fetch.

- **A library first.** The operations are a Coax library,
  `coax/lib/repo/`, over a bare mirror on a disk. The service is that
  library behind sockets and grants. The library runs anywhere there is a
  mirror, so a page under development (`coax/web/api/test.sh --serve`) and
  the checks use it with no VM.

### Wire

The schema starts in `proto/`, as a new package `openagents.repo.v1alpha1`,
and in the protocol spec. The chat audit found that none of `/chat`'s wire shapes is
in the protocol; this one starts there. The HTTP form is the schema's JSON
mapping under `/api/repo/`.

Tool use is recorded as `agent.tool_use` events of the managed session
(`openagents.agent.managed.v1`, protocol spec §18.2), with the operation,
the snapshot and the size of the answer.

## Who may do what

- **A grant per session.** When a turn starts, oa-api decides what the
  signed-in person may do with the conversation's repo and gives the runner
  a short-lived grant: the repo, and `read` or `propose`. The repo service
  accepts a call only with a grant that covers it.
- **Public repos:** any signed-in person may read. `/chat` is admins only
  today, and stays so until that changes.
- **Private repos:** only the project's members, by the membership and
  roles projects already have.
- **`propose`:** admins first, then members with a role that allows it. A
  proposed change still waits for a person's approval in the queue.
- **The model holds nothing.** It never sees a grant, a token or a key.
  The runner attaches the grant to each call.
- **The file that decides this is `#authority`.** It is mutated with zero
  survivors before it ships.

A conversation already has a folder, "the checkout its turns read"
(`docs/chat.md`). A folder becomes a repo and the snapshot the conversation
is reading. A conversation moves to a newer snapshot when the person asks,
or at the start of a new turn if they chose to follow `main`.

## Safety

- **Repository content is data.** The service never runs anything from a
  repo. Mirrors are bare, git hooks are off, and submodules and large-file
  filters are not fetched. Only build hosts run repository code, for a named
  check.
- **No paths on a disk.** Reads go through git objects, so `..`, absolute
  paths and links can't reach outside the repo. A link is returned as a
  link, never followed.
- **No shell.** There is no operation that runs a command line.
- **Checks run untrusted code.** See [Checks](#checks): isolated, with no
  credentials, before phase 3 ships.
- **Secrets.** A change that adds text matching a key or token pattern is
  refused at `change_commit`. A private repo's contents never go to a
  session without a grant for it, and are never logged.
- **Write credentials stay in the service.** They come from Secret
  Manager, are used only for the push and the pull request, and never
  enter a change, an answer or a log.
- **Text in a repo can try to instruct the model.** A read-only grant
  bounds what that can do: nothing is written. With `propose`, the worst
  case is a pull request that a person declines.
- **Limits per session:** calls a minute, bytes returned, open changes and
  check minutes, so one conversation can't exhaust the service.

## How agents use it

One API, two adapters.

**Microcoder.** Its loop owns no I/O: `tools.coax` runs an action's
commands, views and edits and reports the results. A second tool host,
`repo-tools.coax`, does the same against the API:

| The action's field | Local tool host today | Repo tool host |
| --- | --- | --- |
| `view` | A file under the working directory | `read` |
| `edits` | An exact edit to a file | `change_edit` |
| `commands` | `/bin/sh` in the working directory | A fixed set of repository commands |

The action schema, the system prompt's shape and the step log stay as they
are. The repo tool host accepts a small set of command forms, written the
way a model already writes them, and parses them in Coax. None goes to a
shell:

```
ls PATH
find GLOB
cat PATH
head [-n N] PATH
tail [-n N] PATH
sed -n FROM,TOp PATH
wc -l PATH
grep [-i] [-E] [-n] [-r] [-l] [-C N] PATTERN [PATH...]
git log [-n N] [--oneline] [PATH]
git show COMMIT
git diff
git blame PATH -L FROM,TO
symbols NAME
check NAME
```

`cat`, `head`, `tail`, `sed -n` and `wc -l` are here because models reach
for them before anything else; each is a `read`. A pipe, a redirect, `&&`
or `;` is refused whole, with a line saying to ask for one thing at a time.
Anything else is answered with one line listing these forms. This is a
hypothesis to measure, under the rule that Microcoder's loop stays simple:
run the reading and bug-fix tasks from
`docs/research/2026-11-11-microcoder-vs-antigravity.md` on both tool hosts.
If the model's commands are refused often, or tasks pass less often than
with the local tool host, replace the command forms with typed fields in
the action.

**The Claude engine.** The same operations are offered as in-process SDK
tools served by the runner, as the project agent's tools are today
(`docs/project-agent.md`). The CLI's own Read, Glob and Grep are then not
needed, and neither is the checkout in the runner's image.

**The web chat.** `/chat` gets its agent this way: the runner runs
Microcoder with the repo tool host, on a Gemini model through the gateway.
With no checkout to refresh, a cold start has only the process to start.

**Elsewhere.** Coder and the `openagents` command line can use the same
tool host to work on a repo that isn't cloned on the machine.

## What it costs

Estimates, not quotes. They need checking against Google's price list
before the VM is made.

- **The VM and disk:** two dedicated cores, 8 GB of memory and a 100 GB
  SSD, roughly $65 a month, always on.
- **Model spend:** unchanged in kind. Chat turns stay under `/chat`'s daily
  cap (`OA_CHAT_DAILY_USD`, $10 a day). `ask` counts against the same cap.
- **Checks:** build-host minutes, under the build hosts' existing daily
  limit.
- **Embeddings:** not estimated. They are priced before phase 4 starts.

All of it sits under the project's standing limit of $1,500 a month
(`docs/googlecloud/2026-11-11-capacity-update.md`).

## Phases

Each phase ends with something a person can use, and is measured before
the next begins.

1. **Read.** In this order, each usable before the next:
   - The library, `coax/lib/repo/`: `resolve`, `tree`, `read`, `find`,
     `search`, `log`, `show`, `diff` and `blame` over a bare mirror, with
     their limits and their fixed order, checked against a repository the
     checks build.
   - The schema in `proto/` and the spec.
   - The service with one repo, the public `openagents` repository, and
     grants.
   - The tools for the Claude engine, so `/chat`'s Claude turns read
     through the service and the runner's checkout goes.
   - Microcoder's repo tool host, so `/chat` can answer about the
     repository on a Gemini model.
   - Done when an admin asks `/chat` where something is defined and gets
     an answer that cites files at a commit, with no checkout in the
     runner, and read calls answer in under 300 ms and searches in under
     1.5 s at the 95th percentile, measured from the runner.
2. **Propose.**
   - Changes, `change_commit` and `change_propose` into the land queue.
   - Done when an admin asks `/chat` for a small edit, approves it in the
     queue, and it lands with a diff that matches what the chat showed.
3. **Check and symbols.**
   - `change_check` on an isolated host with no credentials, and the symbol
     index.
   - Done when a Microcoder run on the repo tool host passes the same
     tasks, at a similar cost, as a run in a local checkout.
4. **More repos, and meaning.**
   - Private project repos by membership, a second git host if one is
     needed, and a branch and pull request for a repo that has no land
     queue.
   - `ask`, if a small-context model is measured needing it.
   - Embeddings, if phases 1 to 3 left a measured gap that search by
     meaning would close.

Checks come first in every phase, as AGENTS.md requires:

- `#test` and `#forall` checks on path handling, the command forms, edit
  application and answer limits.
- A small repository built by the checks themselves as the fixture.
- A stand-in service for the tool host's checks.
- A live check that times each operation on the deployed service.

## Alternatives considered

- **Use GitHub's API as the store.** No service to run. But its code
  search covers only the default branch, takes no regular expressions and
  is rate-limited, reads are one network call per file, and it ties every
  repo to GitHub. It remains the way pull requests are opened.
- **Keep baking checkouts into the runner's image.** That is today's
  design and its limits are the reason for this spec: one repo, a 1,200 MB
  ceiling, a redeploy to refresh, and no writes.
- **Clone per turn, as the project agent does.** It works for small
  repositories. A cold start of the public repository's checkout took 103
  seconds and ran out of memory before it was moved into the image
  (`fcb9ae1ee6`).
- **Cloud Run with a mounted bucket.** It scales to zero, but git on a
  mounted bucket is slow, and concurrent writers to one mirror need locking
  that a bucket doesn't give.
- **Give the hosted agent a sandbox with a checkout and a shell.** It is
  the most capable option and the most to secure and pay for. `change_check`
  on build hosts gets the part that matters, running the checks, without
  it. This can be revisited if tasks need more than named checks.

## What changed in review

Reviewed 2026-11-11, the evening it was written; the owner then asked for it
to be updated, tracked and built. Six things changed:

1. **Checks are treated as running untrusted code,** with isolation
   required before phase 3 ships ([Checks](#checks)).
2. **A change lands through the land queue,** not a branch and a pull
   request, because this repository's checks live there.
3. **The library comes first** and the service wraps it, so phase 1 is
   usable in steps and on a developer's machine.
4. **`ask` moved from phase 2 to phase 4,** and became conditional.
5. **The command forms gained `cat`, `head`, `tail`, `sed -n` and `wc -l`,**
   and pipes are refused whole.
6. **The VM has two dedicated cores,** and `read` takes several files in
   one call.

## Decisions

Recorded 2026-11-11. The owner asked for the spec to be built as revised;
these follow from that and can be changed.

1. **How a change lands:** the land queue, after a person approves it. A
   GitHub App for pull requests is needed only for a repo with no queue
   (phase 4).
2. **A VM that is always on,** about $65 a month, rather than something
   that scales to zero.
3. **Who gets `propose` first:** admins only, as `/chat` is now.
4. **Whether an agent's change may land with no person's approval:** no.
5. **Private repos** wait for phase 4.

## What was and wasn't checked

- **Read in the code:** the chat page, the chat runner and its checkout,
  the project agent's design, Microcoder's action and tools, and the
  deploy scripts.
- **Measured:** the repository's size by file type, and search times at a
  commit without a working tree, on coderos-4080. That machine is much
  faster than the proposed VM, so search time on the VM is unknown.
- **Not checked:** the VM's price, GitHub's webhook details, how the land
  queue takes a commit from a ref that isn't a builder's branch, how
  the Coax compiler can list a file's symbols, and how well models write
  the command forms. Each is settled in the phase that needs it.
