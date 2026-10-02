//! Offline acceptance against the window's actual GPU layer and native views.
use crate::shell::DesktopApp;
use openagents_desktop::{
    chrome,
    grid::{Grid, Layer},
    model::Intent,
};
use rust_native_desktop::{
    App, Frame, Rect,
    backdrop::{Backdrop, FORMAT, Gpu},
    input::NativeInput,
    wgpu,
};
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires a GPU; run with OPENAGENTS_GRID_EVIDENCE to retain captures"]
fn playable_grid_gpu_and_native_views_at_both_sizes_and_scales() {
    let evidence = std::env::var_os("OPENAGENTS_GRID_EVIDENCE").map(std::path::PathBuf::from);
    if let Some(dir) = &evidence {
        std::fs::create_dir_all(dir).unwrap();
    }
    let home = tempfile::tempdir().unwrap();
    let start = Instant::now();
    let (mut app, _) = DesktopApp::performance_fixture(0, 0, start);
    let grid = Grid::new("ws://127.0.0.1:1".into(), home.path().into(), true);
    app.set_grid(grid.clone());
    app.activate(
        Intent::Navigate {
            action: chrome::Action::Grid,
        },
        start,
    );
    let watcher: openagents_desktop::grid::Watcher = Box::new(|| {
        openagents_desktop::backdrop::GridBackdrop::new("ws://127.0.0.1:1", Box::new(|| true))
    });
    let mut layer = Layer::new(grid.clone(), Some(watcher));
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let gpu = Gpu {
        adapter: &adapter,
        device: &device,
        queue: &queue,
    };
    let mut timings = Vec::new();
    for mode in ["watch", "play", "evals", "gym", "results"] {
        if mode == "play" {
            app.activate(Intent::Grid { key: "play".into() }, start);
            let deadline = Instant::now() + Duration::from_secs(5);
            while grid.borrow().surface.is_none() && Instant::now() < deadline {
                app.tick(Instant::now());
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(grid.borrow().surface.is_some());
        } else if mode == "evals" {
            grid.borrow_mut()
                .surface
                .as_mut()
                .unwrap()
                .command(coder_mobile::verse_surface::Command::GoEvals)
                .unwrap();
            app.tick(Instant::now());
        } else if matches!(mode, "gym" | "results") {
            use coder_mobile::verse_surface::{Command, Panel};
            grid.borrow_mut()
                .surface
                .as_mut()
                .unwrap()
                .command(Command::Close)
                .unwrap();
            app.tick(Instant::now());
            let rect = grid.borrow().rect;
            app.native_input(
                NativeInput::Button {
                    button: 1,
                    pressed: true,
                    x: rect.x + 10.0,
                    y: rect.y + 10.0,
                },
                Instant::now(),
            );
            app.native_input(
                NativeInput::Key {
                    code: "KeyD",
                    pressed: true,
                    repeat: false,
                    command: false,
                    alt: false,
                    control: false,
                    logo: false,
                },
                Instant::now(),
            );
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let near = {
                    let state = grid.borrow();
                    let world = state.surface.as_ref().unwrap().world();
                    if mode == "gym" {
                        world.gym(1.0).near
                    } else {
                        world.results(1.0).near
                    }
                };
                if near {
                    break;
                }
                assert!(Instant::now() < deadline, "walking reaches {mode}");
                render(&mut layer, &gpu, (900, 600), Instant::now());
                std::thread::sleep(Duration::from_millis(16));
            }
            app.native_input(
                NativeInput::Key {
                    code: "KeyD",
                    pressed: false,
                    repeat: false,
                    command: false,
                    alt: false,
                    control: false,
                    logo: false,
                },
                Instant::now(),
            );
            app.native_input(
                NativeInput::Button {
                    button: 1,
                    pressed: false,
                    x: rect.x + 10.0,
                    y: rect.y + 10.0,
                },
                Instant::now(),
            );
            grid.borrow_mut()
                .surface
                .as_mut()
                .unwrap()
                .command(Command::Open(if mode == "gym" {
                    Panel::Gym
                } else {
                    Panel::Results
                }))
                .unwrap();
            let settle = Instant::now() + Duration::from_secs(2);
            while Instant::now() < settle {
                render(&mut layer, &gpu, (900, 600), Instant::now());
                app.tick(Instant::now());
                std::thread::sleep(Duration::from_millis(16));
            }
            // The open board is laid over the world (#10116).
            let (_, scene) = rust_native_desktop::capture_views(&mut app, 1280.0, 800.0, 1.0);
            assert!(scene.bounds.contains_key("grid-board"));
            if mode == "results" {
                let state = grid.borrow();
                let view = state.surface.as_ref().unwrap().results().unwrap();
                assert!(view.error.is_none(), "{:?}", view.error);
                assert!(matches!(
                    view.page,
                    Some(gym_leaderboard::view::Page::Boards(_))
                ));
            }
        }
        // 1280×800, 1920×1080, and a narrow window (#10116), then the
        // Verse in full screen.
        let mut sizes = vec![
            ("", 1280.0, 800.0, 1.0),
            ("", 1280.0, 800.0, 2.0),
            ("", 1920.0, 1080.0, 1.0),
            ("", 760.0, 540.0, 1.0),
            ("", 760.0, 540.0, 2.0),
        ];
        if matches!(mode, "watch" | "play") {
            sizes.push(("full-", 1280.0, 800.0, 1.0));
            sizes.push(("full-", 1920.0, 1080.0, 1.0));
        }
        for (prefix, width, height, scale) in sizes {
            if prefix == "full-" && !grid.borrow().full {
                app.activate(Intent::Grid { key: "full".into() }, start);
                assert!(grid.borrow().full);
            }
            {
                let (views, scene) =
                    rust_native_desktop::capture_views(&mut app, width, height, scale);
                assert!(scene.unsupported.is_empty());
                let rect = layer
                    .surface()
                    .map(|name| {
                        scene
                            .backdrop_rect(name)
                            .expect("the Verse page is laid out")
                    })
                    .unwrap_or(Rect {
                        x: 0.0,
                        y: 0.0,
                        w: width,
                        h: height,
                    });
                layer.next_frame(Instant::now());
                layer.viewport(rect, scale);
                let size = (
                    (rect.w * scale).round() as u32,
                    (rect.h * scale).round() as u32,
                );
                let world = render(&mut layer, &gpu, size, Instant::now());
                let out = composite(&views, &world, rect, scale, app.theme().background, 0.0);
                if let Some(dir) = &evidence {
                    std::fs::write(
                        dir.join(format!(
                            "{prefix}{mode}-{}x{}-{}x.png",
                            width as u32, height as u32, scale as u32
                        )),
                        out.png().unwrap(),
                    )
                    .unwrap();
                }
            }
        }
        if grid.borrow().full {
            app.activate(Intent::Grid { key: "full".into() }, start);
            assert!(!grid.borrow().full);
        }
        if mode == "play" {
            let rect = grid.borrow().rect;
            let before = grid.borrow().surface.as_ref().unwrap().world().player.pos;
            app.native_input(
                NativeInput::Button {
                    button: 1,
                    pressed: true,
                    x: rect.x + 10.0,
                    y: rect.y + 10.0,
                },
                start,
            );
            let key = NativeInput::Key {
                code: "KeyW",
                pressed: true,
                repeat: false,
                command: false,
                alt: false,
                control: false,
                logo: false,
            };
            let began = Instant::now();
            app.native_input(key, start);
            let input_us = began.elapsed().as_micros();
            for _ in 1..=90 {
                app.native_input(NativeInput::Motion { dx: 0.2, dy: 0.0 }, start);
                let began = Instant::now();
                let mut encoder = device.create_command_encoder(&Default::default());
                let target = texture(&device, (900, 600));
                layer
                    .draw(
                        &gpu,
                        &mut encoder,
                        &target.create_view(&Default::default()),
                        (900, 600),
                        Instant::now(),
                    )
                    .unwrap();
                queue.submit([encoder.finish()]);
                device
                    .poll(wgpu::PollType::Wait {
                        submission_index: None,
                        timeout: Some(Duration::from_secs(10)),
                    })
                    .unwrap();
                timings.push(began.elapsed().as_micros());
                std::thread::sleep(Duration::from_millis(16));
            }
            assert!(
                grid.borrow()
                    .surface
                    .as_ref()
                    .unwrap()
                    .world()
                    .player
                    .pos
                    .distance(before)
                    > 1.0
            );
            app.native_input(NativeInput::Focus(false), start);
            assert!(!app.cursor_capture());
            app.native_input(NativeInput::Focus(true), start);
            timings.sort_unstable();
            let report = serde_json::json!({ "adapter": adapter.get_info().name, "samples": timings.len(), "frame_median_us": timings[timings.len()/2], "frame_p95_us": timings[timings.len()*95/100], "input_us": input_us, "scope": "offline 900x600 GPU submission and completion; native views captured separately" });
            println!("{report}");
            if let Some(dir) = &evidence {
                std::fs::write(
                    dir.join("timings.json"),
                    serde_json::to_vec_pretty(&report).unwrap(),
                )
                .unwrap();
            }
        }
    }
    app.activate(
        Intent::Grid {
            key: "watch".into(),
        },
        start,
    );
    assert!(grid.borrow().surface.is_none());
    assert!(!home.path().join(".openagents").exists());
}

