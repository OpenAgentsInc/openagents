# ASD-STE100 adoption audit and conversion plan

- Date: 2026-07-19
- Class: current-status
- Source snapshot: `030754d52632673f17cdd7b2a71a93866dd34800`
- Status: active company requirement with incomplete repository enforcement
- Dispatch: no
- Owner: OpenAgents documentation
- Requirement: All OpenAgents documentation and specifications must use ASD-STE100 Simplified Technical English.

## Result

The OpenAgents document estate does not conform to ASD-STE100 today.
The repository has no STE glossary, document profile, checker, or inspection gate.
The Sol checks examine status, links, manifests, and roadmap facts.
They do not examine language.

The conversion must start with the documents that control agents and authors.
It must then change active specifications and high-risk instructions to STE.
A company glossary and a document checker are necessary before bulk conversion.
An expert must also examine each important document.

This audit starts the conversion.
It does not show that the document estate is compliant now.

## Implementation status

Issue #9048 owns the P0 control system.
The repository policy is in [`docs/ste/README.md`](../ste/README.md).
The repository has a versioned glossary, profile schema, ledger, baseline, checker, and test corpus.

The migration check is a baseline ratchet.
It prevents a new structural defect in a file that is not converted.
The strict check does not use the baseline.
It requires an authorized Issue 9 dictionary outside Git.

The root fast check and the deploy check run the migration ratchet.
Thus, local hooks and OpenAgents-owned runners use the same checker.
This control does not show full conformance.
The P1 through P6 conversions and inspections are still necessary.

The policy has a controlled agent compact profile.
Human-facing text continues to use the base STE profile.
Agent-facing text should also use the base profile when possible.
The compact profile permits a versioned term extension and labeled record fragments.
It cannot relax authority, safety, evidence, or ambiguity controls.

The OpenAgents Desktop RC.25 release shows the dual-audience pattern.
Its human changelog explains user actions and release limits.
Its agent changelog records dense technical data with stable labels.
The released file remains immutable evidence.

## Standard authority

[ASD-STE100 Issue 9](https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf) is the standard now.
Its issue date is 2025-01-15.
Issue 9 fully replaces all earlier issues and revisions.

