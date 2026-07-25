import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "./contract"

/**
 * Omega Agent behavior contracts (OMEGA-AGENT-02, OpenAgentsInc/omega#76).
 *
 * Owner-stated UX expectations for the Omega front door. The Omega Agent
 * product contract is `specs/omega/omega-agent.product-spec.md` revision 1,
 * admitted by the owner on 2026-07-25.
 *
 * These are the first behavior contracts whose oracles live in a different
 * repository. Omega is a Zed fork at OpenAgentsInc/omega and its surfaces are
 * Rust and GPUI, so the oracles are `cargo test` sweeps there rather than bun
 * tests here, and they are recorded with `kind: "script"`. The coverage
 * checker skips that kind, which means this file's oracle refs are not
 * resolved against disk in this repository — the `verification` field of each
 * contract names the exact command and repository that does run them. Do not
 * read a green `pnpm run test:behavior-contracts` as evidence that the Omega
 * oracles passed.
 */
export const omegaAgentContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract binds where an Omega window with no restorable session lands, and whether a thread can be started before a project is opened. It grants no release, packaged-journey, or public-claim authority, and it makes no claim about rendered pixels: the oracles are source-level checks over the launch path, not a screenshot of the running application.",
      blockerRefs: [
        "github:OpenAgentsInc/omega#76",
      ],
      contractId: "omega_agent.chat_first_front_door.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "github:OpenAgentsInc/omega#76",
        "github:OpenAgentsInc/omega#74",
        "specs/omega/omega-agent.product-spec.md",
        "docs/omega/2026-07-25-omega-agent-roadmap.md",
        "docs/omega/2026-07-25-omega-full-auto-chat-fold.md",
        "omega:OMEGA_DELTAS.md#OMEGA-DELTA-0019",
        "omega:crates/omega_front_door/src/omega_front_door.rs",
        "omega:crates/omega_deltas/src/omega_deltas.rs",
      ],
      oracles: [
        {
          description:
            "OMEGA-DELTA-0019. `a_fresh_window_opens_on_the_agent` asserts crates/zed/src/main.rs no longer calls Editor::new_file, reaches AgentPanel::open_front_door from both no-restorable-session paths, and still leaves the launchpad startup behaviour alone. `launch_surface` in crates/omega_front_door states the same rule as a typed function so it can be tested without a window. Both were falsified by restoring Editor::new_file to one path, which fails the check.",
          id: "omega_agent.chat_first_front_door.launch_surface",
          kind: "script",
          mode: "unit",
          ref: "omega:cargo test -p omega_deltas -p omega_front_door",
        },
        {
          description:
            "Planned oracle for the second half of the statement, which is NOT delivered: typing immediately. AgentPanel::activate_new_thread returns early when no project is open, and the no-restorable-session path is by definition the no-project case, so a genuinely fresh install lands on the agent panel's Open Project / Clone Repository state rather than on a focused composer. Binding a thread to a project lazily on its first workspace-touching action is the remaining work on omega#76.",
          id: "omega_agent.chat_first_front_door.project_optional_composer",
          kind: "planned",
          mode: "unit",
          ref: "github:OpenAgentsInc/omega#76",
        },
      ],
      productArea: "Omega front door",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-25",
      },
      state: "pending",
      statement:
        "`cmd-shift-a` opens the main New Agent Thread screen, and the app defaults to showing that screen — welcome as new agent chat, standard chat input, typing immediately.",
      surface: "omega",
      verification:
        "Runs in the Omega repository (OpenAgentsInc/omega) as `cargo test -p omega_deltas -p omega_front_door`, not in this repository's sweep. The landing half holds: a window with nothing to restore reaches AgentPanel::open_front_door instead of Editor::new_file, and `cmd-shift-a` was already bound to agent::NewThread window-globally. This contract stays pending because the typing half does not hold with no project open, and it will move to enforced only when a thread can start project-free and bind on its first workspace-touching action.",
    },
    {
      authorityBoundary:
        "This contract binds the structural fold: that no Full Auto dock panel is registered, that the agent panel constructs and hosts the Full Auto surface, that both retired full_auto_panel actions are still answered, that every Full Auto control is mapped to a home after the fold, and that run authority stays reachable from exactly one click. It makes no claim about rendered pixels, layout quality, or a packaged release; rendered confirmation of the folded surface is an owner observation step, not one of these oracles.",
      blockerRefs: [],
      contractId: "omega_agent.full_auto_folded_into_chat.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "github:OpenAgentsInc/omega#76",
        "github:OpenAgentsInc/omega#74",
        "github:OpenAgentsInc/omega#26",
        "specs/omega/omega-agent.product-spec.md",
        "docs/omega/2026-07-25-omega-full-auto-chat-fold.md",
        "omega:OMEGA_DELTAS.md#OMEGA-DELTA-0020",
        "omega:crates/omega_front_door/src/omega_front_door.rs",
        "omega:crates/agent_ui/src/agent_panel.rs",
        "omega:crates/full_auto_ui/src/panel.rs",
      ],
      oracles: [
        {
          description:
            "OMEGA-DELTA-0020. `full_auto_is_folded_into_the_chat_panel` asserts crates/zed/src/zed.rs names no FullAutoPanel, that crates/agent_ui/src/agent_panel.rs constructs the surface, that it answers both full_auto_panel::OpenLauncher and full_auto_panel::ToggleFocus so no existing keybinding silently becomes a no-op, and that crates/full_auto_ui exports no panel init. Falsified by re-registering the dock panel and by renaming one action handler; each fails the check.",
          id: "omega_agent.full_auto_folded_into_chat.fold_structure",
          kind: "script",
          mode: "unit",
          ref: "omega:cargo test -p omega_deltas",
        },
        {
          description:
            "`every_full_auto_affordance_is_mapped` scans crates/full_auto_ui/src for GPUI element ids and fails if one has no row in FULL_AUTO_AFFORDANCES or if a row names a control that no longer exists. This is what makes the fold provably a move rather than a reduction: all seventeen controls are mapped. FOLD_COSTS records the two capabilities that are not controls and do not survive — independent dock placement, and reading a run's full detail beside a chat thread simultaneously. Falsified by renaming one element id, which fails in both directions at once.",
          id: "omega_agent.full_auto_folded_into_chat.affordance_ledger",
          kind: "script",
          mode: "unit",
          ref: "omega:cargo test -p omega_front_door",
        },
        {
          description:
            "Owner gate 8, restated for the fold: only an explicit human action may start Full Auto authority. `only_a_click_listener_starts_a_full_auto_run` scans every crate outside omega_effectd and asserts the supervisor start_run call exists in exactly one place, that the UI entry into it exists in exactly one place, and that the entry is the Start button's on_click listener. `origins_are_all_human_gestures` and `no_origin_starts_a_run_by_itself` in crates/omega_front_door hold the same law as a closed enum with no variant for a tool call, slash command, restored draft, agent turn, or composer mode flag. Falsified by adding a second start_run caller in the agent panel, which fails the check.",
          id: "omega_agent.full_auto_folded_into_chat.human_start_only",
          kind: "script",
          mode: "unit",
          ref: "omega:cargo test -p omega_deltas -p omega_front_door",
        },
      ],
      productArea: "Omega front door",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-25",
      },
      state: "enforced",
      statement:
        "I don't actually want a Full Auto panel, it should be folded into whatever the chat UI for Omega is - you can decide how to handle this.",
      surface: "omega",
      verification:
        "Runs in the Omega repository (OpenAgentsInc/omega) as `cargo test -p omega_deltas -p omega_front_door`, not in this repository's sweep. The Full Auto dock panel is retired and its launcher and run monitor render as a surface of agent_ui::AgentPanel, reached from the new-thread menu's existing Full Auto entry and from both retired actions. The fold was implemented by rehosting the same views under a new parent rather than by rewriting them, which is why the affordance ledger can prove no control was lost.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-25.1",
}
