# Multi-Project Autopilot Support Plan

## Problem Statement

Currently, Autopilot only supports single-project operation with no UI for:
- Selecting/switching between projects
- Monitoring multiple Autopilot instances
- Viewing session history across projects
- Configuring where an Autopilot should work

## Design Decisions

- **Storage:** Extend `autopilot.db` with projects/sessions tables (per-project DBs)
- **Launch:** Support both UI and CLI for starting Autopilots
- **Visibility:** Separate views per project (manual switching, no cross-project aggregation)

## Current Architecture

### Core Components
- `crates/autopilot/` - CLI with `--cwd` for directory, `--issues_db` for database
- `crates/issues/` - SQLite-based issue storage (per-project)
- `crates/ui/src/recorder/` - Maud/HTMX session viewer components
- `crates/desktop/src/views/autopilot.rs` - Single-session live view

### What Works
- Single Autopilot with full-auto mode
- Issue management via CLI and MCP
- Real-time session streaming to UI
- Trajectory logging and replay

### Gaps
- No project configuration UI
- No multi-session dashboard
- No session history view
- No concurrent Autopilot orchestration

---

## Implementation Plan

### Phase 1: Extend Database Schema

**Goal:** Add projects and sessions tables to autopilot.db

**Changes:**
1. Add `projects` table to database schema (v4 migration)
   - `id` TEXT PRIMARY KEY
   - `name` TEXT UNIQUE - human-readable project name
   - `path` TEXT - absolute path to project directory
   - `description` TEXT - optional
   - `default_model` TEXT - sonnet/opus/haiku
   - `default_budget` REAL - default max budget USD
   - `created_at`, `updated_at` TIMESTAMP

2. Add `sessions` table to track Autopilot runs
   - `id` TEXT PRIMARY KEY (matches trajectory session_id)
   - `project_id` TEXT REFERENCES projects(id)
   - `status` TEXT - running/completed/failed/cancelled
   - `prompt` TEXT - initial task prompt
   - `model` TEXT - model used
   - `pid` INTEGER - process ID when running
   - `trajectory_path` TEXT - path to .rlog file
   - `started_at`, `ended_at` TIMESTAMP
   - `budget_spent` REAL, `issues_completed` INTEGER

**Files to modify:**
- `crates/issues/src/db.rs` - Add v4 migration with new tables
- `crates/issues/src/lib.rs` - Export new types

---

### Phase 2: Project & Session Management CLI

**Goal:** CLI commands to manage projects and track sessions.

**Changes:**
1. Add project management commands
   - `cargo autopilot project add <name> --path <dir>` - Register a project
   - `cargo autopilot project list` - List all registered projects
   - `cargo autopilot project remove <name>` - Unregister (keeps files)
   - `cargo autopilot project set-default <name>` - Set default project

2. Add session lifecycle hooks
   - On `run` start: Create session record with status=running, pid
   - On completion: Update status=completed, ended_at, metrics
   - On error: Update status=failed with error info

3. Add session commands
   - `cargo autopilot session list [--project <name>]`
   - `cargo autopilot session show <id>`

**Files to modify:**
- `crates/autopilot/src/main.rs` - Add project/session subcommands
- New: `crates/issues/src/project.rs` - Project CRUD operations
- New: `crates/issues/src/session.rs` - Session CRUD operations

---

### Phase 3: Dashboard UI (Active Sessions)

**Goal:** Monitor multiple running Autopilots from a single dashboard.

**Components:**

1. **Dashboard View** (`/dashboard`)
   - Grid layout of active session cards
   - Each card shows: project name, current status, elapsed time, cost
   - Live updates via WebSocket
   - Click card → open full session view in new tab/panel

2. **Session Card Component**
   - Compact view: project, prompt snippet, status dot, cost
   - Progress indicator (issues completed, budget used)
   - Actions: View details, Stop

**Files to add:**
- `crates/desktop/src/views/dashboard.rs` - Multi-session grid
- `crates/ui/src/recorder/sections/session_card.rs` - Card component

**Files to modify:**
- `crates/desktop/src/server.rs` - Add `/dashboard` route
- `crates/desktop/src/views/layout.rs` - Add nav link

---

### Phase 4: Project & Session List Views

**Goal:** Manage projects and view session history.

**Components:**

1. **Projects View** (`/projects`)
   - Table: name, path, sessions count, last active
   - Add project form (name, path picker)
   - Remove button per project
   - Click → filter sessions by project

2. **Sessions View** (`/sessions`)
   - Table: session ID, project, status, started, duration, cost
   - Filter by project, status
   - Click → replay trajectory

**Files to add:**
- `crates/desktop/src/views/projects.rs`
- `crates/desktop/src/views/sessions.rs`

**Files to modify:**
- `crates/desktop/src/server.rs` - Add routes

---

### Phase 5: Multi-Autopilot Orchestration

**Goal:** Launch and manage Autopilots from both UI and CLI.

**CLI:**
- `cargo autopilot start <project> [--background]` - Start in project dir
- `cargo autopilot stop <session_id>` - Graceful stop (SIGTERM)
- `cargo autopilot status` - List running sessions

**UI:**
- Start button on project cards / project list
- Stop button on active session cards
- Launch modal: select project, enter prompt, set budget

**Backend:**
- Store PID in sessions table
- Process health check (is PID still running?)
- WebSocket multiplexing: route by session_id

**Files to add:**
- New: `crates/autopilot/src/orchestrator.rs` - Process spawn/kill

