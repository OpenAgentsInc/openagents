# **ARCHIVED** - Directives

> **ARCHIVED:** 2026-01-13. This document is no longer maintained.
>
> For current planning:
> - **Roadmap:** [ROADMAP.md](../../../ROADMAP.md)
> - **What's wired:** [SYNTHESIS_EXECUTION.md](../../../SYNTHESIS_EXECUTION.md)
> - **Terminology:** [GLOSSARY.md](../../../GLOSSARY.md)

---

Directives are high-level goals that set the direction for the project. They represent epics like "Implement 100% of Nostr Protocol" or "Add comprehensive test coverage".

## Format

Each directive is a Markdown file with YAML frontmatter:

```markdown
---
id: "d-001"
title: "Implement 100% of Nostr Protocol"
status: active  # active | paused | completed
priority: high  # urgent | high | medium | low
created: 2025-12-20
updated: 2025-12-20
---

## Goal

Fully implement the Nostr protocol in Rust for both client and relay functionality.

## Success Criteria

- [ ] All NIPs implemented in crates/nostr/core
- [ ] Relay passes all protocol tests
- [ ] Client can connect to public relays

## Notes

Additional context, links to specs, etc.
```

## Current Active Directives

| ID | Title | Focus Area |
|----|-------|------------|
| d-001 | Integrate Breez Spark SDK for Bitcoin Payments | Payments |
| d-002 | Implement 100% of Nostr Protocol | Protocol |
| d-003 | OpenAgents Wallet - Complete Identity & Payment Solution | Application |
| d-004 | Continual Constant Improvement of Autopilot | Meta/Infrastructure |
| d-005 | Build Nostr GitHub Alternative (GitAfter) | Agent Infrastructure |
| d-006 | Operationalize NIP-SA (Sovereign Agents Protocol) | Agent Infrastructure |
| d-007 | Native Rust FROSTR Implementation (Threshold Signatures) | Cryptography |
| d-008 | Unified Data/Compute/Skills Marketplace | Marketplace/Economy |
| d-009 | Autopilot GUI - Visual Agent Interface | Application/GUI |
| d-010 | Unify All Binaries into Single openagents Binary | Architecture/UX |
| d-011 | Comprehensive Storybook Coverage for All Rust Components | UI/Documentation |
| d-012 | No Stubs - Production-Ready Code Only | Code Quality/Policy |
| d-013 | Comprehensive Testing Framework | Testing/Quality |
| d-014 | Full End-to-End NIP-SA and Bifrost Integration Tests | Testing/Integration |
| d-015 | Comprehensive Marketplace and Agent Commerce E2E Tests | Testing/Commerce |
| d-016 | Measure Actions Per Minute (APM) | Metrics/Performance |
| d-017 | Integrate Agent Client Protocol (ACP) | Protocol/Integration |
| d-018 | Parallel Autopilot Container Isolation | Infrastructure/Scaling |
| d-019 | GPT-OSS Local Inference Integration | Local Models/Inference |
| d-020 | WGPUI Integration - GPU-Accelerated UI Components | UI/Performance |
| d-021 | OpenCode SDK Integration | Agent Infrastructure |
| d-022 | Agent Orchestration Framework | Agent Infrastructure |
| d-023 | WGPUI - GPU-Accelerated UI Framework | UI/Graphics |
| d-024 | Achieve 100% Arwes Parity in WGPUI | UI/Graphics |
| d-025 | All-In WGPUI - Delete Web Stack | UI/Architecture |
| d-026 | E2E Test Live Viewer for WGPUI | Testing/UI |
| d-027 | Autopilot Demo + Dogfooding Funnel | Launch/Revenue |

---

*[See git history for full directive descriptions]*
