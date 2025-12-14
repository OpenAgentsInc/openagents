//! Application core - manages the main application lifecycle.

use crate::state::AppState;
use coder_domain::ids::ThreadId;
use coder_domain::message::Role;
use coder_domain::{ChatEntry, ChatView, MessageView, StreamingMessage};
use coder_shell::{Chrome, Navigation, Route, ViewRegistry};
use coder_surfaces_chat::ChatThread;
use coder_ui_runtime::{CommandBus, Scheduler, Signal};
use coder_widgets::context::{EventContext, PaintContext};
use coder_widgets::{EventResult, Widget};
use mechacoder::{ClientMessage, ServerMessage};
use tokio::sync::mpsc;
use wgpui::{Bounds, InputEvent, Scene};

/// The main application.
pub struct App {
    /// Application state.
    state: AppState,

    /// Navigation controller.
    navigation: Navigation,

    /// View registry.
    views: ViewRegistry,

    /// Application chrome.
    chrome: Chrome,

    /// Frame scheduler.
    scheduler: Scheduler,

    /// Command bus.
    commands: CommandBus,

    /// Current window size.
    window_size: (f32, f32),

    /// Scale factor for high-DPI.
    scale_factor: f32,

    /// Chat thread widget.
    chat_thread: ChatThread,

    /// Chat view state (reactive).
    chat_view: Signal<ChatView>,

    /// Channel to send messages to backend.
    client_tx: mpsc::UnboundedSender<ClientMessage>,

    /// Channel to receive messages from backend.
    server_rx: mpsc::UnboundedReceiver<ServerMessage>,

    /// Current working directory.
    cwd: String,
}

impl App {
    /// Create a new application with message channels.
    ///
    /// The channels are used to communicate with the background chat handler.
    pub fn new(
        client_tx: mpsc::UnboundedSender<ClientMessage>,
        server_rx: mpsc::UnboundedReceiver<ServerMessage>,
    ) -> Self {
        // Get current working directory
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        // Create chat view signal
        let thread_id = ThreadId::new();
        let chat_view = Signal::new(ChatView::new(thread_id));

        // Clone what we need for the on_send callback
        let tx = client_tx.clone();
        let cwd_clone = cwd.clone();
        let chat_view_clone = chat_view.clone();

        // Create chat thread widget with on_send callback
        let chat_thread = ChatThread::new(thread_id)
            .chat_view(chat_view.clone())
            .on_send(move |content: &str| {
                log::info!("[App] User sent message: {}", content);

                // Add user message to chat view
                let mut view = chat_view_clone.get();
                view.entries.push(ChatEntry::Message(MessageView {
                    id: coder_domain::ids::MessageId::new(),
                    content: content.to_string(),
                    role: Role::User,
                    timestamp: chrono::Utc::now(),
                    has_tool_uses: false,
                }));
                chat_view_clone.set(view);

                // Send to backend
                let _ = tx.send(ClientMessage::SendMessage {
                    content: content.to_string(),
                    cwd: cwd_clone.clone(),
                });
            });

        Self {
            state: AppState::new(),
            navigation: Navigation::new(),
            views: ViewRegistry::new(),
            chrome: Chrome::new().title("Coder"),
            scheduler: Scheduler::new(),
            commands: CommandBus::new(),
            window_size: (800.0, 600.0),
            scale_factor: 1.0,
            chat_thread,
            chat_view,
            client_tx,
            server_rx,
            cwd,
        }
    }

    /// Initialize the application.
    pub fn init(&mut self) {
        log::info!("Initializing Coder application");

        // Set up initial route
        self.navigation.navigate(Route::Home);

        // Update chrome with initial breadcrumbs
        let crumbs = self.navigation.breadcrumbs();
        self.chrome.set_breadcrumbs(crumbs);

        log::info!("Coder application initialized");
    }

    /// Set the window size.
    pub fn set_size(&mut self, width: f32, height: f32) {
        self.window_size = (width, height);
    }

    /// Get the window size.
    pub fn size(&self) -> (f32, f32) {
        self.window_size
    }

    /// Handle an input event.
    pub fn handle_event(&mut self, event: &InputEvent) -> EventResult {
        let bounds = Bounds::new(0.0, 0.0, self.window_size.0, self.window_size.1);

        // Create event context
        let mut cx = EventContext::new(&mut self.commands);

        // Let chat thread handle events (full screen, no chrome)
        self.chat_thread.event(event, bounds, &mut cx)
    }

    /// Run a frame update cycle.
    pub fn update(&mut self) {
        // Run the scheduler
        let _stats = self.scheduler.run_frame();

        // Poll for server messages (non-blocking)
        while let Ok(msg) = self.server_rx.try_recv() {
            self.handle_server_message(msg);
        }
    }

