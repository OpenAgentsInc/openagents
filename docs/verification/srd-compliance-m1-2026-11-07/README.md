# SRD compliance milestone 1 evidence

Date: 2026-11-07 (America/Chicago). Baseline:
`f7cab6d3a4ef98752ecb7d7cb834a90ea9af7bbc`. The new tooling and documents
are identified by the commit containing this record; relevant source hashes
are retained in [source-hashes.txt](source-hashes.txt).

**Outcome: not SRD compliant.** The requested baseline ledger and focused
regression witnesses are present. Five witnesses fail; those failures are
retained and counted as failures, with no expected-failure suppression. No
product rule behavior was changed. The next work is source inventory/data
reconciliation, strict casting/storage admission and chosen-form transformations.

## Commands and results

All commands ran in the isolated `srd-compliance-ledger` worktree on macOS.
The compiler was installed by `coax/build.sh` from its checked release cache,
fingerprint prefix `94c4583367f4`. No full compiler or application release gate
was rerun. Each command completed within the repository's 15-minute limit.

| Command | Exit | Evidence and scope |
| --- | --- | --- |
| `coax/bin/coax test --library aethyr/rules/srd/compliance-probes.coax` | 0 | [91 checks passed](probes-test.txt): four new checks plus 87 existing imported checks |
| `coax/bin/coax test --library aethyr/rules/srd/compliance-policy.coax` | 0 | [One policy check passed](policy-test.txt), including malformed and single-result boundaries |
| `coax/bin/coax mutate --library aethyr/rules/srd/compliance-policy.coax` | 0 | [13 mutants killed, zero survivors](policy-mutate.txt); only the new report exit policy is qualified by this mutation run |
| `coax/web/tool.sh aethyr/rules/srd/compliance-check.coax` | 1 | [Four PASS, five FAIL](focused-regressions.txt); focused desired-outcome probes, not whole-SRD qualification |
| `coax/web/tool.sh aethyr/rules/srd/compliance-check.coax --inventory` | 0 | [73 top-level candidates, five helpers, four house records](spell-records.tsv); metadata presence only |
| `coax/web/tool.sh tools-check` | 0 | [No new tools without a firm reason](tools-check.txt) |
| `git diff --check` | 0 | No whitespace errors |

The 91-check result precedes refinements to the **unasserted capacity witness**
(distinct effect kinds and detection of a silently dropped incoming effect) and
comments, and the unasserted class-access witness's switch to Guiding Bolt
(Ray of Sickness can be granted by Circle of the Land). Its four ordinary checks
and the 87 imported checks are unchanged;
their passing result was reused rather than repeated. The strict native tool
was rerun after these witness refinements and the final output is retained.
The initial exit-policy mutation exposed a missing single-result boundary; its
test was strengthened and the final zero-survivor run is retained. No mutant was
marked equivalent. Existing product modules were not mutation-qualified here.

## Witness interpretation

- `CAST-ACTOR`, `CAST-SLOT`, `CAST-TARGET-INDEX`, `CAST-RANGE` pass selected
  malformed/unavailable-request boundaries without spending.
- `CAST-CLASS` fails: an unmodified Druid can cast Guiding Bolt through
  `rs_cast`; that boundary has no class/preparation state enforcement.
- `CAST-INVALID-TARGET` fails: Hold Person on a Skeleton refuses before spending.
  SRD p.106 requires an expended slot to remain spent on an invalid-target cast;
  p.141 restricts Hold Person to Humanoids. This differs from a malformed target
  index and must remain a separate admission/execution case.
- `SPELL-POLYMORPH` fails: the current program applies Incapacitated, with no
  chosen Beast statistics or form temp HP. The witness checks only necessary
  properties from pp.153-154; it cannot qualify the full spell even if it passes.
- `CAP-EFFECT` fails: adding beyond six entries silently overwrites an existing
  effect. The witness also rejects silently dropping the new effect. The current
  void API offers no explicit refusal; a future refusal API must expose its result.
- `CAP-ZONE` fails: a lasting cast with all eight unrelated zones occupied spends
  resources and changes concentration without retaining a new zone. Full capacity
  must be handled before spending. This is an implementation storage contract,
  not an SRD limit on spell concurrency.

Refusal snapshots cover a documented state projection, not every byte of a
battle. Record text/page/program presence does not prove correctness. No full
source inventory, spell-by-spell golden suite, browser check, world migration or
complete SRD qualification was performed. This change introduces no runtime
gate and preserves all existing world behavior.

## Source review

The owner-local PDF was read using Poppler text extraction. Its SHA-256 and byte
count are pinned in [the ledger](../../dnd/srd-compliance-ledger.md). Printed
pp.41-46, 104-106, 141 and 153-154 supply the selected class/casting/Polymorph
expectations. Capacity expectations come from the shared engine's complete
effect/result contract, explicitly distinguished from SRD mechanics. No extracted
full source text or PDF is committed. No owner action is needed.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
