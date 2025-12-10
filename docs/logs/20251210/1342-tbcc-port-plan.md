# Gym: TerminalBench Command Center Integration Plan

## Overview

Transform the Gym screen into a comprehensive Terminal-Bench workbench by:
1. Adding multi-view tab navigation (Trajectories, TBCC, HillClimber, TestGen)
2. Building expandable sidebar with type-based grouping (Sessions, HC Runs, TestGen Suites)
3. Porting TBCC to native GPUI components (Dashboard, Tasks, Runs, Settings)
4. Creating real-time HillClimber MAP orchestrator visualization
5. Creating TestGen test suite generation visualizer
6. Keeping existing trajectory chat for future natural language routing

**Approach**: All-in-one parallel implementation (10-15 days total)

## Architecture Decision Summary

- **TBCC**: Full GPUI rewrite (native, no WebView)
- **Trajectory View**: Separate "Trajectories" tab, reusable components for TB runs
- **Sidebar**: Type-based grouping (Sessions → HC Runs → TestGen Suites)
- **Chat**: Keep current implementation for future router/NL features
- **Implementation**: Parallel development of all components

## High-Level Structure

```
Gym Screen (GPUI)
├── Sidebar (Left, 260px)
│   ├── 📁 Sessions
│   │   └── Trajectories from atif-store
│   ├── 📁 HillClimber Runs
│   │   └── Experiments from hillclimber-store
│   └── 📁 TestGen Suites
│       └── Test suites from testgen-store
├── Tab Bar (Top)
│   ├── [Trajectories] - Current chat + trajectory viewer (reusable)
│   ├── [TBCC] - 4 sub-tabs (Dashboard, Tasks, Runs, Settings)
│   ├── [HillClimber] - Real-time MAP orchestrator visualization
│   └── [TestGen] - Test generation progress + test list
└── Main Content (Right)
    └── Active tab content
```

## Critical Files - New

### Core Gym Structure
```
crates/commander/src/
├── gym/
│   ├── mod.rs                          # Module exports
│   ├── gym_screen.rs                   # Main Gym container (tab nav, sidebar)
│   ├── sidebar.rs                      # Expandable tree sidebar
│   └── types.rs                        # Shared types (GymTab, TreeNode, etc.)
```

### Trajectory View (Refactored)
```
crates/commander/src/gym/
├── trajectory_view.rs                  # Current chat/trajectory viewer (extracted)
└── trajectory_detail.rs                # Reusable trajectory step renderer
```

### TBCC Components (Native GPUI)
```
crates/commander/src/gym/tbcc/
├── mod.rs                              # TBCC module exports
├── types.rs                            # TBTask, TBRun, DashboardStats, etc.
├── dashboard.rs                        # Tab 1: KPIs, recent runs, quick actions
├── task_browser.rs                     # Tab 2: Task list + detail (run buttons)
├── run_browser.rs                      # Tab 3: Run list + detail + terminal output
└── settings.rs                         # Tab 4: Execution + logging settings
```

### HillClimber Visualization
```
crates/commander/src/gym/hillclimber/
├── mod.rs                              # HC module exports
├── monitor.rs                          # Main HC monitor view
├── workflow_graph.rs                   # DAG visualization (TestGen→Decomposer→FM→Verifier)
├── turn_log.rs                         # Turn-by-turn action log
├── test_results.rs                     # Test pass/fail display
└── controls.rs                         # Start/stop controls, mode selector
```

### TestGen Visualization
```
crates/commander/src/gym/testgen/
├── mod.rs                              # TestGen module exports
├── visualizer.rs                       # Main TestGen view
├── category_progress.rs                # Progress bars per category
├── test_list.rs                        # Scrollable test case list
└── test_detail.rs                      # Test code viewer with syntax highlighting
```

### Data Layer
```
crates/commander/src/gym/
├── websocket_client.rs                 # WebSocket client for real-time events
├── data_loader.rs                      # SQLite query helpers (reactive)
└── event_protocol.rs                   # HC/TestGen event type definitions
```

## Critical Files - Modified

```
crates/commander/src/
├── main.rs                             # Switch to GymScreen, remove old render_gym_screen
├── actions.rs                          # Add gym tab actions (SwitchToTBCC, etc.)
└── app_menus.rs                        # Add Gym submenu

crates/hillclimber/src/
├── lib.rs                              # Expose event emitter
└── orchestrator.rs                     # Ensure HudEmitter integration

crates/testgen/src/
└── lib.rs                              # Add event emitter (similar to HC)
```

