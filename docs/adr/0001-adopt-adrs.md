# ADR 0001 — Adopt Architectural Decision Records (ADRs)

 - Date: 2025-11-01
 - Status: Accepted

## Context

As the project grows across a mobile app (Expo/React Native) and a Rust bridge (Tricoder), we make high‑impact choices that affect APIs, tooling, testing, and operations. Those decisions need to be:
- Easy to discover and cite in PRs and issues.
- Stable over time, with a clear history of alternatives and trade‑offs.
- Co‑located with the code, versioned, and reviewable.

The industry‑standard practice for this is Architectural Decision Records (ADRs). See https://adr.github.io/. Internal discussion in PR #1345 and comments converged on adopting ADRs to capture decisions like ADR‑0002 (Rust→TS types) and ADR‑0003 (Tinyvex local sync).

## Decision

Adopt ADRs as the authoritative mechanism to capture significant technical decisions. ADRs will live in `docs/adr/` and follow a lightweight, consistent template (Context → Decision → Rationale → Alternatives → Consequences → Acceptance → References). New ADRs are required for decisions with medium/high cross‑cutting impact and must be referenced by related PRs and issues.

## Rationale

- Improves alignment and onboarding; decisions are documented once and cited by PRs.
- Encourages deliberate design, alternatives, and explicit trade‑offs.
- Keeps design history close to code and discoverable in the repo.

## Scope

Write an ADR when a change:
- Modifies public contracts or data formats (WS payloads, Tinyvex schema, snake_case conventions).
- Alters testing architecture or release processes (e.g., Storybook, Maestro, CI lanes).
- Introduces/removes major dependencies or frameworks.
- Impacts security, performance characteristics, or ops defaults.

Minor refactors or isolated UI tweaks do not require an ADR.

## Process

- Location: `docs/adr/`.
- Numbering: zero‑padded sequential files (`0001-*.md`).
- Status values: `Proposed` → `Accepted` → `Superseded`/`Deprecated` (with links).
- Template:
  - Title/number/date/status
  - Context
  - Decision
  - Rationale
  - Alternatives considered
  - Consequences (pros/cons, risks, ops)
  - Acceptance criteria
  - References (PRs, issues, external links)
- Review: open a PR with the ADR; discussion happens in the PR. When merged, status is `Accepted` (or `Proposed` if explicitly left open for incubation).
- Traceability: reference ADR numbers in PR descriptions and code comments where appropriate.

## Backfilling

We will backfill ADRs for already‑made, high‑impact decisions as time permits (e.g., ADR‑0002, ADR‑0003, ADR‑0004, ADR‑0005). New decisions should include an ADR as part of their PR.

## Consequences

- Slight overhead when introducing major changes, offset by clarity and speed in later work.
- Clear expectations for documentation of cross‑cutting decisions.

## Acceptance

- This ADR is merged in `docs/adr/` and referenced by future PRs as appropriate.
- At least the next two high‑impact changes include ADRs at proposal time.

## References

- ADRs overview: https://adr.github.io/
- PR #1345 (internal discussion to adopt ADRs and formalize decision capture)
