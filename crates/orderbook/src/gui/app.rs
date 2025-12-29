//! Main orderbook application with winit integration

use super::colors::orderbook as ob_colors;
use super::state::{GuiState, RelayStatus};
use crate::parser::{parse_order_lenient, P2P_ORDER_KIND};
use crate::state::OrderbookState;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem, theme};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// Event from relay tasks to GUI
pub enum RelayEvent {
    Connected(String),
    Disconnected(String),
    Error(String, String),
    OrderReceived,
}

/// Main orderbook application
pub struct OrderbookApp {
    state: Option<RenderState>,
    orderbook: Arc<RwLock<OrderbookState>>,
    relays: Vec<String>,
    event_rx: Option<mpsc::UnboundedReceiver<RelayEvent>>,
    runtime: Option<Runtime>,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    gui: GuiState,
    last_frame: Instant,
}

impl OrderbookApp {
    pub fn new(orderbook: Arc<RwLock<OrderbookState>>, relays: Vec<String>) -> Self {
        Self {
            state: None,
            orderbook,
            relays,
            event_rx: None,
            runtime: None,
        }
    }

    fn start_relay_tasks(&mut self, event_tx: mpsc::UnboundedSender<RelayEvent>) {
        let orderbook = Arc::clone(&self.orderbook);
        let relays = self.relays.clone();

        // Create tokio runtime for async relay tasks
        let runtime = Runtime::new().expect("Failed to create tokio runtime");

        runtime.spawn(async move {
            use nostr_client::RelayConnection;

            let filter = json!({
                "kinds": [P2P_ORDER_KIND]
            });

            for relay_url in relays {
                let orderbook = Arc::clone(&orderbook);
                let event_tx = event_tx.clone();
                let filter = filter.clone();

                tokio::spawn(async move {
                    let relay = match RelayConnection::new(&relay_url) {
                        Ok(r) => r,
                        Err(e) => {
                            let _ = event_tx.send(RelayEvent::Error(
                                relay_url.clone(),
                                e.to_string(),
                            ));
                            return;
                        }
                    };

                    if let Err(e) = relay.connect().await {
                        let _ = event_tx.send(RelayEvent::Error(relay_url.clone(), e.to_string()));
                        return;
                    }

                    let _ = event_tx.send(RelayEvent::Connected(relay_url.clone()));

                    let rx = match relay
                        .subscribe_with_channel("nip69-orders", &[filter])
                        .await
                    {
                        Ok(rx) => rx,
                        Err(e) => {
                            let _ =
                                event_tx.send(RelayEvent::Error(relay_url.clone(), e.to_string()));
                            return;
                        }
                    };

                    let mut rx = rx;
                    while let Some(event) = rx.recv().await {
                        let order = parse_order_lenient(&event, &relay_url);
                        let mut state = orderbook.write().await;
                        state.process_order(order);
                        let _ = event_tx.send(RelayEvent::OrderReceived);
                    }

                    let _ = event_tx.send(RelayEvent::Disconnected(relay_url));
                });
            }
        });

        self.runtime = Some(runtime);
    }
}