## Implementation Plan - Parallel Tracks

### Track 1: Core Structure (Foundation Team)

**Days 1-3: Multi-View Shell**
1. Create `gym_screen.rs` with tab navigation
2. Create `sidebar.rs` with expandable tree (use recursive rendering pattern from Zed)
3. Create `types.rs` with GymTab enum, TreeNode enum
4. Wire up tab switching with keyboard shortcuts (Cmd+1/2/3/4)
5. Extract current trajectory viewer to `trajectory_view.rs`

**Key Pattern** (Tab Switching):
```rust
pub enum GymTab {
    Trajectories,
    TBCC,
    HillClimber,
    TestGen,
}

impl GymScreen {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .flex_col()
                    .child(self.render_tab_bar(cx))
                    .child(self.render_active_tab(cx))
            )
    }

    fn render_active_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.current_tab {
            GymTab::Trajectories => self.trajectory_view.render(cx).into_any_element(),
            GymTab::TBCC => self.tbcc_view.render(cx).into_any_element(),
            GymTab::HillClimber => self.hillclimber_view.render(cx).into_any_element(),
            GymTab::TestGen => self.testgen_view.render(cx).into_any_element(),
        }
    }
}
```

**Days 4-5: Sidebar Data Integration**
1. Load trajectories from `atif-store`
2. Load HC runs from `hillclimber/store.rs` (SQLite)
3. Load TestGen suites from testgen store
4. Implement tree expansion state management
5. Wire up click handlers (select item → switch tab + load detail)

**Key Pattern** (Tree Rendering):
```rust
pub enum TreeNode {
    Category {
        id: String,
        label: String,
        icon: &'static str,
        children: Vec<TreeNode>,
    },
    Item {
        id: String,
        kind: TreeItemKind, // Session, HCRun, TestGenSuite
        label: String,
        metadata: String,   // e.g., "23 steps", "87% pass"
        status: ItemStatus,
    },
}

impl Sidebar {
    fn render_node(&self, node: &TreeNode, depth: usize, cx: &mut Context<Self>) -> AnyElement {
        match node {
            TreeNode::Category { id, label, icon, children } => {
                let expanded = self.expanded.contains(id);
                div()
                    .child(render_category_header(label, icon, expanded, depth))
                    .when(expanded, |el| {
                        el.children(children.iter().map(|child|
                            self.render_node(child, depth + 1, cx)
                        ))
                    })
                    .into_any_element()
            }
            TreeNode::Item { id, kind, label, metadata, status } => {
                render_tree_item(id, kind, label, metadata, status, depth, cx)
                    .into_any_element()
            }
        }
    }
}
```

### Track 2: TBCC Native GPUI (TBCC Team)

**Days 1-2: TBCC Shell + Types**
1. Create `tbcc/types.rs` with domain types:
   - `TBTask`, `TBRun`, `TBRunStep`, `DashboardStats`
   - `ExecutionSettings`, `LoggingSettings`
2. Create TBCC shell with internal tab navigation
3. Stub out 4 tab views (Dashboard, Tasks, Runs, Settings)

**Days 3-5: Dashboard Tab**
1. KPI grid component (success rate, avg steps, total runs)
2. Recent runs table (reuse Trajectory table rendering pattern)
3. Quick action buttons (Run Benchmark, Run Single Task)
4. Wire to Desktop Server RPC for data

**Key Pattern** (Dashboard):
```rust
pub struct DashboardView {
    stats: Option<DashboardStats>,
    recent_runs: Vec<TBRunSummary>,
    loading: bool,
}

impl DashboardView {
    fn render_kpi_grid(&self) -> impl IntoElement {
        div()
            .grid()
            .grid_cols_4()
            .gap(px(16.0))
            .children([
                render_kpi_card("Success Rate", format!("{}%", self.stats.success_rate)),
                render_kpi_card("Avg Steps", self.stats.avg_steps.to_string()),
                render_kpi_card("Avg Duration", format!("{}s", self.stats.avg_duration)),
                render_kpi_card("Total Runs", self.stats.total_runs.to_string()),
            ])
    }
}
```

