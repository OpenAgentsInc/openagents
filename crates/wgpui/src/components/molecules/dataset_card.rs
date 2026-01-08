//! Dataset card molecule for displaying data marketplace items.
//!
//! Shows dataset name, description, size, format, and pricing.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Data format types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataFormat {
    Json,
    Csv,
    Parquet,
    Arrow,
    Sqlite,
    Custom,
}

impl DataFormat {
    pub fn label(&self) -> &'static str {
        match self {
            DataFormat::Json => "JSON",
            DataFormat::Csv => "CSV",
            DataFormat::Parquet => "Parquet",
            DataFormat::Arrow => "Arrow",
            DataFormat::Sqlite => "SQLite",
            DataFormat::Custom => "Custom",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            DataFormat::Json => Hsla::new(45.0, 0.7, 0.5, 1.0), // Yellow
            DataFormat::Csv => Hsla::new(120.0, 0.6, 0.45, 1.0), // Green
            DataFormat::Parquet => Hsla::new(200.0, 0.7, 0.5, 1.0), // Blue
            DataFormat::Arrow => Hsla::new(280.0, 0.6, 0.55, 1.0), // Purple
            DataFormat::Sqlite => Hsla::new(0.0, 0.6, 0.55, 1.0), // Red
            DataFormat::Custom => theme::text::MUTED,
        }
    }
}

/// License type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataLicense {
    OpenSource,
    Commercial,
    ResearchOnly,
    Attribution,
    Proprietary,
}

impl DataLicense {
    pub fn label(&self) -> &'static str {
        match self {
            DataLicense::OpenSource => "Open Source",
            DataLicense::Commercial => "Commercial",
            DataLicense::ResearchOnly => "Research Only",
            DataLicense::Attribution => "Attribution",
            DataLicense::Proprietary => "Proprietary",
        }
    }
}

/// Dataset info
#[derive(Debug, Clone)]
pub struct DatasetInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub format: DataFormat,
    pub license: DataLicense,
    pub size_bytes: u64,
    pub row_count: Option<u64>,
    pub author: String,
    pub price_sats: Option<u64>,
    pub downloads: u32,
    pub updated_at: String,
}

impl DatasetInfo {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            format: DataFormat::Json,
            license: DataLicense::OpenSource,
            size_bytes: 0,
            row_count: None,
            author: "unknown".to_string(),
            price_sats: None,
            downloads: 0,
            updated_at: "Just now".to_string(),
        }
    }

    pub fn format(mut self, format: DataFormat) -> Self {
        self.format = format;
        self
    }

    pub fn license(mut self, license: DataLicense) -> Self {
        self.license = license;
        self
    }

    pub fn size(mut self, bytes: u64) -> Self {
        self.size_bytes = bytes;
        self
    }

    pub fn rows(mut self, count: u64) -> Self {
        self.row_count = Some(count);
        self
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }

    pub fn price(mut self, sats: u64) -> Self {
        self.price_sats = Some(sats);
        self
    }

    pub fn downloads(mut self, count: u32) -> Self {
        self.downloads = count;
        self
    }

    pub fn updated_at(mut self, time: impl Into<String>) -> Self {
        self.updated_at = time.into();
        self
    }
}

/// Dataset card component
pub struct DatasetCard {
    id: Option<ComponentId>,
    dataset: DatasetInfo,
    hovered: bool,
    download_hovered: bool,
    on_download: Option<Box<dyn FnMut(String)>>,
    on_view: Option<Box<dyn FnMut(String)>>,
}

impl DatasetCard {
    pub fn new(dataset: DatasetInfo) -> Self {
        Self {
            id: None,
            dataset,
            hovered: false,
            download_hovered: false,
            on_download: None,
            on_view: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_download<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_download = Some(Box::new(f));
        self
    }

    pub fn on_view<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_view = Some(Box::new(f));
        self
    }

    fn download_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 12.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 85.0,
            bounds.origin.y + bounds.size.height - padding - 26.0,
            80.0,
            24.0,
        )
    }

    fn format_size(&self) -> String {
        let bytes = self.dataset.size_bytes;
        if bytes >= 1_073_741_824 {
            format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
        } else if bytes >= 1_048_576 {
            format!("{:.1} MB", bytes as f64 / 1_048_576.0)
        } else if bytes >= 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{} B", bytes)
        }
    }

    fn format_rows(&self) -> Option<String> {
        self.dataset.row_count.map(|rows| {
            if rows >= 1_000_000 {
                format!("{:.1}M rows", rows as f64 / 1_000_000.0)
            } else if rows >= 1_000 {
                format!("{:.1}K rows", rows as f64 / 1_000.0)
            } else {
                format!("{} rows", rows)
            }
        })
    }
}

impl Component for DatasetCard {
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