**Files to modify:**
- `crates/desktop/src/ws.rs` - Multi-session routing
- `crates/autopilot/src/main.rs` - start/stop/status commands

---

## File Summary

**New Files:**
- `crates/issues/src/project.rs` - Project CRUD
- `crates/issues/src/session.rs` - Session CRUD
- `crates/autopilot/src/orchestrator.rs` - Process management
- `crates/desktop/src/views/dashboard.rs` - Multi-session view
- `crates/desktop/src/views/projects.rs` - Project management
- `crates/desktop/src/views/sessions.rs` - Session history
- `crates/ui/src/recorder/sections/session_card.rs` - Card component

**Modified Files:**
- `crates/issues/src/db.rs` - v4 schema migration
- `crates/issues/src/lib.rs` - Export new modules
- `crates/autopilot/src/main.rs` - CLI commands
- `crates/desktop/src/server.rs` - New routes
- `crates/desktop/src/ws.rs` - Multi-session support
- `crates/desktop/src/views/layout.rs` - Navigation

---

## Issues to Create

### Issue 1: Database Schema - Projects and Sessions Tables
**Priority:** High | **Type:** Feature

Add v4 migration to autopilot.db with `projects` and `sessions` tables.

**Acceptance Criteria:**
- [ ] Add `projects` table with fields: id, name (unique), path, description, default_model, default_budget, created_at, updated_at
- [ ] Add `sessions` table with fields: id, project_id (FK), status, prompt, model, pid, trajectory_path, started_at, ended_at, budget_spent, issues_completed
- [ ] Version 4 migration in `crates/issues/src/db.rs`
- [ ] Create `crates/issues/src/project.rs` with CRUD: create_project, list_projects, get_project_by_name, delete_project
- [ ] Create `crates/issues/src/session.rs` with CRUD: create_session, update_session_status, list_sessions, get_session
- [ ] Export new modules from `crates/issues/src/lib.rs`
- [ ] Add tests for project and session operations

---

### Issue 2: Project CLI Commands
**Priority:** High | **Type:** Feature

Add CLI commands to register and manage projects.

**Acceptance Criteria:**
- [ ] `cargo autopilot project add <name> --path <dir>` - registers project in DB
- [ ] `cargo autopilot project list` - shows all projects with path, session count
- [ ] `cargo autopilot project remove <name>` - removes from registry (keeps files)
- [ ] Update `run` command to optionally use `--project <name>` instead of `--cwd`
- [ ] When using `--project`, auto-set cwd and issues_db path

---

### Issue 3: Session Lifecycle Integration
**Priority:** High | **Type:** Feature

Auto-register sessions when Autopilot runs and track completion.

**Acceptance Criteria:**
- [ ] On `autopilot run` start: create session record with status=running, pid=current
- [ ] On successful completion: update status=completed, ended_at, budget_spent, issues_completed
- [ ] On error/crash: update status=failed
- [ ] `cargo autopilot session list [--project <name>]` - list sessions
- [ ] `cargo autopilot session show <id>` - show session details
- [ ] Handle orphaned sessions (pid no longer running → mark failed)

---

### Issue 4: Dashboard View - Active Sessions
**Priority:** Medium | **Type:** Feature

Multi-session monitoring dashboard at `/dashboard`.

**Acceptance Criteria:**
- [ ] Create `crates/desktop/src/views/dashboard.rs` with grid layout
- [ ] Create `crates/ui/src/recorder/sections/session_card.rs` component
- [ ] Session card shows: project name, prompt snippet, status dot, elapsed time, cost
- [ ] WebSocket updates cards in real-time
- [ ] Click card → navigate to full `/autopilot?session=<id>` view
- [ ] Add `/dashboard` route to server.rs
- [ ] Add Dashboard link to layout navigation

---

### Issue 5: Projects and Sessions List Views
**Priority:** Medium | **Type:** Feature

Project management and session history pages.

**Acceptance Criteria:**
- [ ] `/projects` view: table of projects with name, path, session count, last active
- [ ] Add project form with name input and path
- [ ] Remove project button (with confirmation)
- [ ] `/sessions` view: table with session ID, project, status, started, duration, cost
- [ ] Filter sessions by project dropdown
- [ ] Click session → open replay view
- [ ] Add nav links in layout

---

### Issue 6: Multi-Autopilot Process Orchestration
**Priority:** Medium | **Type:** Feature

Launch and stop Autopilot processes from CLI and UI.

**Acceptance Criteria:**
- [ ] `cargo autopilot start <project> [--background]` - spawns autopilot in project dir
- [ ] `cargo autopilot stop <session_id>` - sends SIGTERM to process
- [ ] `cargo autopilot status` - lists running sessions with PID, project, uptime
- [ ] Create `crates/autopilot/src/orchestrator.rs` for process management
- [ ] UI: "Start Autopilot" button on projects view → modal with prompt input
- [ ] UI: "Stop" button on active session cards
- [ ] WebSocket multiplexing: route updates by session_id for dashboard

---

## Implementation Order

1. **Issue 1** (Schema) - Foundation for everything else
2. **Issue 2** (Project CLI) - Can register projects
3. **Issue 3** (Session Lifecycle) - Sessions tracked automatically
4. **Issue 4** (Dashboard) - Monitor active sessions
5. **Issue 5** (List Views) - Manage projects and history
6. **Issue 6** (Orchestration) - Launch from UI
