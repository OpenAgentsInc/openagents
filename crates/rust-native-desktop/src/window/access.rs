//! The window's side of [`crate::access`]: the platform's accessibility
//! adapter (`accesskit_winit`: NSAccessibility on the Mac, AT-SPI on Linux,
//! UI Automation on Windows), the tree kept current while a screen reader
//! listens, and its requests run as the window's own input.

use super::Shell;
use crate::access::{Request, Tree};
use crate::{App, Waker};
use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

/// What the platform's handlers share with the window: they may run on any
/// thread, so they only record and wake the event loop.
struct Shared {
    active: AtomicBool,
    requests: Mutex<Vec<ActionRequest>>,
    wake: Waker,
}

impl Shared {
    fn wake(&self) {
        self.wake.wake();
    }
}

struct Handler(Arc<Shared>);

impl ActivationHandler for Handler {
    /// The first tree follows on the event loop, where the view lives.
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.0.active.store(true, Ordering::SeqCst);
        self.0.wake();
        None
    }
}

impl ActionHandler for Handler {
    fn do_action(&mut self, request: ActionRequest) {
        if let Ok(mut requests) = self.0.requests.lock() {
            // A screen reader sends one request at a time; a bound keeps a
            // stalled loop from growing this.
            if requests.len() < 64 {
                requests.push(request);
            }
        }
        self.0.wake();
    }
}

impl DeactivationHandler for Handler {
    fn deactivate_accessibility(&mut self) {
        self.0.active.store(false, Ordering::SeqCst);
    }
}

/// What the last tree was built from, so an unchanged window builds none.
#[derive(Clone, PartialEq)]
struct Built {
    laid_out: Option<super::LaidOut>,
    focus: Option<String>,
    scroll: u32,
    surfaces: Vec<Option<u64>>,
}

/// The platform adapter and the tree it was last given.
pub(super) struct Access {
    adapter: accesskit_winit::Adapter,
    shared: Arc<Shared>,
    tree: Option<Tree>,
    built: Option<Built>,
}

impl Access {
    /// Attaches to `window`, which must not have been shown yet.
    pub(super) fn new(event_loop: &ActiveEventLoop, window: &Window, wake: Waker) -> Access {
        let shared = Arc::new(Shared {
            active: AtomicBool::new(false),
            requests: Mutex::new(Vec::new()),
            wake,
        });
        let adapter = accesskit_winit::Adapter::with_direct_handlers(
            event_loop,
            window,
            Handler(shared.clone()),
            Handler(shared.clone()),
            Handler(shared.clone()),
        );
        Access {
            adapter,
            shared,
            tree: None,
            built: None,
        }
    }

    /// Lets the adapter see a window event before the window handles it.
    pub(super) fn process_event(&mut self, window: &Window, event: &WindowEvent) {
        self.adapter.process_event(window, event);
    }
}

impl<A: App> Shell<A> {
    /// Runs a screen reader's requests, then gives it the current tree
    /// when anything it shows changed. Nothing is built until a screen
    /// reader asks.
    pub(super) fn sync_access(&mut self) {
        let Some(access) = &mut self.access else {
            return;
        };
        if !access.shared.active.load(Ordering::SeqCst) {
            access.built = None;
            return;
        }
        let requests = access
            .shared
            .requests
            .lock()
            .map(|mut requests| std::mem::take(&mut *requests))
            .unwrap_or_default();
        let requests: Vec<Request> = requests
            .iter()
            .filter_map(|request| access.tree.as_ref()?.request(request))
            .collect();
        for request in requests {
            self.run_access(request);
        }
        if self.gpu.is_none() {
            return;
        }
        let scale = self.scale();
        self.scene();
        let (Some(scene), Some(access)) = (&self.scene, &mut self.access) else {
            return;
        };
        let built = Built {
            laid_out: self.laid_out.clone(),
            focus: self.app.access_focus(),
            scroll: self.scroll.to_bits(),
            surfaces: scene
                .ops
                .iter()
                .filter_map(|op| match op {
                    crate::layout::Op::Surface { resource, .. } => {
                        Some(self.app.surface_version(resource))
                    }
                    _ => None,
                })
                .collect(),
        };
        // A surface without a drawing revision (`None`) is read again only
        // when something else moved.
        if access.built.as_ref() == Some(&built) {
            return;
        }
        let tree = Tree::of(
            &self.app,
            scene,
            self.interaction.focus.as_deref(),
            // Window pixels, as the platforms take bounds.
            scale,
            self.scroll,
        );
        let update = tree.update.clone();
        access.adapter.update_if_active(|| update);
        access.tree = Some(tree);
        access.built = Some(built);
    }

    /// Answers a screen reader's request with the input it stands for.
    fn run_access(&mut self, request: Request) {
        let now = Instant::now();
        if let Some(key) = crate::access::answer(&mut self.app, request, now) {
            self.interaction.focus = Some(key);
        }
        self.app.input(now);
        self.tick();
        self.redraw();
    }
}
