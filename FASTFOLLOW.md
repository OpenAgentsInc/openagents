---
fast_follow_spec_format_version: "0.1"
fast_follow_spec_id: "openagents.fast_follow"
fast_follow_revision: 1
title: "OpenAgents Product and Agent-Runtime Fast Follow"
artifact_type: "learning_intent"
lifecycle_state: "admitted"
author: "OpenAgents"
linked_target_repo: "OpenAgentsInc/openagents"
created_at: "2026-07-16T00:00:00Z"
updated_at: "2026-07-16T00:00:00Z"
---

# OpenAgents Fast Follow

## Objective

Continuously learn from the strongest adjacent coding agents, workrooms,
runtime protocols, orchestration systems, capability platforms, and release
systems while preserving OpenAgents' stricter Effect, authority, privacy,
receipt, portability, and verification contracts.

This spec turns the existing teardown library into stable source lessons and
many-to-many learning directives. It does not instruct OpenAgents to clone a
competitor, chase surface parity, or weaken an invariant. The target outcome
is a better OpenAgents system whose adaptations are explicit, target-native,
testable, and evidence-backed.

## Target

```fastfollow-target
{
  "id": "openagents",
  "repository": "OpenAgentsInc/openagents",
  "root": ".",
  "agent_instructions": [
    "AGENTS.md"
  ],
  "invariants": [
    "INVARIANTS.md",
    "apps/openagents.com/INVARIANTS.md",
    "docs/cloud/INVARIANTS.md"
  ],
  "product_specs": [
    "specs/CONVENTIONS.md",
    "specs",
    "docs/mvp"
  ],
  "assurance_specs": [
    "docs/assurance/ASSURANCE_SPEC.md",
    "packages/assurance-spec",
    "specs",
    "docs/mvp"
  ],
  "roadmap_authorities": [
    "docs/sol/MASTER_ROADMAP.md",
    "docs/sol/README.md"
  ],
  "artifact_paths": {
    "studies": "docs/fastfollow/studies",
    "gaps": "docs/fastfollow/gaps",
    "candidates": "docs/fastfollow/candidates",
    "receipts": "docs/fastfollow/receipts"
  }
}
```

## Sources

