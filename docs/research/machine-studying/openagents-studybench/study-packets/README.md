# OpenAgents Repo Study Packet (SA-1)

The canonical **studied-knowledge substrate** for the openagents repo. It is the
foundation EPIC #5337 builds on: SA-2 (#5339, Autopilot-coder context) and SA-3
(#5340, hygiene/refactoring lane) consume it, and SA-4 keeps it fresh.

## What the artifact is

Running the study pipeline over the **real openagents tree** produces four
linked, hash-pinned, public-safe artifacts:

1. **Repo corpus manifest** (`openagents.repo_corpus_manifest.v0`) — every
   admitted file in the tree, with stable `sha256:` content digests, the
   exclusion rules, and line-numbered evidence-span extraction.
2. **Study packet** (`openagents.repo_study_packet.v0`) — corpus manifest ref +
   bounded commit history + evidence spans + rationale sources (root + app
   `AGENTS.md`/`INVARIANTS.md`, the Tassadar audit, the machine-studying
   roadmap, and the `backroom/` pruned-lineage archive) + the seven study
   sections (source map, invariant map, typed-ref glossary, trap catalog, test
   command catalog, edit playbook, retained-failure fixtures).
3. **Studied-knowledge graph** (`openagents.repo_studied_knowledge_graph.v0`) —
   code ↔ commit ↔ doc ↔ invariant ↔ rationale ↔ issue nodes with verified
   edges (`code_explained_by_audit`, `code_warned_by_rejected_lineage`,
   `edit_site_respects_invariant`, `edit_site_commit_context`,
   `issue_tracks_edit_site`, `evidence_span_supports_node`, …).
4. **Verification report** + **eval-harness report** — every graph edge and
   evidence span is replayed and hash-checked (verification-backed), and the
   StudyBench hidden-edit eval measures source-grounded lift vs. a
   no-studied-context baseline.

## Where it lives (and why it is not committed as a blob)

The full packet (~70 KB) and graph (~300 KB) are **regenerated on demand**, not
committed. Committing the full blobs would churn on every commit (commit history
is part of the digest) — exactly the dual-format generated-fixture anti-pattern
retired in #5334.

Instead the **stable identity** is a small, digest-pinned index committed at:

    docs/research/machine-studying/openagents-studybench/study-packets/openagents.study-artifact-index.json

It holds the content hashes (corpus manifest / packet / graph / verification),
the eval-report hash + lift summary, the correctness-gate flag, the generation
inputs (`commitHistoryLimit`, repo, commit), and a commit-independent
`corpusContentHash` (a digest of the admitted file content, used by the SA-4
freshness signal below). It is excluded from the corpus walk, so the artifact's
identity is a pure function of hand-authored sources (regenerating with or
without the index present yields the same hashes).

Schema: `openagents.repo_study_artifact_index.v0` (`OpenAgentsRepoStudyArtifactIndex`).

## Regenerate / verify (the SA-4 freshness receipt)

From `packages/probe/packages/runtime`:

    bun scripts/generate-openagents-study-packet.ts --print   # generate + print the index
    bun scripts/generate-openagents-study-packet.ts --write   # refresh the committed index
    bun scripts/generate-openagents-study-packet.ts --check    # regenerate-and-diff (exit 2 on drift, 1 on gate fail)

`--check` is the freshness/regenerate-and-diff gate: it regenerates the artifact
from the live tree, fails if the verification correctness gate is red, and fails
if the regenerated `indexHash` differs from the committed one. SA-4 can wire it
into CI/cadence.

The artifact studies commit history, so its identity is intrinsically
per-commit: the committed index pins the exact commit it was generated against,
and `--check` will report drift after **any** later commit (this is the freshness
signal, not a defect). Refresh with `--write` as part of the studying cadence;
SA-4 owns when that runs.

## Standing freshness (SA-4)

`--check` is strict regenerate-and-diff: it drifts after **any** commit, because
the `indexHash` (and the `corpusManifestHash`) embed the HEAD commit. That is too
noisy to be the signal SA-2 / SA-3 consume directly. SA-4 layers a *trustworthy*
staleness verdict on top, keyed on the commit-INDEPENDENT `corpusContentHash`
(a digest of the admitted file content, recorded in the SA-1 index), separating
two kinds of drift:

- **content drift** — the corpus **content** hash changed, i.e. an admitted source
  file actually changed. This is the meaningful re-study trigger.
- **commit drift** — HEAD/history moved but the studied content is byte-identical
  (corpus content hash unchanged). Cheap and expected; the studied knowledge is
  still correct.

The verdict (`openagents.repo_study_artifact_freshness.v0`,
`OpenAgentsRepoStudyArtifactFreshness`) is a hash-pinned, public-safe projection:

```jsonc
{
  "status": "fresh | stale | gate_failed",
  "recommendation": "none | refresh_index | reverify_gate",
  "contentDrift": false,        // an admitted source file changed (corpus content hash)
  "commitDrift": true,          // HEAD moved but content is identical
  "correctnessGatePassed": true,
  "commitsBehind": 7,           // commits between staleSinceCommit and HEAD (advisory)
  "staleSinceCommit": "<commit the index was studied against>",
  "headCommit": "<live HEAD>",
  "committedCorpusContentHash": "sha256:…",   // drives contentDrift (commit-independent)
  "regeneratedCorpusContentHash": "sha256:…",
  "committedIndexHash": "sha256:…",            // identity evidence (embeds the commit)
  "regeneratedIndexHash": "sha256:…"
}
```

`status: "fresh"` means **no content drift AND the correctness gate is green** —
the studied substrate still matches the tree, regardless of how many commits have
landed. **SA-2 / SA-3 can consume studied knowledge whenever the verdict is
`fresh`.** `stale` means content drifted (re-study); `gate_failed` means the
regenerated artifact failed the verification gate (investigate, do not refresh).

CLI (from `packages/probe/packages/runtime`):

    bun scripts/generate-openagents-study-packet.ts --freshness        # print the verdict JSON; exit 0 fresh / 2 stale / 1 gate_failed
    bun scripts/generate-openagents-study-packet.ts --refresh-if-stale  # rewrite the committed index ONLY on content drift; exit 0 / 1 gate_failed

### Re-study cadence

`scripts/restudy-openagents-cadence.sh` wraps the CLI for CI / scheduled use:

    scripts/restudy-openagents-cadence.sh verdict   # read-only freshness signal
    scripts/restudy-openagents-cadence.sh refresh   # refresh committed index in place when stale
    scripts/restudy-openagents-cadence.sh ci        # refresh + commit + push the index when stale

`.github/workflows/restudy-openagents.yml` runs the cadence on push to `main`
(CI-on-merge) and on a daily schedule. It rewrites and commits **only the small
index** and **only on real content drift** — pure commit drift produces no commit,
and the multi-MB packet/graph blobs are never committed (they stay
regenerate-on-demand, the #5334 lesson). This is the cheap-incremental property:
the committed index tracks studied *content*, not every commit.

## How SA-2 / SA-3 load it

Consumers depend on `@openagentsinc/probe-runtime` and call the generator
directly — the committed index is the digest they pin against:

```ts
import {
  generateOpenAgentsRepoStudyArtifact,
  decodeOpenAgentsRepoStudyArtifactIndex,
} from "@openagentsinc/probe-runtime";
import { Effect } from "effect";

// Regenerate the live artifact (packet + graph + verification + eval + index).
const artifact = await Effect.runPromise(
  generateOpenAgentsRepoStudyArtifact({ rootDir: repoRoot }),
);

// SA-2 (#5339): feed packet + graph into the Autopilot-coder studied context.
//   buildOpenAgentsAutopilotCoderStudiedContext({ packet: artifact.packet, graph: artifact.graph })
// SA-3 (#5340): start hygiene passes from artifact.graph + artifact.verification.

// Optional: pin against the committed digest index for freshness/identity.
const committed = await Effect.runPromise(
  decodeOpenAgentsRepoStudyArtifactIndex(
    JSON.parse(await Bun.file(indexPath).text()),
  ),
);
const fresh = committed.indexHash === artifact.index.indexHash;
```

`generateOpenAgentsRepoStudyArtifact` returns `{ packet, graph, verification,
evalReport, index }`. `packet` and `graph` are the exact inputs the existing
`buildOpenAgentsAutopilotCoderStudiedContext` (SA-2) and study-graph traversal /
verification (SA-3) already accept.

## Discipline

Internal dogfood only. `sourceBoundary: "public_refs_only"`; no customer /
marketplace / payout claim (that is the gated SA-5 path). The eval lift is the
dogfood signal that the studied substrate distinguishes from a baseline; it is
not a customer-facing promise.
