# Coldcard forensic model-panel and publication-gates audit

Status: **analysis and planning only.** This audit proposes controls for a future
Coldcard forensic workbench run. It does not implement a model panel, authorize a
scan, reproduce the defect, assess a wallet, contact a maintainer, or authorize a
public claim.

Date: 2026-08-02

## Scope and evidence labels

This audit uses three labels deliberately.

| Label                                  | Meaning in this document                                                                                                                                  |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Repository observation**             | A statement reported in a cited repository document. This audit did not re-execute the cited work.                                                        |
| **User-supplied external observation** | An X post or screenshot supplied with this request. Its authorship, capture time, completeness, and underlying claim are not independently verified here. |
| **Recommendation**                     | A proposed future control. It is not a statement that the control exists or is enabled.                                                                   |

The user-supplied X posts and screenshots are intake material, not source,
artifact, generator, chain, or identity evidence. They may be retained only as
redacted external-observation records: a content digest, declared source URL or
capture reference when supplied, capture context, redaction state, and the exact
claim they purport to support. This audit does not repeat their substantive
assertions as facts. A post or screenshot can motivate a check; it cannot make a
negative result, a model output, or an unexecuted explanation affirmative
evidence.

The companion
[wallet-security posts and Omega-thread audit](2026-08-02-wallet-security-posts-and-omega-thread-audit.md)
records the exact supplied URLs, redacted image digests, visible table counts,
thread provenance, and tool-contract failure. This document uses that audit's
public-safe observations rather than republishing the screenshots.

## Repository observations

1. The Coldcard practice run already requires pinned sources, five benchmark
   arms, frozen prompt/model/tool inputs, distinct discovery and verifier
   identities, a six-link source rubric, fixed and clean controls, and a
   claim ladder that stops where evidence stops. It explicitly treats a source
   hit as unverified until later independent evidence exists.
   [Practice runbook](2026-08-01-omega-coldcard-forensic-practice-runbook.md)

2. The existing Coldcard experiment was intentionally narrow: one model family,
   one run per arm, no working verification stage, and a hand-selected,
   post-disclosure scope. Its dependency-complete arm found the known source
   chain, but the result document retains the limits rather than converting that
   run into a general detection claim.
   [Pre-registration](../loupe/2026-08-01-coldcard-prefix-experiment.md),
   [results](../loupe/2026-08-01-coldcard-prefix-experiment-results.md)

