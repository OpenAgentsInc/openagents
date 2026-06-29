# Architecture Decision Records

This directory records accepted OpenAgents architecture decisions as Markdown
Any Decision Records (MADR). ADRs explain decisions that are already made or
being made, the options considered, and the consequences that future work must
respect.

## Process

1. Copy `adr-template.md` to the next numbered file:
   `NNNN-short-title.md`.
2. Use a short lowercase dash-separated title after the number.
3. Set `status: "accepted"` for settled decisions. Use another MADR status only
   when replacing, rejecting, or deprecating a previous ADR.
4. Ground the context in repository sources such as `AGENTS.md`, `CLAUDE.md`,
   `INVARIANTS.md`, app-specific invariant ledgers, deployment runbooks, code,
   package manifests, and tests.
5. Keep each ADR focused on one architectural decision.
6. Update the index below in the same PR.

ADRs do not replace invariant ledgers, runbooks, tests, or product-promise
records. They provide a stable decision log that links those sources together.

## Index

* [ADR-0001: Record architecture decisions](0001-record-architecture-decisions.md)
* [ADR-0002: Adopt Effect as the core runtime model](0002-adopt-effect-as-the-core-runtime-model.md)
* [ADR-0003: Use Bun as the workspace runtime and toolchain](0003-use-bun-as-the-workspace-runtime-and-toolchain.md)
* [ADR-0004: Prefer Cloudflare-native product infrastructure](0004-prefer-cloudflare-native-product-infrastructure.md)
* [ADR-0005: Land through PR review and the `check:deploy` gate](0005-land-through-pr-review-and-check-deploy.md)
* [ADR-0006: Route Khala coding delegation through owner-local Pylon capacity](0006-route-khala-coding-delegation-through-owner-local-pylon-capacity.md)
* [ADR-0007: Gate public product claims through product promises](0007-gate-public-product-claims-through-product-promises.md)
* [ADR-0008: Declare public projection freshness at each surface](0008-declare-public-projection-freshness-at-each-surface.md)
* [ADR-0009: Use Spark as the primary agent payment rail](0009-use-spark-as-the-primary-agent-payment-rail.md)
* [ADR-0010: Store only public-safe ATIF trace projections](0010-store-only-public-safe-atif-trace-projections.md)