```fastfollow-sources
[
  {
    "id": "openai.chatgpt_desktop",
    "title": "ChatGPT and Codex Desktop",
    "role": "upstream",
    "access": "installed_artifact",
    "canonical_ref": "installed-artifact://ChatGPT.app",
    "tracking_policy": "release_or_commit",
    "teardown_refs": [
      "docs/teardowns/2026-07-10-chatgpt-desktop-app-teardown.md"
    ],
    "lessons": [
      {
        "id": "host_engine_split",
        "kind": "architecture",
        "summary": "Keep the privileged desktop shell separate from the open local agent engine and its generated protocol.",
        "stance": "adapt_with_stronger_boundaries"
      },
      {
        "id": "plugin_skill_app_layers",
        "kind": "extension",
        "summary": "Treat plugins, skills, MCP Apps, and computer use as distinct governed extension layers.",
        "stance": "study"
      },
      {
        "id": "ambient_memory_default",
        "kind": "security",
        "summary": "Reject ambient screen capture or inferred personal memory as a default capability.",
        "stance": "reject"
      }
    ]
  },
  {
    "id": "openai.codex",
    "title": "Codex Agent Runtime",
    "role": "upstream",
    "access": "public_source",
    "canonical_ref": "https://github.com/openai/codex",
    "tracking_policy": "pinned_each_run",
    "teardown_refs": [
      "docs/teardowns/2026-07-10-codex-agent-runtime-teardown.md",
      "docs/teardowns/2026-07-10-codex-subagents-rendering-analysis.md",
      "docs/teardowns/2026-07-15-codex-app-server-client-support-analysis.md"
    ],
    "lessons": [
      {
        "id": "thread_turn_item",
        "kind": "protocol",
        "summary": "Use a stable Thread, Turn, and Item semantic model with one generated app-server contract.",
        "stance": "adapt"
      },
      {
        "id": "lossless_native_plane",
        "kind": "protocol",
        "summary": "Retain a lossless provider-native event plane beside portable product projections.",
        "stance": "adapt"
      },
      {
        "id": "explicit_agent_graph",
        "kind": "product_ux",
        "summary": "Persist the complete child-agent graph and render both navigable topology and causal inline activity.",
        "stance": "adapt"
      },
      {
        "id": "permission_containment_split",
        "kind": "security",
        "summary": "Keep approvals, policy, OS enforcement, egress, and containment as separately observable facts.",
        "stance": "adapt_with_stronger_boundaries"
      }
    ]
  },
  {
    "id": "anthropic.claude_desktop",
    "title": "Claude Desktop",
    "role": "upstream",
    "access": "installed_artifact",
    "canonical_ref": "installed-artifact://Claude.app",
    "tracking_policy": "release_or_commit",
    "teardown_refs": [
      "docs/teardowns/2026-07-10-claude-desktop-app-teardown.md"
    ],
    "lessons": [
      {
        "id": "stock_electron_host",
        "kind": "architecture",
        "summary": "Keep stock Electron as a narrow orchestration host with a local renderer and explicit native bridges.",
        "stance": "adapt"
      },
      {
        "id": "component_update_planes",
        "kind": "release",
        "summary": "Version and verify the shell, engine, and isolated guest as independently moving signed components.",
        "stance": "adapt_with_stronger_boundaries"
      },
      {
        "id": "cowork_isolation",
        "kind": "security",
        "summary": "Separate host authority from hardware-isolated guest execution instead of treating prompts as containment.",
        "stance": "study"
      }
    ]
  },
  {
    "id": "anthropic.claude_code",
    "title": "Claude Code",
    "role": "upstream",
    "access": "mixed",
    "canonical_ref": "public-artifact://claude-code",
    "tracking_policy": "release_or_commit",
    "teardown_refs": [
      "docs/teardowns/2026-07-10-claude-code-teardown.md",
      "docs/teardowns/2026-07-10-claude-subagents-rendering-analysis.md"
    ],
    "lessons": [
      {
        "id": "bidirectional_control_stream",
        "kind": "protocol",
        "summary": "Expose one bidirectional engine control stream with explicit task, integration, question, and lifecycle states.",
        "stance": "adapt"
      },
      {
        "id": "durable_sidechains",
        "kind": "reliability",
        "summary": "Preserve independent child histories and reconstruct topology without treating provider message links as authority.",
        "stance": "adapt_with_stronger_boundaries"
      },
      {
        "id": "checkpoint_rewind_worktree",
        "kind": "reliability",
        "summary": "Make checkpoint, rewind, durable output, and outcome-sensitive worktree lifecycle first-class.",
        "stance": "study"
      }
    ]
  },
  {
    "id": "anomalyco.opencode",
    "title": "OpenCode",
    "role": "upstream",
    "access": "public_source",
    "canonical_ref": "https://github.com/anomalyco/opencode",
    "tracking_policy": "pinned_each_run",
    "teardown_refs": [
      "docs/teardowns/2026-07-10-opencode-desktop-app-teardown.md",
      "docs/teardowns/2026-07-10-opencode-effect-architecture-teardown.md",
      "docs/teardowns/2026-07-10-opencode-v2-architecture-teardown.md",
      "docs/teardowns/2026-07-15-codex-app-server-client-support-analysis.md",
      "docs/teardowns/2026-07-16-t3-code-opencode-electron-build-update-analysis.md"
    ],
    "lessons": [
      {
        "id": "effect_scope_topology",
        "kind": "architecture",
        "summary": "Model global, process, WorkContext, run, request, and foreign-host scope explicitly in the Effect service graph.",
        "stance": "adapt"
      },
      {
        "id": "durable_admission_execution",
        "kind": "reliability",
        "summary": "Separate durable admission, process-local execution, replay, current projections, and volatile events.",
        "stance": "adapt"
      },
      {
        "id": "one_http_contract",
        "kind": "protocol",
        "summary": "Use one typed request processor and generated contract across embedded, network, and test transports.",
        "stance": "adapt"
      },
      {
        "id": "server_owned_workbench",
        "kind": "product_ux",
        "summary": "Keep files, Git, PTY, providers, tools, and persistence server-owned behind a sandboxed renderer.",
        "stance": "adapt_with_stronger_boundaries"
      },
      {
        "id": "six_target_release",
        "kind": "release",
        "summary": "Support a complete macOS, Windows, and Linux x64 and arm64 release matrix with target-specific staging.",
        "stance": "adapt_with_stronger_boundaries"
      }
    ]
  },
  {
    "id": "cursor.cursor",
    "title": "Cursor",
    "role": "upstream",
    "access": "installed_artifact",
    "canonical_ref": "installed-artifact://Cursor.app",
    "tracking_policy": "release_or_commit",
    "teardown_refs": [
      "docs/teardowns/2026-07-11-cursor-product-teardown.md"
    ],
    "lessons": [
      {
        "id": "startup_restore_oracle",
        "kind": "evaluation",
        "summary": "Treat startup and restoration predictability as a standing measured behavior contract.",
        "stance": "adapt"
      },
      {
        "id": "best_of_n",
        "kind": "product_ux",
        "summary": "Express best-of-N as typed fleet fan-out, comparison, and owner disposition rather than hidden model magic.",
        "stance": "study"
      },
      {
        "id": "usage_model_truth",
        "kind": "economics",
        "summary": "Preserve exact model identity plus pre-spend and post-spend usage truth.",
        "stance": "adapt_with_stronger_boundaries"
      }
    ]
  },
  {
    "id": "executor",
    "title": "Executor",
    "role": "upstream",
    "access": "public_source",
    "canonical_ref": "teardown-source://executor",
    "tracking_policy": "pinned_each_run",
    "teardown_refs": [
      "docs/teardowns/2026-07-12-executor-architecture-teardown.md"
    ],
    "lessons": [
      {
        "id": "capability_artifact_compiler",
        "kind": "extension",
        "summary": "Compile authenticated operations into reproducible authored capability artifacts that re-enter the same catalog.",
        "stance": "adapt_with_stronger_boundaries"
      },
      {
        "id": "account_parametric_tools",
        "kind": "security",
        "summary": "Bind connections as invocation parameters and intersect artifact, caller, account, and runtime authority.",
        "stance": "adapt"
      },
      {
        "id": "semantic_catalog",
        "kind": "architecture",
        "summary": "Select capabilities semantically through typed catalog constraints rather than keyword search.",
        "stance": "adapt"
      }
    ]
  },
  {
    "id": "openchamber",
    "title": "OpenChamber",
    "role": "upstream",
    "access": "public_source",
    "canonical_ref": "teardown-source://openchamber",
    "tracking_policy": "pinned_each_run",
    "teardown_refs": [
      "docs/teardowns/2026-07-12-openchamber-product-teardown.md"
    ],
    "lessons": [
      {
        "id": "persistent_workroom",
        "kind": "product_ux",
        "summary": "Use a dense persistent workroom with timeline, review, blocker, file, terminal, and preview surfaces.",
        "stance": "adapt"
      },
      {
        "id": "durable_goals_schedules",
        "kind": "reliability",
        "summary": "Build goals and schedules on leases, fencing, recovery, and server-owned reconciliation rather than timers.",
        "stance": "adapt_with_stronger_boundaries"
      },
      {
        "id": "mobile_attention",
        "kind": "product_ux",
        "summary": "Treat mobile continuation, push, deep links, and attention as one supervision system.",
        "stance": "study"
      }
    ]
  },
  {
    "id": "openclaw.crabbox",
    "title": "Crabbox",
    "role": "upstream",
    "access": "public_source",
    "canonical_ref": "teardown-source://crabbox",
    "tracking_policy": "pinned_each_run",
    "teardown_refs": [
      "docs/teardowns/2026-07-13-crabbox-teardown.md"
    ],
    "lessons": [
      {
        "id": "lease_state_machine",
        "kind": "reliability",
        "summary": "Make leases, cost reservation, heartbeat, idle, expiry, cleanup, and run handles honest first-class state.",
        "stance": "adapt"
      },
      {
        "id": "control_data_split",
        "kind": "architecture",
        "summary": "Let a coordinator hold authority without becoming an unnecessary workload data plane.",
        "stance": "study"
      },
      {
        "id": "failure_capsule_receipt",
        "kind": "evaluation",
        "summary": "Produce portable failure capsules and countersigned receipts with explicit provenance.",
        "stance": "adapt_with_stronger_boundaries"
      }
    ]
  },
  {
    "id": "pingdotgg.t3code",
    "title": "T3 Code",
    "role": "upstream",
    "access": "public_source",
    "canonical_ref": "https://github.com/pingdotgg/t3code",
    "tracking_policy": "pinned_each_run",
    "teardown_refs": [
      "docs/teardowns/2026-07-13-t3-code-teardown.md",
      "docs/teardowns/2026-07-15-codex-app-server-client-support-analysis.md",
      "docs/teardowns/2026-07-15-t3-code-openagents-desktop-full-gap-analysis.md",
      "docs/teardowns/2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md",
      "docs/teardowns/2026-07-16-t3-code-opencode-electron-build-update-analysis.md"
    ],
    "lessons": [
      {
        "id": "provider_neutral_cqrs",
        "kind": "architecture",
        "summary": "Wrap distinct agent harnesses behind versioned provider-neutral events and an event-sourced Effect core.",
        "stance": "adapt_with_stronger_boundaries"
      },
      {
        "id": "effect_projection_kernel",
        "kind": "architecture",
        "summary": "Share an Effect projection and intent kernel across web, desktop, and native renderers without duplicating authority.",
        "stance": "adapt"
      },
      {
        "id": "worktree_parallelism",
        "kind": "product_ux",
        "summary": "Make worktree-parallel threads and checkpoint flows legible in the supervision surface.",
        "stance": "adapt_with_stronger_boundaries"
      },
      {
        "id": "acp_peer_profiles",
        "kind": "protocol",
        "summary": "Use one ACP foundation with provider-specific peer profiles, version negotiation, and capability enforcement.",
        "stance": "adapt"
      },
      {
        "id": "dpop_environment_access",
        "kind": "security",
        "summary": "Bind local and remote environment access to proof-of-possession credentials and explicit endpoints.",
        "stance": "study"
      }
    ]
  },
  {
    "id": "xai.grok_build",
    "title": "Grok Build",
    "role": "upstream",
    "access": "mixed",
    "canonical_ref": "public-artifact://grok-build",
    "tracking_policy": "release_or_commit",
    "teardown_refs": [
      "docs/teardowns/2026-07-15-grok-build-teardown.md",
      "docs/teardowns/2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md"
    ],
    "lessons": [
      {
        "id": "session_actor",
        "kind": "architecture",
        "summary": "Use one actor to own session activity, prompt delivery, queued follow-ups, steering, and cancellation.",
        "stance": "adapt"
      },
      {
        "id": "acp_edge_adapter",
        "kind": "protocol",
        "summary": "Treat ACP as a managed edge adapter over the shared domain instead of a second product model.",
        "stance": "adapt"
      },
      {
        "id": "terminal_proof",
        "kind": "evaluation",
        "summary": "Require real PTY, signal, resize, paste, fuzz, race, and performance evidence for terminal claims.",
        "stance": "adapt"
      }
    ]
  },
  {
    "id": "amp.code",
    "title": "Amp Code",
    "role": "upstream",
    "access": "mixed",
    "canonical_ref": "public-artifact://amp-code",
    "tracking_policy": "release_or_commit",
    "teardown_refs": [
      "docs/teardowns/2026-07-16-amp-code-teardown.md"
    ],
    "lessons": [
      {
        "id": "thread_as_product",
        "kind": "product_ux",
        "summary": "Treat the durable thread and exact work history as the canonical portable user object.",
        "stance": "adapt_with_stronger_boundaries"
      },
      {
        "id": "specialist_composition",
        "kind": "architecture",
        "summary": "Make specialist model diversity and review fan-out explicit, typed, and receipted.",
        "stance": "study"
      },
      {
        "id": "broad_extension_grammar",
        "kind": "extension",
        "summary": "Support tools, skills, MCP, plugins, agents, and modes under one non-amplifying capability law.",
        "stance": "adapt_with_stronger_boundaries"
      }
    ]
  },
  {
    "id": "command.code",
    "title": "Command Code",
    "role": "upstream",
    "access": "mixed",
    "canonical_ref": "public-artifact://command-code",
    "tracking_policy": "release_or_commit",
    "teardown_refs": [
      "docs/teardowns/2026-07-16-command-code-teardown.md"
    ],
    "lessons": [
      {
        "id": "governed_corrections",
        "kind": "product_ux",
        "summary": "Turn corrections into a governed observation, candidate, reviewed generation, application, outcome, and disposition lifecycle.",
        "stance": "adapt_with_stronger_boundaries"
      },
      {
        "id": "memory_class_separation",
        "kind": "security",
        "summary": "Separate instructions, learned preferences, retrieved history, and presentation preferences with visible provenance.",
        "stance": "adapt"
      },
      {
        "id": "bounded_history_import",
        "kind": "security",
        "summary": "Use explicit previewable import jobs instead of ambient history surveillance.",
        "stance": "adapt"
      }
    ]
  },
  {
    "id": "factory.droid",
    "title": "Factory Desktop and Droid",
    "role": "upstream",
    "access": "mixed",
    "canonical_ref": "public-artifact://factory-droid",
    "tracking_policy": "release_or_commit",
    "teardown_refs": [
      "docs/teardowns/2026-07-16-factory-desktop-cli-teardown.md"
    ],
    "lessons": [
      {
        "id": "one_engine_many_clients",
        "kind": "architecture",
        "summary": "Use one long-lived authenticated engine for TUI, headless, Desktop, mobile, SDK, and remote clients.",
        "stance": "adapt"
      },
      {
        "id": "hierarchical_policy",
        "kind": "security",
        "summary": "Compile hierarchical deny-precedence policy that cannot be weakened by a child scope.",
        "stance": "adapt_with_stronger_boundaries"
      },
      {
        "id": "extension_supply_chain",
        "kind": "extension",
        "summary": "Treat plugins, skills, agents, hooks, and MCP servers as immutable generation-owned supply-chain inputs.",
        "stance": "study"
      },
      {
        "id": "closed_schema_telemetry",
        "kind": "security",
        "summary": "Test local, sync, model, and telemetry disclosures independently through a closed schema.",
        "stance": "adapt"
      }
    ]
  },
  {
    "id": "openagents.synthesis",
    "title": "OpenAgents Teardown Synthesis",
    "role": "local_synthesis",
    "access": "public_source",
    "canonical_ref": "repo://docs/teardowns",
    "tracking_policy": "pinned_each_run",
    "teardown_refs": [
      "docs/teardowns/2026-07-10-openagents-product-adaptation-analysis.md",
      "docs/teardowns/2026-07-10-openagents-subagents-design.md"
    ],
    "lessons": [
      {
        "id": "target_native_adaptation",
        "kind": "architecture",
        "summary": "Reconcile every source lesson with current OpenAgents authority and adapt it into target-native contracts instead of copying a product wholesale.",
        "stance": "adapt"
      },
      {
        "id": "complete_graph_causal_ui",
        "kind": "product_ux",
        "summary": "Keep the complete child roster and independent transcripts while linking exact causal activity in the parent timeline.",
        "stance": "adapt"
      }
    ]
  }
]
```