        // Dataset name
        let name_run = cx.text.layout(
            &self.dataset.name,
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(name_run);

        // Format badge
        let format = &self.dataset.format;
        let fmt_x = bounds.origin.x + bounds.size.width - padding - 60.0;
        let fmt_bounds = Bounds::new(fmt_x, y - 2.0, 50.0, 18.0);
        cx.scene.draw_quad(
            Quad::new(fmt_bounds)
                .with_background(format.color().with_alpha(0.2))
                .with_border(format.color(), 1.0),
        );
        let fmt_run = cx.text.layout(
            format.label(),
            Point::new(fmt_x + 6.0, y),
            theme::font_size::XS,
            format.color(),
        );
        cx.scene.draw_text(fmt_run);

        y += 22.0;

        // Description
        let desc = if self.dataset.description.len() > 55 {
            format!("{}...", &self.dataset.description[..52])
        } else {
            self.dataset.description.clone()
        };
        let desc_run = cx.text.layout(
            &desc,
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(desc_run);

        y += 20.0;

        // Size and row count
        let size_text = self.format_size();
        let size_run = cx.text.layout(
            &size_text,
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(size_run);

        if let Some(rows_text) = self.format_rows() {
            let rows_run = cx.text.layout(
                &rows_text,
                Point::new(bounds.origin.x + padding + 70.0, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(rows_run);
        }

        // License
        let license_text = self.dataset.license.label();
        let license_run = cx.text.layout(
            license_text,
            Point::new(bounds.origin.x + padding + 160.0, y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(license_run);

        y += 18.0;

        // Author and updated
        let author_text = format!("by {} | {}", self.dataset.author, self.dataset.updated_at);
        let author_run = cx.text.layout(
            &author_text,
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(author_run);

        // Downloads
        let downloads_text = format!("\u{2B07} {}", self.dataset.downloads);
        let downloads_run = cx.text.layout(
            &downloads_text,
            Point::new(bounds.origin.x + padding + 200.0, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(downloads_run);

        // Price
        if let Some(price) = self.dataset.price_sats {
            let price_text = format!("{} sats", price);
            let price_run = cx.text.layout(
                &price_text,
                Point::new(bounds.origin.x + padding + 260.0, y),
                theme::font_size::XS,
                Hsla::new(35.0, 0.9, 0.5, 1.0),
            );
            cx.scene.draw_text(price_run);
        } else {
            let free_run = cx.text.layout(
                "Free",
                Point::new(bounds.origin.x + padding + 260.0, y),
                theme::font_size::XS,
                Hsla::new(120.0, 0.6, 0.5, 1.0),
            );
            cx.scene.draw_text(free_run);
        }

        // Download button
        let download_bounds = self.download_button_bounds(&bounds);
        let btn_bg = if self.download_hovered {
            theme::accent::PRIMARY.with_alpha(0.3)
        } else {
            theme::accent::PRIMARY.with_alpha(0.2)
        };
        cx.scene.draw_quad(
            Quad::new(download_bounds)
                .with_background(btn_bg)
                .with_border(theme::accent::PRIMARY, 1.0),
        );
        let btn_run = cx.text.layout(
            "Download",
            Point::new(
                download_bounds.origin.x + 8.0,
                download_bounds.origin.y + 5.0,
            ),
            theme::font_size::XS,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(btn_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let download_bounds = self.download_button_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hovered;
                let was_download = self.download_hovered;

                self.hovered = bounds.contains(point);
                self.download_hovered = download_bounds.contains(point);

                if was_hovered != self.hovered || was_download != self.download_hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    if download_bounds.contains(point) {
                        if let Some(callback) = &mut self.on_download {
                            callback(self.dataset.id.clone());
                        }
                        return EventResult::Handled;
                    }

                    if bounds.contains(point) {
                        if let Some(callback) = &mut self.on_view {
                            callback(self.dataset.id.clone());
                        }
                        return EventResult::Handled;
                    }
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
        (None, Some(105.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_info() {
        let dataset = DatasetInfo::new(
            "d1",
            "Training Corpus",
            "Large language model training data",
        )
        .format(DataFormat::Parquet)
        .size(5_368_709_120) // 5 GB
        .rows(10_000_000)
        .author("openai");

        assert_eq!(dataset.name, "Training Corpus");
        assert_eq!(dataset.format, DataFormat::Parquet);
    }

    #[test]
    fn test_format_size() {
        let dataset = DatasetInfo::new("d1", "Test", "Test").size(1_073_741_824);
        let card = DatasetCard::new(dataset);
        assert_eq!(card.format_size(), "1.0 GB");
    }

    #[test]
    fn test_format_labels() {
        assert_eq!(DataFormat::Parquet.label(), "Parquet");
        assert_eq!(DataFormat::Json.label(), "JSON");
    }
}