/// The Episode 289 deck's title slide over the live Grid, as the window
/// composites it: the slide viewer's views over the real GPU layer, which
/// tours the plaza behind the slide. Two frames twenty seconds apart show
/// the camera moving.
#[test]
#[ignore = "requires a GPU; run with OPENAGENTS_GRID_EVIDENCE to retain captures"]
fn episode_289_title_slide_over_the_grid() {
    let evidence = std::env::var_os("OPENAGENTS_GRID_EVIDENCE").map(std::path::PathBuf::from);
    if let Some(dir) = &evidence {
        std::fs::create_dir_all(dir).unwrap();
    }
    let home = tempfile::tempdir().unwrap();
    let start = Instant::now();
    let (mut app, _) = DesktopApp::performance_fixture(0, 0, start);
    let grid = Grid::new("ws://127.0.0.1:1".into(), home.path().into(), true);
    app.set_grid(grid.clone());
    app.open_presentation("episode-289", start).unwrap();
    let open = start + Duration::from_millis(400);
    app.tick(open);
    let watcher: openagents_desktop::grid::Watcher = Box::new(|| {
        openagents_desktop::backdrop::GridBackdrop::new("ws://127.0.0.1:1", Box::new(|| false))
    });
    let mut layer = Layer::new(grid.clone(), Some(watcher));
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let gpu = Gpu {
        adapter: &adapter,
        device: &device,
        queue: &queue,
    };
    let (width, height, scale) = (1280.0, 800.0, 2.0);
    let (views, _) = rust_native_desktop::capture_views(&mut app, width, height, scale);
    let rect = layer.rect().expect("the title slide asks for the Grid");
    assert!(rect.w > 600.0 && rect.h > 300.0, "{rect:?}");
    let first = layer
        .next_frame(open)
        .expect("the layer draws the slide's Grid");
    let look = layer.look().unwrap();
    assert!(look.dim < 0.5, "{look:?}");
    layer.viewport(rect, scale);
    let size = (
        (rect.w * scale).round() as u32,
        (rect.h * scale).round() as u32,
    );
    let mut frames = Vec::new();
    for (name, at) in [("a", first), ("b", first + Duration::from_secs(20))] {
        let world = render(&mut layer, &gpu, size, at);
        let out = composite(
            &views,
            &world,
            rect,
            scale,
            app.theme().background,
            look.dim,
        );
        if let Some(dir) = &evidence {
            std::fs::write(
                dir.join(format!("episode-289-title-{name}.png")),
                out.png().unwrap(),
            )
            .unwrap();
        }
        frames.push(world);
    }
    let moved = frames[0]
        .pixels
        .iter()
        .zip(&frames[1].pixels)
        .filter(|(a, b)| a.abs_diff(**b) > 24)
        .count();
    assert!(
        moved > frames[0].pixels.len() / 50,
        "the camera moved: {moved}"
    );
    // The next slide has no scene: the layer lets go of the slide's place.
    app.text_input(
        rust_native_desktop::input::TextInput::Key {
            key: "ArrowRight",
            text: None,
            control: false,
            command: false,
            alt: false,
            shift: false,
        },
        open,
    );
    assert!(layer.rect().is_none());
}

