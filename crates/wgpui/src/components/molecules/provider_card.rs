//! Provider card molecule for displaying compute providers.
//!
//! Shows compute provider name, specs, availability, and pricing.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Provider availability status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProviderStatus {
    Online,
    Busy,
    Offline,
    Maintenance,
}

impl ProviderStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ProviderStatus::Online => "Online",
            ProviderStatus::Busy => "Busy",
            ProviderStatus::Offline => "Offline",
            ProviderStatus::Maintenance => "Maintenance",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            ProviderStatus::Online => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            ProviderStatus::Busy => Hsla::new(45.0, 0.7, 0.5, 1.0),     // Yellow
            ProviderStatus::Offline => Hsla::new(0.0, 0.0, 0.5, 1.0),   // Gray
            ProviderStatus::Maintenance => Hsla::new(200.0, 0.6, 0.5, 1.0), // Blue
        }
    }
}

/// Provider specs
#[derive(Debug, Clone)]
pub struct ProviderSpecs {
    pub cpu_cores: u32,
    pub ram_gb: u32,
    pub gpu: Option<String>,
    pub storage_gb: u32,
}

impl ProviderSpecs {
    pub fn new(cpu_cores: u32, ram_gb: u32, storage_gb: u32) -> Self {
        Self {
            cpu_cores,
            ram_gb,
            gpu: None,
            storage_gb,
        }
    }

    pub fn gpu(mut self, gpu: impl Into<String>) -> Self {
        self.gpu = Some(gpu.into());
        self
    }
}

/// Provider info
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub status: ProviderStatus,
    pub specs: ProviderSpecs,
    pub price_per_hour_sats: u64,
    pub rating: f32,
    pub jobs_completed: u32,
    pub location: Option<String>,
}

impl ProviderInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>, specs: ProviderSpecs) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            status: ProviderStatus::Online,
            specs,
            price_per_hour_sats: 1000,
            rating: 0.0,
            jobs_completed: 0,
            location: None,
        }
    }

    pub fn status(mut self, status: ProviderStatus) -> Self {
        self.status = status;
        self
    }

    pub fn price(mut self, sats_per_hour: u64) -> Self {
        self.price_per_hour_sats = sats_per_hour;
        self
    }

    pub fn rating(mut self, rating: f32) -> Self {
        self.rating = rating;
        self
    }

    pub fn jobs(mut self, count: u32) -> Self {
        self.jobs_completed = count;
        self
    }

    pub fn location(mut self, loc: impl Into<String>) -> Self {
        self.location = Some(loc.into());
        self
    }
}

/// Provider card component
pub struct ProviderCard {
    id: Option<ComponentId>,
    provider: ProviderInfo,
    hovered: bool,
    on_select: Option<Box<dyn FnMut(String)>>,
}

impl ProviderCard {
    pub fn new(provider: ProviderInfo) -> Self {
        Self {
            id: None,
            provider,
            hovered: false,
            on_select: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_select<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_select = Some(Box::new(f));
        self
    }

    fn format_price(&self) -> String {
        let sats = self.provider.price_per_hour_sats;
        if sats >= 1000 {
            format!("{}K sats/hr", sats / 1000)
        } else {
            format!("{} sats/hr", sats)
        }
    }
}

impl Component for ProviderCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;

        // Background
        let bg = if self.hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let mut y = bounds.origin.y + padding;

        // Provider name and status
        let name_run = cx.text.layout(
            &self.provider.name,
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(name_run);

        // Status badge
        let status_x = bounds.origin.x + bounds.size.width - padding - 70.0;
        let status_bounds = Bounds::new(status_x, y - 2.0, 65.0, 18.0);
        cx.scene.draw_quad(
            Quad::new(status_bounds)
                .with_background(self.provider.status.color().with_alpha(0.2))
                .with_border(self.provider.status.color(), 1.0),
        );
        let status_run = cx.text.layout(
            self.provider.status.label(),
            Point::new(status_x + 6.0, y),
            theme::font_size::XS,
            self.provider.status.color(),
        );
        cx.scene.draw_text(status_run);

        y += 24.0;

        // Specs row
        let specs = &self.provider.specs;
        let specs_text = format!(
            "{} CPU | {} GB RAM | {} GB Storage",
            specs.cpu_cores, specs.ram_gb, specs.storage_gb
        );
        let specs_run = cx.text.layout(
            &specs_text,
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(specs_run);

        y += 18.0;

        // GPU if present
        if let Some(gpu) = &specs.gpu {
            let gpu_text = format!("GPU: {}", gpu);
            let gpu_run = cx.text.layout(
                &gpu_text,
                Point::new(bounds.origin.x + padding, y),
                theme::font_size::XS,
                Hsla::new(280.0, 0.6, 0.55, 1.0), // Purple for GPU
            );
            cx.scene.draw_text(gpu_run);
            y += 18.0;
        }

        y += 4.0;

        // Price
        let price_color = Hsla::new(35.0, 0.9, 0.5, 1.0); // Bitcoin orange
        let price_run = cx.text.layout(
            &self.format_price(),
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::SM,
            price_color,
        );
        cx.scene.draw_text(price_run);

        // Rating
        let rating_text = format!("\u{2605} {:.1}", self.provider.rating);
        let rating_run = cx.text.layout(
            &rating_text,
            Point::new(bounds.origin.x + padding + 100.0, y),
            theme::font_size::XS,
            Hsla::new(45.0, 0.8, 0.5, 1.0), // Yellow for stars
        );
        cx.scene.draw_text(rating_run);

        // Jobs completed
        let jobs_text = format!("{} jobs", self.provider.jobs_completed);
        let jobs_run = cx.text.layout(
            &jobs_text,
            Point::new(bounds.origin.x + padding + 170.0, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(jobs_run);

        // Location
        if let Some(location) = &self.provider.location {
            let loc_run = cx.text.layout(
                location,
                Point::new(bounds.origin.x + bounds.size.width - padding - 80.0, y),
                theme::font_size::XS,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(loc_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    if let Some(callback) = &mut self.on_select {
                        callback(self.provider.id.clone());
                    }
                    return EventResult::Handled;
                }
            }
            _ => {}
        }
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let height = if self.provider.specs.gpu.is_some() {
            110.0
        } else {
            95.0
        };
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_info() {
        let specs = ProviderSpecs::new(8, 32, 500).gpu("NVIDIA RTX 4090");
        let provider = ProviderInfo::new("p1", "FastCompute", specs)
            .status(ProviderStatus::Online)
            .price(5000)
            .rating(4.8)
            .jobs(142);

        assert_eq!(provider.name, "FastCompute");
        assert_eq!(provider.rating, 4.8);
    }

    #[test]
    fn test_status_colors() {
        assert_eq!(ProviderStatus::Online.label(), "Online");
        assert_eq!(ProviderStatus::Busy.label(), "Busy");
    }

    #[test]
    fn test_format_price() {
        let specs = ProviderSpecs::new(4, 16, 100);
        let provider = ProviderInfo::new("p1", "Test", specs).price(5000);
        let card = ProviderCard::new(provider);
        assert_eq!(card.format_price(), "5K sats/hr");
    }
}
