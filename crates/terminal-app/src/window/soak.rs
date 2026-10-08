//! One-hour native drawing and isolated PTY retention measurement, without a window.
use super::*;
use std::time::{Duration, Instant};

const RSS_BUDGET: u64 = 768 * 1024 * 1024;
const GROWTH_BUDGET: u64 = 128 * 1024 * 1024;
const IDLE_CPU_PERCENT: f64 = 0.5;
const BUSY_CPU_PERCENT: f64 = 80.0;

fn rss() -> u64 {
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("Read process RSS");
    std::str::from_utf8(&output.stdout)
        .unwrap()
        .trim()
        .parse::<u64>()
        .unwrap()
        * 1024
}
fn free_disk(root: &std::path::Path) -> u64 {
    let output = std::process::Command::new("df")
        .arg("-k")
        .arg(root)
        .output()
        .expect("Read free disk");
    assert!(output.status.success());
    std::str::from_utf8(&output.stdout)
        .unwrap()
        .lines()
        .last()
        .unwrap()
        .split_whitespace()
        .nth(3)
        .unwrap()
        .parse::<u64>()
        .unwrap()
        * 1024
}

fn cpu_seconds() -> f64 {
    // SAFETY: getrusage fills the initialized record and retains no pointer.
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        assert_eq!(libc::getrusage(libc::RUSAGE_SELF, &mut usage), 0);
        let seconds = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
        seconds(usage.ru_utime) + seconds(usage.ru_stime)
    }
}

fn targets(device: &wgpu::Device, size: [u32; 2]) -> (wgpu::TextureView, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Native terminal soak offscreen target"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let depth = Gpu::depth(device, size[0], size[1]).create_view(&Default::default());
    (view, depth)
}