fn texture(device: &wgpu::Device, size: (u32, u32)) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Grid acceptance"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn render(layer: &mut Layer, gpu: &Gpu<'_>, size: (u32, u32), now: Instant) -> Frame {
    let target = texture(gpu.device, size);
    let row = (size.0 * 4).div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row) * u64::from(size.1),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    layer
        .draw(
            gpu,
            &mut encoder,
            &target.create_view(&Default::default()),
            size,
            now,
        )
        .unwrap();
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(size.1),
            },
        },
        target.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
        .unwrap();
    let mapped = slice.get_mapped_range();
    let mut frame = Frame::transparent(size.0 as usize, size.1 as usize);
    for y in 0..size.1 as usize {
        frame.pixels[y * size.0 as usize * 4..(y + 1) * size.0 as usize * 4]
            .copy_from_slice(&mapped[y * row as usize..y * row as usize + size.0 as usize * 4]);
    }
    drop(mapped);
    buffer.unmap();
    frame
}

fn composite(
    views: &Frame,
    world: &Frame,
    rect: Rect,
    scale: f32,
    background: rust_native::style::Color,
    dim: f32,
) -> Frame {
    let mut out = Frame::new(views.width, views.height, background);
    for y in 0..out.height {
        for x in 0..out.width {
            let local = (x as f32 / scale - rect.x, y as f32 / scale - rect.y);
            let inside = local.0 >= 0.0 && local.1 >= 0.0 && local.0 < rect.w && local.1 < rect.h;
            let under = if inside {
                world.pixel(
                    ((local.0 / rect.w * world.width as f32) as usize).min(world.width - 1),
                    ((local.1 / rect.h * world.height as f32) as usize).min(world.height - 1),
                )
            } else {
                [background.red, background.green, background.blue]
            };
            let at = (y * out.width + x) * 4;
            let pixel = rust_native_desktop::backdrop::composite_pixel(
                under,
                background,
                dim,
                views.pixels[at..at + 4].try_into().unwrap(),
            );
            out.pixels[at..at + 3].copy_from_slice(&pixel);
        }
    }
    out
}