## Learning Directives

```fastfollow-directives
[
  {
    "id": "runtime_protocol_provider_lanes",
    "title": "One engine contract with lossless provider lanes",
    "priority": 100,
    "source_refs": [
      "openai.codex#thread_turn_item",
      "openai.codex#lossless_native_plane",
      "anthropic.claude_code#bidirectional_control_stream",
      "anomalyco.opencode#one_http_contract",
      "pingdotgg.t3code#provider_neutral_cqrs",
      "pingdotgg.t3code#acp_peer_profiles",
      "xai.grok_build#acp_edge_adapter",
      "factory.droid#one_engine_many_clients"
    ],
    "target_scopes": [
      "apps/openagents-desktop",
      "packages/agent-client-protocol",
      "packages/codex-app-server-protocol",
      "packages/provider-lane"
    ],
    "desired_outcome": "One generated, version-negotiated, provider-neutral product plane beside lossless native event planes, with exact lifecycle and capability accounting across local and ACP providers.",
    "work_products": [
      "study_packet",
      "gap_assessment",
      "candidate_issue",
      "assurance_delta",
      "implementation"
    ],
    "constraints": [
      "Do not normalize away provider facts required for supervision or recovery.",
      "Do not create a provider-specific second domain model.",
      "Do not claim support from decoded method counts alone."
    ],
    "acceptance_refs": [
      "FF-AC-02",
      "FF-AC-09"
    ]
  },
  {
    "id": "effect_runtime_topology",
    "title": "Explicit Effect service and scope topology",
    "priority": 95,
    "source_refs": [
      "anomalyco.opencode#effect_scope_topology",
      "anomalyco.opencode#durable_admission_execution",
      "pingdotgg.t3code#effect_projection_kernel",
      "openclaw.crabbox#control_data_split",
      "openagents.synthesis#target_native_adaptation"
    ],
    "target_scopes": [
      "apps/openagents-desktop",
      "packages",
      "crates"
    ],
    "desired_outcome": "Explicit process, WorkContext, run, request, generation, and foreign-host scopes with interruption, replacement, replay, and test topology represented as Effect data and services.",
    "work_products": [
      "study_packet",
      "gap_assessment",
      "candidate_issue",
      "product_spec_delta",
      "assurance_delta",
      "implementation"
    ],
    "constraints": [
      "Keep ManagedRuntime at true host edges.",
      "Do not reintroduce ambient workspace context or Promise-owned lifecycles.",
      "Preserve the repository Effect and Node runtime invariants."
    ],
    "acceptance_refs": [
      "FF-AC-03"
    ]
  },
  {
    "id": "subagent_supervision",
    "title": "Complete multi-agent topology and causal supervision",
    "priority": 92,
    "source_refs": [
      "openai.codex#explicit_agent_graph",
      "anthropic.claude_code#durable_sidechains",
      "pingdotgg.t3code#worktree_parallelism",
      "amp.code#specialist_composition",
      "openagents.synthesis#complete_graph_causal_ui"
    ],
    "target_scopes": [
      "apps/openagents-desktop",
      "apps/openagents-mobile",
      "packages/agent-runtime-schema",
      "packages/world-client"
    ],
    "desired_outcome": "Every client can navigate the complete durable agent graph, open independent child histories, and see exact causal child activity without flattening, duplicating, or leaking topology.",
    "work_products": [
      "study_packet",
      "gap_assessment",
      "candidate_issue",
      "assurance_delta",
      "implementation"
    ],
    "constraints": [
      "Provider history is an import source, never canonical target authority.",
      "Preserve uncertainty when a source lacks explicit parent edges.",
      "Keep provider-private raw histories out of Sync and public receipts."
    ],
    "acceptance_refs": [
      "FF-AC-02",
      "FF-AC-09"
    ]
  },
  {
    "id": "persistent_workroom_attention",
    "title": "Persistent workroom, files, tools, and attention",
    "priority": 88,
    "source_refs": [
      "anomalyco.opencode#server_owned_workbench",
      "openchamber#persistent_workroom",
      "openchamber#mobile_attention",
      "amp.code#thread_as_product",
      "factory.droid#one_engine_many_clients",
      "anthropic.claude_desktop#stock_electron_host"
    ],
    "target_scopes": [
      "apps/openagents-desktop",
      "apps/openagents-mobile",
      "packages/ui"
    ],
    "desired_outcome": "A dense, durable, local-first workroom shared across Desktop and mobile, with server-owned files, Git, terminal, preview, blockers, notifications, and exact thread history.",
    "work_products": [
      "study_packet",
      "gap_assessment",
      "candidate_issue",
      "product_spec_delta",
      "assurance_delta",
      "implementation"
    ],
    "constraints": [
      "Render projections; do not hand raw host authority to clients.",
      "A live stream is not persistence and a notification is not accepted state.",
      "Tap, click, keyboard, and voice must resolve to the same typed intent."
    ],
    "acceptance_refs": [
      "FF-AC-07"
    ]
  },
  {
    "id": "durable_autonomy",
    "title": "Durable goals, schedules, leases, and Full Auto",
    "priority": 85,
    "source_refs": [
      "openchamber#durable_goals_schedules",
      "openclaw.crabbox#lease_state_machine",
      "xai.grok_build#session_actor",
      "amp.code#thread_as_product",
      "anomalyco.opencode#durable_admission_execution",
      "cursor.cursor#startup_restore_oracle"
    ],
    "target_scopes": [
      "apps/openagents-desktop",
      "apps/pylon",
      "packages/pylon-core"
    ],
    "desired_outcome": "A restart-safe, claim-aware autonomy system whose goals, schedules, work selection, leases, retries, interruption, cost, and stop state are visible and recoverable.",
    "work_products": [
      "study_packet",
      "gap_assessment",
      "candidate_issue",
      "product_spec_delta",
      "assurance_delta",
      "implementation"
    ],
    "constraints": [
      "Do not treat an event-only goal record or timer as recovery.",
      "Do not add a parallel Fast Follow dispatch path.",
      "Preserve Full Auto workspace binding, provider profile, lease, failure, and stop invariants."
    ],
    "acceptance_refs": [
      "FF-AC-06",
      "FF-AC-07",
      "FF-AC-08"
    ]
  },
  {
    "id": "capability_corrections_memory",
    "title": "Corrections become governed reusable capabilities",
    "priority": 82,
    "source_refs": [
      "executor#capability_artifact_compiler",
      "executor#account_parametric_tools",
      "executor#semantic_catalog",
      "command.code#governed_corrections",
      "command.code#memory_class_separation",
      "command.code#bounded_history_import",
      "openai.chatgpt_desktop#plugin_skill_app_layers",
      "amp.code#broad_extension_grammar",
      "factory.droid#extension_supply_chain"
    ],
    "target_scopes": [
      "packages/probe",
      "packages/assurance-spec",
      "apps/openagents-desktop",
      "docs/khala"
    ],
    "desired_outcome": "An explicit correction or solved problem can become a provenance-bound candidate preference, behavior contract, test, skill, or authored capability without widening authority or silently mining private history.",
    "work_products": [
      "study_packet",
      "gap_assessment",
      "candidate_issue",
      "product_spec_delta",
      "assurance_delta",
      "implementation"
    ],
    "constraints": [
      "Observed preference is not an active instruction.",
      "Imports are bounded, previewable, deletable, and local-only by default.",
      "Capabilities use typed semantic selection and authority intersection, never keyword dispatch."
    ],
    "acceptance_refs": [
      "FF-AC-05",
      "FF-AC-10"
    ]
  },
  {
    "id": "authority_isolation_receipts",
    "title": "Authority, isolation, provenance, and receipts",
    "priority": 100,
    "source_refs": [
      "openai.codex#permission_containment_split",
      "anthropic.claude_desktop#cowork_isolation",
      "cursor.cursor#usage_model_truth",
      "executor#account_parametric_tools",
      "openclaw.crabbox#failure_capsule_receipt",
      "pingdotgg.t3code#dpop_environment_access",
      "factory.droid#hierarchical_policy",
      "factory.droid#closed_schema_telemetry",
      "openai.chatgpt_desktop#ambient_memory_default"
    ],
    "target_scopes": [
      "apps/openagents-desktop",
      "apps/pylon",
      "crates",
      "packages/environment-auth",
      "packages/assurance-spec"
    ],
    "desired_outcome": "Every Fast Follow adaptation preserves non-amplifying authority, separately observed containment, exact provenance, public-safe receipts, and explicit privacy and telemetry boundaries.",
    "work_products": [
      "study_packet",
      "gap_assessment",
      "candidate_issue",
      "assurance_delta",
      "implementation"
    ],
    "constraints": [
      "Never weaken target policy to match an upstream default.",
      "Never infer containment from permissions or a sandbox label.",
      "Never put private source, prompts, credentials, or raw command bodies in shared artifacts."
    ],
    "acceptance_refs": [
      "FF-AC-04",
      "FF-AC-09",
      "FF-AC-11"
    ]
  },
  {
    "id": "release_component_graph",
    "title": "Signed cross-platform component and update graph",
    "priority": 78,
    "source_refs": [
      "anomalyco.opencode#six_target_release",
      "anthropic.claude_desktop#component_update_planes",
      "factory.droid#extension_supply_chain"
    ],
    "target_scopes": [
      "apps/openagents-desktop",
      "apps/oa-updates",
      "docs/deploy"
    ],
    "desired_outcome": "One immutable signed release set spans six native targets and independently versioned engine, extension, and guest components with staging, publication barriers, compatibility, rollback, and receipts.",
    "work_products": [
      "study_packet",
      "gap_assessment",
      "candidate_issue",
      "assurance_delta",
      "implementation"
    ],
    "constraints": [
      "Retain OpenAgents signed-manifest and fail-closed rollback invariants.",
      "Do not use GitHub-hosted CI.",
      "A built artifact is not a published, installable, or accepted release."
    ],
    "acceptance_refs": [
      "FF-AC-09"
    ]
  },
  {
    "id": "evaluation_distillation",
    "title": "Research, QA, and failures distill into reusable proof",
    "priority": 80,
    "source_refs": [
      "openai.chatgpt_desktop#plugin_skill_app_layers",
      "cursor.cursor#startup_restore_oracle",
      "openclaw.crabbox#failure_capsule_receipt",
      "xai.grok_build#terminal_proof",
      "openagents.synthesis#target_native_adaptation"
    ],
    "target_scopes": [
      "apps/qa-runner",
      "packages/assurance-spec",
      "packages/probe",
      "docs/fastfollow"
    ],
    "desired_outcome": "Each material observation can become a source-grounded StudyPacket, falsifiable gap, typed proof-design delta, rerunnable regression, or retained rejection with cost and outcome evidence.",
    "work_products": [
      "study_packet",
      "gap_assessment",
      "candidate_issue",
      "assurance_delta",
      "implementation"
    ],
    "constraints": [
      "Keep source evidence, target fit, implementation, verification, and owner disposition as separate axes.",
      "A cached study is reusable evidence, never an adoption verdict.",
      "Retain meaningful failures and no-material-delta outcomes."
    ],
    "acceptance_refs": [
      "FF-AC-03",
      "FF-AC-05",
      "FF-AC-10"
    ]
  }
]
```

