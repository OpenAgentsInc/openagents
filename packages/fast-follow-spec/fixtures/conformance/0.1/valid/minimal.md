---
fast_follow_spec_format_version: "0.1"
fast_follow_spec_id: "example.fast_follow"
fast_follow_revision: 1
title: "Minimal Fast Follow"
artifact_type: "learning_intent"
lifecycle_state: "proposed"
author: "Example"
linked_target_repo: "owner/repo"
created_at: "2026-07-17T00:00:00Z"
updated_at: "2026-07-17T00:00:00Z"
---

# Minimal Fast Follow

## Objective

Study one lesson without admitting implementation.

## Target

```fastfollow-target
{"id":"example","repository":"owner/repo","root":".","agent_instructions":["AGENTS.md"],"invariants":["INVARIANTS.md"],"product_specs":[],"assurance_specs":[],"roadmap_authorities":[],"artifact_paths":{"studies":"docs/studies","gaps":"docs/gaps","candidates":"docs/candidates","receipts":"docs/receipts"}}
```

## Sources

```fastfollow-sources
[{"id":"source","title":"Source","role":"upstream","access":"public_source","canonical_ref":"https://example.com/source","tracking_policy":"pinned_each_run","teardown_refs":[],"lessons":[{"id":"lesson","kind":"architecture","summary":"Study a bounded lesson.","stance":"study"}]}]
```

## Learning Directives

```fastfollow-directives
[{"id":"directive","title":"Directive","priority":1,"source_refs":["source#lesson"],"target_scopes":["src"],"desired_outcome":"Produce evidence.","work_products":["study_packet"],"constraints":[]}]
```

## Work Generation

```fastfollow-work-generation
{"activation":"manual","allowed_stages":["research"],"selection_policy":{"higher_authority_precedence":true,"one_concrete_unit_per_turn":true,"dedupe_key_fields":["directive"],"no_material_delta":true},"capacity_profiles":{"backlog_available":{"delivery":1,"research":0,"implementation":0},"backlog_empty":{"delivery":0,"research":1,"implementation":0}},"implementation_requirements":["admitted_issue_or_work_packet"]}
```

## Reuse and Evidence

```fastfollow-reuse
{"shareable_visibility":"public_only","study_packet_key_fields":["source_digest"],"freshness_days":30,"private_target_analysis":"target_private_by_default","cross_tenant_private_cache":false,"cache_hit_means":"reusable_evidence_not_adoption"}
```

## Guardrails

```fastfollow-guardrails
{"must_preserve":["target authority"],"must_reject":["source self-admission"]}
```

## Authority Boundaries

```fastfollow-authority
{"allowed":["research"],"denied":["deployment"],"research_write_paths":["docs/fastfollow"],"implementation_requirements":["separate target authority"]}
```
