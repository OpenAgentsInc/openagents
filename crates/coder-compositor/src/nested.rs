//! The nested backend: one window inside another compositor.
//!
//! `cargo run -p coder-compositor` inside a session opens a window in it,
//! and that window is a Wayland compositor with a socket of its own. It is
//! the development loop for everything else in the crate, and it stays
//! beside the hardware backend in `udev.rs`, because a compositor you can
//! restart without leaving the session is worth keeping.
//!
//! The window is one screen, named `nested-1`. Its scale is 1 until a
//! `scale` request changes it, which the nested backend answers the way the
//! hardware backend does, so `presentation-mode` can be tried here.

use std::cell::RefCell;
use std::os::unix::fs::MetadataExt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use smithay::backend::egl::{EGLDevice, EGLDisplay};
use smithay::backend::renderer::ImportDma;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::utils::draw_render_elements;
use smithay::backend::renderer::{Color32F, Frame, Renderer};
use smithay::backend::winit::{self, WinitEvent};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::{Display, ListeningSocket};
use smithay::reexports::winit::platform::pump_events::PumpStatus;
use smithay::reexports::winit::window::Window as WinitWindow;
use smithay::utils::{Rectangle, Transform};
use smithay::wayland::dmabuf::DmabufFeedbackBuilder;

use crate::drive;
use crate::exec::Session;
use crate::input;
use crate::layout;
use crate::render::{self, Element};
use crate::screencopy;
use crate::state::{ClientState, Coder, Graphics, Parts};
use coder_desk::serve as desk;

/// The name this compositor's screen answers to in the desk protocol.
const SCREEN_NAME: &str = "nested-1";

/// How long the loop sleeps when it has nothing to do. A shorter sleep
/// burns a core; a longer one shows in the pointer.
const IDLE: Duration = Duration::from_millis(4);

/// How often the compositor draws a frame.
const FRAME: Duration = Duration::from_millis(16);

/// The transform the compositor renders through.
///
/// The nested window's framebuffer counts its rows from the bottom, which
/// this flip about the same axis cancels. It is how the compositor draws
/// and not what the screen is, so the output tells clients it has no
/// transform: a recorder that read this one off `wl_output` would flip
/// every frame it captured.
const RENDER_TRANSFORM: Transform = Transform::Flipped180;

