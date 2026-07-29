# OpenAgents public documentation STE policy

- Standard: ASD-STE100 Issue 9
- Glossary revision: `openagents-ste-glossary-v1`
- Policy revision: `openagents-ste-policy-v3`
- Scope: public-facing documentation only

## Publication boundary

STE applies only to text files in these configured publication roots:

- `apps/openagents.com/apps/start/content/docs/`
- `apps/openagents.com/apps/start/public/`

The checker reads this boundary from
[`checker-config.v1.json`](./checker-config.v1.json). A file is governed only
when its path starts with a configured `governedPrefixes` value and its
extension is governed. Path and extension checks are both required.

The first root contains authored website documentation. The second contains
documents shipped through the public static surface, including public agent
instructions. Generated public documentation mirrors and public font license
text are source data.

If a new publication surface lives outside these roots, add its precise root
to the checker configuration before publishing it. Do not broaden the scope to
an internal directory merely because that directory contains Markdown.

## Internal documents are excluded

Internal documents are not governed by STE. This includes strategy,
teardowns, roadmaps, audits, plans, specifications, runbooks, receipts,
research notes, issue records, and agent working documents. In the current
repository shape, that means documents under `docs/`, `specs/`, packages, and
application-internal documentation unless a path is also inside one of the
explicit publication roots above.

Internal documents can use the vocabulary, tone, sentence length, and
structure that best communicate their technical meaning. They do not need an
STE profile, ledger entry, inventory digest, baseline update, semantic capture,
or STE check. Copying or generating their content into a configured public
root makes the published copy governed.

## Public document requirements

Write governed public documents in Simplified Technical English. Use the rules
in ASD-STE100 Issue 9 and the approved OpenAgents terms in
[`glossary.v1.json`](./glossary.v1.json).

Use the base STE profile for public human-facing text. Public agent-facing
instructions can use the
[`agent compact profile`](./agent-compact-profile.v1.md) when its controlled
extensions improve precision or scan speed. The compact profile does not relax
safety, authority, evidence, or ambiguity controls. Do not apply it to public
human-facing text.

Do not copy the ASD dictionary into this repository. A strict lexical check
requires an authorized local dictionary file through
`ASD_STE100_DICTIONARY`.

## Source data

Do not rewrite commands, paths, URLs, identifiers, protocol values, quoted
legal text, third-party text, or generated output merely to satisfy STE. Treat
those spans as source data and add a clear STE frame when readers need context.

The generator writes public Markdown mirrors and `llms` files under
`apps/openagents.com/apps/start/public/docs/`. Do not edit those generated
mirrors directly. Change the authored source under
`apps/openagents.com/apps/start/content/docs/` and run the site generator.

## Profiles and inventory

The migration ledger gives each governed public file a profile. A profile
selects descriptive, procedural, mixed, or source-data text and records its
review state:

- `migration`: the public file has a temporary baseline and needs conversion.
- `checked`: the checker passed, but an inspector did not approve the text.
- `inspected`: a technical reviewer and an STE inspector approved the text.
- `source-data`: the public file contains immutable, generated, or third-party
  source data.
- `superseded`: a current public STE document replaces the file.

Only `inspected`, `source-data`, and `superseded` are terminal states. A tool
result is not proof of full STE conformance.

The final inventory binds governed public paths and terminal states to exact
SHA-256 digests. It does not inventory internal documents.

## Checks

For a change that touches governed public documentation, run:

```bash
pnpm run check:ste
```

That command checks changed governed files. To check the complete public
documentation corpus, run:

```bash
pnpm run check:ste:public
pnpm run check:ste-public-semantics
pnpm run test:ste
```

`check:ste:all` and `check:ste-control-semantics` remain compatibility aliases.
They do not expand enforcement beyond the configured public scope.

After an approved change to governed paths or terminal review state, regenerate
the digest-bound records in this order:

```bash
pnpm run generate:ste-final-inventory
pnpm run generate:ste-ledger
```

Use `generate:ste-baseline` only for an approved public migration reset. Use
`--refresh-path=<path>` for an approved update to one public migration file.

Strict mode does not use the migration baseline:

```bash
pnpm run check:ste:strict -- <public-paths>
```

It requires an authorized Issue 9 dictionary outside Git.

The public semantic baseline protects configured public control documents. It
compares normative keywords, code literals, URLs, issue references, and numeric
values. After an approved control change, capture only that public path:

```bash
node --import tsx scripts/check-ste-semantic.ts --capture-path=<public-path>
```

This comparison does not prove semantic equality. A technical reviewer must
also inspect the change.

## Checker limits

The deterministic checker reports selected structural and vocabulary signals,
including sentence length, semicolons, contractions, selected word forms,
possible passive voice, paragraph density, and OpenAgents terminology. Some
signals require human judgment. The checker cannot prove that a sentence has
one topic or that the complete meaning is safe and correct.

An identified review can accept only the screening rules permitted by the
profile. It cannot accept prohibited punctuation, contractions, word forms,
terms, or profile defects.

## Copyright

ASD owns the copyright for ASD-STE100 and its dictionary. This repository
stores only OpenAgents policy, profiles, and technical terms. Use the
[official Issue 9 source](https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf).