impl ApplicationHandler for OrderbookApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("NIP-69 Orderbook - Bloomberg Style")
            .with_inner_size(winit::dpi::LogicalSize::new(1400, 900));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance
                .create_surface(window.clone())
                .expect("Failed to create surface");

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Failed to create device");

            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let renderer = Renderer::new(&device, surface_format);
            let scale_factor = window.scale_factor() as f32;
            let text_system = TextSystem::new(scale_factor);

            let orderbook = self.orderbook.clone();
            let mut gui = GuiState::new(orderbook);

            // Initialize relay status
            for relay_url in &self.relays {
                gui.set_relay_status(relay_url, RelayStatus::Connecting);
            }

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                gui,
                last_frame: Instant::now(),
            }
        });

        self.state = Some(state);

        // Start relay connection tasks
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        self.event_rx = Some(event_rx);
        self.start_relay_tasks(event_tx);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                        PhysicalKey::Code(KeyCode::Tab) => {
                            // Cycle through markets
                            let current = state.gui.selected_index().unwrap_or(0);
                            let next = (current + 1) % state.gui.markets.len().max(1);
                            state.gui.select_market(next);
                        }
                        PhysicalKey::Code(KeyCode::Digit1) => state.gui.select_market(0),
                        PhysicalKey::Code(KeyCode::Digit2) => state.gui.select_market(1),
                        PhysicalKey::Code(KeyCode::Digit3) => state.gui.select_market(2),
                        PhysicalKey::Code(KeyCode::Digit4) => state.gui.select_market(3),
                        PhysicalKey::Code(KeyCode::Digit5) => state.gui.select_market(4),
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                // Process relay events
                if let Some(rx) = &mut self.event_rx {
                    while let Ok(event) = rx.try_recv() {
                        match event {
                            RelayEvent::Connected(url) => {
                                state.gui.set_relay_status(&url, RelayStatus::Connected);
                            }
                            RelayEvent::Disconnected(url) => {
                                state.gui.set_relay_status(&url, RelayStatus::Disconnected);
                            }
                            RelayEvent::Error(url, err) => {
                                state.gui.set_relay_status(&url, RelayStatus::Error(err));
                            }
                            RelayEvent::OrderReceived => {
                                state.gui.event_count += 1;
                                state.gui.last_event_time = Instant::now();
                            }
                        }
                    }
                }

                // Update markets from orderbook state
                let markets = {
                    if let Ok(orderbook) = state.gui.orderbook.try_read() {
                        Some(orderbook.get_markets())
                    } else {
                        None
                    }
                };
                if let Some(markets) = markets {
                    state.gui.update_markets(markets);
                }

                let width = state.config.width as f32;
                let height = state.config.height as f32;
                let _delta = state.last_frame.elapsed();
                state.last_frame = Instant::now();

                let mut scene = Scene::new();
                render_orderbook(&mut scene, &mut state.text_system, &state.gui, width, height);

                let output = state
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encoder"),
                        });

                state
                    .renderer
                    .resize(&state.queue, Size::new(width, height), 1.0);

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                state.renderer.prepare(&state.device, &scene);
                state.renderer.render(&mut encoder, &view);

                state.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

/// Render the complete orderbook interface
fn render_orderbook(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gui: &GuiState,
    width: f32,
    height: f32,
) {
    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    // Header bar (40px)
    render_header(scene, text_system, gui, width);

    // Divider line
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 40.0, width, 1.0))
            .with_background(theme::accent::PRIMARY.with_alpha(0.5)),
    );

    // Main content area
    let content_y = 42.0;
    let content_height = height - 42.0 - 180.0; // Reserve 180px for feed

    // Market tabs (left sidebar, 180px)
    render_market_tabs(scene, text_system, gui, 180.0, content_y, content_height);

    // Orderbook panel (center)
    let orderbook_x = 182.0;
    let orderbook_width = width - 182.0;
    render_orderbook_panel(
        scene,
        text_system,
        gui,
        orderbook_x,
        content_y,
        orderbook_width,
        content_height,
    );

    // Event feed (bottom, 180px)
    let feed_y = height - 180.0;
    render_event_feed(scene, text_system, gui, feed_y, width, 180.0);
}

/// Render header bar with connection status
fn render_header(scene: &mut Scene, text_system: &mut TextSystem, gui: &GuiState, width: f32) {
    // Header background
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, 40.0)).with_background(theme::bg::SURFACE),
    );

    // Title
    let title = "NIP-69 ORDERBOOK";
    let title_run = text_system.layout(title, Point::new(16.0, 14.0), 16.0, ob_colors::HIGHLIGHT);
    scene.draw_text(title_run);

    // Connection status
    let status_text = format!(
        "{}/{} relays | {} events",
        gui.connected_count, gui.relay_count, gui.event_count
    );
    let status_color = if gui.connected_count > 0 {
        ob_colors::BID
    } else {
        ob_colors::ASK
    };
    let status_run = text_system.layout(&status_text, Point::new(200.0, 14.0), 12.0, status_color);
    scene.draw_text(status_run);

    // Selected market indicator
    if let Some(market) = &gui.selected_market {
        let market_text = market.to_string();
        let market_run =
            text_system.layout(&market_text, Point::new(width - 200.0, 14.0), 14.0, theme::text::PRIMARY);
        scene.draw_text(market_run);
    }

    // Keyboard hints
    let hints = "[Tab] Switch Market  [1-5] Select  [Esc] Exit";
    let hints_run =
        text_system.layout(hints, Point::new(width - 380.0, 26.0), 10.0, theme::text::MUTED);
    scene.draw_text(hints_run);
}