/// Runs the compositor in a window of the session this process started in.
pub fn run() -> Result<(), String> {
    let mut display: Display<Coder> = Display::new().map_err(|err| format!("display: {err}"))?;
    let handle = display.handle();
    let attributes = WinitWindow::default_attributes()
        .with_title("Coder compositor")
        .with_inner_size(smithay::reexports::winit::dpi::LogicalSize::new(
            1280.0, 800.0,
        ));
    let (mut backend, mut winit_loop) = winit::init_from_attributes::<GlesRenderer>(attributes)
        .map_err(|err| format!("the nested window did not open: {}", with_causes(&err)))?;
    // The compositor draws the pointer itself, the cursor a client asks for
    // included, so the session's pointer stays out of the window.
    backend.window().set_cursor_visible(false);

    let size = backend.window_size();
    let socket = ListeningSocket::bind_auto("wayland", 1..32)
        .map_err(|err| format!("the compositor's socket: {err}"))?;
    let socket_name = socket
        .socket_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or("the compositor's socket has no name")?;
    let desk_server = desk::bind()?;
    let session = Session::read(socket_name.clone(), desk_server.path());

    // A client that draws through Vulkan asks for its buffers over
    // `zwp_linux_dmabuf_v1`, and gets no surface when the compositor
    // advertises none. Version 4 carries the feedback, which names the
    // device a client should allocate on; `wf-recorder` binds that version
    // and disconnects with a protocol error when the compositor announces
    // less. A host whose EGL display names no device falls back to version
    // 3, which carries the renderer's formats and no device.
    let formats: Vec<_> = backend.renderer().dmabuf_formats().into_iter().collect();
    let node = render_device(backend.renderer().egl_context().display());
    let feedback = node.and_then(|device| {
        DmabufFeedbackBuilder::new(device, formats.clone())
            .build()
            .map_err(|err| log::warn!("the dmabuf feedback was not built: {err}"))
            .ok()
    });

    // The idle notifier holds one timer for each timeout a client asked
    // about, and a timer needs a loop to fire on. The nested backend polls
    // its inputs rather than waiting on this loop, so the loop is
    // dispatched with no timeout once a pass and runs the timers and the
    // Xwayland sources: the server's readiness and its X11 events. The
    // hardware backend in `udev.rs` reads every input through its loop.
    let mut events: EventLoop<'static, Coder> =
        EventLoop::try_new().map_err(|err| format!("the timer loop: {err}"))?;

    let mut state = Coder::new(Parts {
        display: handle.clone(),
        events: events.handle(),
        graphics: Graphics::Winit(Rc::new(RefCell::new(backend))),
        seat: "nested".to_string(),
        session,
    })?;
    state.dmabuf_global = Some(match &feedback {
        Some(feedback) => state
            .dmabuf_state
            .create_global_with_default_feedback::<Coder>(&handle, feedback),
        None => state.dmabuf_state.create_global::<Coder>(&handle, formats),
    });

    let output = Output::new(
        SCREEN_NAME.to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "OpenAgents".into(),
            model: "Coder compositor".into(),
        },
    );
    let _output_global = output.create_global::<Coder>(&handle);
    state.add_output(
        output,
        Mode {
            size,
            refresh: 60_000,
        },
        1.0,
    );

    // Every program the compositor starts reads the session from these two,
    // and so does every program one of those starts.
    //
    // SAFETY: the only other thread this process runs now is the desk
    // socket's reader, which reads the socket and never the environment,
    // and no program has started yet.
    unsafe {
        std::env::set_var("WAYLAND_DISPLAY", &socket_name);
        std::env::set_var(
            coder_desk::protocol::SOCKET_VAR,
            desk_server.path().as_os_str(),
        );
    }
    log::info!(
        "the compositor listens on {socket_name}, and its desk socket is {}",
        desk_server.path().display()
    );

    let mut clients = Vec::new();
    let mut last_frame = Instant::now() - FRAME;
    while state.running {
        let status = winit_loop.dispatch_new_events(|event| on_winit(&mut state, event));
        if let PumpStatus::Exit(_) = status {
            break;
        }
        while let Ok(call) = desk_server.calls.try_recv() {
            // A `shot` leaves the dispatch and waits for the frame that
            // fills it, so the caller hears once the file is written.
            let Some(call) = drive::take_shot(&mut state, call) else {
                continue;
            };
            let answer = desk::answer(call.request.clone(), &mut state);
            call.answer(answer);
        }
        state.hands_pass();
        while let Ok(Some(stream)) = socket.accept() {
            match handle
                .clone()
                .insert_client(stream, Arc::new(ClientState::default()))
            {
                Ok(client) => clients.push(client),
                Err(err) => log::warn!("a client was refused: {err}"),
            }
        }
        if last_frame.elapsed() >= FRAME {
            last_frame = Instant::now();
            if let Err(err) = draw(&mut state) {
                log::warn!("{err}");
            }
        }
        display
            .dispatch_clients(&mut state)
            .map_err(|err| format!("dispatch: {err}"))?;
        if let Err(err) = events.dispatch(Some(Duration::ZERO), &mut state) {
            log::warn!("a timer did not run: {err}");
        }
        state.space.refresh();
        state.popups.cleanup();
        display
            .flush_clients()
            .map_err(|err| format!("flush: {err}"))?;
        std::thread::sleep(IDLE);
    }
    Ok(())
}

/// One error and every error under it.
///
/// The error a window failure carries says `Failed to initialize an event
/// loop` and nothing about why, and the reason is two errors down: a host
/// that cannot load the Wayland client library reads the same as one with
/// no session.
fn with_causes(err: &dyn std::error::Error) -> String {
    let mut line = err.to_string();
    let mut under = err.source();
    while let Some(cause) = under {
        line.push_str(&format!(": {cause}"));
        under = cause.source();
    }
    line
}

