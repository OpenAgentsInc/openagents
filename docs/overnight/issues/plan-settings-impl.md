e is Claude's plan:
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
 Implementation Plan for Issue 1462: Orchestration Aiming UI (Phase 3)

 Overview

 Enable users to configure overnight orchestration settings (goals, schedule, constraints, agent preferences) before bed, then wake up to a Morning
 Briefing showing results. This builds on the existing DecisionEngine, TaskQueue, and AgentCoordinator infrastructure (already implemented in commits
 b05dde8e and c98e762e).

 Implementation Phases (6-8 days, ~2000 lines)

 Phase 1: Data Model & Storage (1-2 days)

 Goal: Define OrchestrationConfig and persist in Tinyvex

 Files to create:
 - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationConfig.swift (~200 lines)
   - Define main config struct with sub-types (Schedule, Constraints, AgentPreferences, Focus, PRAutomation)
   - Add Codable, Sendable, Equatable conformance
   - Include validation helpers

 Files to modify:
 - ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift
   - Add orchestration_configs table creation in migrate()
   - Add CRUD methods: insertOrUpdateConfig, getConfig, listConfigs, deleteConfig

 Tests to create:
 - ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/OrchestrationConfigTests.swift (~150 lines)
   - Test all CRUD operations
   - Test Codable serialization
   - Test validation logic

 Phase 2: Bridge Protocol (1 day)

 Goal: Add JSON-RPC methods for config management

 Files to create:
 - ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+OrchestrationConfig.swift (~300 lines)
   - Implement orchestrate/config.get
   - Implement orchestrate/config.set with validation (cron, time windows, globs)
   - Implement orchestrate/config.list
   - Add validation helpers (cron parser, glob validator)

 Tests to create:
 - ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Bridge/OrchestrationConfigBridgeTests.swift (~200 lines)
   - Test all bridge methods
   - Test validation rules
   - Test error responses

 Phase 3: Backend Integration (2 days)

 Goal: Make DecisionEngine and AgentCoordinator honor config

 Files to modify:
 - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift
   - Add overload: decideNextTask(from insights, config: OrchestrationConfig)
   - Implement goal bias logic (boost confidence when goals align)
   - Filter agent selection by config.agentPreferences.allow
   - Override with config.agentPreferences.prefer when set
 - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/AgentCoordinator.swift
   - Add runCycle(config: OrchestrationConfig, workingDirectory: URL?)
   - Pass config to DecisionEngine
   - Store config.id in task.metadata["config_id"]
   - Use config.timeBudgetSec for timeout calculation

 Tests to add:
 - DecisionEngineTests.swift - Add 6 new tests for config-biased decisions
 - AgentCoordinatorTests.swift - Add 4 new tests for config integration

 Phase 4: macOS UI (2-3 days)

 Goal: Full Orchestration Console for configuration

 Files to create:
 - ios/OpenAgents/ViewModels/OrchestrationConsoleViewModel.swift (~250 lines)
   - @Published config, validationErrors, isRunning
   - Methods: loadConfig, saveConfig, validateCronExpression
   - Bridge integration for config.get/set
 - ios/OpenAgents/Views/OrchestrationConsoleView.swift (~600 lines)
   - Schedule section (cron builder with presets + preview)
   - Constraints toggles
   - Time budget slider
   - Goals text editor
   - Agent preferences (prefer radio + allow checkboxes)
   - Focus include/exclude
   - PR automation settings
   - Save/Start/Stop buttons
 - ios/OpenAgents/Helpers/CronExpressionHelper.swift (~100 lines)
   - Preset expressions ("Every 30 min", "Daily at 2 AM")
   - Human-readable formatter
   - Validation

 Files to modify:
 - ios/OpenAgents/SimplifiedMacOSView.swift
   - Add "Orchestration" section to sidebar
   - Add navigation to OrchestrationConsoleView

 Phase 5: iOS UI (1-2 days)

 Goal: Lightweight config editing on iOS

 Files to create:
 - ios/OpenAgents/Views/OrchestrationSettingsView.swift (~300 lines)
   - Goals editor
   - Agent preference picker
   - PR automation toggles
   - Read-only schedule/constraints summary
   - Save button → bridge config.set

 Files to modify:
 - iOS main navigation to include settings entry point

 Phase 6: Morning Briefing Updates (1 day)

 Goal: Show applied config in briefing

 Files to modify:
 - ios/OpenAgents/Views/MorningBriefingDemoView.swift
   - Add config summary section
   - Display goals that were addressed
   - Show agent preferences used
   - Add "Re-run tonight" button
 - ios/OpenAgents/MockData/OrchestrationMockData.swift
   - Add mock config_id to tasks
   - Add mock config summary

 Phase 7: Testing & Documentation (Throughout)

 Tests: ~800 lines total
 - Unit tests for all new components
 - Integration tests for bridge + coordinator
 - E2E test: config.set → runCycle → verify behavior
 - UI tests for form validation

 Documentation:
 - Update docs/overnight/architecture.md with config section
 - Update docs/overnight/ui-specification.md with new views
 - Comment on issue 1462 with this plan
 - Close issue 1462 when complete

 Key Design Decisions

 1. Config as Parameter: Pass config to runCycle() and decideNextTask() (not stored state)
 2. JSON Blob Storage: Store entire config as JSON in Tinyvex (schema flexibility)
 3. Server-Side Validation: Bridge validates all config fields before persist
 4. Goal Bias Heuristic: Extend existing heuristic with config alignment (+10% confidence)
 5. MVP Scope: Config editing only - SchedulerService and PRAutomationService deferred

 Success Criteria

 - ✅ Users can edit/save/load orchestration config on macOS
 - ✅ iOS can edit subset of config fields
 - ✅ Config persists in Tinyvex and survives restart
 - ✅ DecisionEngine biases decisions based on goals
 - ✅ Agent selection respects agentPreferences.allow/prefer
 - ✅ Task metadata includes config_id
 - ✅ Morning Briefing shows applied config summary
 - ✅ All tests pass (32 existing + ~40 new = 72 total)
 - ✅ Builds succeed on iOS and macOS

 Out of Scope (Future Issues)

 - SchedulerService (cron-based triggering)
 - PRAutomationService (actual PR creation)
 - Full upgrade manifest integration
 - Advanced constraints (DND, CPU)

 Next Steps After Approval

 1. Comment on issue 1462 with this plan
 2. Begin Phase 1 (OrchestrationConfig model + storage)
 3. Follow TDD: write tests first, then implementation
 4. Build and test after each phase
 5. Commit frequently with descriptive messages
