╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
 Overnight Agents Tech Demo - Implementation Plan

 Overview

 Build a production-ready system where Claude Code and OpenAI Codex agents work autonomously overnight, orchestrated by on-device Foundation Models
 making periodic decisions about what work to delegate. All logic encapsulated as deterministic "upgrade" JSON manifests, demonstrating the future
 compute marketplace vision.

 Project Structure

 Create comprehensive documentation in /Users/christopherdavid/code/openagents/private/overnight/:
 - README.md - Architecture overview, demo flow, design principles
 - architecture.md - Detailed technical design, component diagrams, data flow
 - testing-plan.md - Integration testing strategy, manual test scenarios
 - issues/ - 10-12 GitHub issues covering all components
 - examples/ - Sample upgrade manifests (nightly-refactor.json, feature-worker.json)
 - demo-script.md - Step-by-step guide for recording the demo video

 Core Components (7 Issues)

 1. SchedulerService - Timer-based orchestration wake-up

 - macOS-only background service
 - Configurable intervals (10/30/60 min)
 - Time window enforcement (e.g., 1am-5am)
 - Constraint checking (power, network, idle state)
 - Integration with existing orchestration layer

 2. DecisionOrchestrator - FM-powered "what's next" engine

 - Uses real Foundation Models (macOS 26+)
 - Session history analysis via existing tools
 - Generates work items based on repo state
 - Selects appropriate agent (Codex vs Claude Code)
 - Returns structured decisions (task + agent + priority)

 3. TaskQueue - Persistent work queue

 - SQLite-backed state (Tinyvex integration)
 - Task lifecycle (pending → in_progress → completed/failed)
 - Priority scheduling
 - Deduplication via opHash
 - Observable state for UI updates

 4. AgentCoordinator - Multi-agent session manager

 - Delegates tasks to AgentProvider instances
 - Monitors progress via ACP SessionUpdate stream
 - Handles concurrent sessions (Codex + Claude in parallel)
 - Resume on failure with context
 - Collects results for PR generation

 5. PRAutomationService - GitHub integration

 - Uses gh CLI for PR creation
 - Branch management per agent session
 - Commit message generation from ACP tool calls
 - PR description from session summary
 - Links back to orchestration decisions

 6. UpgradeExecutor - JSON manifest runtime

 - Parses upgrade JSON manifests
 - Executes declarative pipelines (ops registry)
 - Integrates with SchedulerService
 - Validation and safety checks
 - Telemetry and logging

 7. PolicyEnforcer - Safety and constraints

 - Foundation Models AUP compliance checks
 - Resource limits (CPU, memory, disk)
 - Workspace permissions validation
 - Time budget enforcement
 - User-configurable guardrails

 Integration Issues (3 Issues)

 8. Bridge Integration - iOS monitoring UI

 - Real-time task queue visualization
 - Agent session status cards
 - Decision rationale display (FM explanations)
 - Manual intervention controls (pause/resume/cancel)
 - PR preview before push

 9. Upgrade Manifests - Demo JSON configs

 - nightly-refactor.json - Code quality improvements
 - feature-worker.json - Implement small features
 - Schedule configs, constraints, pipeline ops
 - Documentation and examples

 10. End-to-End Testing - Integration test suite

 - Mock overnight run (compressed timeline)
 - Multi-agent coordination tests
 - FM decision quality validation
 - PR creation pipeline tests
 - Error recovery scenarios

 Polish & Documentation (2 Issues)

 11. Demo Preparation - Video recording assets

 - Demo script with timestamps
 - Sample repo state setup
 - Expected agent outputs
 - PR examples with explanations
 - Upgrade JSON visualization

 12. Documentation - Technical docs + ADR

 - New ADR: "Overnight Agent Orchestration Architecture"
 - Update existing ADRs (ACP, Upgrades, Nostr)
 - API documentation for new components
 - Deployment guide (macOS setup)

 Key Technical Decisions

 - Real Foundation Models for decisions (macOS 26+, on-device)
 - Real GitHub PRs via gh CLI
 - SQLite/Tinyvex for persistent state
 - ACP protocol for all agent communication
 - JSON upgrade manifests as deterministic logic representation
 - macOS-only execution, iOS shows monitoring UI via bridge

 Success Criteria

 1. Agents work overnight autonomously (8+ hour run)
 2. FM makes ~15-20 decisions based on repo state
 3. 5-10 real PRs created with quality work (refactoring + features)
 4. All logic encapsulated in upgrade JSON
 5. iOS app shows real-time progress
 6. Video demonstrates: control, orchestration, upgrade concept, future Nostr/Bitcoin integration

 Timeline

 Build properly over 2-3 weeks, record video when stable and producing quality results.
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
