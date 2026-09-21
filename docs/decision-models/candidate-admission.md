# Candidate admission

Candidate admission compares a frozen base and candidate before a tenant door
changes artifacts. `gym regress` retains its same-identity contract. Admission
uses the declared cross-artifact comparison and requires improvement on one
winning metric, passing guards, locked confirmation, transfer evidence, and
an affordable deployment.

## Freeze the comparison

Create a `gym::admission::Plan` with schema
`openagents.gym.admission_plan.v1`, then call `seal` before collecting selection
evidence. Retain the plan outside the executor's write grant. The plan pins:

- Base and candidate identities, including artifact and execution settings.
  `differences` lists the identity fields permitted to change.
- Development suite, question-set and wording digests, selection partitions,
  optional gate digest, and admitted family scope.
- Estimator, sample count, option order, and either `seed_base` or distinct
  `seed_blocks`. Every declared block must cover every selected item once on
  both sides. Missing rows and duplicate rows cannot establish coverage.
- Exactly one winning metric, its workload-specific variance, required effect
  size, and minimum block count. Synthetic test values are not launch criteria.
- Family, confident-error, refusal, and ECE/Brier/NLL calibration guards.
- A distinct transfer suite, its complete selection and question pins, and
  its own variance. Development variance cannot substitute for transfer data.
- Deployment measurement rules and workload-specific latency, cost, and
  refusal ceilings.

Use development data to select the frozen candidate. Calibration and admission
are not training. Training remains a separate process. Review-policy evaluation
and image feasibility require their own compatible evidence contracts.

## Retain evidence

Write native Gym result stores and retain their report commitments separately.
Admission verifies the receipt chain, the commitment digest, the complete row
count, the frozen selection and door names, and the exact evaluated rows.
An internally valid shortened chain cannot replace a retained whole-report
commitment. Appended rows require a new reviewed commitment.

Supply separate commitments for development, locked confirmation, and transfer.
The Rust API requires these through `Evidence.reports`; the CLI uses the same
verifier. Missing reports make admission unverifiable. An arbitrary digest
string does not authorize an artifact.

Spend locked evidence through `LockedLedger::read_locked` under
`plan.ledger_subject()` and the candidate adapter. Confirmation requires exactly
one original read. An override records exposure but cannot qualify admission.
After a search spends a locked set, a later search or version needs genuinely
unexposed confirmation data with a new suite digest and a new frozen plan.
Relabeling previously exposed items does not create independent evidence.

A retained commitment authenticates content relative to the copy the operator
trusts. It does not attest remote weights, truthful measurements, label quality,
or the independence of held-out data. Protect the ledger, plan, commitments, and
measurement provenance from the executor whose result is under evaluation.

## Evaluate

```sh
cargo run --locked -p gym --bin gym -- admit \
  --plan admission-plan.json --suite caller-suite.json \
  --store development.jsonl --commitment development-report.json \
  --locked confirmation.jsonl --ledger locked-exposure.jsonl \
  --locked-commitment confirmation-report.json \
  --transfer-suite transfer-suite.json --transfer-store transfer.jsonl \
  --transfer-commitment transfer-report.json \
  --deployment-evidence deployment.json --out admission-decision.json
```

`deployment.json` serializes `gym::gate::Deployment`: the frozen workload name
in `group`, and measured `baseline` and `candidate` profiles. Profiles report
call count, latency percentiles, refusal count, and explicit cost evidence.
The plan supplies the authoritative budget. Unknown cost is not zero or an
unmetered local lane. Without explicit deployment evidence, the CLI derives
complete timing observations from development rows and leaves cost unknown;
incomplete timing rows cannot establish a complete profile.

The decision records criteria, rulings, verified commitment references, scope,
and an `admission:` digest. Inspect the structured ruling. A winning metric
alone does not authorize activation; failed, refused, or unverifiable evidence
cannot activate.

## Activate or roll back

Use `tenancy::admission::Record::evaluate` with the frozen plan and complete
evidence, or `Record::verify` to replay a serialized decision against them.
Pass the resulting record to `Registry::activate`. Parsing a self-digested
success claim alone is not an activation path.

Activation requires an existing tenant and door whose current artifact still
matches the evaluated base. It preserves credentials, principals, quota, and
capacity, and writes the candidate identity, admitted scope, and promotion
reference into a new revision. A stale base refuses activation. Ordinary
registry installation and updates cannot introduce a trained binding by naming
a promotion digest.

`Registry::rollback` restores the prior binding as another revision. Earlier
revisions remain readable; rollback does not rewrite historical receipts.
Registry mutations serialize through the registry lock. Keep registry writes
under operator control.

## Verification scope

Synthetic tests cover a complete retained-evidence winner, an underpowered
comparison, ties and losses, missing coverage, repeated locked exposure,
transfer regressions, changed rows, missing commitments, exceeded deployment
cost, activation against a stale base, and rollback with retained history.
These tests establish mechanism behavior. They do not establish model quality,
production latency, deployment prices, or a particular candidate's eligibility.