    /// Handle a message from the backend.
    fn handle_server_message(&mut self, msg: ServerMessage) {
        let mut view = self.chat_view.get();

        match msg {
            ServerMessage::TextDelta { text } => {
                // Update streaming message
                if let Some(streaming) = &mut view.streaming_message {
                    streaming.content_so_far.push_str(&text);
                } else {
                    view.streaming_message = Some(StreamingMessage {
                        id: coder_domain::ids::MessageId::new(),
                        content_so_far: text,
                        is_complete: false,
                        started_at: chrono::Utc::now(),
                    });
                }
                self.chat_view.set(view);
            }
            ServerMessage::Done { error } => {
                // Complete the streaming message
                if let Some(streaming) = view.streaming_message.take() {
                    if let Some(err) = error {
                        log::error!("[App] Backend error: {}", err);
                    }

                    // Add as completed message
                    view.entries.push(ChatEntry::Message(MessageView {
                        id: streaming.id,
                        content: streaming.content_so_far,
                        role: Role::Assistant,
                        timestamp: chrono::Utc::now(),
                        has_tool_uses: false,
                    }));
                }
                self.chat_view.set(view);
            }
            ServerMessage::SessionInit { session_id } => {
                log::info!("[App] Session initialized: {}", session_id);
            }
            ServerMessage::ToolStart { tool_use_id, tool_name } => {
                log::info!("[App] Tool started: {} ({})", tool_name, tool_use_id);
            }
            ServerMessage::ToolResult { tool_use_id, output, is_error } => {
                log::info!(
                    "[App] Tool result: {} (error={}): {}",
                    tool_use_id,
                    is_error,
                    output.chars().take(100).collect::<String>()
                );
            }
            ServerMessage::ToolInput { .. } | ServerMessage::ToolProgress { .. } => {
                // Ignore for now
            }
        }
    }

    /// Send a message to the backend.
    #[allow(dead_code)]
    pub fn send_message(&self, content: String) {
        // Add user message to chat view
        let mut view = self.chat_view.get();
        view.entries.push(ChatEntry::Message(MessageView {
            id: coder_domain::ids::MessageId::new(),
            content: content.clone(),
            role: Role::User,
            timestamp: chrono::Utc::now(),
            has_tool_uses: false,
        }));
        self.chat_view.set(view);

        // Send to backend
        let _ = self.client_tx.send(ClientMessage::SendMessage {
            content,
            cwd: self.cwd.clone(),
        });
    }

    /// Paint the application to a scene.
    pub fn paint(&mut self, scene: &mut Scene, text_system: &mut wgpui::TextSystem) {
        let bounds = Bounds::new(0.0, 0.0, self.window_size.0, self.window_size.1);

        // Create paint context
        let mut cx = PaintContext::new(scene, text_system, self.scale_factor);

        // Paint background
        cx.scene.draw_quad(
            wgpui::Quad::new(bounds).with_background(wgpui::theme::bg::APP),
        );

        // Paint chat thread (full screen, no chrome)
        self.chat_thread.paint(bounds, &mut cx);
    }

    /// Navigate to a route.
    pub fn navigate(&mut self, route: Route) {
        self.navigation.navigate(route.clone());

        // Update chrome breadcrumbs
        let crumbs = self.navigation.breadcrumbs();
        self.chrome.set_breadcrumbs(crumbs);

        // Activate view for route
        self.views.activate_for_route(&route);
    }

    /// Get the current route.
    pub fn current_route(&self) -> Route {
        self.navigation.current()
    }

    /// Get the navigation controller.
    pub fn navigation(&self) -> &Navigation {
        &self.navigation
    }

    /// Get mutable navigation controller.
    pub fn navigation_mut(&mut self) -> &mut Navigation {
        &mut self.navigation
    }

    /// Get the app state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get mutable app state.
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_app() -> App {
        let (client_tx, _client_rx) = mpsc::unbounded_channel();
        let (_server_tx, server_rx) = mpsc::unbounded_channel();
        App::new(client_tx, server_rx)
    }

    #[test]
    fn test_app_creation() {
        let app = create_test_app();
        assert_eq!(app.window_size, (800.0, 600.0));
    }

    #[test]
    fn test_app_set_size() {
        let mut app = create_test_app();
        app.set_size(1920.0, 1080.0);
        assert_eq!(app.size(), (1920.0, 1080.0));
    }

    #[test]
    fn test_app_init() {
        let mut app = create_test_app();
        app.init();
        assert!(app.current_route().is_home());
    }

    #[test]
    fn test_app_navigation() {
        let mut app = create_test_app();
        app.init();

        app.navigate(Route::Settings);
        assert_eq!(app.current_route(), Route::Settings);
    }
}
