# Use the shared knowledge base

This guide covers every `microcoder kb` command: finding entries, writing
them, measuring and admitting them, and sharing them over a Nostr relay. The
[design document](../design/knowledge-base.md) explains why the knowledge
base exists and how retrieval works inside Microcoder's loop, and
[NIP-KB](../../../nips/openagents/NIP-KB.md) specifies the events.

`microcoder kb --help` prints the same list.

## Before you begin

- Entries live in `knowledge/` at the repository root, one Markdown file per
  entry. Set `OPENAGENTS_KNOWLEDGE` or pass `--dir DIR` to use another
  directory.
- The `kb harvest` commands call the model through the operator's Codex
  login in `~/.codex/auth.json` (run `codex login`), as Microcoder does.
  `--provider openrouter` uses OpenRouter instead, with `OPENROUTER_API_KEY`
  or `~/.openagents/openrouter.json`.
- Embeddings call OpenAI's `text-embedding-3-small` directly when
  `OPENAI_API_KEY` or `~/.openagents/openai.json` holds a key, and otherwise
  go through OpenRouter. Put the key in the file as `{"api_key": "..."}`
  with mode 600; a file others can read is refused. Both providers serve
  the same model, and the cache in `~/.openagents/knowledge/embeddings.json`
  is keyed by the model name `openai/text-embedding-3-small` and each
  entry's digest, so vectors cached through OpenRouter stay valid. Without
  a key, or when a call fails, search ranks by words alone and says why.
- The lint checks entries against the installed Terminal-Bench 4 tasks under
  `~/.openagents/terminal-bench/`. Pass `--corpus DIR`, which can repeat, to
  check other task directories.

## Find entries

Search the base the way a run does, and show the scores:

```sh
microcoder kb search mmd kernel two-sample estimator
microcoder kb search --candidates --limit 5 "empty histogram bins"
microcoder kb search --lexical mmd         # words only, no embeddings
```