**Days 6-8: Task Browser Tab**
1. Task list from `tasks/terminal-bench-2.json`
2. Filter controls (difficulty, status)
3. Task detail panel (description, tags, timeout)
4. Run buttons (Quick/Standard/Full modes)
5. Wire to Desktop Server to start runs

**Days 9-11: Run Browser Tab**
1. Unified run list (local TB runs + HF trajectories)
2. Status filters (passed/failed/running)
3. Run detail panel with step accordion (reuse trajectory step renderer)
4. Terminal output panel
5. Data source toggle (Local / HuggingFace)

**Days 12-13: Settings Tab**
1. Execution settings form (maxAttempts, timeout, etc.)
2. Logging settings form (saveTrajectories, autoPrune)
3. localStorage persistence
4. Reset to defaults button

### Track 3: HillClimber Visualization (HC Team)

**Days 1-3: Monitor Shell + Controls**
1. Create `hillclimber/monitor.rs` with layout (graph area, turn log, metrics)
2. Control panel (Start/Stop, mode selector, session dropdown)
3. Metrics panel (progress bar, test results, turn counter)
4. WebSocket client setup

**Days 4-7: Workflow Graph**
1. Graph layout engine (hierarchical positioning)
   - Nodes: Task → TestGen → Decomposer → Subtasks → FM → Verifier → Results
2. Node rendering with status colors (Waiting/Active/Success/Failed)
3. Connection rendering (Bezier curves or straight lines)
4. Viewport controls (pan, zoom)

**Key Pattern** (Graph):
```rust
pub struct WorkflowGraph {
    nodes: Vec<GraphNode>,
    connections: Vec<GraphConnection>,
    viewport: Viewport,
}

pub struct GraphNode {
    id: String,
    kind: NodeKind, // TestGen, Decomposer, FMActor, etc.
    position: Point,
    size: Size,
    status: NodeStatus, // Waiting, Active, Success, Failed
    label: String,
    metadata: Option<String>,
}

impl WorkflowGraph {
    fn render_node(&self, node: &GraphNode, cx: &mut Context<Self>) -> impl IntoElement {
        let (bg, border) = match node.status {
            NodeStatus::Waiting => (bg::CARD, border::DEFAULT),
            NodeStatus::Active => (bg::HOVER, border::SELECTED),
            NodeStatus::Success => (status::SUCCESS_BG, status::SUCCESS),
            NodeStatus::Failed => (status::ERROR_BG, status::ERROR),
        };

        div()
            .absolute()
            .left(px(node.position.x))
            .top(px(node.position.y))
            .w(px(node.size.width))
            .h(px(node.size.height))
            .bg(bg)
            .border_2()
            .border_color(border)
            .rounded(px(8.0))
            .when(matches!(node.status, NodeStatus::Active), |el| {
                el.with_animation(/* pulse animation */)
            })
            .child(node.label.clone())
    }
}
```

**Days 8-10: Turn Log + Event Handling**
1. Turn-by-turn action log (scrollable, auto-scroll to bottom)
2. WebSocket event subscription
3. State updates on HC events (map_turn_start, map_verify, map_heartbeat)
4. Test results display (X/Y passed, failed test names)

**Days 11-12: Polish**
1. Smooth animations (pulse for active nodes, progress bars)
2. Error handling (disconnected state, failed runs)
3. Multi-session support (switch between active runs)

### Track 4: TestGen Visualization (TestGen Team)

**Days 1-3: Visualizer Shell + Category Progress**
1. Create `testgen/visualizer.rs` with layout (categories, test list, detail)
2. Category progress bars (anti_cheat, existence, correctness, boundary, integration)
3. Comprehensiveness score display (current vs target)
4. Quality metrics (balance, anti-cheat coverage, overlaps)

**Days 4-6: Test List + Detail**
1. Scrollable test list with status badges (Generated, Running, Passed, Failed)
2. Test detail panel (description, code, reasoning)
3. Syntax highlighting for test code (reuse markdown rendering)
4. Test confidence scores

**Days 7-8: Event Integration**
1. WebSocket subscription to TestGen events
2. Real-time updates as tests generate
3. Iteration progress tracking
4. Test status updates during verification

**Days 9-10: Polish**
1. Loading states
2. Empty states (no tests yet)
3. Error handling (generation failed)

### Track 5: Data Layer (Infrastructure Team)