/// Closes the isolated process group on success, assertion failure, or GPU failure.
struct IsolatedTerminal(Overlay);
impl std::ops::Deref for IsolatedTerminal {
    type Target = Overlay;
    fn deref(&self) -> &Overlay {
        &self.0
    }
}
impl std::ops::DerefMut for IsolatedTerminal {
    fn deref_mut(&mut self) -> &mut Overlay {
        &mut self.0
    }
}
impl Drop for IsolatedTerminal {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

#[test]
#[ignore = "Requires a quiet one-hour native raster/PTY run; no display or GPU compute"]
fn native_idle_and_busy_retention_soak() {
    let out = PathBuf::from(
        std::env::var_os("TERMINAL_SOAK_OUT")
            .expect("Set TERMINAL_SOAK_OUT under openagents scratch"),
    );
    let seconds: u64 = std::env::var("TERMINAL_SOAK_SECONDS")
        .ok()
        .map(|s| s.parse().unwrap())
        .unwrap_or(3600);
    assert!(seconds >= 4);
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let root = tempfile::Builder::new()
        .prefix("terminal-soak-")
        .tempdir_in(out.parent().unwrap())
        .unwrap();
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let info = adapter.get_info();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut atlas = Atlas::new(16.0);
    atlas.reserve_glyphs(terminal_gfx::GLYPH_ROWS).unwrap();
    let mut painter = Painter::new(&device, &queue, wgpu::TextureFormat::Rgba8UnormSrgb, &atlas);
    let (mut view, mut depth) = targets(&device, [1200, 800]);
    let shell = if cfg!(target_os = "macos") {
        "/bin/zsh"
    } else {
        "/bin/bash"
    };
    let mut terminal = IsolatedTerminal(Overlay::with(
        root.path(),
        shell.into(),
        terminal_gfx::pty::Program::Shell,
    ));
    terminal.mount = Mount::Window;
    terminal
        .apply(&terminal_gfx::control::Request::Open)
        .unwrap();
    terminal.fit(&atlas, [1200.0, 800.0]);
    let mut submissions = 0u64;
    let mut draw = |terminal: &mut Overlay, painter: &mut Painter, atlas: &mut Atlas| {
        let mut batch = UiBatch::default();
        terminal.draw(&mut batch, atlas, [1200.0, 800.0]);
        painter
            .draw(&device, &queue, [1200, 800], &view, &depth, &batch, atlas)
            .unwrap();
        terminal.frame_done(Instant::now());
        terminal.core.paper.presented();
        terminal.presented(Instant::now());
        submissions += 1;
    };
    // Consume the shell's startup, hook table, and resize before measuring idle.
    let warm = Instant::now();
    while warm.elapsed() < Duration::from_secs(3) {
        terminal.tick();
        if terminal.redraw_needed(Instant::now()) {
            draw(&mut terminal, &mut painter, &mut atlas);
        }
        std::thread::sleep(terminal_gfx::presentation::IDLE_POLL);
    }
    drop(draw);
    let started = Instant::now();
    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let initial_rss = rss();
    let initial_bytes: u64 = terminal.panes.values().map(|p| p.session.bytes).sum();
    let mut rows = Vec::new();
    let mut phases = Vec::new();
    let mut commands = 0u64;
    let mut total_bytes = 0u64;
    let mut interactions = 0u64;
    let mut size = [1200, 800];
    let mut interaction_stage = 0;
    let mut echo_generation = 0;
    terminal.stats.record = true;
    let mut peak_rss = initial_rss;
    for (phase, duration) in [("idle", seconds / 2), ("busy", seconds - seconds / 2)] {
        let phase_start = Instant::now();
        let phase_cpu = cpu_seconds();
        let phase_frames = submissions;
        let mut sample_at = Instant::now();
        let mut first_sample = true;
        let mut command_at = Instant::now();
        let mut interact_at = Instant::now();
        while phase_start.elapsed() < Duration::from_secs(duration) {
            if phase == "busy" {
                if interaction_stage == 0
                    && interact_at.elapsed() >= Duration::from_secs(2)
                    && !terminal.paper_running()
                {
                    terminal.paper.on = false;
                    terminal.fit(&atlas, [size[0] as f32, size[1] as f32]);
                    let id = terminal.focus_id().unwrap();
                    let inner =
                        terminal_gfx::layout::inner(terminal.rect_of(id).unwrap(), terminal.cell);
                    let point = [
                        inner.x + terminal.cell[0] * 0.5,
                        inner.y + terminal.cell[1] * 0.5,
                    ];
                    terminal.wheel(point, 5.0);
                    terminal.press(point);
                    let head = [point[0] + terminal.cell[0] * 4.0, point[1]];
                    terminal.pointer(head);
                    terminal.release(head);
                    interaction_stage = 1;
                } else if interaction_stage == 1 {
                    terminal.enter_copy(false);
                    terminal.copy.as_mut().unwrap().cursor.col = 2;
                    interaction_stage = 2;
                } else if interaction_stage == 2 {
                    terminal.copy = None;
                    echo_generation = terminal.focused_generation().unwrap();
                    terminal.key(&KeyIn {
                        code: KeyCode::KeyX,
                        logical: winit::keyboard::Key::Character("x".into()),
                        text: Some("x".into()),
                        plain: Some("x".into()),
                        pressed: true,
                        repeat: false,
                        synthetic: false,
                    });
                    terminal.key(&KeyIn {
                        code: KeyCode::Backspace,
                        logical: winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace),
                        text: None,
                        plain: None,
                        pressed: true,
                        repeat: false,
                        synthetic: false,
                    });
                    terminal.scroll_focused(-5.0);
                    interaction_stage = 3;
                } else if interaction_stage == 3
                    && terminal.focused_generation() != Some(echo_generation)
                {
                    size = if size[0] == 1200 {
                        [1000, 700]
                    } else {
                        [1200, 800]
                    };
                    (view, depth) = targets(&device, size);
                    terminal.fit(&atlas, [size[0] as f32, size[1] as f32]);
                    terminal.invalidate();
                    terminal.paper.on = true;
                    interactions += 1;
                    interact_at = Instant::now();
                    interaction_stage = 0;
                }
            }
            if phase == "busy"
                && terminal.paper.on
                && command_at.elapsed() >= Duration::from_millis(50)
                && !terminal.paper_running()
            {
                // Shell input and command hooks exercise the real history/transcript owners.
                terminal.paste(&format!("for i in {{1..100}}; do printf 'soak {commands} %s output output output output output output\\n' $i; done"));
                terminal.key(&KeyIn {
                    code: KeyCode::F5,
                    logical: winit::keyboard::Key::Named(winit::keyboard::NamedKey::F5),
                    text: None,
                    plain: None,
                    pressed: true,
                    repeat: false,
                    synthetic: false,
                });
                commands += 1;
                command_at = Instant::now();
            }
            terminal.tick();
            if terminal.redraw_needed(Instant::now()) {
                let frame_start = Instant::now();
                let mut batch = UiBatch::default();
                terminal.draw(&mut batch, &mut atlas, [size[0] as f32, size[1] as f32]);
                painter
                    .draw(&device, &queue, size, &view, &depth, &batch, &atlas)
                    .unwrap();
                terminal.frame_done(frame_start);
                terminal.core.paper.presented();
                terminal.presented(Instant::now());
                submissions += 1;
            }
            if sample_at.elapsed() >= Duration::from_secs(30) || first_sample {
                let resident = rss();
                let disk = free_disk(root.path());
                peak_rss = peak_rss.max(resident);
                let bytes: u64 = terminal.panes.values().map(|p| p.session.bytes).sum();
                total_bytes = total_bytes.max(bytes);
                rows.push(serde_json::json!({"phase":phase,"elapsed_s":started.elapsed().as_secs_f64(),"rss_bytes":resident,"free_disk_bytes":disk,"cpu_seconds":cpu_seconds(),"submissions":submissions,"parsed_bytes":bytes,"commands":commands,"interaction_cycles":interactions,"transcript_entries":terminal.paper.entries.len(),"transcript_bytes":terminal.paper.retained_bytes(),"history_entries":terminal.paper.history.len(),"history_bytes":terminal.paper.history.iter().map(String::len).sum::<usize>(),"stats_frame_samples":terminal.stats.frames.len(),"stats_key_echo_samples":terminal.stats.latencies.len(),"scrollback_lines":terminal.panes.values().map(|p|p.session.vt.scrollback_len()).sum::<usize>(),"shell_blocks":terminal.panes.values().map(|p|p.session.blocks.records.len()).sum::<usize>(),"latency_samples":terminal.paper.latencies.len(),"atlas_bytes":atlas.pixels.len(),"vertex_buffer_bytes":painter.capacity,"in_flight_submissions":painter.in_flight.len()}));
                std::fs::write(&out, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
                assert!(disk >= 25_000_000_000, "Free disk fell below 25 GB");
                assert!(
                    resident <= RSS_BUDGET,
                    "RSS budget exceeded at {:.1}s: {} bytes",
                    started.elapsed().as_secs_f64(),
                    resident
                );
                assert!(
                    resident.saturating_sub(initial_rss) <= GROWTH_BUDGET,
                    "Memory growth exceeded at {:.1}s: {} bytes",
                    started.elapsed().as_secs_f64(),
                    resident.saturating_sub(initial_rss)
                );
                first_sample = false;
                sample_at = Instant::now();
            }
            std::thread::sleep(if phase == "idle" {
                terminal_gfx::presentation::IDLE_POLL
            } else {
                terminal_gfx::presentation::ACTIVE_POLL
            });
        }
        let resident = rss();
        let bytes: u64 = terminal.panes.values().map(|p| p.session.bytes).sum();
        total_bytes = total_bytes.max(bytes);
        peak_rss = peak_rss.max(resident);
        phases.push(serde_json::json!({"phase":phase,"seconds":phase_start.elapsed().as_secs_f64(),"cpu_percent":100.0*(cpu_seconds()-phase_cpu)/phase_start.elapsed().as_secs_f64(),"submissions":submissions-phase_frames,"final_rss_bytes":resident,"parsed_bytes":bytes,"commands":commands,"transcript_entries":terminal.paper.entries.len(),"history_entries":terminal.paper.history.len(),"stats_key_echo_samples":terminal.stats.latencies.len()}));
    }
    terminal.shutdown();
    let report = serde_json::json!({"schema":"openagents.native-terminal.retention-soak.v1","full_hour":seconds==3600,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"started_at_ms":started_at_ms,"adapter":format!("{info:?}"),"source_commit":option_env!("OPENAGENTS_BUILD_COMMIT"),"duration_s":started.elapsed().as_secs_f64(),"initial_rss_bytes":initial_rss,"peak_rss_bytes":peak_rss,"growth_bytes":peak_rss.saturating_sub(initial_rss),"commands":commands,"interaction_cycles":interactions,"parsed_bytes":total_bytes,"busy_parsed_bytes":total_bytes.saturating_sub(initial_bytes),"submissions":submissions,"budgets":{"rss_bytes":RSS_BUDGET,"growth_bytes":GROWTH_BUDGET,"idle_cpu_percent":IDLE_CPU_PERCENT,"busy_cpu_percent":BUSY_CPU_PERCENT,"idle_submissions":0},"phases":phases,"samples":rows,"peak_in_flight_submissions":painter.peak_in_flight,"scope":"Actual native UI pipeline and isolated shell PTY, raster only, offscreen. No WindowServer or physical display measurement."});
    std::fs::write(&out, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    assert!(painter.peak_in_flight <= MAX_IN_FLIGHT);
    assert!(peak_rss <= RSS_BUDGET, "RSS budget exceeded: {report}");
    assert!(
        peak_rss.saturating_sub(initial_rss) <= GROWTH_BUDGET,
        "Memory growth exceeded: {report}"
    );
    assert_eq!(
        report["phases"][0]["submissions"], 0,
        "Idle presents no frames"
    );
    assert!(
        report["phases"][0]["cpu_percent"].as_f64().unwrap() <= IDLE_CPU_PERCENT,
        "Idle CPU exceeded: {report}"
    );
    assert!(
        report["phases"][1]["cpu_percent"].as_f64().unwrap() <= BUSY_CPU_PERCENT,
        "Busy CPU exceeded: {report}"
    );
    assert!(
        commands > 0 && total_bytes > initial_bytes,
        "The busy phase must exercise PTY output"
    );
    if seconds >= 10 {
        assert!(
            interactions > 0 && terminal.stats.latencies.len() > 0,
            "Busy work must render real key echoes and interactions"
        );
    }
    if seconds == 3600 {
        assert!(
            commands > (terminal_core::paper::MAX_ENTRIES * 2) as u64,
            "Busy commands must cross retention budgets"
        );
    }
}