3. A different-model-family verifier is useful, but it is not independent proof.
   Two systems can share an incomplete bundle, a contaminated historical prompt,
   or the same incorrect inference. The roadmap therefore gives factual source
   and artifact checks to deterministic evidence, not to a model judge.
   [Codex analysis](../loupe/2026-08-01-codex-analysis.md),
   [roadmap](../loupe/2026-08-01-omega-forensic-analysis-roadmap.md#independent-evaluation)

4. The observed Loupe self-scan provides a concrete warning against trusting
   unverified model output: 132 discovery claims were left unverified when the
   verifier pipeline failed. A productive scan, an asserted verdict, or a
   plausible PoC diff did not become evidence.
   [Preliminary scan](../loupe/2026-07-31-omega-first-scan-preliminary.md)

5. The existing workbench rationale already calls for a model matrix across
   Claude, Codex, Grok, and open-weight systems, a matched run matrix, isolated
   arm state, and treating divergence as a lead rather than noise. It also
   identifies single-provider policy changes as a material resilience risk.
   [Workbench speculation](../loupe/2026-07-31-omega-first-class-pentester-speculation.md),
   [coordination analysis](../loupe/2026-08-01-coordination-not-scanners.md),
   [Fix-as-a-Service thesis](../loupe/2026-07-31-fix-as-a-service-company-thesis.md)

6. The current evidence model already preserves `not proven`, incomplete input,
   unavailable usage, missed and right-censored runs, and reconciliation states
   such as `MATCH`, `DRIFT`, and `UNAVAILABLE`. It also keeps source selection,
   artifact reality, generator behavior, owned-fixture recovery, fingerprinting,
   movement, and identity as separate claim rungs.
   [Practice runbook](2026-08-01-omega-coldcard-forensic-practice-runbook.md),
   [evidence derivation](../loupe/2026-08-01-coldcard-evidence-derivation.md),
   [historical fingerprint scan](../loupe/2026-08-01-coldcard-historical-fingerprint-scan.md)

7. The repository's independent node analysis illustrates the desired behavior
   when a published mechanism does not reproduce: it records the discrepancy,
   narrows the conclusion, and does not smooth it into a pass. That is the right
   precedent for model disagreement and for external-observation conflicts.
   [Bitcoin-node forensic capability](2026-08-01-bitcoin-node-forensic-capability.md)

## External-observation reconciliation

The four supplied screenshots show a wallet-security comparison, two replies,
and the Omega conversation that attempted to analyze them. The visible table
contains 15 wallet rows: two red, four green, and nine yellow. Coldcard and
older Trezor models are the two red rows. A prior text-only summary missed the
Trezor row. The screenshot therefore demonstrates why the original image must
remain available to a reviewer even when an agent summary exists.

The color table is not a forensic scorecard. Its rows visually compress
distinct questions—entropy generation, physical attack resistance, source
openness, reproducible builds, maintenance, and vendor dependence—into one
categorical mark. The supplied rebuttal is useful counterevidence to the source
post, but neither post establishes the underlying technical claims. The bench
should preserve both as disputed intake and route each proposition to the
evidence ladder that could settle it.

The delegated agent in the captured Omega thread received a textual
characterization of the screenshots rather than the image bytes. The later
Luna response then failed before producing a final answer because the provider
rejected an invalid function-declaration payload. The visible error is a tool
or request-schema incompatibility, not evidence that the model refused the
security topic. Provider and tool failures must stay separate from model
refusals and from claims about the target.

## Audit conclusion

**Recommendation:** The workbench should use a pre-registered, diverse model
panel for candidate generation and review. It must never treat one model, a
majority vote, a refusal, a security filter, an outage, an unsupported input, or
panel agreement as proof of either presence or absence. Mechanical reproduction
and independently executed evidence remain the path from a model claim to a
forensic claim.

The panel is a coverage and prioritization instrument. It is not a truth oracle,
and it does not replace the practice run's source-completeness, artifact,
generator, fixed-control, clean-control, or claim-rung gates.

## Recommended model-panel protocol

### 1. Freeze a roster before input inspection

**Recommendation:** Each evaluation series should record a roster before any
arm-specific result is examined. The roster should identify, at minimum:

- model family and provider or local-runtime class;
- exact model revision or provider-reported version, effort and parameters;
- role: discovery, challenger, verifier, evaluator, or unavailable fallback;
- prompt, tool-policy, schema, source-bundle, benchmark-arm, worker-image, and
  evaluator digests;
- allowed input types and known modality/tool limits;
- independent execution identity, provider session, writable state, and worker
  placement; and
- declared fallback order and per-role budget.

The intended discovery roster is four diverse families: Claude, Codex, Grok,
and an approved open-weight model. Names are examples of model-family classes,
not a claim that any particular revision is available. The open-weight member
should run independently enough to expose provider-policy and training-data
monoculture; it should not be represented by a second endpoint for the same
underlying model.

**Recommendation:** A smaller panel is acceptable only as an explicitly named
partial-panel experiment. It should include at least two distinct model families
for comparative discovery and should never be described as full attacker-parity
coverage. A one-model result is a single-model observation, not a panel result.

### 2. Use fallbacks without concealing substitutions

**Recommendation:** Fallbacks should preserve the role and create a new,
visible arm. They should not silently replace a missing family with a different
revision, a same-family endpoint, or a different prompt/tool capability.

| Requested role is unavailable because of     | Recommended fallback                                                                                                                   | Required recorded limitation                            |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| One closed-model provider is unavailable     | Use the next pre-registered _different_ family; retain the missing arm.                                                                | `provider_unavailable` and `panel_partial`              |
| A provider rejects the task under its policy | Route only the allowed, defensive, redacted task to a pre-approved different family or open-weight member.                             | `model_refusal` or `provider_policy_restriction`        |
| A security filter blocks input or output     | Use a safely reduced, authorized representation only when it preserves the frozen task; otherwise stop that arm.                       | `security_filter_triggered` with a redacted reason code |
| An input modality is unsupported             | Provide the same normalized, digest-pinned text or metadata only when the manifest says it is equivalent; otherwise do not substitute. | `unsupported_input` or `unsupported_modality`           |
| An open-weight runtime is unavailable        | Continue only as a declared closed-panel partial result; do not relabel it as attacker parity.                                         | `open_weight_unavailable` and `panel_partial`           |
| The independent verifier is unavailable      | Do not promote a discovery result. Queue a later distinct verifier or retain `unverified`.                                             | `verifier_unavailable`                                  |

Fallbacking must not broaden authorization, add network access, bypass a safety
control, or alter the benchmark. A fallback is an availability decision, not a
license to weaken the evidence boundary.

### 3. Separate roles and prevent circular confirmation

**Recommendation:** Discovery models may submit typed findings or hypotheses.
A challenger should try to falsify the causal chain, especially its missing
dependency, configuration, and final-artifact assumptions. A verifier must use
a distinct identity from discovery and lock its verdict before any repair work.
The frozen deterministic evaluator should judge rubric facts such as source
reference validity, the six required causal links, fixed/clean outcomes, and
holdout membership.

No model may verify its own discovery result. No count of agreeing models may
replace the artifact witness, an observed vulnerable-target failure, a fixed
target success, or the appropriate evidence rung. The recommended decision
logic is therefore:

```text
panel output -> typed finding or hypothesis
             -> independent mechanical and/or executed check
             -> claim rung with explicit non-implications
             -> separate human review and publication decision
```

## Typed limitations, abstention, and disagreement

**Recommendation:** Every non-successful or non-comparable model outcome should
be retained as a typed limitation in the run record and scorecard. It is not a
zero, a clean result, or evidence that the target lacks the suspected property.

| Typed outcome                     | What it means                                                                                                                    | Default evidentiary effect                                                   |
| --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `model_refusal`                   | The model declined the task.                                                                                                     | No inference about the target.                                               |
| `security_filter_triggered`       | Provider safety controls blocked some input or output.                                                                           | No inference about the target; retain only redacted diagnostics.             |
| `provider_policy_restriction`     | The requested role or task is disallowed by provider policy.                                                                     | No inference about the target or model capability.                           |
| `provider_unavailable`            | An outage, capacity loss, rate limit, or auth-health event prevented an arm.                                                     | Right-censor the arm; do not count it as a miss.                             |
| `unsupported_input`               | The provider cannot accept a required source, artifact, language, modality, or tool shape.                                       | Mark the arm ineligible for that task; do not use a negative result.         |
| `input_incomplete`                | Required source, dependency, artifact, or external-observation provenance is absent.                                             | `completed_incomplete` or preflight refusal; no qualified denominator entry. |
| `output_schema_invalid`           | The model produced output that cannot enter the typed finding or hypothesis lane.                                                | Diagnostic only; no finding and no absence claim.                            |
| `request_schema_invalid`          | The provider rejected the request before an eligible model response because the serialized request did not match its API schema. | Infrastructure diagnostic only; the model did not evaluate the target.       |
| `tool_contract_incompatible`      | The declared tool surface cannot be represented or accepted by the selected provider/model route.                                | Mark the arm ineligible until repaired; do not call it a refusal or miss.    |
| `model_response_failed`           | A response started or was requested but no valid terminal model result was retained.                                             | Right-censor the arm and retain the narrower provider/tool cause when known. |
| `model_abstained`                 | The model explicitly says the evidence is insufficient.                                                                          | A useful uncertainty observation, never a clearance.                         |
| `model_disagreement`              | Eligible arms differ on claim identity, causal links, scope, or stated limits.                                                   | A lead for targeted independent checking; never settled by majority vote.    |
| `verifier_unavailable`            | The distinct verifier cannot run or settle.                                                                                      | Discovery remains unverified.                                                |
| `mechanical_check_unavailable`    | Required build, artifact, hardware, fixture, or execution evidence is absent.                                                    | Stop at the prior rung as `not_proven`.                                      |
| `budget_exhausted` or `cancelled` | The arm did not reach a terminal analytic result under its pre-registered cap.                                                   | Retain actual spend and right-censor boundary; no absence claim.             |

**Recommendation:** The scorecard should separate these outcomes from an
eligible `MISS`. A miss is meaningful only when the source bundle was complete,
the arm was accepted, the model actually ran under the frozen conditions, and
the evaluation population says that the required signal was available. Even
then, it establishes a detection miss for that exact arm—not absence of a
vulnerability in the target.

**Recommendation:** A disagreement ticket should bind the diverging model-arm
digests, exact disputed proposition, shared evidence, missing evidence, and the
next deterministic or executed check. For example, a disagreement over whether
a source tree can select a fallback provider should trigger preprocessing,
symbol-provider, link-map, and fault-build checks; it should not be resolved by
asking another model to vote.

## Reproducibility and independent verification gates

### Preconditions

**Recommendation:** A qualified panel comparison should begin only after all of
the following are frozen:

1. complete target and dependency manifest, or an explicit incomplete arm;
2. vulnerable, fixed, structural-variant, and clean-control populations;
3. model roster, roles, fallbacks, prompts, tool policy, schemas, budgets, and
   metric/evaluator revisions;
4. per-arm isolated writable state and separate provider sessions; and
5. scoring rules, sample size, censoring, and eligibility definitions.

The Coldcard arms must retain their existing meaning. In particular, a model
cannot claim a qualified hit by naming only a macro, path, or published
incident; it needs the six-link causal account and exact source references.
Missing dependency material blocks a comprehensive claim before inference.

### Required evidence after discovery

**Recommendation:** The panel can advance no farther than a typed source
finding or hypothesis until a distinct verifier and reproducible evidence
support the next rung. For the known Coldcard benchmark, confirmation requires
the source-reference and macro/symbol receipts already named in the runbook,
plus an observed failure on the pinned vulnerable target and success on the
pinned fixed target. Artifact-reality claims additionally require the exact
toolchain, preprocessing, object/symbol provider, link-map, firmware digest,
and a fault build.

The generator, owned-fixture, historical-fingerprint, and evidence-graph lanes
remain separate. A model's explanation of the source path does not establish a
recoverable secret, a third-party wallet, a chain actor, unauthorized movement,
or an identity.

### Holdouts and fixed scoring gates

**Recommendation:** Coldcard and its visible variants should remain development
data. Model selection, prompt editing, fallback tuning, and panel composition
may use development results but must not inspect evaluator-only vulnerable or
clean holdouts. Every compared candidate must use the same untouched holdout,
the same evaluator revision, and frozen score definitions.

Panel performance should be reported per family and as a matrix, not as a
single pooled finding count. Required columns are:

- complete-vulnerable and structural-variant qualified hit rate;
- fixed-control regression rate and clean-control false-positive rate;
- causal-link coverage and evidence-reference validity;
- verifier and mechanical-check completion rate;
- panel coverage, abstention, refusal, filter, outage, unsupported-input, and
  disagreement counts;
- time, token, and cost measures with exactness; and
- eligible misses, cancellations, and right-censored observations.

**Recommendation:** Promotion should stay lexicographic. First pass all
completeness, control, evidence, budget, cleanup, and independence gates; then
improve qualified holdout detection and causal coverage; only after that compare
time, token, or cost. A candidate that wins by omitting a model class, treating
a filtered run as a miss, hiding disagreement, or weakening a control is not an
improvement.

## Provenance, claim separation, and publication gates

### Redacted provenance

**Recommendation:** Retain two linked records rather than publishing raw model
or external material:

- a private evidence record with immutable inputs, raw model events where
  authorized, full receipts, and access-controlled reviewer metadata; and
- a public-safe provenance projection containing content digests, model-family
  labels, role, arm, timestamps, limitation class, score/evaluator revision,
  and non-sensitive aggregate outcomes.

The public-safe projection must exclude raw prompts, hidden reasoning, repository
paths when private, provider credentials, private source bytes, secret-bearing
images, mnemonics, xprvs, node cookies, wallet data, and the full content of
user-supplied screenshots or posts. A redaction decision should itself have a
reason and digest so that public provenance does not silently sever the audit
trail.

### Claim separation

**Recommendation:** Keep the following statements in separate records and do
not let any one of them promote another:

| Claim                           | Minimum basis                                                                  | It does not establish                                  |
| ------------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------ |
| External observation exists     | User-supplied post/screenshot record and redacted provenance.                  | That its content is accurate or complete.              |
| Model produced a candidate      | Typed finding or hypothesis with model-arm provenance.                         | That the candidate is true.                            |
| Source flaw                     | Complete source bundle, exact citations, and the six-link rubric.              | Final linked artifact or practical exploitability.     |
| Artifact reality                | Reproducible build, provider/link receipts, artifact digest, and fault result. | Third-party recovery or on-chain attribution.          |
| Generator behavior              | Independent vectors and trace receipts.                                        | Any real device's complete state space.                |
| Owned-fixture recovery          | Authorization, isolated fixture, and deterministic reproduction.               | Authority to touch live value or third-party material. |
| Program fingerprint or grouping | Frozen chain inputs, controls, base rates, and deterministic derivation.       | Vendor, person, intent, theft, or identity.            |
| Publication                     | Separate authority, review, disclosure scope, and safe projection.             | That all downstream claims are established.            |

### Publication decision

**Recommendation:** No model-panel result should be publishable merely because
the panel agrees, because a supplied post is compelling, or because a model
declines to find a problem. A future publication gate should require all of the
following:

1. a bounded proposition at a named claim rung;
2. reproducible, independently reviewed evidence appropriate to that rung;
3. completed fixed and clean controls, recorded limitations, and an honest
   description of panel coverage and disagreement;
4. a redaction review and a public-safe provenance projection;
5. applicable maintainer/disclosure, embargo, and human-review decisions; and
6. separate authority for the public claim.

Until that gate passes, the result stays private and is described only as a
model observation, a hypothesis, a source finding, or `not_proven`, whichever
the evidence supports.

## Recommended next planning artifact

**Recommendation:** Before any panel run, create one frozen evaluation packet
that contains the roster, fallback table, source/holdout split identifiers,
scoring rubric, panel-coverage denominator, typed-limitation taxonomy, redaction
rules, verifier assignment, and publication disposition. The packet should be
reviewed independently of the people selecting prompts or models.

That packet would make the requested model diversity testable: it could show
which families see different causal paths, which limitations prevented a fair
comparison, and whether a mechanically verified result improves without
overstating what the workbench knows. It would not itself authorize a run or
publication.

## Short-term Comet workbench acceptance

The owner-directed
[entropy-first dashboard](../loupe/2026-08-02-entropy-first-comet-dashboard-roadmap.md)
precedes this broader evidence interface. It first proves editable prompts,
repository traversal, live file progress, source observations, and immutable
rerun comparison without a model panel or publication workflow.

After that slice, the broader presentation should be a read-only case reader
inside the simplified Comet-shaped Omega shell. It should use Omega projections and
intents; it must not create a second forensic authority or copy evidence into a
presentation-owned store. The first vertical slice should contain:

1. a sticky case header with target, commit, benchmark arm, source completeness,
   privacy boundary, proof rung, and terminal run state;
2. separate **Evidence**, **Claims**, **Limitations**, and **Publication**
   queues, with counts that never merge hypotheses, failures, and verified
   evidence;
3. a claim inspector that shows the exact proposition, provenance, supporting
   and disputing evidence, missing rung, non-implications, and next mechanical
   check;
4. a model-panel matrix that shows family, role, input eligibility, outcome,
   and limitation type without a majority-vote verdict;
5. a persistent limitations strip that makes `input_incomplete`,
   `request_schema_invalid`, `tool_contract_incompatible`, unavailable usage,
   and missing cleanup truth impossible to overlook; and
6. a publication panel that is visibly blocked until evidence, redaction,
   independent review, disclosure scope, and separate publication authority
   all exist.

This reader can use the checked-in Coldcard fixture immediately as development
evidence. Live **Prepare**, **Launch**, **Cancel**, and cleanup controls remain
disabled or explicitly unavailable until the accepted runtime and source-
delivery evidence represented by OpenAgents issues #9289 and #9290 exists.
Issue #9300 remains open and is the authoritative warning against describing
the complete live program as accepted.

## Sources consulted

- [Omega Coldcard forensic practice-run runbook](2026-08-01-omega-coldcard-forensic-practice-runbook.md)
- [Wallet-security posts and Omega-thread audit](2026-08-02-wallet-security-posts-and-omega-thread-audit.md)
- [Bitcoin-node forensic capability](2026-08-01-bitcoin-node-forensic-capability.md)
- [Kelbie independent postmortem analysis](2026-08-01-kelbie-independent-postmortem-analysis.md)
- [Project Loupe reference study](../loupe/README.md)
- [Coldcard experiment pre-registration](../loupe/2026-08-01-coldcard-prefix-experiment.md)
- [Coldcard experiment results](../loupe/2026-08-01-coldcard-prefix-experiment-results.md)
- [Omega forensic analysis roadmap](../loupe/2026-08-01-omega-forensic-analysis-roadmap.md)
- [Omega forensic implementation and operator guide](../loupe/2026-08-01-omega-forensics-implementation-and-operator-guide.md)
- [Forensic prompt optimization governance](../loupe/2026-08-01-forensic-prompt-optimization-governance.md)
- [Codex analysis](../loupe/2026-08-01-codex-analysis.md)
- [Coordination-not-scanners analysis](../loupe/2026-08-01-coordination-not-scanners.md)
- [Hardening against AI-assisted attacks](../loupe/2026-08-01-hardening-against-ai-assisted-attacks.md)
- User-supplied X posts and screenshots (external observations; not independently verified in this audit)