**Days 1-5: WebSocket + Event Protocol**
1. Create `websocket_client.rs` (connect to Desktop Server on port 8080)
2. Define `event_protocol.rs` with all event types:
   - HillClimber: map_turn_start, map_fm_action, map_verify, map_heartbeat, map_run_complete
   - TestGen: testgen_start, testgen_iteration, testgen_test, testgen_complete
3. Event distribution to active views
4. Reconnection logic

**Key Pattern** (Event Handling):
```rust
impl GymScreen {
    fn setup_websocket(&mut self, cx: &mut Context<Self>) {
        let client = self.websocket_client.clone();

        cx.spawn(async move |view, cx| {
            loop {
                let message = client.recv().await;

                view.update(cx, |view, cx| {
                    view.handle_event(message, cx);
                }).ok();
            }
        }).detach();
    }

    fn handle_event(&mut self, event: WebSocketEvent, cx: &mut Context<Self>) {
        match event.type_.as_str() {
            "map_turn_start" | "map_heartbeat" => {
                self.hillclimber_view.update(cx, |hc, cx| {
                    hc.handle_event(event, cx);
                });
            }
            "testgen_start" | "testgen_test" => {
                self.testgen_view.update(cx, |tg, cx| {
                    tg.handle_event(event, cx);
                });
            }
            _ => {}
        }
        cx.notify();
    }
}
```

**Days 6-10: Data Loaders**
1. Trajectory loader (from atif-store SQLite)
2. HillClimber run loader (from hillclimber store)
3. TestGen suite loader (from testgen store)
4. Reactive queries (refresh on events, polling for historical data)
5. Caching layer

**Days 11-12: Performance**
1. Virtualized lists for large datasets (use `uniform_list`)
2. Event throttling (60 FPS max, debounce rapid updates)
3. Lazy loading for tree nodes
4. Efficient diff rendering

### Track 6: Integration + Testing (All Teams)

**Days 13-15: End-to-End Integration**
1. Wire all components together in `gym_screen.rs`
2. Cross-view navigation:
   - Click HC run in sidebar → switch to HillClimber tab + load session
   - Click "View Tests" in HC monitor → switch to TestGen tab
   - Start run from TBCC → auto-switch to HillClimber monitor
3. State persistence (tab selection, sidebar expansion)
4. Keyboard shortcuts for all actions

**Key Actions**:
```rust
actions!(
    gym,
    [
        // Tab navigation
        SwitchToTrajectories,  // Cmd+1
        SwitchToTBCC,          // Cmd+2
        SwitchToHillClimber,   // Cmd+3
        SwitchToTestGen,       // Cmd+4

        // Sidebar
        ToggleGymSidebar,      // Cmd+\
        ExpandAll,
        CollapseAll,

        // Run control
        StartQuickRun,         // Cmd+R
        StopCurrentRun,        // Cmd+.

        // View actions
        LoadTrajectory,
        LoadHCRun,
        LoadTestGenSuite,
    ]
);
```

## Data Flow Architecture

### HillClimber Event Flow
```
HillClimber Rust Crate
    │ (orchestrator.rs emits events via HudEmitter)
    ▼
Desktop Server WebSocket (port 8080)
    │ (broadcasts to all clients)
    ▼
Gym WebSocket Client
    │ (filters by sessionId)
    ▼
HillClimber Monitor View
    │ (updates graph nodes, turn log, metrics)
    ▼
GPUI Render
```

### TestGen Event Flow
```
TestGen Rust Crate
    │ (lib.rs emits events via TestGenEmitter)
    ▼
Desktop Server WebSocket
    │
    ▼
Gym WebSocket Client
    │
    ▼
TestGen Visualizer View
    │ (updates categories, test list, comprehensiveness)
    ▼
GPUI Render
```

### TBCC Data Flow
```
User clicks "Run Task" in TBCC Task Browser
    ▼
Desktop Server RPC: startTBRun(taskId, mode)
    ▼
HillClimber orchestrator starts
    ▼
Events stream via WebSocket
    ▼
Gym auto-switches to HillClimber tab
    ▼
Real-time visualization updates
```

## Component Reuse Strategy

### From Zed (Adapt Patterns):
- Panel trait for standardized views
- TreeViewItem disclosure pattern
- ListItem slot-based layout (start/content/end/end_hover)
- Virtualized lists via `uniform_list`
- Tab bar navigation
- Keyboard action handling

### From Current Commander:
- Trajectory step rendering (reuse for TBCC runs)
- Markdown rendering (for test code, task descriptions)
- Status badge rendering
- Theme colors and fonts

