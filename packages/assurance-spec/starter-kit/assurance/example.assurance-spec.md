---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.starter.assurance.adoption"
assurance_revision: 1
title: "Starter Assurance Adoption AssuranceSpec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "Repository owner"
---

## Assurance Objective

This proposed AssuranceSpec creates exact criterion-to-obligation coverage without claiming that proof design, execution, evidence, admission, or release is complete.

## Subject

The proposal is bound to the exact ProductSpec bytes, revision, path, and stable criterion identifiers below.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "SK-AC-01"
    ],
    "document_digest": "sha256:61ba3a36686dcfc0562105ed999e04c1db3b525846780c4a5418f35471dbab43",
    "path": "docs/product-specs/example.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 1
  }
}
```

## Risk Model

No risk objects are inferred from ProductSpec prose. Reviewers must design the applicable risk model.

```assurancespec-risks
{
  "risks": [],
  "source_digest": "sha256:1084a9edb589ef44d32854a76620d88fe517c9cce4ceba15b537b96bf839849c",
  "source_snapshot": "The source ProductSpec contains no Risks section. Assurance risk modeling remains required."
}
```

## Assurance Scope

Every executable ProductSpec criterion is in assurance scope. No criterion is silently excluded or marked not applicable.

## Environments

Repository facts are proposal context only. No Environment Profile, adapter, capability, or permission is selected by inventory.

```assurancespec-environments
{
  "profiles": [],
  "repository_inventory": {
    "candidate_artifact_refs": [],
    "declared_scripts": [],
    "diagnostics": [
      "repository_not_supplied"
    ],
    "inventory_digest": "sha256:13cef510a746daf9c1d6b2766fef971b7f66c7392a70709fd61ccd271f1b02e4",
    "repository_label": "not-supplied",
    "state": "absent",
    "tracked_file_count": 0,
    "truncated": false
  }
}
```

## Obligations

Each criterion receives one incomplete proposed obligation. Missing proof-design fields project as needs_design and prevent admission or execution.

```assurancespec-obligations
[
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "SK-AC-01"
    ],
    "disposition": "required",
    "id": "AO-SK-AC-01-01",
    "source_claim_digest": "sha256:e6ddb15b248d1998759f58024df0a5673a2c253f096598a8ddbe6f87173b6095",
    "source_claim_snapshot": "The starter repository can validate its ProductSpec and\nAssuranceSpec, pin their exact identity, and report separate coverage ledgers.",
    "title": "Assure SK-AC-01"
  }
]
```

## Gates

No execution or release gates are inferred. Gate design remains blocked pending review.

```assurancespec-gates
[]
```

## Evidence Policy

Links are pointers, not verdicts. Missing or unreviewed evidence remains INCONCLUSIVE.

```assurancespec-evidence-policy
{
  "links_are_verdicts": false,
  "missing_evidence_verdict": "INCONCLUSIVE",
  "policy_state": "needs_design",
  "required_for_ready_obligation": [
    "oracle_observation",
    "falsifier_observation",
    "environment_binding",
    "independent_review"
  ]
}
```

## Authority Boundaries

This proposal cannot admit, execute, verify, waive, release, or change a public promise.

```assurancespec-authority
{
  "admitted_roles": [],
  "policy_state": "needs_design",
  "proposal_may_change_public_promises": false,
  "proposal_may_execute": false,
  "proposal_may_release": false,
  "proposal_may_self_admit": false,
  "proposal_may_verify": false,
  "release_roles": [],
  "verifier_roles": []
}
```