/// Render market tabs sidebar
fn render_market_tabs(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gui: &GuiState,
    _tab_width: f32,
    y: f32,
    height: f32,
) {
    // Sidebar background
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, y, 180.0, height)).with_background(theme::bg::SURFACE),
    );

    // Section title
    let title = "MARKETS";
    let title_run = text_system.layout(title, Point::new(12.0, y + 12.0), 11.0, theme::text::MUTED);
    scene.draw_text(title_run);

    // Market tabs
    let selected_idx = gui.selected_index();
    for (i, market) in gui.markets.iter().enumerate() {
        let tab_y = y + 35.0 + (i as f32 * 32.0);
        let is_selected = selected_idx == Some(i);

        // Selection highlight
        if is_selected {
            // Yellow left accent bar
            scene.draw_quad(
                Quad::new(Bounds::new(0.0, tab_y, 3.0, 28.0)).with_background(ob_colors::HIGHLIGHT),
            );
            // Background highlight
            scene.draw_quad(
                Quad::new(Bounds::new(3.0, tab_y, 177.0, 28.0))
                    .with_background(theme::bg::HOVER.with_alpha(0.5)),
            );
        }

        // Market name
        let color = if is_selected {
            ob_colors::HIGHLIGHT
        } else {
            theme::text::PRIMARY
        };
        let market_text = market.to_string();
        let market_run = text_system.layout(&market_text, Point::new(12.0, tab_y + 8.0), 12.0, color);
        scene.draw_text(market_run);

        // Order count for this market (try to get from orderbook)
        if let Ok(orderbook) = gui.orderbook.try_read() {
            let count = orderbook.get_orders_by_market(market).len();
            let count_text = format!("{}", count);
            let count_run = text_system.layout(
                &count_text,
                Point::new(155.0, tab_y + 8.0),
                11.0,
                theme::text::MUTED,
            );
            scene.draw_text(count_run);
        }
    }

    // Show placeholder if no markets
    if gui.markets.is_empty() {
        let placeholder = "Waiting for events...";
        let placeholder_run =
            text_system.layout(placeholder, Point::new(12.0, y + 50.0), 11.0, theme::text::MUTED);
        scene.draw_text(placeholder_run);
    }
}

