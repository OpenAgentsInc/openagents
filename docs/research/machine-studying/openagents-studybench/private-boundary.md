# OpenAgents StudyBench Private Split Boundary

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-17
Status: MVP policy boundary

This document defines how OpenAgents private validation and private holdout
StudyBench rows may exist without leaking hidden benchmark material into the
public repository.

## Public Repository Boundary

Committed public material may include:

- public-retained StudyBench rows;
- external calibration dataset refs;
- private split refs;
- private split checksum refs;
- corpus manifest refs;
- scorer, packet, and closeout policy refs.

Committed public material must not include:

- private validation or private holdout task text;
- private validation or private holdout `gold_answer` values;
- private validation or private holdout rubric claim statements;
- private validation or private holdout evidence excerpts;
- raw candidate transcripts that reveal hidden task text;
- raw judge rationales that reveal hidden rubric or evidence content;
- private repo content, secrets, wallet material, provider payloads, or
  customer-private material.

The committed public-retained rows are examples and regression fixtures. They
are not hidden benchmark material.

## Split Refs

Initial private split refs:

- `split.openagents_studybench.private_validation.v0`
- `split.openagents_studybench.private_holdout.v0`

Initial dataset refs:

- `dataset.openagents_studybench.private_validation.v0`
- `dataset.openagents_studybench.private_holdout.v0`

Initial checksum refs:

- `checksum.openagents_studybench.private_validation.v0.withheld`
- `checksum.openagents_studybench.private_holdout.v0.withheld`

The public repo can record these refs before private rows are populated. Once a
private split is populated, replace the withheld checksum ref with a digest ref
that does not disclose row content, for example:

- `sha256:<64-hex-character-manifest-digest>`

The digest input must be a canonical manifest over private row ids, row hashes,
source commit refs, and split policy refs. It must not include raw hidden task
text in public docs.

## Local Private Row Path

The local authoring path is:

- `docs/research/machine-studying/openagents-studybench/private/`

That directory is ignored except for its `.gitignore` file. It may hold
operator-local drafts, private validation rows, private holdout rows, and local
scorer artifacts while authoring. Nothing in that directory should be required
for public tests to pass.

## Candidate And Scorer Access

Candidate agents may see the private task prompt only during a controlled
evaluation run. Candidate agents must not see private gold answers, private
rubric claims, private evidence excerpts, private judge rationales, hidden
holdout manifests, or split checksums before or during the run.

Scorers may see the private task prompt, gold answer, rubric, and evidence spans
needed to grade the candidate answer or patch. Scorer outputs that leave the
private evaluation boundary must be refs-only:

- candidate answer or patch artifact hash;
- rubric score artifact ref;
- claim ids and score values when public-safe;
- redacted rationale refs;
- source corpus manifest ref;
- private split ref and checksum ref.

If a scorer rationale quotes hidden task text, hidden rubric text, or hidden
evidence excerpts, it stays inside the private evaluation boundary and is not
committed.

## Training And Study Packet Boundary

Private validation rows may be used to tune evaluator wiring and optimizer
feedback only when the candidate path cannot read private gold answers, private
rubrics, or private evidence excerpts.

Private holdout rows are stricter:

- they cannot feed public-retained examples;
- they cannot feed study packets;
- they cannot feed GEPA training, GEPA reflection, or candidate instruction
  proposals;
- they cannot be used as launch-product proof once their hidden content has
  been exposed to a candidate optimizer.

Public study packets may include only public-retained rows, repo corpus refs,
public evidence refs, and refs to private split scores. They must not include
private holdout task text, gold answers, rubrics, or evidence excerpts.

## Leak Response

If any private validation or private holdout row leaks into public repo content,
public packets, candidate prompts, optimizer feedback, external logs, or public
closeouts:

1. Freeze the affected split immediately.
2. Mark the affected row ids and split ref retired.
3. Remove the affected split ref from active benchmark gates.
4. Mint a replacement split ref and checksum ref.
5. Regenerate any public score summaries that depended on the leaked rows.
6. Update this boundary document and the machine-studying audit with the
   retired refs and replacement refs.

Leaked private holdout rows must not be reused as hidden holdout rows. They may
be moved to a future public-retained or calibration lane only after a separate
redaction and product-claim review.

## Verification

Use this public-docs search before merging private-boundary changes:

```bash
rg -n "private_holdout|gold_answer" docs/research/machine-studying/openagents-studybench
```

Expected public hits are limited to public-retained example rows and boundary
docs. The ignored `private/` directory is local-only and must not appear in
Git-tracked row content.