### From TBCC TypeScript (Port to GPUI):
- Dashboard KPI layout
- Task list filtering
- Run list with status indicators
- Settings form patterns

## Testing Strategy

### Unit Tests
- Tree expansion/collapse state
- Event parsing and routing
- Data loader queries
- Node status transitions

### Integration Tests
- Tab switching preserves state
- Sidebar selection → tab content updates
- WebSocket reconnection
- Multi-session handling

### E2E Tests
```rust
#[gpui::test]
async fn test_hillclimber_run_flow(cx: &mut TestAppContext) {
    // 1. Start app, navigate to Gym
    let app = TestApp::new(cx);
    app.go_to_gym();

    // 2. Switch to TBCC, start run
    app.gym().switch_to_tbcc();
    let session_id = app.start_run("regex-log", HCMode::Quick);

    // 3. Verify auto-switch to HC tab
    assert_eq!(app.gym().current_tab(), GymTab::HillClimber);

    // 4. Simulate events
    app.send_ws_event(json!({
        "type": "map_turn_start",
        "sessionId": session_id,
        "turn": 1,
        "maxTurns": 3
    }));

    // 5. Verify graph updates
    let monitor = app.gym().hillclimber_monitor();
    assert_eq!(monitor.current_turn(), 1);
}
```

## Performance Targets

- Initial load: <500ms
- Tab switch: <100ms
- Tree expand/collapse: <50ms
- Graph render: 30fps minimum during animations
- Event handling: <200ms latency from WebSocket to UI update
- Large lists: Support 1000+ items with virtualization

## Risk Mitigation

### Risk 1: Graph Rendering Performance
**Mitigation**:
- Start with simple positioned divs (not full SVG)
- Limit graph to 20-30 nodes max (sufficient for MAP)
- Pre-compute layouts, cache as much as possible
- Use canvas rendering if div performance insufficient

### Risk 2: WebSocket Event Volume
**Mitigation**:
- Debounce heartbeat events (update UI at 10fps, not 60fps)
- Use delta updates, not full state
- Implement backpressure (queue events if UI can't keep up)

### Risk 3: Parallel Development Conflicts
**Mitigation**:
- Clear module boundaries (each track owns its directory)
- Shared types defined early in `gym/types.rs`
- Daily integration syncs
- Feature flags for incomplete components

### Risk 4: GPUI Learning Curve
**Mitigation**:
- Reference Zed codebase extensively
- Start with simple patterns, iterate
- Pair programming for complex GPUI features
- Incremental testing at each step

## Success Criteria

- [ ] All 4 tabs render correctly with real data
- [ ] Sidebar supports 100+ items without lag
- [ ] HillClimber monitor updates in real-time (<200ms latency)
- [ ] TestGen visualizer shows all generation phases
- [ ] TBCC can start/stop runs successfully
- [ ] Cross-component navigation works (TBCC → HC → TestGen)
- [ ] Keyboard shortcuts work for all actions
- [ ] No console errors during normal operation
- [ ] Passes all unit + integration + E2E tests
- [ ] Performance targets met

## Timeline Summary

**Total: 10-15 days (parallel tracks)**

| Track | Duration | Team Size | Output |
|-------|----------|-----------|--------|
| Core Structure | 5 days | 1-2 people | Multi-tab shell + sidebar |
| TBCC Native | 13 days | 2-3 people | 4 TBCC tabs fully functional |
| HillClimber Viz | 12 days | 1-2 people | Real-time MAP visualization |
| TestGen Viz | 10 days | 1-2 people | Test generation progress |
| Data Layer | 12 days | 1-2 people | WebSocket + SQLite loaders |
| Integration | 3 days | All teams | E2E testing + polish |

**Critical Path**: TBCC Native (13 days) → Integration (3 days) = **16 days minimum**

With parallel execution and team coordination, realistic completion: **12-15 days**

## Next Steps After Plan Approval

1. Create module structure (`crates/commander/src/gym/`)
2. Define shared types in `gym/types.rs`
3. Set up WebSocket event protocol
4. Kick off all 6 tracks simultaneously
5. Daily standup for integration sync
6. Merge to main after E2E tests pass

---

This plan transforms Gym into the comprehensive Terminal-Bench workbench needed to push toward 100% on TB2 using only local Apple FM inference.
