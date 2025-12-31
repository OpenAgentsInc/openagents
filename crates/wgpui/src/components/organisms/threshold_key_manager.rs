//! Threshold key manager organism for FROSTR key management.
//!
//! Provides UI for managing threshold signature key shares, peers, and signing requests.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Key share information
#[derive(Debug, Clone)]
pub struct KeyShare {
    pub id: String,
    pub index: u32,
    pub threshold: u32,
    pub total_shares: u32,
    pub created_at: String,
    pub backed_up: bool,
}

impl KeyShare {
    pub fn new(id: impl Into<String>, index: u32, threshold: u32, total: u32) -> Self {
        Self {
            id: id.into(),
            index,
            threshold,
            total_shares: total,
            created_at: String::new(),
            backed_up: false,
        }
    }

    pub fn created_at(mut self, ts: impl Into<String>) -> Self {
        self.created_at = ts.into();
        self
    }

    pub fn backed_up(mut self, backed_up: bool) -> Self {
        self.backed_up = backed_up;
        self
    }
}

/// Peer status in the threshold group
#[derive(Debug, Clone)]
pub struct ThresholdPeer {
    pub npub: String,
    pub name: String,
    pub share_index: u32,
    pub status: PeerStatus,
    pub last_seen: String,
}

impl ThresholdPeer {
    pub fn new(npub: impl Into<String>, name: impl Into<String>, index: u32) -> Self {
        Self {
            npub: npub.into(),
            name: name.into(),
            share_index: index,
            status: PeerStatus::Offline,
            last_seen: String::new(),
        }
    }

    pub fn status(mut self, status: PeerStatus) -> Self {
        self.status = status;
        self
    }

    pub fn last_seen(mut self, ts: impl Into<String>) -> Self {
        self.last_seen = ts.into();
        self
    }
}

/// Peer connection status
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PeerStatus {
    Online,
    #[default]
    Offline,
    Signing,
}

/// Signing request
#[derive(Debug, Clone)]
pub struct SigningRequest {
    pub id: String,
    pub message_preview: String,
    pub requester: String,
    pub timestamp: String,
    pub signatures_collected: u32,
    pub signatures_required: u32,
}

impl SigningRequest {
    pub fn new(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            message_preview: message.into(),
            requester: String::new(),
            timestamp: String::new(),
            signatures_collected: 0,
            signatures_required: 0,
        }
    }

    pub fn requester(mut self, req: impl Into<String>) -> Self {
        self.requester = req.into();
        self
    }

    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = ts.into();
        self
    }

    pub fn progress(mut self, collected: u32, required: u32) -> Self {
        self.signatures_collected = collected;
        self.signatures_required = required;
        self
    }
}

/// Manager tab
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum KeyManagerTab {
    #[default]
    Overview,
    Peers,
    Requests,
    Backup,
}

/// Threshold key manager organism
pub struct ThresholdKeyManager {
    id: Option<ComponentId>,
    key_share: Option<KeyShare>,
    peers: Vec<ThresholdPeer>,
    requests: Vec<SigningRequest>,
    active_tab: KeyManagerTab,
    tab_hovered: Option<KeyManagerTab>,
    scroll_offset: f32,
    backup_button_hovered: bool,
    on_backup: Option<Box<dyn FnMut()>>,
    on_approve_request: Option<Box<dyn FnMut(String)>>,
    on_reject_request: Option<Box<dyn FnMut(String)>>,
}

impl ThresholdKeyManager {
    pub fn new() -> Self {
        Self {
            id: None,
            key_share: None,
            peers: Vec::new(),
            requests: Vec::new(),
            active_tab: KeyManagerTab::Overview,
            tab_hovered: None,
            scroll_offset: 0.0,
            backup_button_hovered: false,
            on_backup: None,
            on_approve_request: None,
            on_reject_request: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn key_share(mut self, share: KeyShare) -> Self {
        self.key_share = Some(share);
        self
    }

    pub fn peers(mut self, peers: Vec<ThresholdPeer>) -> Self {
        self.peers = peers;
        self
    }

    pub fn requests(mut self, requests: Vec<SigningRequest>) -> Self {
        self.requests = requests;
        self
    }

    pub fn on_backup<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_backup = Some(Box::new(f));
        self
    }

    pub fn on_approve<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_approve_request = Some(Box::new(f));
        self
    }

