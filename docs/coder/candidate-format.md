# The candidate: one object for a review proposal and an optimizer mutation

Date: 2026-08-26. Status: the format of record for
OpenAgentsInc/openagents#122, #121, and #123. Companions:
`docs/coder/autoimprove.md` §3 and §7.6, `docs/coder/runbook.md`,
`docs/coder/best-practices.md`, and the analysis
`docs/coder/2026-08-26-dspy-gepa-coder-optimization.md` §4.

A candidate is a proposed change to the coder's optimizable text, carrying
everything needed to judge it: which surfaces it edits, where it came from,
which model family it was written or evolved against, the evidence behind it,
and how it would be confirmed or refuted.

The standing law it exists to serve: **an optimizer output is a candidate,
never a deployment.** A candidate becomes a change the way every other lever
does — a fresh worktree, a landing commit stating the measured delta, the
review, the ledger.

## 1. Why there is one format and not two

`autoimprove.md` §7.6 says the optimizer lane shares the review schema "so a
reflection and a mutation are the same object". If a review proposal and a
GEPA mutation were different shapes, the Pareto pool could not hold both, a
human cycle could not seed the optimizer, and every consumer would need two
readers. So the schema is defined once.

The definition lives in code, at
`packages/coder-review/src/candidate.ts`, under the schema id
`openagents.coder_candidate.v1`. That file is the normative artifact; this
document explains it and pins the vocabulary the `surfaces` field draws on.
A review proposal is a candidate whose `lineage.origin` is `review`; an
optimizer mutation is one whose origin is `optimizer`; a hand-written cycle
lever is `human`. Nothing else about the object changes between them.

## 2. The shape

```
CoderCandidate {
  schema:        "openagents.coder_candidate.v1"
  candidateId:   "candidate:<8 hex>"      // computed, never supplied
  lever: {
    axis:        "process" | "plugin" | "harness" | "optimizer" | "routing" | "ledger"
    summary:     string                   // one sentence naming the change
  }
  surfaces:      [{ surface: string, diff: string }]
  lineage: {
    origin:      "review" | "optimizer" | "human"
    parent:      string | null            // the candidateId this came from
    producedBy:  string                   // e.g. "coder-review:<jobDir>:<reviewer ref>"
  }
  transferLabel: { modelFamily: string, lane: string }
  evidence:      [{ ref: string, note: string }]
  risk:          string
  verification:  { suite: string, metric: string, expectedDirection: "up" | "down" | "unchanged" }
}
```

Three fields carry most of the weight.

**`transferLabel`** is ledger O5 and the evolve-the-harness finding behind it:
code mechanisms transferred across model families and tuned prompts did not.
A candidate evolved against one family is not evidence for another, and the
label is what stops a pool from quietly pretending otherwise.

**`evidence`** is checked, not decorative.
`autoimprove.md` §6 lists "confident review without understanding" as a failure
mode. The parser refuses a proposal with no evidence, and refuses one whose
refs do not resolve against the artifacts the producer was actually given. The
ref grammar is small on purpose, because every scheme in it has to be
resolvable: `trial:<task>#step-<id>`, `trial:<task>#outcome`,
`row:<suite>#<recordedAt>`, `ledger:<id>`, `diff:<path>`.

**`candidateId`** is FNV-1a over the candidate's own facts — an identity for a
pool entry, not a receipt. `bench-results` owns tamper-evidence; borrowing its
`receipt:` vocabulary here would suggest this digest carries the same weight.

## 3. The surface vocabulary

`surfaces[].surface` names a staged artifact from `surfaces/coder/index.json`.
The whole current vocabulary:

| `surface`           | Artifact                                   | What it holds                                                                                                                 |
| ------------------- | ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| `system-prompt`     | `surfaces/coder/system-prompt.v1.json`     | The instructions, the concision sentence, the no-tools and tool-list sentences, and the lane notices, for all three harnesses |
| `tool-descriptions` | `surfaces/coder/tool-descriptions.v1.json` | The description of each declared tool, plus the per-model-family emphasis overrides                                           |
| `catalog-lines`     | `surfaces/coder/catalog-lines.v1.json`     | Each installed plugin's catalog line, keyed by plugin id                                                                      |

`diff` is a unified diff over that artifact file where one exists, and the
proposed text otherwise. The artifact's `text` map is flat and keyed, so a
one-sentence change is a one-line diff and reads as one.

A surface a candidate does not touch is simply absent from the array. A
candidate touching none of them is legitimate — the `plugin`, `process`, and
`routing` axes change code, skills, or procedure rather than staged text — and
its `surfaces` array is empty.

