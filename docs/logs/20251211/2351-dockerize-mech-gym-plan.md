# Unified Docker-Based TB2 Streaming Architecture

## Goal

Replace the broken `tbench` runner (which runs claude CLI on host without Docker) with a proper Docker-based system using `DockerRunner`. Ensure the GYM panel receives perfect streaming with full container metadata (container ID, image name, resource info).

## Current Problem

**Two incompatible systems exist:**

1. **tbench (harbor crate)** - Currently used by MechaCoder
   - ❌ Just spawns `claude` CLI directly on host
   - ❌ NO Docker containers
   - ❌ NO TB2 verification environment
   - ❌ Working dir is project root, not isolated
   - ❌ Times out waiting for tool results (hung processes)

2. **DockerRunner (gym crate)** - Proper implementation but unused
   - ✅ Runs Claude inside Docker containers
   - ✅ Uses actual TB2 Docker images from ~/code/terminal-bench-2
   - ✅ Emits `DockerEvent` with container ID, image name
   - ✅ Proper isolation with volume mounts
   - ✅ TestGen protocol wrapping
   - ✅ TB2 verification integration

**Result:** MechaCoder GYM panel has no container info and runs fail because there's no Docker environment.

## Solution Architecture

**Unify on ONE path: Move DockerRunner to mechacoder and integrate with streaming UI**

```
Terminal-Bench 2 Tasks (~/code/terminal-bench-2/)
              ↓
    TB2TaskLoader (load task.toml + instruction.md)
              ↓
    DockerRunner (spawn Docker with TB2 image)
              ↓
    DockerEvent Stream (container_id, image, output)
              ↓
    GymPanel UI (display container metadata + stream)
```

## Implementation Plan

### Phase 1: Move DockerRunner to mechacoder crate

**Goal:** Make DockerRunner available to MechaCoder with full TestGen protocol and verification

**Files to create:**

1. **Create `crates/mechacoder/src/panels/docker_runner.rs`**
   - Copy from `crates/gym/src/mechacoder/docker_runner.rs`
   - Keep all DockerEvent types and streaming logic
   - Keep TestGen protocol integration (requires testgen_wrapper)