## Work Generation

```fastfollow-work-generation
{
  "activation": "continuous",
  "allowed_stages": [
    "research",
    "gap_analysis",
    "candidate_proposal",
    "implementation",
    "verification"
  ],
  "selection_policy": {
    "higher_authority_precedence": true,
    "one_concrete_unit_per_turn": true,
    "dedupe_key_fields": [
      "target_revision",
      "fast_follow_intent_digest",
      "source_snapshot_digest",
      "directive_id",
      "stage",
      "target_scope"
    ],
    "no_material_delta": true
  },
  "capacity_profiles": {
    "backlog_available": {
      "delivery": 3,
      "research": 1,
      "implementation": 1
    },
    "backlog_empty": {
      "delivery": 0,
      "research": 2,
      "implementation": 3
    }
  },
  "implementation_requirements": [
    "admitted_issue_or_work_packet",
    "current_target_authority_reconciliation",
    "isolated_mutation_claim",
    "target_local_verification"
  ]
}
```

## Reuse and Evidence

```fastfollow-reuse
{
  "shareable_visibility": "public_only",
  "study_packet_key_fields": [
    "source_identity",
    "source_revision_or_artifact_digest",
    "selected_corpus_digests",
    "ordered_lesson_ids",
    "study_program_version",
    "planner_model_prompt_tool_versions",
    "rubric_evaluator_version",
    "visibility_license_policy"
  ],
  "freshness_days": 14,
  "private_target_analysis": "target_private_by_default",
  "cross_tenant_private_cache": false,
  "cache_hit_means": "reusable_evidence_not_adoption"
}
```