### The catalog-lines surface is a mirror, not a move

Stated plainly, because it changes how a candidate against it is applied. The
system prompt and the tool descriptions were string literals in earlier Coder
implementations. Staging moved the text out of code and into the artifact,
which is now where you edit it.

A plugin's catalog line was never a literal. It is the top-level `description`
of `plugins/<id>/manifest.json`, discovered at runtime by `discover_catalog()`
(`crates/openagents-cli/src/plugins.rs`). The
manifest stays where that text is edited. `catalog-lines.v1.json` mirrors those
descriptions into one diffable object with a content digest, so an optimizer
has a single file to diff and a bench row has a single digest to record — and
`pnpm run check:coder-surfaces` fails when a manifest and the mirror disagree.

So: applying a candidate against `system-prompt` or `tool-descriptions` means
patching the artifact. Applying one against `catalog-lines` means patching the
named `plugins/<id>/manifest.json`. Both then need the rebuild.

## 4. Applying a candidate

1. Patch the artifact (or, for `catalog-lines`, the manifest).
2. `pnpm run build:coder-surfaces` — re-pins `surfaces/coder/index.json` and
   regenerates the embedded modules the two CLIs compile.
3. Run the suite named in `verification.suite`. The run announces its staged
   text as `[oa:surfaces <id>=<digest>,…]`, `bench-results` records those
   digests on the row, and `pnpm run effectiveness:compare` names the change as
   a variable when two compared rows carry different pins.
4. Land or discard on the measured delta, per `runbook.md`.

Skipping step 2 is the failure this staging exists to prevent, and it is a
named check rather than a silence: `check:coder-surfaces` runs inside
`check:fast` and refuses a tree where an artifact and its build disagree.

## 5. A worked example

A real observation from the staging work, kept here as a candidate rather than
as a change: the `shell` tool description has forked between the two harnesses.
The TypeScript one steers the model toward an installed capability before it
writes a script and carries a paragraph of token economy; the Rust one carries
neither. A candidate proposing to close that gap looks like this.

```json
{
  "schema": "openagents.coder_candidate.v1",
  "candidateId": "candidate:computed",
  "lever": {
    "axis": "harness",
    "summary": "Give the Rust shell description the capability-first steer the TypeScript one already carries."
  },
  "surfaces": [
    {
      "surface": "tool-descriptions",
      "diff": "--- a/surfaces/coder/tool-descriptions.v1.json\n+++ b/surfaces/coder/tool-descriptions.v1.json\n@@\n-    \"rust.shell\": \"Run a shell command on this machine. The working directory is {cwd}, …\",\n+    \"rust.shell\": \"Run a shell command on this machine. The working directory is {cwd}, … When an installed capability covers the task — the `capability` tool names what is installed — load and call it instead of scripting the same thing here: it is sandboxed, bounded, and returns structured output. …\",\n"
    }
  ],
  "lineage": {
    "origin": "human",
    "parent": null,
    "producedBy": "openagents#122 staging"
  },
  "transferLabel": {
    "modelFamily": "unmeasured",
    "lane": "unmeasured"
  },
  "evidence": [
    {
      "ref": "diff:surfaces/coder/tool-descriptions.v1.json",
      "note": "The two harnesses' `shell` descriptions, side by side in one artifact, which is what made the fork visible."
    },
    {
      "ref": "ledger:P3",
      "note": "A plugin competes for rank in a five-result search; a shell description that never mentions the capability tool is one reason a search never happens."
    }
  ],
  "risk": "The steer is unmeasured on the Rust harness. It may cost tokens on tasks no installed capability covers, and it lengthens a description that is currently short.",
  "verification": {
    "suite": "tb2-quick",
    "metric": "successRate",
    "expectedDirection": "up"
  }
}
```

Note what the object refuses to pretend. `transferLabel` says `unmeasured`
rather than naming a family, because this candidate came from reading two
files and not from a run. Under ledger O5 that is a candidate worth screening
and not a change worth landing — which is the whole point of the format.

## 6. What is deliberately not here

- **No optimizer.** #123 owns the GEPA lane; this document owns the object it
  emits.
- **No adoption rule.** `runbook.md` owns the acceptance gate and
  `best-practices.md` owns the ledger operations. A candidate that passes its
  verification is still a candidate until a cycle lands it.
- **No parameter-level tool text.** The staged `tool-descriptions` surface
  holds each tool's own description and the family overrides. The JSON-Schema
  `description` of an individual argument is still a literal in code, and
  staging it is a later slice rather than a silent part of this one.