/// Render the main orderbook panel
fn render_orderbook_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gui: &GuiState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    let half_width = width / 2.0 - 2.0;

    // Bids header (left)
    let bids_label = "BIDS";
    let bids_run = text_system.layout(bids_label, Point::new(x + 12.0, y + 10.0), 13.0, ob_colors::BID);
    scene.draw_text(bids_run);

    // Asks header (right)
    let asks_label = "ASKS";
    let asks_run =
        text_system.layout(asks_label, Point::new(x + half_width + 12.0, y + 10.0), 13.0, ob_colors::ASK);
    scene.draw_text(asks_run);

    // Column headers
    let col_y = y + 30.0;
    let headers = ["Premium", "Amount", "Methods"];
    let header_color = theme::text::MUTED;

    // Bid column headers
    text_system.layout(&headers[0].to_string(), Point::new(x + 12.0, col_y), 10.0, header_color);
    text_system.layout(&headers[1].to_string(), Point::new(x + 80.0, col_y), 10.0, header_color);
    text_system.layout(&headers[2].to_string(), Point::new(x + 160.0, col_y), 10.0, header_color);

    // Ask column headers
    text_system.layout(
        &headers[0].to_string(),
        Point::new(x + half_width + 12.0, col_y),
        10.0,
        header_color,
    );
    text_system.layout(
        &headers[1].to_string(),
        Point::new(x + half_width + 80.0, col_y),
        10.0,
        header_color,
    );
    text_system.layout(
        &headers[2].to_string(),
        Point::new(x + half_width + 160.0, col_y),
        10.0,
        header_color,
    );

    // Vertical divider
    scene.draw_quad(
        Quad::new(Bounds::new(x + half_width - 1.0, y, 2.0, height))
            .with_background(theme::bg::HOVER),
    );

    // Render orders if we have a selected market
    if let Some(market) = &gui.selected_market {
        if let Ok(orderbook) = gui.orderbook.try_read() {
            let orders = orderbook.get_orders_by_market(market);

            // Separate bids and asks
            let mut bids: Vec<_> = orders.iter().filter(|o| o.is_buy()).collect();
            let mut asks: Vec<_> = orders.iter().filter(|o| o.is_sell()).collect();

            // Sort bids by premium descending (best first)
            bids.sort_by(|a, b| {
                b.premium
                    .partial_cmp(&a.premium)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Sort asks by premium ascending (best first)
            asks.sort_by(|a, b| {
                a.premium
                    .partial_cmp(&b.premium)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Find max amount for depth bar scaling
            let max_amount = orders
                .iter()
                .filter_map(|o| o.amount_sats)
                .max()
                .unwrap_or(1) as f32;

            // Render bids
            let row_height = 24.0;
            let start_y = y + 48.0;
            let max_rows = ((height - 60.0) / row_height) as usize;

            for (i, order) in bids.iter().take(max_rows).enumerate() {
                let row_y = start_y + (i as f32 * row_height);
                render_order_row(
                    scene,
                    text_system,
                    order,
                    x,
                    row_y,
                    half_width - 4.0,
                    row_height,
                    true,
                    i == 0,
                    max_amount,
                );
            }

            // Render asks
            for (i, order) in asks.iter().take(max_rows).enumerate() {
                let row_y = start_y + (i as f32 * row_height);
                render_order_row(
                    scene,
                    text_system,
                    order,
                    x + half_width + 2.0,
                    row_y,
                    half_width - 4.0,
                    row_height,
                    false,
                    i == 0,
                    max_amount,
                );
            }

            // Show spread in center
            if let (Some(best_bid), Some(best_ask)) = (
                bids.first().and_then(|o| o.premium),
                asks.first().and_then(|o| o.premium),
            ) {
                let spread = best_ask - best_bid;
                let spread_text = format!("Spread: {:.1}%", spread);
                let spread_run = text_system.layout(
                    &spread_text,
                    Point::new(x + half_width - 40.0, y + height - 20.0),
                    11.0,
                    theme::text::MUTED,
                );
                scene.draw_text(spread_run);
            }
        }
    }
}

/// Render a single order row
fn render_order_row(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    order: &crate::parser::ParsedOrder,
    x: f32,
    y: f32,
    width: f32,
    _height: f32,
    is_bid: bool,
    is_best: bool,
    max_amount: f32,
) {
    let color = if is_bid { ob_colors::BID } else { ob_colors::ASK };
    let depth_color = if is_bid {
        ob_colors::BID_DEPTH
    } else {
        ob_colors::ASK_DEPTH
    };

    // Depth bar
    let amount = order.amount_sats.unwrap_or(0) as f32;
    let depth_width = (amount / max_amount) * (width - 8.0);
    scene.draw_quad(
        Quad::new(Bounds::new(x + 4.0, y + 2.0, depth_width.max(0.0), 20.0))
            .with_background(depth_color),
    );

    // Highlight best bid/ask
    let text_color = if is_best { ob_colors::HIGHLIGHT } else { color };

    // Premium
    let premium_text = order.premium_display();
    let premium_run = text_system.layout(&premium_text, Point::new(x + 12.0, y + 4.0), 11.0, text_color);
    scene.draw_text(premium_run);

    // Amount
    let amount_text = format_sats(order.amount_sats.unwrap_or(0));
    let amount_run = text_system.layout(&amount_text, Point::new(x + 80.0, y + 4.0), 11.0, theme::text::PRIMARY);
    scene.draw_text(amount_run);

    // Payment methods (truncated)
    let methods = order.payment_methods.join(", ");
    let methods_display = if methods.len() > 20 {
        format!("{}...", &methods.chars().take(18).collect::<String>())
    } else {
        methods
    };
    let methods_run = text_system.layout(&methods_display, Point::new(x + 160.0, y + 4.0), 10.0, theme::text::MUTED);
    scene.draw_text(methods_run);
}

/// Render the event feed panel
fn render_event_feed(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    gui: &GuiState,
    y: f32,
    width: f32,
    height: f32,
) {
    // Feed background
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, y, width, height)).with_background(Hsla::new(0.0, 0.0, 0.05, 0.95)),
    );

    // Top border
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, y, width, 1.0))
            .with_background(theme::accent::PRIMARY.with_alpha(0.3)),
    );

    // Title
    let title = "EVENT FEED";
    let title_run = text_system.layout(title, Point::new(12.0, y + 10.0), 11.0, theme::text::MUTED);
    scene.draw_text(title_run);

    // Event rows
    if let Ok(orderbook) = gui.orderbook.try_read() {
        let feed = orderbook.get_raw_feed(10);
        let row_height = 16.0;
        let start_y = y + 30.0;

        for (i, order) in feed.iter().enumerate() {
            let row_y = start_y + (i as f32 * row_height);

            // Alternate row background
            if i % 2 == 1 {
                scene.draw_quad(
                    Quad::new(Bounds::new(0.0, row_y, width, row_height))
                        .with_background(Hsla::new(0.0, 0.0, 0.08, 0.5)),
                );
            }

            // Side indicator
            let side_color = if order.is_buy() {
                ob_colors::BID
            } else {
                ob_colors::ASK
            };
            let side_text = if order.is_buy() { "BUY " } else { "SELL" };
            let side_run = text_system.layout(side_text, Point::new(12.0, row_y + 1.0), 10.0, side_color);
            scene.draw_text(side_run);

            // Currency
            let currency = order.currency.as_deref().unwrap_or("?");
            let currency_run =
                text_system.layout(currency, Point::new(60.0, row_y + 1.0), 10.0, theme::text::PRIMARY);
            scene.draw_text(currency_run);

            // Amount
            let amount_text = format_sats(order.amount_sats.unwrap_or(0));
            let amount_run =
                text_system.layout(&amount_text, Point::new(110.0, row_y + 1.0), 10.0, theme::text::PRIMARY);
            scene.draw_text(amount_run);

            // Fiat
            let fiat_text = order.fiat_display();
            let fiat_run =
                text_system.layout(&fiat_text, Point::new(180.0, row_y + 1.0), 10.0, theme::text::MUTED);
            scene.draw_text(fiat_run);

            // Premium
            let premium_text = order.premium_display();
            let premium_run =
                text_system.layout(&premium_text, Point::new(250.0, row_y + 1.0), 10.0, theme::text::PRIMARY);
            scene.draw_text(premium_run);

            // Platform
            let platform = order.platform.as_deref().unwrap_or("?");
            let platform_run =
                text_system.layout(platform, Point::new(320.0, row_y + 1.0), 10.0, theme::text::MUTED);
            scene.draw_text(platform_run);

            // Relay (shortened)
            let relay = order
                .relay_url
                .trim_start_matches("wss://")
                .chars()
                .take(20)
                .collect::<String>();
            let relay_run =
                text_system.layout(&relay, Point::new(420.0, row_y + 1.0), 10.0, theme::text::MUTED);
            scene.draw_text(relay_run);
        }
    }
}

/// Format satoshi amount for display
fn format_sats(sats: u64) -> String {
    if sats == 0 {
        "Range".to_string()
    } else if sats >= 1_000_000 {
        format!("{:.2}M", sats as f64 / 1_000_000.0)
    } else if sats >= 1_000 {
        format!("{}k", sats / 1_000)
    } else {
        format!("{}", sats)
    }
}
