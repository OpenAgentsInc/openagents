# Engineering audits

Dated engineering audits and after-action records. These preserve findings,
remediation, and verification at their recorded revision. A historical finding
is not automatically a current defect; read its remediation and newer code.
Use the [master roadmap](../roadmap.md) for current work.

| Document | Recorded topic |
| --- | --- |
| [2026-10-22-owned-renderer-webgpu-and-effect.md](2026-10-22-owned-renderer-webgpu-and-effect.md) | Building our own editor and runtime: pinned three-effect source inspection, Effect boundaries, direct WebGPU versus Rust/wgpu, incremental game compilation, and measured performance experiments |
| [2026-10-22-browser-game-editor-comparison.md](2026-10-22-browser-game-editor-comparison.md) | PlayCanvas, Babylon.js, and Three.js for an OpenAgents-owned browser editor: service ownership, live iteration, Kiln integration, retained Rust, migration costs, and proposed evaluation |
| [2026-10-21-voice-infrastructure.md](2026-10-21-voice-infrastructure.md) | Voice infrastructure past and present: every voice system in this repository and other OpenAgentsInc repositories since 2024, a comparison table, what works today, leftover LiveKit infrastructure, lessons, and one Rust voice stack with draft issues V0 to V8 |
| [2026-10-20-gpui-everywhere.md](2026-10-20-gpui-everywhere.md) | GPUI everywhere on openagents.com: every route classified SIMPLE, APP, or RETIRE, a readiness audit with the feature registry that hides unready features (`OPENAGENTS_WEB_FEATURES`), the flip plan for #12226, deprecations, the deploy checklist, and the page-group issues (#12325 to #12337) |
| [../auth/2026-10-20-workos-source-of-truth.md](../auth/2026-10-20-workos-source-of-truth.md) | WorkOS as the one auth object: today's nine identity and session stores, one WorkOS User per person with Nostr keys linked in our table, whether a Nostr signature can start a WorkOS session (no direct API; a Magic Auth bridge or our own session as fallback), workos-rust, and phases W0 to W4 (#12320 to #12324) |
| [2026-10-20-openagents-com-unified-plan.md](2026-10-20-openagents-com-unified-plan.md) | One openagents.com: every transcript theme with episodes and code, earlier synthesis and its conflicts, a Nostr-first plan for profiles, agent identities, social agent use, the browser wallet and the route map, and epics U1 to U8 (#12254 to #12261) |
| [2026-10-20-desktop-web-parity.md](2026-10-20-desktop-web-parity.md) | Bringing the whole desktop app to the web: every desktop feature with its file, what the web has, how it works in a browser (the browser as a paired client of computers over `coder-sync`), backend needs, and size; phases 7 to 13 after the GPUI web chat plan, and what the web can never do (#12262 to #12294) |
| [2026-10-19-agentgraph-unit.md](2026-10-19-agentgraph-unit.md) | AgentGraph after Unit: Unit and its MIT license, our earlier Unit, Arbiter, and Blueprint Map audits, what AgentGraph meant in the transcripts, the spec and GPUI view behind `/agent/foreman`, and how to graph a new agent (#12229 to #12233) |
| [2026-10-19-gpui-web-chat.md](2026-10-19-gpui-web-chat.md) | Replacing the web app's chat surfaces with the desktop GPUI UI on the web: surface inventory, architecture from the desktop crates, measured wasm size and first paint, accessibility gaps, and phased rollout (#12216 to #12226) |
| [2026-10-19-rust-compile-times.md](2026-10-19-rust-compile-times.md) | Rust compile times: the edit loop, land-queue checks and deploy builds measured, incremental leaf crates and line tables (edits 81 to 82% faster), ranked options, and why not to rewrite |
| [2026-10-17-spark-wallet.md](2026-10-17-spark-wallet.md) | Spark wallet: what the Breez SDK offers at `fc7ae22`, what the phone, CLI, and x402 payer use, the ranked gaps for a full wallet, x402 fit, custody with one seed, and a phased plan |
| [2026-10-16-coder-desktop-monorepo-merge.md](2026-10-16-coder-desktop-monorepo-merge.md) | Merging the Coder Desktop (Zeron) fork into the monorepo as the OpenAgents desktop and mobile apps: size, conflicts, identity, plan |
| [2026-10-14-computers-for-chats-audit.md](2026-10-14-computers-for-chats-audit.md) | Computers for chats: your own computers, cloud computers, environments, GPU, Claude setup tokens and Code Graph, against "type in the web app, a computer starts, the work shows on the web" |
| [2026-10-12-claude-usage-burn/README.md](2026-10-12-claude-usage-burn/README.md) | After-action report: an unattended `claude bg` issue blitz drained the owner's subscription, spawned orphaned processes and cloud VMs, and how to gate, cap, and kill-switch future runs |
| [2026-10-10-self-improving-codebases-audit/README.md](2026-10-10-self-improving-codebases-audit/README.md) | Self-improving codebases as a product: execution, learning, experiments, operations, customer data, economics, all open issues, 32 proposed actions, and pinned evidence from the sales and prior audits |
| [2026-10-10-training-system-audit/README.md](2026-10-10-training-system-audit/README.md) | Training system audit: Psionic training code, Pylon, Tassadar/Psion/Percepta history, kitchen-sink promises, and a roadmap ([roadmap.md](2026-10-10-training-system-audit/roadmap.md)) for decision-model training in the Verse and V1 products |
| [2026-10-10-codebase-health-audit/README.md](2026-10-10-codebase-health-audit/README.md) | Codebase health audit across all 184 crates: 34 sections, 709 verified findings, scorecard, top actions, remediation roadmap, and [action register](2026-10-10-codebase-health-audit/action-register.md) |
| [2026-10-03-independent-efficiency/README.md](2026-10-03-independent-efficiency/README.md) | Independent cost and context audit: Claude/Codex comparisons, historical controls, System One preparation experiments, and a runnable issue briefing prototype |
| [2026-09-26-nostr-adoption/README.md](2026-09-26-nostr-adoption/README.md) | Nostr adoption across all 44 workspace packages: existing paths, missing integrations, pre-adoption blockers, and local boundaries |
| [2026-09-19-codebase-audit/README.md](2026-09-19-codebase-audit/README.md) | Workspace audit: September 19, 2026 |
| [2026-09-19-codebase-audit/issues.md](2026-09-19-codebase-audit/issues.md) | Open issue map |
| [2026-09-19-codebase-audit/remediation.md](2026-09-19-codebase-audit/remediation.md) | Remediation register |
| [2026-09-19-codebase-audit/verification.md](2026-09-19-codebase-audit/verification.md) | Verification record |
| [2026-09-20-delegation-after-action-report/README.md](2026-09-20-delegation-after-action-report/README.md) | After-action report: stalled issue burndown and missing task history |
| [2026-09-21-project-completion-failure/README.md](2026-09-21-project-completion-failure/README.md) | Audit of project completion failure |
| [2026-09-22-jev-opportunities/README.md](2026-09-22-jev-opportunities/README.md) | What cheaper semantic decisions could change in OpenAgents |