    pub fn on_reject<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_reject_request = Some(Box::new(f));
        self
    }

    fn header_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 50.0)
    }

    fn tabs_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 50.0,
            bounds.size.width,
            36.0,
        )
    }

    fn content_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 86.0,
            bounds.size.width,
            bounds.size.height - 86.0,
        )
    }

    fn tab_bounds(&self, bounds: &Bounds, tab: KeyManagerTab) -> Bounds {
        let tabs = self.tabs_bounds(bounds);
        let tab_width = tabs.size.width / 4.0;
        let idx = match tab {
            KeyManagerTab::Overview => 0,
            KeyManagerTab::Peers => 1,
            KeyManagerTab::Requests => 2,
            KeyManagerTab::Backup => 3,
        };
        Bounds::new(
            tabs.origin.x + idx as f32 * tab_width,
            tabs.origin.y,
            tab_width,
            36.0,
        )
    }

    fn tab_from_index(idx: usize) -> Option<KeyManagerTab> {
        match idx {
            0 => Some(KeyManagerTab::Overview),
            1 => Some(KeyManagerTab::Peers),
            2 => Some(KeyManagerTab::Requests),
            3 => Some(KeyManagerTab::Backup),
            _ => None,
        }
    }

    fn online_peers_count(&self) -> usize {
        self.peers
            .iter()
            .filter(|p| p.status == PeerStatus::Online || p.status == PeerStatus::Signing)
            .count()
    }

    fn paint_overview(&self, content: Bounds, cx: &mut PaintContext) {
        let padding = 16.0;
        let y = content.origin.y + padding;

        if let Some(share) = &self.key_share {
            // Key share info
            let key_label = cx.text.layout(
                "Key Share",
                Point::new(content.origin.x + padding, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(key_label);

            let key_id = format!("Share #{} of {}", share.index, share.total_shares);
            let key_run = cx.text.layout(
                &key_id,
                Point::new(content.origin.x + padding, y + 18.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(key_run);

            // Threshold indicator
            let thresh_y = y + 50.0;
            let thresh_label = cx.text.layout(
                "Threshold",
                Point::new(content.origin.x + padding, thresh_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(thresh_label);

            let thresh_value = format!("{} of {} required", share.threshold, share.total_shares);
            let thresh_run = cx.text.layout(
                &thresh_value,
                Point::new(content.origin.x + padding, thresh_y + 18.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(thresh_run);

            // Backup status
            let backup_y = y + 100.0;
            let backup_label = cx.text.layout(
                "Backup Status",
                Point::new(content.origin.x + padding, backup_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(backup_label);

            let (backup_text, backup_color) = if share.backed_up {
                ("Backed up", Hsla::new(120.0, 0.6, 0.45, 1.0))
            } else {
                ("Not backed up", Hsla::new(0.0, 0.7, 0.5, 1.0))
            };
            let backup_run = cx.text.layout(
                backup_text,
                Point::new(content.origin.x + padding, backup_y + 18.0),
                theme::font_size::SM,
                backup_color,
            );
            cx.scene.draw_text(backup_run);

            // Online peers
            let peers_y = y + 150.0;
            let peers_label = cx.text.layout(
                "Online Peers",
                Point::new(content.origin.x + padding, peers_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(peers_label);

            let online = self.online_peers_count();
            let peers_value = format!("{} / {}", online, self.peers.len());
            let peers_color = if online >= share.threshold as usize {
                Hsla::new(120.0, 0.6, 0.45, 1.0)
            } else {
                Hsla::new(45.0, 0.7, 0.5, 1.0)
            };
            let peers_run = cx.text.layout(
                &peers_value,
                Point::new(content.origin.x + padding, peers_y + 18.0),
                theme::font_size::SM,
                peers_color,
            );
            cx.scene.draw_text(peers_run);

            // Pending requests
            let req_y = y + 200.0;
            let req_label = cx.text.layout(
                "Pending Requests",
                Point::new(content.origin.x + padding, req_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(req_label);

            let req_value = format!("{}", self.requests.len());
            let req_run = cx.text.layout(
                &req_value,
                Point::new(content.origin.x + padding, req_y + 18.0),
                theme::font_size::SM,
                if self.requests.is_empty() {
                    theme::text::PRIMARY
                } else {
                    theme::accent::PRIMARY
                },
            );
            cx.scene.draw_text(req_run);
        } else {
            let empty_run = cx.text.layout(
                "No key share configured",
                Point::new(content.origin.x + padding, y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(empty_run);

            let hint_run = cx.text.layout(
                "Create or import a key share to get started",
                Point::new(content.origin.x + padding, y + 24.0),
                theme::font_size::XS,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(hint_run);
        }
    }

    fn paint_peers(&self, content: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;
        let mut y = content.origin.y + padding - self.scroll_offset;

        if self.peers.is_empty() {
            let empty_run = cx.text.layout(
                "No peers in threshold group",
                Point::new(content.origin.x + padding, y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(empty_run);
            return;
        }

        for peer in &self.peers {
            let row_height = 50.0;

            // Status indicator
            let status_color = match peer.status {
                PeerStatus::Online => Hsla::new(120.0, 0.6, 0.45, 1.0),
                PeerStatus::Offline => theme::text::DISABLED,
                PeerStatus::Signing => Hsla::new(200.0, 0.7, 0.5, 1.0),
            };
            let dot_bounds = Bounds::new(content.origin.x + padding, y + 18.0, 8.0, 8.0);
            cx.scene
                .draw_quad(Quad::new(dot_bounds).with_background(status_color));

            // Name
            let name_run = cx.text.layout(
                &peer.name,
                Point::new(content.origin.x + padding + 16.0, y + 8.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(name_run);

            // Share index
            let index_text = format!("Share #{}", peer.share_index);
            let index_run = cx.text.layout(
                &index_text,
                Point::new(content.origin.x + padding + 16.0, y + 26.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(index_run);

            // Status text
            let status_text = match peer.status {
                PeerStatus::Online => "Online",
                PeerStatus::Offline => "Offline",
                PeerStatus::Signing => "Signing...",
            };
            let status_run = cx.text.layout(
                status_text,
                Point::new(
                    content.origin.x + content.size.width - padding - 60.0,
                    y + 16.0,
                ),
                theme::font_size::XS,
                status_color,
            );
            cx.scene.draw_text(status_run);

            y += row_height;
        }
    }

    fn paint_requests(&self, content: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;
        let mut y = content.origin.y + padding - self.scroll_offset;

        if self.requests.is_empty() {
            let empty_run = cx.text.layout(
                "No pending signing requests",
                Point::new(content.origin.x + padding, y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(empty_run);
            return;
        }

        for request in &self.requests {
            let row_height = 70.0;

            // Message preview
            let preview = if request.message_preview.len() > 50 {
                format!("{}...", &request.message_preview[..47])
            } else {
                request.message_preview.clone()
            };
            let preview_run = cx.text.layout(
                &preview,
                Point::new(content.origin.x + padding, y + 8.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(preview_run);

            // Requester and timestamp
            let meta = format!("From: {} • {}", request.requester, request.timestamp);
            let meta_run = cx.text.layout(
                &meta,
                Point::new(content.origin.x + padding, y + 28.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(meta_run);

            // Signature progress
            let progress = format!(
                "{}/{} signatures",
                request.signatures_collected, request.signatures_required
            );
            let progress_run = cx.text.layout(
                &progress,
                Point::new(content.origin.x + padding, y + 46.0),
                theme::font_size::XS,
                theme::accent::PRIMARY,
            );
            cx.scene.draw_text(progress_run);

            y += row_height;
        }
    }

    fn paint_backup(&self, content: Bounds, cx: &mut PaintContext) {
        let padding = 16.0;
        let y = content.origin.y + padding;

        // Backup instructions
        let title_run = cx.text.layout(
            "Backup Your Key Share",
            Point::new(content.origin.x + padding, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        let desc_run = cx.text.layout(
            "Your key share is essential for threshold signing.",
            Point::new(content.origin.x + padding, y + 24.0),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(desc_run);

        let desc2_run = cx.text.layout(
            "Back it up securely and never share it.",
            Point::new(content.origin.x + padding, y + 40.0),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(desc2_run);

        // Warning
        let warn_y = y + 80.0;
        let warn_bounds = Bounds::new(
            content.origin.x + padding,
            warn_y,
            content.size.width - padding * 2.0,
            60.0,
        );
        cx.scene.draw_quad(
            Quad::new(warn_bounds)
                .with_background(Hsla::new(45.0, 0.7, 0.5, 0.1))
                .with_border(Hsla::new(45.0, 0.7, 0.5, 1.0), 1.0),
        );

        let warn_run = cx.text.layout(
            "\u{26A0} If you lose your key share and don't have",
            Point::new(content.origin.x + padding + 8.0, warn_y + 12.0),
            theme::font_size::XS,
            Hsla::new(45.0, 0.8, 0.5, 1.0),
        );
        cx.scene.draw_text(warn_run);

        let warn2_run = cx.text.layout(
            "threshold peers online, funds may be lost.",
            Point::new(content.origin.x + padding + 8.0, warn_y + 28.0),
            theme::font_size::XS,
            Hsla::new(45.0, 0.8, 0.5, 1.0),
        );
        cx.scene.draw_text(warn2_run);

        // Backup button
        let btn_y = y + 160.0;
        let btn_bounds = Bounds::new(content.origin.x + padding, btn_y, 140.0, 36.0);
        let btn_bg = if self.backup_button_hovered {
            theme::accent::PRIMARY.with_alpha(0.3)
        } else {
            theme::accent::PRIMARY.with_alpha(0.2)
        };
        cx.scene.draw_quad(
            Quad::new(btn_bounds)
                .with_background(btn_bg)
                .with_border(theme::accent::PRIMARY, 1.0),
        );
        let btn_run = cx.text.layout(
            "Export Backup",
            Point::new(btn_bounds.origin.x + 20.0, btn_bounds.origin.y + 10.0),
            theme::font_size::SM,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(btn_run);
    }

    fn backup_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let content = self.content_bounds(bounds);
        Bounds::new(
            content.origin.x + 16.0,
            content.origin.y + 176.0,
            140.0,
            36.0,
        )
    }
}

impl Default for ThresholdKeyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ThresholdKeyManager {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Header
        let header = self.header_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(header).with_background(theme::bg::MUTED));

        // Title
        let title_run = cx.text.layout(
            "FROSTR Keys",
            Point::new(bounds.origin.x + padding, bounds.origin.y + 10.0),
            theme::font_size::BASE,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        // Subtitle
        let sub_run = cx.text.layout(
            "Threshold Signature Management",
            Point::new(bounds.origin.x + padding, bounds.origin.y + 30.0),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(sub_run);

        // Tabs
        let tabs = self.tabs_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(tabs).with_background(theme::bg::APP));

        let tab_labels = ["Overview", "Peers", "Requests", "Backup"];
        for (i, label) in tab_labels.iter().enumerate() {
            let tab = Self::tab_from_index(i).unwrap();
            let tab_bounds = self.tab_bounds(&bounds, tab);

            let is_active = self.active_tab == tab;
            let is_hovered = self.tab_hovered == Some(tab);

            if is_active {
                cx.scene
                    .draw_quad(Quad::new(tab_bounds).with_background(theme::bg::SURFACE));
            } else if is_hovered {
                cx.scene
                    .draw_quad(Quad::new(tab_bounds).with_background(theme::bg::HOVER));
            }

            // Request count badge
            if tab == KeyManagerTab::Requests && !self.requests.is_empty() {
                let badge_bounds = Bounds::new(
                    tab_bounds.origin.x + tab_bounds.size.width - 24.0,
                    tab_bounds.origin.y + 8.0,
                    18.0,
                    18.0,
                );
                cx.scene.draw_quad(
                    Quad::new(badge_bounds).with_background(Hsla::new(0.0, 0.7, 0.5, 1.0)),
                );
                let badge_text = format!("{}", self.requests.len().min(9));
                let badge_run = cx.text.layout(
                    &badge_text,
                    Point::new(badge_bounds.origin.x + 5.0, badge_bounds.origin.y + 2.0),
                    theme::font_size::XS,
                    Hsla::new(0.0, 0.0, 1.0, 1.0),
                );
                cx.scene.draw_text(badge_run);
            }

            let text_color = if is_active {
                theme::accent::PRIMARY
            } else {
                theme::text::MUTED
            };
            let label_run = cx.text.layout(
                label,
                Point::new(tab_bounds.origin.x + 8.0, tab_bounds.origin.y + 10.0),
                theme::font_size::XS,
                text_color,
            );
            cx.scene.draw_text(label_run);
        }

        // Content
        let content = self.content_bounds(&bounds);
        cx.scene.push_clip(content);

        match self.active_tab {
            KeyManagerTab::Overview => self.paint_overview(content, cx),
            KeyManagerTab::Peers => self.paint_peers(content, cx),
            KeyManagerTab::Requests => self.paint_requests(content, cx),
            KeyManagerTab::Backup => self.paint_backup(content, cx),
        }

        cx.scene.pop_clip();
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let old_tab_hovered = self.tab_hovered;
                let old_backup_hovered = self.backup_button_hovered;

                self.tab_hovered = None;
                for i in 0..4 {
                    let tab = Self::tab_from_index(i).unwrap();
                    if self.tab_bounds(&bounds, tab).contains(point) {
                        self.tab_hovered = Some(tab);
                        break;
                    }
                }

                self.backup_button_hovered = self.active_tab == KeyManagerTab::Backup
                    && self.backup_button_bounds(&bounds).contains(point);

                if old_tab_hovered != self.tab_hovered
                    || old_backup_hovered != self.backup_button_hovered
                {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    // Tab clicks
                    for i in 0..4 {
                        let tab = Self::tab_from_index(i).unwrap();
                        if self.tab_bounds(&bounds, tab).contains(point) {
                            self.active_tab = tab;
                            self.scroll_offset = 0.0;
                            return EventResult::Handled;
                        }
                    }

                    // Backup button
                    if self.active_tab == KeyManagerTab::Backup
                        && self.backup_button_bounds(&bounds).contains(point)
                    {
                        if let Some(callback) = &mut self.on_backup {
                            callback();
                        }
                        return EventResult::Handled;
                    }
                }
            }
            InputEvent::Scroll { dy, .. } => {
                let content = self.content_bounds(&bounds);
                let content_height = match self.active_tab {
                    KeyManagerTab::Overview => 250.0,
                    KeyManagerTab::Peers => self.peers.len() as f32 * 50.0,
                    KeyManagerTab::Requests => self.requests.len() as f32 * 70.0,
                    KeyManagerTab::Backup => 220.0,
                };
                let max_scroll = (content_height - content.size.height).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy).clamp(0.0, max_scroll);
                return EventResult::Handled;
            }
            _ => {}
        }
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(400.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_key_manager() {
        let manager = ThresholdKeyManager::new()
            .key_share(KeyShare::new("key-1", 1, 2, 3).backed_up(true))
            .peers(vec![
                ThresholdPeer::new("npub1...", "Alice", 1).status(PeerStatus::Online),
                ThresholdPeer::new("npub2...", "Bob", 2).status(PeerStatus::Offline),
            ]);
        assert!(manager.key_share.is_some());
        assert_eq!(manager.peers.len(), 2);
    }

    #[test]
    fn test_online_peers_count() {
        let manager = ThresholdKeyManager::new().peers(vec![
            ThresholdPeer::new("npub1...", "Alice", 1).status(PeerStatus::Online),
            ThresholdPeer::new("npub2...", "Bob", 2).status(PeerStatus::Offline),
            ThresholdPeer::new("npub3...", "Carol", 3).status(PeerStatus::Signing),
        ]);
        assert_eq!(manager.online_peers_count(), 2);
    }

    #[test]
    fn test_signing_request() {
        let request = SigningRequest::new("req-1", "Sign this message")
            .requester("Alice")
            .progress(1, 2);
        assert_eq!(request.signatures_collected, 1);
        assert_eq!(request.signatures_required, 2);
    }
}
