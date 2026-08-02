# Wallet-security posts and Omega analysis-thread audit

Status: **source and product-shape audit.** This document records what can and
cannot be concluded from four owner-supplied screenshots and the preserved
Omega analysis that consumed them. It does not verify any wallet finding,
endorse any product rating, reproduce the disputed Passport claim, authorize a
scan, contact a maintainer, or authorize publication.

Date: 2026-08-02

## 1. Evidence boundary

The retained evidence consists of four PNG captures supplied by the owner:

| Capture                 | Public page visible in the capture                                                                           | What is visible                                                                           | SHA-256 of retained private capture                                |
| ----------------------- | ------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Wallet comparison table | No standalone public URL is visible                                                                          | A 15-row red/yellow/green table with entropy and open-source columns                      | `32e10ba1c09a234330567b67e0f561127a168f8fb540273a06dc6c57d37f962a` |
| Wallet-analysis post    | [`x.com/bitcoinsbanker/status/2083612094713180249`](https://x.com/bitcoinsbanker/status/2083612094713180249) | The post's stated method, yellow-status explanation, and an embedded copy of the table    | `b82d8d6916a1928b9bf6b048ba4ae1bfd55acd9a1bac1fedb6d80a4276f7ea71` |
| OpenAgents response     | [`x.com/OpenAgents/status/2083754704392134810`](https://x.com/OpenAgents/status/2083754704392134810)         | A warning about single-model output, direct publication, and use of a structured verifier | `9552accdfc9dd712f76b38cc810c5c263ed6a4eac17fb366b511f18c538b97e6` |
| Zach Herbert response   | [`x.com/zherbert/status/2083710256270503978`](https://x.com/zherbert/status/2083710256270503978)             | A disputed Passport Core claim and an opinion about comparative model capability          | `3643a18eea93aa2281f6755b274db417b8a5c5aa860e83d9a364a8f75578fc6f` |

The captures include surrounding desktop chrome and a camera overlay. They are
not committed because they contain unnecessary personal imagery and local UI
context. The content digests permit a private reviewer to bind a future
redacted evidence record to the exact retained captures without publishing the
captures themselves.

The captures establish only that the displayed statements and table were
present in the supplied images. They do not establish authorship, completeness,
current availability, correctness, the omitted evidence dossiers, or the
technical truth of any finding or rebuttal. The live X pages were not available
to the reviewing agent through its unauthenticated fetch path, so the audit
does not silently fill missing text from a mutable page.

## 2. Corrected visual observations

The table contains 15 product rows:

- two red rows: Coldcard MK4/Q1 and Trezor Model One/Model T;
- four green rows: SeedSigner, Sparrow, Trezor Safe 3/5/7, and BitBox02; and
- nine yellow rows: Opendime, Bitkey, BlueWallet, Phoenix, Blockstream Jade,
  Ledger, SpecterDIY, Electrum for Android, and the discontinued Samourai
  Wallet.

An earlier Omega summary mentioned the Coldcard red row but omitted the red
Trezor Model One/Model T row. The omission matters because the two red rows do
not assert the same kind of problem. The Coldcard row alleges an exploited RNG
failure. The older Trezor row describes physical seed extraction and recommends
a passphrase or migration. A single color therefore combines different threat
models and cannot be read as one normalized vulnerability verdict.

The table also mixes at least these axes:

1. entropy and key-generation behavior;
2. physical extraction resistance;
3. source availability;
4. build reproducibility;
5. closed secure-element or firmware boundaries;
6. vendor-operated recovery or oracle dependence;
7. maintenance state; and
8. migration or patch posture.

The accompanying post explicitly says that yellow does not mean broken and
that its nine yellow products have no confirmed fund-loss-by-default flaw. It
uses yellow for several unrelated limitations, including unverifiable closed
components, vendor dependencies, bounded weaknesses, maintenance concerns, and
recently patched findings.

**Audit conclusion:** the table can be an intake index, but its product-level
color must not enter the forensic system as evidence or severity. The
workbench needs one version-bound claim per asserted mechanism and one status
per evidence axis. Product-level rollups, if shown at all, must expose their
derivation and must not hide a disputed or missing component behind a color.

## 3. Claim and counterclaim are both unverified intake

The OpenAgents capture argues that one model can produce careless results,
that vulnerability claims should not be published directly to a social feed,
and that Project Loupe at least supplies structured prompts and some
verification. The Zach Herbert capture says that a critical Passport Core
finding attributed to Kimi was incorrect and expresses the opinion that other
named systems perform better on embedded firmware.

These captures support three product requirements:

- a model output is a candidate, not a vulnerability;
- a public rebuttal is counterevidence, not automatic falsification; and
- comparative model reputation is a hypothesis to benchmark, not a permanent
  routing policy.

The screenshots do not include the disputed Passport finding, target revision,
source references, build configuration, proof, verifier transcript, or
maintainer evidence. The forensic record should therefore represent the
episode as a disputed external observation with two visible assertions and an
open next check. It should not decide the dispute by trusting either poster.

Loupe is also not a truth oracle. The repository study records valuable prompt
and verdict-ordering discipline, but it also records an apply-only PoC check and
a preliminary Omega scan whose verifier pipeline did not complete. “Ran
Loupe” is provenance; executed reproduction and independent evidence determine
the claim rung.

## 4. Preserved Omega thread and delegation provenance

The owner-supplied captures were analyzed in a native Omega thread using the
Luna model. That thread:

1. summarized the Loupe and Coldcard documents and issue #9300;
2. described the intended workbench in plain language;
3. summarized the first two wallet-analysis captures;
4. summarized the two critical-response captures;
5. proposed a multi-model, evidence-gated workbench; and
6. delegated creation of the model-panel and publication-gates audit.

The delegated documentation agent received the parent agent's textual
characterization and requested controls. It did **not** receive the four image
payloads. The resulting
[model-panel audit](2026-08-02-forensic-model-panel-and-publication-gates-audit.md)
is appropriately conservative, but it cannot be cited as an independent visual
inspection. This audit supplies the missing pixel-level reconciliation.

The Luna thread ended before its intended final response because the hosted
request was rejected as an invalid tool-schema payload. The visible error
named unexpected `type` fields inside function-declaration parameter
properties. This is evidence of a request/tool-contract compatibility failure,
not a model security refusal and not evidence that the model could not analyze
the subject.

The forensic run vocabulary therefore needs distinct outcomes for:

- `request_schema_invalid`;
- `tool_contract_incompatible`;
- `security_filter_triggered`;
- `model_refusal`;
- `provider_policy_restriction`;
- `provider_unavailable`; and
- `model_response_failed`.

None of these outcomes is a target miss or a clean result. A fallback creates a
new visible arm and retains the failed requested arm.

## 5. Product consequences

### 5.1 Agreement is not verification

One mechanically reproduced result can be correct despite panel disagreement.
Several models can share the same incomplete source bundle, historical
contamination, or mistaken inference. Model agreement is useful routing and
review evidence, but it is neither required nor sufficient for confirmation.

### 5.2 The model panel is task-specific

The roster should be frozen before result inspection and should cover distinct
model families, providers or local runtimes, and relevant competencies such as
embedded firmware, C/C++ build systems, cryptography, and adversarial review.
Named-model claims in a social post motivate a benchmark. They do not establish
a permanent “best model” list.

### 5.3 Publication is a separate product state

A finding can be analytically confirmed and still remain non-publishable. The
publication gate should require the exact target/version, bounded claim,
supporting and refuting evidence, reproduction, fixed and clean controls,
independent review, disclosure state, redaction review, and an authorized human
decision. The default state is `private_not_publishable`.

### 5.4 External observations need first-class intake

The workbench should let an operator attach a URL, redacted capture digest,
capture time, claimed author, exact proposition, counterclaim, and missing
evidence. External intake should never be silently converted into a finding.

## 6. Short-term Comet UI implications

The simplified Comet-based Omega shell should make the evidence boundary
visible before it attempts a broad forensic dashboard. The first useful slice
is one read-only Coldcard case projection with these elements:

1. a sticky case header with target pin, source completeness, worker authority,
   model-panel coverage, run state, cleanup state, and publication state;
2. separate **Observations**, **Hypotheses**, **Findings**, and **Disputes**
   queues;
3. a claim inspector that shows supporting and refuting evidence, assumptions,
   non-implications, and the next mechanical check;
4. a persistent limitations strip that names unavailable models, refusals,
   request-schema failures, unsupported inputs, censoring, and incomplete
   evidence without converting them to zero; and
5. an always-visible publication gate that begins blocked and explains exactly
   which evidence or authority is missing.

The next slice adds the live target/preflight and run lifecycle only after
issues #9289 and #9290 have accepted evidence for the exact managed worker and
source-delivery path. Until then, the UI must show the implemented fixture or
contract projection as development evidence and render live launch as
unavailable. It must not use a green fixture state to imply a deployed run.

## 7. Disposition

- Retain the four captures privately by digest; publish only redacted,
  proposition-level observations.
- Correct prior summaries to include both red rows and the mixed-axis nature of
  the table.
- Treat the Passport assertion and rebuttal as a dispute pending exact evidence.
- Extend the limitation vocabulary for request and tool-contract failures.
- Use diverse models to broaden candidate coverage, not to vote on truth.
- Make the first Comet forensic surface a clear evidence and limitation reader.
- Keep live launch, disclosure, and public claims blocked until their separate
  gates have accepted evidence.
