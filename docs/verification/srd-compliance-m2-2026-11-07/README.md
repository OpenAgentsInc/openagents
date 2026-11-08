# SRD compliance milestone 2 spell pipeline evidence

Date: 2026-11-07, America/Chicago. Baseline:
`593ceb26af8762425a7b83d8bc45b26e749f9546`. The commit containing this record
identifies the new importer, schema, source catalog and generated files.
[Source hashes](source-hashes.txt) bind the checked code and outputs.

**Outcome:** the Coax pipeline imports all 339 spell descriptions and all 875
base class-table entries from the pinned SRD 5.2.1 PDF. It preserves nine reviewed
source differences rather than changing either reading. Native regeneration is
byte-identical and malformed/wrong-source/stale-output cases refuse. This is
source-data infrastructure, not new executable spell coverage or full SRD
compliance. Other milestone 2 catalogs and mechanical-clause interpretation are
still open.

## Source and runtime

PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`,
6,031,375 bytes. The importer checks both before extraction, snapshots the exact
hashed bytes into private temporary storage, and directs Poppler 26.06.0 with
`-raw -enc UTF-8 -eol unix`. Spell descriptions are pp.107-175 and class tables
pp.28-82. Extraction hashes and per-class counts are retained in
[the manifest](../../../aethyr/rules/srd/data/manifest.txt).

The source count of 339 was independently checked by counting the description
level/cantrip headings before implementing the parser. Class-table entries are
parsed separately from description class masks and compared by spell identity,
level, school, concentration, ritual and material flags. The discrepancies were
reviewed against the relevant PDF text/page references, not resolved by trusting
one parser's output. No network source, model-generated prose or inferred effect
program enters the data.

The checked compiler was installed from the release cache by `coax/build.sh`,
fingerprint prefix `94c4583367f4`. Normal catalog builds do not invoke Poppler or
read the PDF. All work and commands ran in the isolated worktree; no visible
browser or audio was used. Each command completed within 15 minutes.

## Checks

| Command | Result | Evidence |
| --- | --- | --- |
| `coax/bin/coax test --library aethyr/rules/srd/import-emit.coax` | 13 checks passed | [Importer/parser/emitter/reconciliation fixtures](import-tests.txt) |
| `coax/bin/coax test --library aethyr/rules/srd/import-lists.coax --only 'class tables refuse'` | Updated table-flag check passed | [Final dash-glyph refinement](table-flags-test.txt) |
| `coax/bin/coax test --library aethyr/rules/srd/catalog.coax` | 3 checks passed | [Complete metadata bounds/uniqueness, independent spell fixtures and source contradictions](catalog-tests.txt) |
| `coax/bin/coax mutate --library aethyr/rules/srd/import-policy.coax` | 110 killed, zero survivors | [Source pin, publication and exact discrepancy policy](policy-mutate.txt) |
| `coax/bin/coax mutate --library aethyr/rules/srd/import-reconcile.coax --root-only` | 142 killed, zero survivors | [Root reconciliation authority](reconcile-mutate.txt) |
| `coax/web/tool.sh aethyr/rules/srd/import-check.coax PDF aethyr/rules/srd/data` | 6 native checks passed | [Pipeline receipts](native-pipeline.txt) |
| `coax/web/tool.sh tools-check` | No new tool without a reason | [Tools check](tools-check.txt) |
| `git diff --check` | Passed | Whitespace review |

The native fixtures check committed output against regeneration, fresh output
against committed bytes, refusal to replace existing output with all existing
files unchanged, read-only failure on corrupted generated code, same-size wrong
PDF hash with no output directory, and truncated PDF with no output directory.
The fresh destination includes spaces, exercising argv/path handling. Fixtures
clean up their owned temporary directory.

Mutation testing initially exposed missing source-boundary and integration
fixtures. Those checks were strengthened; no mutant was marked equivalent. The
final source-policy and reconciliation authorities have zero survivors. The
reconciliation receipt is explicitly **root-only**: it does not qualify mutation
coverage of imported parsing/serialization helpers. Those helpers have ordinary
fixtures and the real-PDF native pipeline checks. The final class-flag refinement
accepts both en dash and em dash printed by the source; its changed check was run
alone and the unchanged passing checks were reused. No unchanged passing gate
was rerun merely for confirmation.

The existing executable spell/combat modules received comments only. Their
milestone 1 result at `593ceb26af` is retained in
[the previous record](../srd-compliance-m1-2026-11-07/README.md); it was not
repeated. The five unmet milestone 1 witnesses are not fixed or suppressed by
importing data.

## Interpretation and remaining limits

The nine differences are the three Flaming Sphere school entries, four True
Strike material markers, Mind Spike's class-mask difference and Phantasmal
Force's absence from the base tables. Both readings remain in the generated
records; exact ID/field/value/page bindings authorize their preservation. Unknown
or changed differences prevent publication. This does not choose future casting
legality, allow an equivalent-mutant exemption or excuse missing mechanics.

Text retains the raw extractor's wrapping, hyphenation and content order; styled
PDF text can have merged spaces. Metadata wraps are joined with spaces, while
cost/trigger/alternative expressions retain their source strings. It is a full
extracted source representation, not a typography-fidelity claim. Targets, areas,
scaling, transformations and effect programs still need mechanical interpretation
and independent rule fixtures. All source inventory entries remain mechanically
unqualified. Subclass/feat grants are not base class table membership.

The pipeline uses bounded buffers and owned staging. No existing output directory
is replaced in ordinary operation; the native filesystem bridge does not offer
an adversarial no-replace rename primitive, so output publication assumes a single
local writer. Abrupt termination may leave temporary storage. Parser/serializer
correctness, the native compiler/host, Poppler and the filesystem remain trust
inputs beyond the scoped checks above. No owner action is required.

See [the pipeline guide](../../../aethyr/rules/srd/data/README.md),
[the ledger](../../dnd/srd-compliance-ledger.md), and
[the full roadmap](../../dnd/srd-compliance-roadmap.md).

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
