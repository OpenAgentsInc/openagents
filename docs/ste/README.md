# OpenAgents Simplified Technical English policy

- Standard: ASD-STE100 Issue 9
- Glossary revision: `openagents-ste-glossary-v1`
- Policy revision: `openagents-ste-policy-v2`
- Scope: OpenAgents technical documents and specifications
- Status: active migration control

## Requirement

Write all new technical text in Simplified Technical English (STE).
Use the rules in ASD-STE100 Issue 9.
Use the approved OpenAgents terms in [`glossary.v1.json`](./glossary.v1.json).

## Audience profiles

Use the base STE profile for human-facing text.
Also use it for agent-facing text when it gives a clear and short result.

Some agent records need dense technical data.
Use the [agent compact profile](./agent-compact-profile.v1.md) for these records.
The profile adds [controlled terms](./agent-compact-terms.v1.json) and labeled record fragments.
It does not relax safety, authority, evidence, or ambiguity controls.
Strict lexical checks permit the extension only in an identified agent section.

A dual-audience document must identify the human and agent sections.
The agent extension applies only to the identified agent section.
Do not make human instructions depend on an agent section.

Do not copy the ASD dictionary into this repository.
Use an authorized dictionary file for a strict lexical check.
Set `ASD_STE100_DICTIONARY` to the path of that file.

## Source data

Do not change code, commands, paths, URLs, identifiers, or protocol values.
Treat this content as source data.
Also treat quoted legal text and third-party text as source data.

Add an STE frame around source data when readers need an explanation.
The frame must identify the source and its purpose.

The transcript archive is immutable source data.
Do not change transcript bodies.
Write new transcript labels and summaries in STE.

## Document profiles

The migration ledger gives each governed file a document profile.
The profile selects descriptive, procedural, mixed, or source-data text.
It also records the glossary revision and the review state.

Use these states:

- `migration`: The file has a temporary baseline and needs conversion.
- `checked`: The checker passed, but an inspector did not approve the text.
- `inspected`: A technical reviewer and an STE inspector approved the text.
- `source-data`: The file contains immutable or third-party source data.
- `superseded`: A current STE document replaces the file.

Only `inspected`, `source-data`, and `superseded` are terminal states.
A tool result is not proof of full STE conformance.

## Checks

Run `pnpm run check:ste` before you commit a document change.
The check uses the migration baseline for files in the `migration` state.
It rejects a new structural defect.
The normal ledger command keeps the prior baseline counts.

### Baseline controls

Only `generate:ste-baseline` replaces all baseline counts.
Use that command only for an approved baseline reset or migration start.
Use `--refresh-path=<path>` with the ledger generator for an approved file update.
This option replaces the baseline only for the specified migration file.

Run `pnpm run check:ste:strict -- <paths>` for converted documents.
Strict mode does not use the migration baseline.
It requires the authorized dictionary file.

The checker reports a rule, path, line, column, and corrective action.
The checker is an aid.
It does not replace a technical review or an STE inspection.

The deterministic check finds these conditions:

- An unapproved word in strict mode
- A sentence that exceeds the selected word limit
- A semicolon
- A contraction
- A selected British English word form
- An `-ing` form that needs inspection
- A passive construction in procedural or mixed text
- A paragraph with more than six sentences
- An OpenAgents technical noun with more than three words
- A prohibited OpenAgents synonym
- An absent or inconsistent document profile.

Use `pnpm run rewrite:ste-semicolons -- <paths>` for a mechanical punctuation pass.
The command does not change code fences, inline code, or URLs.
It also refuses a result that changes the normalized word sequence.
This pass does not complete an STE review.

Some grammar checks give possible defects.
They can report text that an STE inspector accepts.
The checker does not decide if a sentence has one topic.
It does not decide if a general noun group has more than three words.
The STE inspector must make these decisions.

A profile can accept a possible-defect rule after an identified review.
All profiles can accept the `-ing` and passive-voice rules.
An agent-only compact profile can also accept sentence and paragraph density rules.
The reviewer must confirm that the dense unit improves fast and precise agent communication.
A profile cannot accept a semicolon, contraction, word-form, term, or profile defect.

## Review

The author must confirm that the technical content is correct.
The STE inspector must confirm that the language is correct.
A subject expert must review normative and safety text.

Create a new revision when a conversion changes stable document bytes.
Keep the old revision as source data when another record identifies its bytes.
Record a semantic comparison before the new revision becomes authoritative.

### Semantic baseline

The P1 control conversion, P2 specification conversion, and P3 procedure conversion have an additional semantic baseline.
The baseline protects normative keywords, code literals, URLs, issue references, and numeric values.
Run `pnpm run check:ste-control-semantics` after each control document change.
This comparison does not prove semantic equality.
The technical reviewer must also examine the change.

When ProductSpec bytes change, update each exact AssuranceSpec subject digest.
Increment the assurance revision when this binding changes.
Record the source and converted digests in the conversion receipt.

### Semantic baseline update

Use `--capture-path=<path>` with the semantic checker after an approved control update.
This option replaces the semantic baseline only for the specified control file.

## Copyright

ASD has the copyright for ASD-STE100 and its dictionary.
This repository stores only OpenAgents policy and OpenAgents technical terms.
Use the [official Issue 9 source](https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf).

## Owned runner

The root `check:deploy` command runs the full migration ratchet and its tests.
OpenAgents-owned runners use this command before a release or publication.
The root fast check also runs the full migration ratchet before a push to `main`.

Install an authorized dictionary on the owned runner for strict lexical checks.
Keep the dictionary outside Git.
Set `ASD_STE100_DICTIONARY` to its absolute path.
Run strict checks only for files that have completed conversion.

The authorized dictionary adapter accepts a JSON object.
The object must contain `steIssue: 9` and an `entries` array.
Each entry must contain a `permittedForms` string array.