Each result shows the entry's kind, status, and author. Add `--trust listed`
or `--trust all` to include entries synced from other authors; see
[Choose whose entries a run sees](#choose-whose-entries-a-run-sees).

Print one entry with its digest:

```sh
microcoder kb show statistics.mmd-estimators
```

When a newer version of the entry waits in `knowledge/versions/`, `kb show`
says so.

## Write an entry by hand

1. Write a template with an ID, a kind, and a title. The kind is `method`,
   `edge-case`, `slip`, `environment`, or `tool`.

   ```sh
   microcoder kb add numerics.kahan-summation --kind method \
     --title "Compensated (Kahan) summation" --author "Your Name"
   ```

   The file is a `candidate`, and every field it can't guess is marked
   `TODO`.

2. Fill in the summary, `applies_when`, tags, citations under
   `provenance.cites`, and the body's `## Details` and `## How to check`
   sections.

3. Check it:

   ```sh
   microcoder kb lint
   ```

   The lint refuses an entry that cites nothing, still has `TODO` text, has a
   summary over 600 characters, names a benchmark task, or shares 40 or more
   characters with a task's tests.

## Harvest entries from a run

After a run is graded, ask a model to propose general entries from its
record:

```sh
microcoder kb harvest sound-change-cascade-1790395251
microcoder kb harvest ~/.openagents/microcoder/runs/<run> --model openai/gpt-6-sol
```

The run is a directory, or a name under `~/.openagents/microcoder/runs/`.
One model call, `gpt-6-luna` on the Codex login by default, reads each
step's rationale, commands, and output, the acceptance tests, and the
verifier's verdict. It proposes at most three entries: what went wrong, what
fixed it, and what would have saved steps. On the Codex login the request
declares one strict tool, `knowledge_entries`, whose arguments are the
proposals, and it goes through the same `microluna::oneshot` call as
Microcoder's steps. A harvest costs about $0.002 with the default model.

The last lines report the cost and how it was reached: the model's cost at
list price from the reported tokens (`list_price`) on the Codex login, or
what OpenRouter billed (`billed`) with `--provider openrouter`, and the
embeddings' cost. A cost that isn't known, such as an unpriced model or a
call that failed after it was sent, is printed as unknown with the known
lower bound, never as $0. The output also says whether the near-duplicate
check used embeddings or fell back to words, and why.

`--dir` sets where candidates are written. To try a harvest without
touching `knowledge/`, copy it and pass the copy:

```sh
cp -r knowledge /tmp/kb-try
microcoder kb harvest-contrast <run> <trajectory.json> --task <task> --dir /tmp/kb-try
```

What happens to each proposal:

- It's checked by the lint, with the run's own task name added to the names
  it refuses. A proposal that fails is reported and not written.
- A new entry is written as a `candidate`, authored by
  `microcoder kb harvest (<model>)`, with the run's ID in
  `provenance.written_from`.
- A proposal with an existing entry's ID, or whose search text has a cosine
  similarity of 0.9 or more with an existing entry, becomes that entry's next
  version. When the existing version is admitted, the new one waits in
  `knowledge/versions/<id>.v<N>.md` until `kb admit` promotes it, so a
  proposal never hides an admitted entry. Otherwise the new version replaces
  the file, and the old one moves to `versions/`.

- A revision keeps the current entry's body and summary whole and adds the
  proposal under an "Added in version N" heading: the model sees only the
  existing entries' titles, so it can't rewrite a body it hasn't read.
- A proposed tag that equals a task's name, such as a domain name, is split
  into its words.

Read every harvested entry before you admit it.

## Harvest entries from other agents' winning runs

Public runs by stronger agents, such as the Fable 5.1 trajectories under
`~/.openagents/terminal-bench/public-replays/`, carry what a cheaper agent
lacks. Two commands read an ATIF trajectory:

```sh
# What the winner knew or did that a cheaper agent would miss.
microcoder kb harvest-trace <trajectory.json> --task <task>

# A failed Microcoder run beside a winning trajectory on the same task:
# the decisions that explain the failure.
microcoder kb harvest-contrast <run> <trajectory.json> --task <task>
```

`--task` names the task the trajectory solved; no entry may name it, and it
goes into each entry's `written_from`, so that task never counts as
evidence for the entry. The rest works as `kb harvest` does.

Of the two, the contrast finds the decisive detail more often. A trajectory
read on its own yields sound, broad method entries; a contrast points at the
one choice the failed run got wrong, such as which scale an analysis step
used. Entries from a task's own winning runs are in-sample for that task:
report runs that use them as knowledge-assisted.

## Run on the shared base alone

To run with only the entries a relay holds, sync them, then point the local
entry directory at an empty one:

```sh
scripts/kb-relay.sh &                         # a local relay on port 7490
microcoder kb publish --relay ws://127.0.0.1:7490
microcoder kb sync --relay ws://127.0.0.1:7490 --author <npub>
mkdir -p ~/.openagents/knowledge/empty-local
OPENAGENTS_KNOWLEDGE=~/.openagents/knowledge/empty-local \
  microcoder <task> --kb candidates
```

`kb search` with the same variable shows "0 local entries and N synced".

## Measure entries

Every run's `summary.json` lists the entries its prompts showed. Measure
every entry against the recorded runs:

```sh
microcoder kb evidence
microcoder kb evidence statistics.mmd-estimators --attach
```

`kb evidence` performs **historical screening**. Retrieval decided which entries
runs saw, so a difference between those runs does not isolate an entry's effect.
The report's verdict is `inconclusive` for admission, even when its descriptive
counts favor an entry. A declared prospective assignment in a summary is not a
verified pre-run study.

Every run directory remains in intake, including a missing, malformed, or
unreadable summary. Original summary bytes are content-addressed artifacts.
Absent or invalid model, Jev, embedding, or declared additional costs are
unknown. Reports separate the known cost lower bound from a comparable total;
an unknown outcome or cost prevents a cost-per-run comparison.

A shown entry's digest must match the current entry's exact bytes. A different
digest or explicit version is excluded from that version's groups; a missing
pin remains visibly unpinned. Source tasks in `written_from` never count.
Recorded complete configuration identities split groups by harness, model,
effort, budget, environment, workload, context, partition, and source group.
Legacy task/model matches remain labeled observations with incomplete identity.
They do not become held-out data merely because they are absent from
`written_from`.

Each entry gets a report in the NIP-EVAL shape in
`~/.openagents/knowledge/evidence/<id>.v<N>.json`, with its artifacts in
`artifacts/`. Replacing a report retains the previous exact bytes in
`history/<digest>.json`. `--attach` adds a `measured` line to the entry's
`evidence` list; this changes its file digest and does not relabel earlier runs.
Pass `--runs DIR` or `--evidence-dir DIR` to read or write elsewhere.

See the [evidence intake contract](../runtime/knowledge-evidence.md) for the
identity fields, denominators, and limitations.

### Measure another author's synced entry

To measure entries you synced from another author, name the author:

```sh
microcoder kb evidence --author npub1...
microcoder kb evidence --author npub1... git.reflog-recovery
```

This measures the version in your sync cache
(`~/.openagents/knowledge/remote/<author>/`, or `--remote DIR`), by its exact
digest. A run is *with* the entry when its `summary.json` records that ID
with that digest, and *without* it when the run didn't show the ID at all. A
run that showed the ID with another digest showed another version, or
another author's entry with the same ID, so it counts in neither arm; so
does a run that recorded the ID without a digest. The output says how many
runs were left out. Tasks the entry was written from never count, as for
your own entries. Reports go to
`~/.openagents/knowledge/evidence/remote/<author hex>/<id>.v<N>.json`, and
`--attach` is refused, because the entry file isn't yours to edit.

## Admit, withdraw, and review entries

An entry is shown by default only once it is admitted. After reviewing it,
record the reviewer's name:

```sh
microcoder kb admit numerics.kahan-summation --reviewer "Your Name"
```

`kb admit <id> --evidence` refuses historical reports, including retained legacy
reports with a `pass` verdict. Automatic admission requires the prospective
study verifier. The [frozen comparison runner](../runtime/knowledge-studies.md)
retains a prospective cohort but does not authorize automatic admission.
A refusal leaves the current entry and any waiting candidate version unchanged.
An accepted operator review can promote the waiting version and records who
admitted it and when.

Mark a wrong entry as withdrawn. It is never shown again, and its file stays
so earlier runs can be explained:

```sh
microcoder kb withdraw <id> --reason "the formula has the wrong sign"
```

`kb review` and `kb review --apply` do not automatically demote or admit entries
from historical correlations. Use the screening report to inform an explicit
operator review; it does not establish that an entry caused success or harm.

## Run with or without the base

```sh
microcoder <task>                    # admitted entries
microcoder <task> --kb candidates    # unreviewed entries too
microcoder <task> --kb off           # no knowledge base
```

A run whose prompts listed or showed any entry is knowledge-assisted. The
loop's end line and the final line say so, and `summary.json` has
`"knowledge_assisted": true`. Report those results apart from runs without
the base.

## Share entries over Nostr

Entries are signed with the key in `~/.openagents/nostr/knowledge-key`. The
first `kb publish` or `kb sync` creates it with mode 0600. Never copy the key
into a file, a log, or an issue; the commands print only its `npub`.

`kb publish`, `kb sync`, and `kb publish-evidence` need `--relay`: there's no
default relay, because publishing sends entries to other people.

### Publish entries

```sh
microcoder kb publish --relay ws://127.0.0.1:7447
microcoder kb publish --relay ws://127.0.0.1:7447 statistics.mmd-estimators
```

Each entry becomes a kind-`3190` event, one immutable version, and a
kind-`30190` head that points at it. Publishing again is safe:

- A version already on the relay with the same content is left alone.
- A version already on the relay with different content is refused. Raise
  the entry's `version` and publish again.
- A withdrawn entry publishes a kind-`3191` withdrawal for each version of it
  on the relay.

To publish to the OpenAgents relay, run:

```sh
microcoder kb publish --relay wss://relay.openagents.com
```

The relay's default limit is 60 events a minute for each key; 14 entries and
their heads are 28 events. The OpenAgents relay allows 3,000 a minute.

The OpenAgents relay holds the base. On 2026-09-26 the 58 entries in
`knowledge/` and their heads (116 events) and 16 evidence reports were
published to it from the execution host, signed by
`npub15krnek9tl9gwjdaqn9hzayet8l05z5spg3e8d7xp7al7fujvkcvqnaf3wp`. A sync
into an empty cache returned all 58 entries and all 16 reports, and every
signature checked. Reads and writes don't need NIP-42 authentication. To
see whose entries you're trusting, add that `npub` to your trust file.

### Sync other authors' entries

```sh
microcoder kb sync --relay wss://relay.openagents.com
microcoder kb sync --relay wss://relay.openagents.com --author npub1... --author npub1...
```

Without `--author`, a sync fetches every author's entries. For each author
and entry, it keeps the version the author's head names, else the highest
version, and refuses a version that is withdrawn, has two different
documents, fails its signature, or fails the lint. Accepted entries go to
`~/.openagents/knowledge/remote/<author>/`, or `--remote DIR`. Evidence that
cites them goes to `evidence/` there. Loading the cache checks every
signature again, so an edited cache file is refused.

### Choose whose entries a run sees

Trust is yours to decide. A signature proves who wrote an entry, not that
it's right.

| `--kb-trust` | A run also searches |
| --- | --- |
| `own` (default) | Synced entries signed with your own key. |
| `listed` | Those, plus entries from the authors in the trust file. |
| `all` | Those, plus every other synced author's entries, as candidates. |

Entries from your key and from listed authors keep the status their author
gave them. Everyone else's entries are candidates at most, so they're shown
only with `--kb candidates`. A local entry wins over a synced one with the
same ID.

The trust file, `~/.openagents/knowledge/trust.json`, sets the default mode
and lists authors:

```json
{"mode": "listed", "authors": ["npub1..."]}
```

`kb search` takes the same setting as `--trust`.

### Publish evidence

```sh
microcoder kb publish-evidence --relay wss://relay.openagents.com
```

For each published entry with paired runs, this publishes a NIP-EVAL
kind-`3189` report signed by your key and citing the entry's kind-`3190`
event. An entry whose current file isn't published is skipped; publish it
first. Readers weigh evidence by who published it.

To publish evidence about another author's synced entry, name the author:

```sh
microcoder kb publish-evidence --relay wss://relay.openagents.com \
  --author npub1... git.reflog-recovery
```

Each synced version is measured as `kb evidence --author` measures it, by
its exact digest. The report cites the author's kind-`3190` event that the
relay holds with that digest, names the entry by the author's qualified ID,
and is signed by your key. The command refuses a version the relay doesn't
hold with that digest (publish to the relay you synced from, or sync again),
a version its author withdrew, and your own key as `--author`. It prints
each evidence event's full ID, which is what a referee needs.

Evidence you publish about someone else's entry can complete a quest: a
referee can accept it and award XP to the entry's author and to you, the
runner. The [XP guide](xp.md) covers quests, awards, and the ledger, and the
[contributor guide](contribute-knowledge.md) walks through a quest end to
end.

## Contribute to the shared base

The [contributor guide](contribute-knowledge.md) is the one-page path for
people outside OpenAgents: pick a quest from the
[quest board](../../terminal-bench/quest-board.md), find the missing detail,
publish it under your key, and let other operators' runs decide whether it
counts.

## Freeze snapshots and deliver private entries

The [knowledge bundle guide](../runtime/knowledge-bundles.md) covers signed,
immutable NIP-EXT snapshots and encrypted kind-`3188` delivery. A snapshot
replaces ambient knowledge rather than silently combining it with the local
base. Private inputs require explicit permission for their model recipients;
the run retains the exact input manifest.

Use the [frozen study runner](../runtime/knowledge-studies.md) for a separately
reviewed fixed snapshot comparison. Its task, executable, provider, cost basis,
configuration, and assignment pins are established before dispatch. This
infrastructure does not replace an already registered experimental protocol.
