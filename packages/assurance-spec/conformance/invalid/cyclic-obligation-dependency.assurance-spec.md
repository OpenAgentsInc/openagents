---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.conformance.cyclic.obligation.dependency"
assurance_revision: 1
title: "Conformance: cyclic obligation dependency"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "assurance-spec conformance corpus"
---

## Assurance Objective

This proposed AssuranceSpec creates exact criterion-to-obligation coverage without claiming that proof design, execution, evidence, admission, or release is complete.

## Subject

The proposal is bound to the exact ProductSpec bytes, revision, path, and stable criterion identifiers below.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "EX-AC-1",
      "EX-AC-2",
      "EX-AC-3"
    ],
    "document_digest": "sha256:ec24c68a355a5c5e411757d2142df4d0e19f9f358c8d4b1212734fc1b71d8d8c",
    "path": "docs/example.product-spec.md",
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
  "source_digest": "sha256:06344e0ce8ef2287e5b7994bf139aff9eb67ac092764f05d361263078bb61437",
  "source_snapshot": "The declared example behavior may not be present."
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
      "EX-AC-1"
    ],
    "dependency_refs": [
      "AO-EX-AC-2-01"
    ],
    "disposition": "required",
    "id": "AO-EX-AC-1-01",
    "source_claim_digest": "sha256:a7439c223154a48d321da0dd7b319be6cb12c0c1106a613e7c3fa7e2d7991f18",
    "source_claim_snapshot": "Example criterion 1 holds.",
    "title": "Assure EX-AC-1"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "EX-AC-2"
    ],
    "dependency_refs": [
      "AO-EX-AC-3-01"
    ],
    "disposition": "required",
    "id": "AO-EX-AC-2-01",
    "source_claim_digest": "sha256:98658ba76b3cc5513c1e75af19fe3c63fe702b26b20ac8bdd6375affbe8c30ca",
    "source_claim_snapshot": "Example criterion 2 holds.",
    "title": "Assure EX-AC-2"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "EX-AC-3"
    ],
    "dependency_refs": [
      "AO-EX-AC-1-01"
    ],
    "disposition": "required",
    "id": "AO-EX-AC-3-01",
    "source_claim_digest": "sha256:6a4797526fb26d2aa557030ce2d254f43408bbced3e598ddbecc5ee5064c4084",
    "source_claim_snapshot": "Example criterion 3 holds.",
    "title": "Assure EX-AC-3"
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