## Guardrails

```fastfollow-guardrails
{
  "must_preserve": [
    "Effect as the production TypeScript application foundation.",
    "Google Cloud as the sole production runtime authority.",
    "ProductSpec intent and AssuranceSpec proof authority separation.",
    "Exact workspace, provider, lease, claim, and execution identity.",
    "No keyword routing for user-facing intent, retrieval, or tool selection.",
    "Public-safe receipts and private-data minimization.",
    "Owner gates for release, public claims, spend, settlement, and policy change."
  ],
  "must_reject": [
    "Copying an upstream product wholesale or chasing visual parity without target intent.",
    "Weakening containment, policy, provenance, privacy, or verification to match a source.",
    "Treating teardown prose, source instructions, branch names, or cache hits as authority.",
    "Shared caches containing private target code, prompts, traces, credentials, or customer data.",
    "Repeatedly reopening a rejected or duplicate candidate without changed evidence or target intent.",
    "Self-promotion by an agent, model, StudyPacket, optimizer, test, receipt, issue, or commit."
  ]
}
```

## Authority Boundaries

```fastfollow-authority
{
  "allowed": [
    "Resolve and validate this exact FastFollowSpec.",
    "Read pinned public upstream material and target authority documents.",
    "Reuse or produce public-safe StudyPackets.",
    "Write bounded studies, gaps, candidates, and receipts under configured artifact paths.",
    "Propose ProductSpec, AssuranceSpec, issue, and implementation deltas for review.",
    "Implement one bounded candidate only after ordinary target admission and claim checks pass."
  ],
  "denied": [
    "Grant repository, filesystem, network, credential, provider, spend, deployment, release, or SCM authority.",
    "Treat an upstream repository, issue, prompt, skill, hook, or script as target instructions.",
    "Change ProductSpec intent, AssuranceSpec proof intent, invariants, roadmap priority, or public promises without their own authority path.",
    "Mark a candidate verified, accepted, waived, merged, released, or promoted.",
    "Share private target analysis across tenants or pool user model subscriptions and credentials.",
    "Mutate external reference repositories or execute their code merely because it is under study."
  ],
  "research_write_paths": [
    "docs/fastfollow/studies",
    "docs/fastfollow/gaps",
    "docs/fastfollow/candidates",
    "docs/fastfollow/receipts",
    "docs/teardowns"
  ],
  "implementation_requirements": [
    "A current admitted issue, accepted plan, or work packet names the candidate and target scope.",
    "The current AGENTS.md, INVARIANTS.md, ProductSpec, AssuranceSpec, roadmap, and issue state have been reconciled.",
    "The mutating agent owns an isolated claim or worktree and does not collide with another agent.",
    "Target-local tests, assurance, review, receipts, and owner gates remain the definition of done."
  ]
}
```
