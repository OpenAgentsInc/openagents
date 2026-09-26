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
- Embeddings and `kb harvest` need an OpenRouter key:
  `OPENROUTER_API_KEY`, or `api_key` in `~/.openagents/openrouter.json`.
  Without one, search ranks by words alone.
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
One structured OpenRouter call, `openai/gpt-6-luna` by default, reads each
step's rationale, commands, and output, the acceptance tests, and the
verifier's verdict. It proposes at most three entries: what went wrong, what
fixed it, and what would have saved steps. A harvest costs about $0.001 with
the default model.

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

Read every harvested entry before you admit it.

## Measure entries

Every run's `summary.json` lists the entries its prompts showed. Measure
every entry against the recorded runs:

```sh
microcoder kb evidence
microcoder kb evidence statistics.mmd-estimators --attach
```

For one entry, a run is *with* the entry when its prompts showed it, and
*without* it otherwise. Runs are paired by task and model, and a task the
entry was written from never counts. A paired task is *for* the entry when
the runs with it pass more often, or pass as often (at least once) at under
90% of the cost per run. It's *against* the entry when they pass less often,
or as often at over 110% of the cost.

Each entry gets a report in the NIP-EVAL shape in
`~/.openagents/knowledge/evidence/<id>.v<N>.json`, with the artifacts it
references in `artifacts/`. `--attach` also adds a `measured` line to the
entry's `evidence` list. Pass `--runs DIR` or `--evidence-dir DIR` to read or
write elsewhere.

## Admit, withdraw, and review entries

An entry is shown by default only once it's admitted. Admit it in one of two
ways:

- After you've read it:

  ```sh
  microcoder kb admit numerics.kahan-summation --reviewer "Your Name"
  ```

- When its recorded evidence passes the rule: at least 2 paired tasks for it
  and none against it. Run `kb evidence` first.

  ```sh
  microcoder kb admit <id> --evidence
  ```

Either way, the entry's `evidence` list records who or what admitted it, and
when. If a newer version waits in `versions/`, `kb admit` promotes it first.

Mark a wrong entry as withdrawn. It's never shown again, and its file stays
so earlier runs can be explained:

```sh
microcoder kb withdraw <id> --reason "the formula has the wrong sign"
```

List admitted entries shown often that never help, and candidates the rule
would admit:

```sh
microcoder kb review
microcoder kb review --apply    # demote the listed entries to candidate
```

An admitted entry is demoted when at least 5 runs showed it, out of sample,
it has paired tasks, and none is for it.

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
their heads are 28 events.

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
