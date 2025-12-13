# MechaCoder: Unified Router & UI Simplification

**Date:** 2025-12-12 21:35
**Author:** Claude (assisted by Chris)

## Overview

Simplified MechaCoder UI by removing agent switching dropdown from main view and adding a unified backend router that automatically selects the best available AI backend on startup.

## Changes

### New Files

#### `crates/mechacoder/src/router.rs`
New unified backend router module:
- `Backend` enum: ClaudeCode, OpenAI, Ollama, Pi, OpenAgentsCloud
- `Router` struct with detection and routing logic
- `RouterConfig` for user preferences (disable backends, set preferred)
- Detection logic:
  - Claude Code: checks if `claude` CLI exists via `which`
  - OpenAI: checks for `OPENAI_API_KEY` environment variable
  - Ollama: checks TCP connection to `127.0.0.1:11434`
  - Pi: always available (built-in)
- Priority routing: ClaudeCode > Ollama > Pi > OpenAgentsCloud

### Modified Files

#### `crates/mechacoder/src/actions.rs`
- Kept panel toggle actions: ToggleGymPanel, ToggleClaudePanel, TogglePiPanel
- Added ToggleSettings action
- Removed agent switching actions (SwitchToClaudeAgent, SwitchToPiAgent)

#### `crates/mechacoder/src/screen.rs`
- Added Router field and initialization
- Added `connect_to_best_backend()` method that uses router to auto-select
- Added `toggle_settings()` handler (opens Claude panel for now)
- Removed AgentType enum
- Removed agent_dropdown_open field
- Removed switch_to_claude_agent/switch_to_pi_agent methods
- Kept all panel infrastructure intact

#### `crates/mechacoder/src/lib.rs`
- Added `router` module
- Exported Router types: Backend, Router, RouterConfig, RouterStatus

#### `crates/mechacoder/src/main.rs`
- Changed Claude panel binding to Cmd+Shift+C (avoid conflict with copy)
- Added Pi panel binding: Cmd+P / Ctrl+P
- Added Settings binding: Cmd+, / Ctrl+,

#### `crates/mechacoder/src/app_menus.rs`
- Fixed Settings menu item to use ToggleSettings
- Updated View menu with panel toggle actions

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Cmd+G / Ctrl+G / F2 | Toggle Gym Panel |
| Cmd+Shift+C / Ctrl+Shift+C | Toggle Claude Panel |
| Cmd+P / Ctrl+P | Toggle Pi Panel |
| Cmd+, / Ctrl+, | Toggle Settings |
| Cmd+Enter | Send Message |
| Escape | Cancel/Close Panel |
| Cmd+L / Tab | Focus Input |
| Cmd+Q | Quit |

## Behavior Changes

1. **Automatic Backend Selection**: On startup, router detects available backends and connects to the best one automatically
2. **Clean Main View**: No agent dropdown cluttering the main interface
3. **Panels Still Accessible**: All panels (Gym, Claude, Pi) remain accessible via keybindings
4. **Graceful Fallback**: If no backends available, shows error message prompting user to install Claude Code or run Ollama

## Tests

All 7 tests pass:
- router::tests::test_router_config
- router::tests::test_backend_display_name
- router::tests::test_router_priority
- panels::testgen_wrapper::tests::test_is_wrapped
- panels::testgen_wrapper::tests::test_wrap_instruction
- panels::verifier::tests::test_ctrf_summary_deserialize
- panels::verifier::tests::test_verification_result_default