/// The device the renderer allocates on, as the dmabuf feedback names it.
///
/// The feedback names a device by the number the kernel gives its node, so
/// the compositor asks EGL which node the display runs on and reads that
/// number off the file.
fn render_device(display: &EGLDisplay) -> Option<u64> {
    let device = EGLDevice::device_for_display(display)
        .map_err(|err| log::warn!("the EGL display names no device: {err}"))
        .ok()?;
    let path = device
        .drm_device_path()
        .map_err(|err| log::warn!("the EGL device names no node: {err}"))
        .ok()?;
    let node = std::fs::metadata(&path)
        .map_err(|err| log::warn!("the node {} did not read: {err}", path.display()))
        .ok()?;
    log::info!("the renderer allocates on {}", path.display());
    Some(node.rdev())
}

/// Draws one frame: the desk's background, each window's border, the
/// windows themselves, and the pointer.
fn draw(state: &mut Coder) -> Result<(), String> {
    let Graphics::Winit(held) = &state.graphics else {
        return Ok(());
    };
    let held = held.clone();
    let mut backend = held.borrow_mut();
    let Some(output) = state.output_named(SCREEN_NAME).cloned() else {
        return Ok(());
    };
    let size = backend.window_size();
    let damage = Rectangle::from_size(size);
    let scale = output.current_scale().fractional_scale();
    let overlay = state.overlay();

    let (renderer, mut framebuffer) = backend
        .bind()
        .map_err(|err| format!("the frame did not bind: {err}"))?;
    // Every element the space returns draws: a window arrives as an
    // element and a layer surface as a surface, and dropping either kind
    // leaves a hole on the screen. A surface that carries alpha blends with
    // what is under it, which is what the camera circle and a transparent
    // terminal need.
    let elements: Vec<Element<GlesRenderer>> =
        render::output_elements(renderer, &state.space, &output, &overlay, &mut state.decor)?;
    let mut frame = renderer
        .render(&mut framebuffer, size, RENDER_TRANSFORM)
        .map_err(|err| format!("the frame did not start: {err}"))?;
    frame
        .clear(Color32F::from(layout::BACKGROUND), &[damage])
        .map_err(|err| format!("the frame did not clear: {err}"))?;
    draw_render_elements(&mut frame, scale, &elements, &[damage])
        .map_err(|err| format!("a window did not draw: {err}"))?;
    let _sync = frame
        .finish()
        .map_err(|err| format!("the frame did not finish: {err}"))?;
    // The copies a screencopy client asked for read the framebuffer the
    // compositor just drew, before it reaches the screen, so what the
    // client saves is what the next refresh shows. Handing them over waits
    // until after the swap, because a read maps through the renderer's own
    // context and the swap needs the window's.
    let waiting = state.screencopy.take_for(SCREEN_NAME);
    let mode = layout::Screen {
        width: size.w,
        height: size.h,
    };
    let captured = screencopy::capture(waiting, renderer, &framebuffer, mode);
    // A `shot` reads the same framebuffer, and is written after the swap
    // for the same reason a client's copy is handed over after it.
    let shots = state.drive.take_shots(SCREEN_NAME);
    let read = (!shots.is_empty()).then(|| screencopy::read_screen(renderer, &framebuffer, mode));
    drop(framebuffer);
    backend
        .submit(Some(&[damage]))
        .map_err(|err| format!("the frame did not reach the screen: {err}"))?;
    if !captured.is_empty() {
        screencopy::deliver(
            captured,
            backend.renderer(),
            state.started.elapsed(),
            screencopy::Rows::BottomUp,
        );
    }
    if let Some(read) = read {
        let pixels = read.and_then(|mapping| screencopy::mapped(backend.renderer(), &mapping));
        drive::write_shots(shots, &pixels, mode, screencopy::Rows::BottomUp);
    }
    drop(backend);
    state.send_frames(&output);
    Ok(())
}

/// What one event from the nested window does.
fn on_winit(state: &mut Coder, event: WinitEvent) {
    match event {
        WinitEvent::Resized { size, .. } => {
            state.resize_output(
                SCREEN_NAME,
                Mode {
                    size,
                    refresh: 60_000,
                },
            );
        }
        WinitEvent::CloseRequested => state.running = false,
        WinitEvent::Input(event) => input::on_input(state, event),
        _ => {}
    }
}
