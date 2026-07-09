---
spec_format_version: "0.1"
title: "openagents.com Sales Landing"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-08T00:00:00Z"
updated_at: "2026-07-08T00:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_epic: "8565"
  openagents_lane: "WEB-1 (MASTER_ROADMAP P1 Track A)"
  openagents_assurance_level: "hosted"
  openagents_note: "retro-spec written under #8593 to calibrate the format against in-flight work; MASTER_ROADMAP and #8565 remain the sequencing and work authorities"
---

## Problem

Prospects who reach openagents.com today land on a product-era surface that
does not sell: it does not state who the product is for, what it costs, what
is live right now, or how to start a purchase conversation. The owner's
directive is "sales sales sales" — inbound attention (launch posts, outbound
replies, assessment links) currently lands on a page that converts none of it,
so every marketing and outbound win leaks at the last step.

## Hypothesis

If the landing becomes a sales-focused site — live proof counters, credit-tier
pricing, and two direct calls to action (business intake and "Talk to Sarah")
— then a meaningfully larger share of visitors will start an intake or a Sarah
conversation, because the page finally gives a motivated visitor a next step
that matches buying intent instead of product archaeology.

## Scope

```productspec-scope
in:
  - launch-ui section kit ported to TanStack Start (hero, navbar, pricing, stats, logos, faq, cta, footer)
  - one Protoss-blue theme (next-themes/mode-toggle dropped)
  - stats section wired to the live public counters
  - pricing wired to the shipped credit tiers
  - CTAs to business intake and Talk to Sarah
  - production serving on our own cloud with root cutover and rollback notes
out:
  - product routes (absorbed route-by-route later per ONE-UI; the current worker keeps serving them)
  - the business dashboard shell (lands with P4)
  - new legacy Foldkit surfaces (banned by the standing ONE-UI decision)
cut:
  - light/dark mode toggle (one theme only, by owner mandate)
  - reusing the old landing copy verbatim (copy goes through the promise-registry gates instead)
```

## User Experience

The staged landing renders at the staging route before the root flip; the
landed React replica remains the visual baseline it is compared against. The
production surface is the openagents.com root after owner-approved cutover.

## Acceptance Criteria

- Every section renders from the ported kit on TanStack Start with the single
  Protoss-blue theme; no `next/*` imports remain.
- The stats section reads the live public counters — never hardcoded numbers
  (exact-only law; counters are projections of receipted rows).
- Pricing shows the shipped credit tiers; every CTA resolves: intake posts to
  the business pipeline, Talk to Sarah reaches the Sarah surface.
- All landing copy passes the promise-registry copy gates with owner sign-off
  recorded before the root flip; no claim appears that the registry cannot
  back.
- Root cutover has a rollback note proven by exercising the rollback path once
  on staging.
- The relevant Eval Suite fixtures for the landing surface are green in the
  normal sweep before and after the flip.

## Success Metrics

```productspec-success-metrics
- id: intake_start_rate
  metric: business_intake_submissions_per_100_unique_landing_visitors
  target: ">= 2"
  window: first 30 days after root cutover
  segment: non-bot unique visitors to the openagents.com root
  source: worker_analytics_and_business_pipeline_rows
- id: sarah_conversation_start_rate
  metric: sarah_conversations_started_from_landing_cta
  target: ">= 1 per 100 unique visitors"
  window: first 30 days after root cutover
  segment: non-bot unique visitors to the openagents.com root
  source: sarah_session_records_with_lg6_source_attribution
- id: outbound_report_click_conversion
  metric: outbound_report_clicks_reaching_intake_or_sarah
  target: ">= 10%"
  window: weekly, once OB-4 sends ramp
  segment: prospects arriving from agent-readiness report links
  source: lg6_attribution_chain
```

## Risks

- Copy gates are the schedule risk: no flip without owner sign-off, so drafting
  toward the gates must start early (owner gate below).
- The Effect Native rescope (§EN) makes the forward landing the first
  production Effect Native surface; regression risk is bounded by keeping the
  React replica as the visual baseline and comparing before the flip.

## Owner Gates

- Copy sign-off through the promise-registry gates before the root flip.
- The root-cutover go decision itself.
- Pricing presentation for any tier not already public.

## Receipts

- Copy-gate pass receipts per section; root-flip receipt with rollback note;
  staging rollback exercised once; Eval Suite green runs before/after flip;
  counter wiring verified against the public endpoints (never synthesized).

## Promise Links

- Landing claims must dereference to registry-backed promises only; the
  live-counter and pricing claims link to their existing registry entries.
  This spec's success metrics are conversion measures and feed no public
  claim directly.