The standard has two main parts.
Part 1 gives the writing rules.
Part 2 gives the controlled dictionary.
The official [STE information page](https://www.asd-ste100.org/about_STE.html) gives information about the purpose and status now.

The local PDF and the official PDF have the same SHA-256 digest.
The digest is `d1f4ea9e7cd6e46b47aa9057209f99e78c0e9cfc4e27a5b07895b05c1a166431`.

ASD has the copyright and the registered trademark.
OpenAgents must not copy the standard or its dictionary into this repository without permission.
The repository can refer to the official source.
It can also keep OpenAgents policy, profiles, and technical terms.

## Need for this requirement

OpenAgents has human readers, agents, operators, contributors, and customers.
Some readers do not use English as their first language.
All readers can misunderstand long or ambiguous text.

STE gives OpenAgents these benefits:

- One approved term for one meaning
- Short sentences with clear topics
- Direct commands for procedures
- Consistent words across products and teams
- Better inputs for search, retrieval, translation, and evaluation
- Less ambiguity in requirements and acceptance criteria
- Less operator risk in release, security, payment, and recovery tasks.

Clear language also helps agents.
It decreases the number of possible interpretations of a requirement.
It makes conflicts between documents easier to find.
It also makes semantic checks more reproducible.

## Scope

The requirement applies to all OpenAgents technical documentation and specifications.
It applies to new text and to text that is in the repository now.

The scope includes:

- Markdown, MDX, AsciiDoc, reStructuredText, and text documents
- ProductSpec and AssuranceSpec documents
- Architecture decisions, roadmaps, runbooks, receipts, and audits
- API and OpenAPI descriptions
- Human-readable text in JSON, YAML, schemas, and manifests
- Explanatory text and comments in TLA+ and CFG specifications
- Public technical guides and generated reference text
- Documentation comments in source code.

Code, identifiers, paths, URLs, commands, and protocol literals are source data.
Authors must keep their values without changes.
Authors must write all text around that source data in STE.

Quoted legal text and third-party text are also source data.
Authors must identify this text as quoted text.
They must not change its meaning or its evidence bytes.

The preserved transcript archive is immutable evidence.
Its transcript bodies are quoted source data.
All new labels, indexes, summaries, and instructions for that archive must use STE.

## Company-wide boundary

This first audit measures only the `openagents` monorepo.
The company requirement also applies to all OpenAgents-owned repositories.
It also applies to technical documents that OpenAgents keeps outside Git.

The full program must make a company-wide inventory.
The inventory must include each root and child-repository control document.
It must also include hosted guides, API portals, operator documents, and specification systems.

One central glossary must contain all company terms.
Each repository can add subject terms through an STE inspection.
No repository can give a different meaning to an approved company term.

Use one versioned checker package in all repositories.
Each check result must identify Issue 9 and the company glossary revision.

## Technical terms in this audit

OpenAgents does not yet have an approved company glossary.
This audit uses a temporary set of technical nouns.
The conversion program must replace this set with a controlled glossary.

| Group           | Technical nouns                                                                                                        |
| --------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Standard        | ASD-STE100, STE, Issue 9                                                                                               |
| Company         | OpenAgents, Sol                                                                                                        |
| Document types  | ProductSpec, AssuranceSpec, roadmap, runbook, receipt, audit, transcript                                               |
| Formats         | Markdown, MDX, AsciiDoc, reStructuredText, JSON, YAML, TLA+, CFG, OpenAPI                                              |
| Repository      | Git, GitHub, repository, monorepo, main, commit, push, SHA-256, path, URL                                              |
| Source text     | source data, code block, inline code, identifier, protocol literal, documentation comment                              |
| Controls        | glossary, profile, parser, checker, linter, gate, manifest, schema, baseline, ratchet, STE inspection                  |
| Audit terms     | conversion, screening signal, word token, document estate, conformance receipt, semantic comparison, normative keyword |
| Shared controls | company-wide inventory, checker package, control document, API portal                                                  |
| Automation      | agent, subagent, CI, GCE, pre-commit check, pre-push check                                                             |

The glossary must also control technical verbs.
For example, OpenAgents must decide if `lint` is an approved technical verb.
The glossary must give one approved meaning and the permitted forms.

## Audit method

The audit used the checked-in files at the source snapshot.
It used `git ls-files` to get the tracked set.

The scanner removed code fences, inline code, URLs, and common Markdown marks.
It then counted simple language signals.
It did not make a full grammar analysis.

The scanner used these file types:

- `.md`
- `.mdx`
- `.rst`
- `.adoc`
- `.txt`.

The audit also counted TLA+ and CFG files below `specs/`.
It did not examine prose in JSON, YAML, source comments, or generated API files.
The full inventory must add these surfaces.

The audit did not count other OpenAgents-owned repositories.
Thus, the company-wide document count is larger than this result.

The counts are screening signals.
They are not conformance decisions.
Tables and unusual Markdown can cause false results.

## Inventory now

| Measure                           |    Result |
| --------------------------------- | --------: |
| Tracked prose files               |     2,732 |
| Preserved transcript files        |       268 |
| Prose candidates now              |     2,464 |
| TLA+ and CFG specification files  |        15 |
| Approximate prose word tokens now | 3,419,269 |

The prose set does not contain the preserved transcript files.
It includes first-party text, reference text, and some generated text.
The conversion inventory must put each file in a category before conversion.

## Divergence signals

| Signal                                  |                Result | Related STE control                 |
| --------------------------------------- | --------------------: | ----------------------------------- |
| Possible sentences longer than 25 words | 39,895 in 2,346 files | Descriptions have a 25-word maximum |
| Possible sentences longer than 20 words |                54,911 | Procedures have a 20-word maximum   |
| Semicolons                              | 48,794 in 2,019 files | STE does not permit semicolons      |
| Contractions                            |  3,892 in 1,120 files | STE does not permit contractions    |
| `should` or `shall`                     |    8,158 in 995 files | The dictionary controls modal verbs |
| Possible passive voice                  | 23,145 in 2,172 files | STE uses active voice by default    |
| Words with an `-ing` form               | 83,237 in 2,394 files | STE restricts the `-ing` form       |

The sentence scanner does not know the document type.
Thus, the 20-word result is an upper limit for possible procedure defects.

The passive voice result uses a simple pattern.
It does not find all defects and can report correct text.
The `-ing` result also includes permitted technical nouns and modifiers.

The audit did not test approved words, parts of speech, or approved meanings.
The repository has no authorized machine-readable dictionary or company glossary.
This absence is the largest conformance gap.

## Priority sample

The sample shows the scale in high-authority documents.
The values are screening results after the basic Markdown removal.

| Document                      |  Words | Sentences over 25 words | Semicolons | `should` or `shall` |
| ----------------------------- | -----: | ----------------------: | ---------: | ------------------: |
| `AGENTS.md`                   |  7,225 |                     100 |         95 |                  14 |
| `INVARIANTS.md`               | 20,265 |                     291 |        251 |                   1 |
| `AUTHORITY.md`                |    666 |                       7 |          2 |                   0 |
| `docs/sol/MASTER_ROADMAP.md`  |  6,273 |                      67 |        124 |                   1 |
| `docs/sol/OPERATING_MODEL.md` |  1,292 |                      13 |         54 |                   2 |
| Workroom ProductSpec          |  3,278 |                      40 |         24 |                   1 |
| Workroom AssuranceSpec        |    381 |                       5 |          3 |                   0 |
| `specs/CONVENTIONS.md`        |  1,068 |                       8 |         21 |                   0 |

These documents have effects on many subsequent documents.
Their conversion has a high multiplier.

## Main types of divergence

### Uncontrolled vocabulary

The repository uses many words for the same concept.
It also uses single words with different meanings.
There is no approved term list with a revision.

OpenAgents must have one controlled glossary.
The glossary must give technical nouns and technical verbs.
It must also give prohibited synonyms and approved short forms.

### Long and complex sentences

Many documents now use long sentences, nested clauses, and dense tables.
Some sentences contain several requirements and exceptions.
This structure makes each obligation difficult to test.

Descriptions must use a maximum of 25 words per sentence.
Procedures must use a maximum of 20 words per sentence.
Each descriptive sentence must have one topic.

### Indirect procedures

Many runbooks describe an action and do not give a direct command.
Some steps use passive voice.
Some steps contain more than one instruction.

Procedures must use the imperative form.
Each sentence must give one instruction unless actions occur at the same time.
A necessary condition must occur before the command.

### Weak document structure

The repository does not identify each text as procedural or descriptive.
Some documents mix the two types without clear sections.
Thus, a checker cannot apply the correct sentence limit.

Each document needs a profile.
Each mixed document needs a profile for each applicable section.

### Normative keyword risk

Some specifications use `MUST`, `SHOULD`, and `MAY` as protocol terms.
These terms can have meanings from RFC 2119 and RFC 8174.
An automatic word replacement can change the requirement.

The glossary must identify each normative keyword as a technical term.
The specification must state the source of its normative meanings.
Writers must not replace these keywords without a semantic inspection.

### Evidence and identity risk

Some specifications and receipts have digests or stable revision identities.
A text conversion changes the bytes and the digest.
It can also make a signature or a reference incorrect.

Make a new revision for each active specification conversion.
Keep the source revision as immutable source data.
Add a semantic comparison before the new revision becomes authoritative.

### Tooling gap

The repository checks Sol links, classes, snapshots, and manifests.
It has no STE language check.
Local Git hooks alone cannot enforce the company requirement.
An author can bypass them with `--no-verify`.

The repository prohibits GitHub-hosted CI.
Therefore, OpenAgents must do the necessary check on owned infrastructure.
A GCE runner or a release gate can do this check.

## What OpenAgents needs

### 1. A company policy

Add an STE policy to the root agent contract and the invariant ledger.
Set Issue 9 as the authority now.
Give rules for scope, inspections, source data, and exceptions.

Do not make a permanent legacy exception.
Use a time-bounded migration state for mutable documents.
Use the source-data state only for text that must stay without changes.

### 2. A controlled glossary

Keep the OpenAgents glossary in a versioned data file.
Do not copy the ASD dictionary into that file.

Each glossary row must have:

- A stable term identifier
- The approved term
- The part of speech
- One approved meaning
- The permitted forms
- The approved short form
- The prohibited synonyms
- An owner and a source
- A status and a revision.

Add different rows for product names, proper nouns, protocol keywords, and API literals.
An STE inspector must examine each new technical verb.
Keep the set of technical terms as small as possible.

### 3. Document profiles

Add machine-readable front matter to each governed document.
Use fields that have these meanings:

| Field                   | Purpose                                                    |
| ----------------------- | ---------------------------------------------------------- |
| `ste_issue`             | Select the ASD-STE100 issue                                |
| `ste_mode`              | Select descriptive, procedural, mixed, or source-data text |
| `ste_glossary_revision` | Select the company terms                                   |
| `ste_status`            | Show migration, checked, inspected, or source-data state   |
| `ste_reviewer`          | Identify the inspection role                               |
| `ste_reviewed_at`       | Give the inspection date                                   |

The last schema can use different field names.
It must keep these meanings.

### 4. A deterministic checker

Make one repository parser for all supported document formats.
The parser must identify prose and source data.
It must also identify procedural and descriptive sections.

The first hard checks must include:

- Approved terms and permitted forms
- Sentence word limits
- Semicolons
- Contractions
- Multi-word nouns with more than three words
- American English spelling
- Restricted `-ing` forms
- Passive voice in procedures
- Paragraphs with more than six sentences
- Must-have document profile fields.

The checker must give a rule number, location, and proposed action.
It must not silently write a normative requirement again.

### 5. A semantic inspection

A checker cannot make a decision about all STE rules.
The official [STEMG software guidance](https://www.asd-ste100.org/software.html) says that checkers are aids.
It also says that software does not replace the standard.

An STE inspector must make sure of:

- The technical meaning
- One topic in each sentence and paragraph
- The correct approved meaning for each word
- The correct technical noun or technical verb
- The logical order of information
- The approved meaning of normative keywords
- The risk and result in each safety instruction.

The author must make sure that the technical content is correct.
The STE inspector must make sure that the language is correct.

### 6. Author and agent support

Add an STE instruction to each document author agent prompt.
Add STE templates for audits, specifications, runbooks, and receipts.
Give authors access to Issue 9 and company training.

Add editor feedback for fast local corrections.
Do not let an automatic change modify code, identifiers, or requirement strength.

### 7. A migration ledger

Add one inventory row for each governed file.
Give each row an owner, risk class, profile, glossary revision, and status.

Use these terminal states:

- Human-inspected STE text
- Immutable source data with an STE frame
- Superseded text with an STE replacement
- Deleted duplicate with a correct provenance record.

The ledger must not use `tool-checked` as proof of full conformance.

## Highest-value conversion order

### P0: Make the control system

Make the policy, glossary schema, document profiles, checker, and test corpus.
Add the checks to owned infrastructure.

Make the policy and checker available to all OpenAgents-owned repositories.
Give one central owner the glossary and profile schemas.

This work prevents new divergence.
It also gives all subsequent conversions the same rules.

### P1: Change the author control plane to STE

Change these documents to STE first:

- `workspace/AGENTS.md`
- `workspace/INVARIANTS.md`
- `AGENTS.md`
- `INVARIANTS.md`
- `AUTHORITY.md`
- `docs/sol/README.md`
- `docs/sol/MASTER_ROADMAP.md`
- `docs/sol/OPERATING_MODEL.md`
- `docs/sol/CLAIM_PROTOCOL.md`
- `specs/CONVENTIONS.md`.

Then change local `AGENTS.md` and `INVARIANTS.md` files in all owned repositories.
Also change all company document templates and author instructions.

These documents control agent behavior and subsequent author work.
A defect in one of them can spread to many files.

### P2: Change active specification templates to STE

Change ProductSpec, AssuranceSpec, decision, and acceptance templates to STE.
Then change each active specification to STE with a new revision.

Do specifications with release, authority, payment, privacy, and safety effects first.
Use semantic comparison tests to prevent requirement drift.

### P3: Change high-risk procedures to STE

Change release, deploy, security, authentication, payment, recovery, and incident runbooks to STE.
These documents can cause direct operational harm when a reader misunderstands a step.

### P4: Change public technical text to STE

Change user guides, API guidance, setup instructions, and generated reference text to STE.
This work gives the largest external reader benefit.

### P5: Change active plans and audits to STE

Change active roadmaps, implementation plans, issue sources, and active audits to STE.
Keep the evidence links and revision identities without changes.

### P6: Finish the legacy estate

Change all remaining mutable documents to STE.
Put immutable evidence and third-party source text in the correct categories.
Remove duplicate or superseded text only with a correct provenance record.

## Enforcement model

Use more than one enforcement layer.

### Authoring layer

- Give each document a profile.
- Make the approved glossary available in author tools.
- Use STE templates for new documents.
- Give immediate local feedback.

### Repository layer

- Do a fast check on changed files before commit.
- Do the complete affected-document check before push.
- Reject missing profiles and unapproved terms.
- Reject expired migration exceptions.

### Owned infrastructure layer

- Do the full repository check on an OpenAgents-owned runner.
- Use the same versioned checker in all owned repositories.
- Record company-wide conformance states in one inventory.
- Make sure that the result is green before release or publication.
- Keep a public-safe conformance receipt.
- Put the document digest and glossary revision in the receipt.

This layer is necessary because an author can bypass local checks.
It must not use GitHub-hosted CI.

### STE inspection layer

- An STE inspector must examine all P0 through P3 documents.
- A subject expert must examine normative and safety text.
- Record the two roles in the conformance receipt.
- Do a new inspection when technical meaning changes.

## Conversion workflow

Use this workflow for each active document:

1. Identify the document owner and document profile.
2. Freeze the source revision and its digest.
3. Mark code, quoted text, and other source data.
4. Identify technical terms in the approved glossary.
5. Put procedural text and descriptive text in different sections.
6. Write the text again by sentence structure, not by word replacement.
7. Use the deterministic checker.
8. Compare normative meaning with the source revision.
9. Complete the technical and STE inspections.
10. Release the new revision and its conformance receipt.

Do not use bulk search-and-replace for this conversion.
It can change protocol strength, technical meaning, and evidence identity.

## Adoption rules

Apply these rules immediately:

- All new governed documents must use STE.
- All new specification prose must use STE.
- A change to a mutable document must include the complete active document.
- A changed signed specification must get a new revision.
- Add all new technical terms to the controlled glossary.
- A checker result is only one part of the conformance evidence.
- A source-data exception must identify the protected text without changes.

Do not add a large file or directory allowlist.
Each temporary exception must have an owner, reason, and end date.

## Risks and controls

| Risk                                                   | Control                                                    |
| ------------------------------------------------------ | ---------------------------------------------------------- |
| A simple text change modifies technical meaning        | Use a subject expert and semantic comparison               |
| A keyword change changes requirement strength          | Treat normative keywords as controlled technical terms     |
| A conversion breaks a digest or signature              | Make a new revision and keep the source bytes              |
| The glossary becomes an uncontrolled second dictionary | Limit it to OpenAgents technical terms                     |
| The checker gives false confidence                     | Use an STE inspector                                       |
| The checker reports too many false defects             | Use document profiles and accurate source-data marks       |
| Bulk churn breaks links and provenance                 | Change bounded groups to STE and make new manifests        |
| A local bypass defeats the gate                        | Use an owned infrastructure receipt                        |
| Copyrighted standard text enters the repository        | Refer to the official standard and keep only company terms |

## Success measures

The conversion is complete only when all these statements are true:

- The inventory contains all governed documentation surfaces.
- Each governed document has a document profile.
- Each technical term has an entry in the approved glossary.
- Each mutable active document has a human-inspected STE state.
- Each immutable source has an STE frame and a correct source-data state.
- Each active specification conversion has a semantic comparison.
- All deterministic hard checks pass.
- All P0 through P3 documents have technical experts and STE inspectors.
- The owned infrastructure gate prevents a nonconforming document.
- The legacy migration count is zero.

Record these program measures:

- Number of files in each conformance state
- Number of unapproved term uses
- Number of sentence, voice, and structure defects
- Checker false-positive and false-negative rates
- Time from draft to inspection
- Number of semantic changes found during conversion.

## Next work packet

The next packet must make the P0 control system.
It must not start a large text change.

The packet must deliver:

1. The root STE policy and invariant
2. The OpenAgents glossary schema and seed terms
3. The document profile schema
4. The deterministic checker and test corpus
5. The owned infrastructure gate
6. The migration ledger
7. The first P1 conversion plan.

After P0 passes, change the author control plane to STE as one inspected program.
Then start the active specification conversions.

## Audit limitation

This audit uses short sentences, active voice, controlled terms, and direct procedures.
The author did a manual STE structure inspection.
The repository has no approved-word checker or approved company glossary.
Thus, one more lexical inspection is still necessary.

The audit gives enough information to start the conversion program.
It is not a conformance receipt for the repository.