2. **Create `crates/mechacoder/src/panels/testgen_wrapper.rs`**
   - Copy from `crates/gym/src/mechacoder/testgen_wrapper.rs`
   - Wraps task instructions with TestGen protocol v2
   - Ensures anti-cheating: no reading /tests/* files

3. **Create `crates/mechacoder/src/panels/verifier.rs`**
   - Copy from `crates/gym/src/mechacoder/verifier.rs`
   - Runs TB2 test.sh in container after completion
   - Parses reward.txt (0 or 1) and ctrf.json
   - Returns VerificationResult

**Files to modify:**

1. **Update `crates/mechacoder/src/panels/mod.rs`**
   - Add: `pub mod docker_runner;`
   - Add: `pub mod testgen_wrapper;`
   - Add: `pub mod verifier;`
   - Export: `pub use docker_runner::{DockerRunner, DockerEvent, DockerRunConfig, DockerRunResult};`
   - Export: `pub use verifier::{TB2Verifier, VerificationResult};`

2. **Update `crates/mechacoder/Cargo.toml`**
   - Add dependency: `sandbox = { path = "../sandbox" }`
   - Add dependency: `testgen = { path = "../testgen" }`
   - Already has: `terminalbench = { path = "../terminalbench" }`
   - Already has: `harbor = { path = "../harbor" }`

### Phase 2: Create unified event types for UI

**Goal:** Bridge DockerEvent to UI-compatible events

**Files to create/modify:**

1. **Create `crates/mechacoder/src/panels/runner_event.rs`**
   ```rust
   /// Unified event type for TB2 runs with container metadata
   pub enum TB2RunnerEvent {
       RunStart {
           run_id: String,
           task_id: String,
           task_name: String,
           container_id: String,
           image_name: String,
       },
       ContainerStarting {
           run_id: String,
           image: String,
       },
       ContainerStarted {
           run_id: String,
           container_id: String,
       },
       AssistantMessage {
           run_id: String,
           turn: u32,
           text: String,
       },
       ToolUse {
           run_id: String,
           tool_name: String,
           tool_id: String,
       },
       ToolResult {
           run_id: String,
           tool_id: String,
           output: Option<String>,
           error: Option<String>,
       },
       ContainerStopped {
           run_id: String,
           exit_code: i32,
       },
       RunComplete {
           run_id: String,
           success: bool,
           turns: u32,
           cost_usd: f64,
           error: Option<String>,
       },
       Error {
           run_id: String,
           message: String,
       },
   }
   ```

2. **Add conversion from DockerEvent to TB2RunnerEvent**
   ```rust
   impl TB2RunnerEvent {
       pub fn from_docker_event(run_id: String, event: DockerEvent) -> Vec<Self> {
           match event {
               DockerEvent::ContainerStarting { image } => vec![
                   Self::ContainerStarting { run_id, image }
               ],
               DockerEvent::ContainerStarted { container_id } => vec![
                   Self::ContainerStarted { run_id: run_id.clone(), container_id }
               ],
               DockerEvent::AssistantMessage { text, turn } => vec![
                   Self::AssistantMessage { run_id, turn, text }
               ],
               // ... handle all variants
           }
       }
   }
   ```

### Phase 3: Update GymPanel to use DockerRunner

**Goal:** Replace TBenchRunner with DockerRunner

**Files to modify:**

1. **Update `crates/mechacoder/src/screen.rs`**

   Replace:
   ```rust
   tbench_runner: TBenchRunner
   ```

   With:
   ```rust
   docker_runner: DockerRunner
   tb2_task_loader: TB2TaskLoader
   ```

   In `handle_gym_panel_event()`:
   ```rust
   GymPanelEvent::StartTB2Run { run_id, task, model } => {
       // Load full TB2Task with Docker image
       let tb2_task = self.tb2_task_loader.load_task(&task.id)
           .expect("Failed to load TB2 task");

       // Create workspace and logs dirs
       let workspace_dir = tempfile::tempdir().unwrap();
       let logs_dir = workspace_dir.path().join("logs");
       std::fs::create_dir_all(&logs_dir).unwrap();

       // Build config
       let config = DockerRunConfig::new(tb2_task, workspace_dir.into_path(), logs_dir)
           .max_turns(task.max_turns)
           .model(model.id());

       // Create event channel
       let (event_tx, mut event_rx) = mpsc::unbounded_channel();
       let (abort_tx, abort_rx) = tokio::sync::oneshot::channel();

       // Add TB2 run entry to thread
       if let Some(sdk_thread) = &self.sdk_thread {
           sdk_thread.update(cx, |thread, cx| {
               thread.add_tb2_run_entry(TB2RunEntry {
                   run_id: run_id.clone(),
                   task_id: task.id.clone(),
                   task_name: task.name.clone(),
                   status: TBRunStatus::Running,
                   turns: 0,
                   max_turns: task.max_turns,
                   cost: None,
                   error: None,
                   container_id: None,  // Will be filled when container starts
                   image_name: Some(config.task.docker_image().to_string()),
               }, cx);
           });
       }

       // Spawn DockerRunner task
       let docker_runner = self.docker_runner.clone();
       let run_id_clone = run_id.clone();
       let gym_panel = self.gym_panel.clone();
       let sdk_thread = self.sdk_thread.clone();

       cx.spawn(async move |_this, cx| {
           // Run Docker container
           let result = docker_runner.run_claude(&config, event_tx.clone(), abort_rx).await;

           // Process events from Docker
           while let Some(docker_event) = event_rx.recv().await {
               let tb2_events = TB2RunnerEvent::from_docker_event(run_id_clone.clone(), docker_event);

               for event in tb2_events {
                   // Update thread view
                   if let Some(sdk_thread) = &sdk_thread {
                       let _ = sdk_thread.update(cx, |thread, cx| {
                           thread.add_tb2_event(event.clone(), cx);
                       });
                   }

                   // Update gym panel
                   let _ = gym_panel.update(cx, |panel, cx| {
                       panel.handle_tb2_event(&event, cx);
                   });
               }
           }

           // Handle completion
           match result {
               Ok(run_result) => {
                   // Run TB2 verification
                   let verifier = TB2Verifier::new();
                   let verification = verifier.run_tests(
                       &config.task,
                       &config.workspace_dir,
                       &config.logs_dir,
                   ).await;

                   let final_success = run_result.success && verification.passed;
                   let verification_error = if !verification.passed {
                       Some(format!("Tests failed: {}/{} passed. Reward: {}",
                           verification.tests_passed,
                           verification.tests_total,
                           verification.reward))
                   } else {
                       run_result.error
                   };

                   let _ = gym_panel.update(cx, |panel, cx| {
                       panel.handle_tb2_complete(
                           &run_id_clone,
                           final_success,
                           run_result.turns,
                           Some(run_result.cost_usd),
                           verification_error,
                           cx,
                       );
                   });

                   // Log verification results
                   log::info!(
                       "TB2 verification: {} - {}/{} tests passed, reward: {}",
                       if verification.passed { "PASS" } else { "FAIL" },
                       verification.tests_passed,
                       verification.tests_total,
                       verification.reward
                   );
               }
               Err(e) => {
                   log::error!("Docker runner error: {}", e);
                   let _ = gym_panel.update(cx, |panel, cx| {
                       panel.handle_tb2_complete(
                           &run_id_clone,
                           false,
                           0,
                           None,
                           Some(format!("Docker error: {}", e)),
                           cx,
                       );
                   });
               }
           }
       }).detach();
   }
   ```

2. **Update `crates/mechacoder/src/sdk_thread.rs`**

   Add container metadata to TB2RunEntry:
   ```rust
   pub struct TB2RunEntry {
       pub run_id: String,
       pub task_id: String,
       pub task_name: String,
       pub status: TBRunStatus,
       pub turns: u32,
       pub max_turns: u32,
       pub cost: Option<f64>,
       pub error: Option<String>,
       // NEW:
       pub container_id: Option<String>,
       pub image_name: Option<String>,
   }
   ```

   Add method to update container info:
   ```rust
   impl SdkThread {
       pub fn update_tb2_container_info(
           &mut self,
           run_id: &str,
           container_id: String,
           cx: &mut Context<Self>,
       ) {
           for entry in &mut self.entries {
               if let ThreadEntry::TB2Run(ref mut run_entry) = entry {
                   if run_entry.run_id == run_id {
                       run_entry.container_id = Some(container_id);
                       cx.notify();
                       return;
                   }
               }
           }
       }
   }
   ```

3. **Update `crates/mechacoder/src/panels/gym_panel.rs`**

   Add container info to ActiveRunState:
   ```rust
   pub struct ActiveRunState {
       pub run_id: String,
       pub task_id: String,
       pub task_name: String,
       pub turns: u32,
       pub max_turns: u32,
       // NEW:
       pub container_id: Option<String>,
       pub image_name: Option<String>,
   }
   ```

   Update `handle_tb2_event()` to track container info:
   ```rust
   pub fn handle_tb2_event(&mut self, event: &TB2RunnerEvent, cx: &mut Context<Self>) {
       match event {
           TB2RunnerEvent::ContainerStarted { run_id, container_id } => {
               if let Some(ref mut run) = self.active_run {
                   if run.run_id == *run_id {
                       run.container_id = Some(container_id.clone());
                       cx.notify();
                   }
               }
           }
           TB2RunnerEvent::AssistantMessage { run_id, turn, .. } => {
               if let Some(ref mut run) = self.active_run {
                   if run.run_id == *run_id {
                       run.turns = *turn;
                       cx.notify();
                   }
               }
           }
           // ... handle other events
       }
   }
   ```

### Phase 4: Update GymPanel UI to display container info

**Goal:** Show container metadata in the UI

**Files to modify:**

1. **Update `crates/mechacoder/src/panels/gym_panel.rs` - render_active_run()**

   Add container info display:
   ```rust
   fn render_active_run(&self) -> impl IntoElement {
       if let Some(run) = &self.active_run {
           let progress = run.turns as f32 / run.max_turns as f32;
           let progress_width = (progress * 100.0).min(100.0);
           let bar_filled = (progress * 10.0) as usize;
           let bar_empty = 10 - bar_filled;
           let progress_bar = format!(
               "[{}{}]",
               "#".repeat(bar_filled),
               "-".repeat(bar_empty)
           );

           div()
               .px(px(12.0))
               .py(px(8.0))
               .border_b_1()
               .border_color(border::DEFAULT)
               .child(
                   div()
                       .text_xs()
                       .text_color(text::MUTED)
                       .font_weight(gpui::FontWeight::BOLD)
                       .mb(px(4.0))
                       .child("ACTIVE")
               )
               .child(
                   div()
                       .text_sm()
                       .text_color(text::PRIMARY)
                       .child(format!("{} - Turn {}/{}", run.task_name, run.turns, run.max_turns))
               )
               .child(
                   div()
                       .text_sm()
                       .text_color(text::SECONDARY)
                       .font_family(FONT_FAMILY)
                       .child(format!("{} {:.0}%", progress_bar, progress_width))
               )
               // NEW: Container info
               .when(run.image_name.is_some(), |el| {
                   el.child(
                       div()
                           .mt(px(4.0))
                           .text_xs()
                           .text_color(text::MUTED)
                           .child(format!("Image: {}", run.image_name.as_ref().unwrap()))
                   )
               })
               .when(run.container_id.is_some(), |el| {
                   el.child(
                       div()
                           .text_xs()
                           .text_color(text::MUTED)
                           .child(format!("Container: {}", &run.container_id.as_ref().unwrap()[..12]))
                   )
               })
       } else {
           div()
       }
   }
   ```

2. **Update `crates/mechacoder/src/ui/tbench_view.rs` - TBenchRunView**

   Add container info to run header:
   ```rust
   fn render_run_header(&self, entry: &TB2RunEntry, cx: &mut Context<Self>) -> impl IntoElement {
       div()
           .flex()
           .flex_col()
           .gap(px(4.0))
           .child(
               div()
                   .flex()
                   .flex_row()
                   .items_center()
                   .gap(px(8.0))
                   .child(status_symbol)
                   .child(
                       div()
                           .text_sm()
                           .font_weight(gpui::FontWeight::BOLD)
                           .child(format!("[TB2] {}", entry.task_name))
                   )
           )
           // NEW: Container metadata row
           .when(entry.image_name.is_some() || entry.container_id.is_some(), |el| {
               el.child(
                   div()
                       .flex()
                       .flex_row()
                       .gap(px(12.0))
                       .text_xs()
                       .text_color(text::MUTED)
                       .when(entry.image_name.is_some(), |el| {
                           el.child(format!("Image: {}", entry.image_name.as_ref().unwrap()))
                       })
                       .when(entry.container_id.is_some(), |el| {
                           el.child(format!("Container: {}", &entry.container_id.as_ref().unwrap()[..12]))
                       })
               )
           })
           .child(
               div()
                   .text_xs()
                   .text_color(text::SECONDARY)
                   .child(format!("Turn {}/{} • Cost: ${:.4}",
                       entry.turns, entry.max_turns, entry.cost.unwrap_or(0.0)))
           )
   }
   ```

### Phase 5: Remove obsolete tbench runner

**Goal:** Clean up the old non-Docker path

**Files to delete:**

1. **Delete `crates/mechacoder/src/panels/tbench_runner.rs`**
   - No longer needed, replaced by DockerRunner

**Files to modify:**

1. **Update `crates/mechacoder/src/panels/mod.rs`**
   - Remove: `pub mod tbench_runner;`
   - Remove: `pub use tbench_runner::{TBenchRunner, ...};`
   - Keep: `pub mod docker_runner;`

2. **Update `crates/mechacoder/src/screen.rs`**
   - Remove: `use crate::panels::TBenchRunner;`
   - Remove: `tbench_runner: TBenchRunner` field
   - Add: `docker_runner: DockerRunner`
   - Add: `tb2_task_loader: TB2TaskLoader`

### Phase 6: Ensure TB2TaskLoader is available to mechacoder

**Goal:** Make TB2 task loading available

**Files to modify:**

1. **Update `crates/terminalbench/Cargo.toml`**
   - Already has: `serde`, `chrono`, `uuid`, `harbor`
   - Add if missing: `toml = "0.8"`

2. **Create `crates/terminalbench/src/tb2_loader.rs`**
   - Copy from `crates/gym/src/mechacoder/tb2_loader.rs`
   - Keep all TB2Task, TaskToml, TB2TaskLoader types

3. **Update `crates/terminalbench/src/lib.rs`**
   - Add: `pub mod tb2_loader;`
   - Export: `pub use tb2_loader::{TB2Task, TB2TaskLoader, TaskToml, ...};`

4. **Update `crates/mechacoder/Cargo.toml`**
   - Already has: `terminalbench = { path = "../terminalbench" }`
   - Already has: `harbor = { path = "../harbor" }`
   - Add: `sandbox = { path = "../sandbox" }`

## Critical Files to Modify

| File | Action | Purpose |
|------|--------|---------|
| `crates/mechacoder/src/panels/docker_runner.rs` | CREATE | Copy DockerRunner from gym |
| `crates/mechacoder/src/panels/testgen_wrapper.rs` | CREATE | TestGen protocol v2 wrapping |
| `crates/mechacoder/src/panels/verifier.rs` | CREATE | TB2 test.sh execution and parsing |
| `crates/mechacoder/src/panels/runner_event.rs` | CREATE | Unified TB2RunnerEvent type |
| `crates/mechacoder/src/panels/mod.rs` | MODIFY | Export new modules |
| `crates/mechacoder/src/screen.rs` | MODIFY | Use DockerRunner with verification |
| `crates/mechacoder/src/sdk_thread.rs` | MODIFY | Add container_id/image_name to TB2RunEntry |
| `crates/mechacoder/src/panels/gym_panel.rs` | MODIFY | Track container info, update UI |
| `crates/mechacoder/src/ui/tbench_view.rs` | MODIFY | Display container + verification metadata |
| `crates/terminalbench/src/tb2_loader.rs` | CREATE | Move TB2TaskLoader to shared crate |
| `crates/terminalbench/src/lib.rs` | MODIFY | Export TB2 types |
| `crates/mechacoder/Cargo.toml` | MODIFY | Add sandbox, testgen dependencies |
| `crates/mechacoder/src/panels/tbench_runner.rs` | DELETE | Obsolete non-Docker runner |

## Testing Plan

1. **Build verification**
   ```bash
   cargo build -p mechacoder
   ```

2. **Run MechaCoder**
   ```bash
   ./target/debug/MechaCoder
   ```

3. **Test GYM panel workflow**
   - Open GYM panel (Cmd+G / Ctrl+G)
   - Select a TB2 task (e.g., "fm-list-directory")
   - Select model (Claude Haiku 4.5)
   - Click "Run TB2"
   - Verify:
     - Container image name appears in ACTIVE section
     - Container ID appears (first 12 chars)
     - Turn progress updates
     - Events stream to main chat timeline
     - TestGen protocol in action (ANALYZE→EXPAND→REVIEW→IMPLEMENT)
     - Container stops on completion
     - TB2 verification runs (test.sh executes)
     - Reward shown (0 or 1, with test count)

4. **Verify Docker integration**
   ```bash
   docker ps  # Should show running container during execution
   docker images | grep alexgshaw  # Should show TB2 images
   ```

## Success Criteria

- ✅ GYM panel shows Docker container ID and image name
- ✅ TB2 runs execute inside proper Docker containers
- ✅ Uses actual TB2 Docker images from ~/code/terminal-bench-2
- ✅ TestGen protocol v2 wrapping enforced (anti-cheating)
- ✅ TB2 verification runs after completion (test.sh → reward.txt)
- ✅ Events stream in real-time to UI
- ✅ Container metadata visible in both GYM panel and thread view
- ✅ No more timeouts or hung processes
- ✅ Proper isolation with volume mounts (/app, /logs, /tests)
- ✅ Single unified code path (no tbench/DockerRunner split)
- ✅ Verification results shown in UI (X/Y tests passed, reward: 0 or 1)

## Dependencies

- TB2 tasks directory: `~/code/terminal-bench-2/`
- Docker daemon running
- ANTHROPIC_API_KEY set
- TB2 Docker images pulled (or will pull on first run)
- sandbox crate (already exists)
- terminalbench crate (already exists)

## Notes

- This removes the harbor/tbench dependency from MechaCoder entirely
- DockerRunner is battle-tested from gym crate with 100+ successful TB2 runs
- Streaming is already proven to work with DockerEvent
- TestGen protocol v2 ensures anti-cheating (no reading /tests/* files)
- TB2 verification provides immediate pass/fail feedback with test counts
- This aligns with the original vision: proper TB2 evaluation with Docker isolation
- All 92 TB2 tasks will work out of the box with their official Docker images


