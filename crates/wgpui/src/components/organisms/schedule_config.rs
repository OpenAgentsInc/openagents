//! Schedule configuration organism for agent heartbeat and task scheduling.
//!
//! Provides a configuration panel for setting up agent schedules, heartbeat intervals,
//! and tick event timing for sovereign agents (NIP-SA).

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

/// Schedule type for agents
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ScheduleType {
    #[default]
    Continuous,
    Interval,
    Cron,
    Manual,
}

impl ScheduleType {
    pub fn label(&self) -> &str {
        match self {
            Self::Continuous => "Continuous",
            Self::Interval => "Interval",
            Self::Cron => "Cron",
            Self::Manual => "Manual",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Continuous => "Agent runs continuously with heartbeat",
            Self::Interval => "Agent wakes at fixed intervals",
            Self::Cron => "Agent follows cron schedule",
            Self::Manual => "Agent only runs when triggered",
        }
    }
}

/// Interval unit for scheduling
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum IntervalUnit {
    Seconds,
    #[default]
    Minutes,
    Hours,
    Days,
}

impl IntervalUnit {
    pub fn label(&self) -> &str {
        match self {
            Self::Seconds => "sec",
            Self::Minutes => "min",
            Self::Hours => "hr",
            Self::Days => "day",
        }
    }

    pub fn to_seconds(&self, value: u32) -> u64 {
        match self {
            Self::Seconds => value as u64,
            Self::Minutes => value as u64 * 60,
            Self::Hours => value as u64 * 3600,
            Self::Days => value as u64 * 86400,
        }
    }
}

/// Schedule configuration data
#[derive(Debug, Clone)]
pub struct ScheduleData {
    pub schedule_type: ScheduleType,
    pub heartbeat_interval: u32,
    pub heartbeat_unit: IntervalUnit,
    pub tick_interval: u32,
    pub tick_unit: IntervalUnit,
    pub cron_expression: String,
    pub enabled: bool,
    pub next_run: Option<u64>,
    pub last_run: Option<u64>,
}

impl Default for ScheduleData {
    fn default() -> Self {
        Self {
            schedule_type: ScheduleType::Continuous,
            heartbeat_interval: 30,
            heartbeat_unit: IntervalUnit::Seconds,
            tick_interval: 5,
            tick_unit: IntervalUnit::Minutes,
            cron_expression: String::new(),
            enabled: true,
            next_run: None,
            last_run: None,
        }
    }
}

impl ScheduleData {
    pub fn new(schedule_type: ScheduleType) -> Self {
        Self {
            schedule_type,
            ..Default::default()
        }
    }

    pub fn heartbeat(mut self, interval: u32, unit: IntervalUnit) -> Self {
        self.heartbeat_interval = interval;
        self.heartbeat_unit = unit;
        self
    }

    pub fn tick(mut self, interval: u32, unit: IntervalUnit) -> Self {
        self.tick_interval = interval;
        self.tick_unit = unit;
        self
    }

    pub fn cron(mut self, expression: impl Into<String>) -> Self {
        self.cron_expression = expression.into();
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn next_run(mut self, timestamp: u64) -> Self {
        self.next_run = Some(timestamp);
        self
    }

    pub fn last_run(mut self, timestamp: u64) -> Self {
        self.last_run = Some(timestamp);
        self
    }

    pub fn heartbeat_seconds(&self) -> u64 {
        self.heartbeat_unit.to_seconds(self.heartbeat_interval)
    }

    pub fn tick_seconds(&self) -> u64 {
        self.tick_unit.to_seconds(self.tick_interval)
    }
}

/// Configuration section being edited
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ConfigSection {
    #[default]
    Type,
    Heartbeat,
    Tick,
    Cron,
}

/// Schedule configuration organism
pub struct ScheduleConfig {
    id: Option<ComponentId>,
    config: ScheduleData,
    #[allow(dead_code)]
    active_section: ConfigSection,
    hovered_type: Option<ScheduleType>,
    save_button_hovered: bool,
    reset_button_hovered: bool,
    on_save: Option<Box<dyn FnMut(ScheduleData)>>,
    on_reset: Option<Box<dyn FnMut()>>,
}

impl ScheduleConfig {
    pub fn new(config: ScheduleData) -> Self {
        Self {
            id: None,
            config,
            active_section: ConfigSection::Type,
            hovered_type: None,
            save_button_hovered: false,
            reset_button_hovered: false,
            on_save: None,
            on_reset: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_save<F>(mut self, f: F) -> Self
    where
        F: FnMut(ScheduleData) + 'static,
    {
        self.on_save = Some(Box::new(f));
        self
    }

    pub fn on_reset<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_reset = Some(Box::new(f));
        self
    }

    fn header_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 50.0)
    }

    fn type_section_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 50.0,
            bounds.size.width,
            100.0,
        )
    }

    fn type_button_bounds(&self, bounds: &Bounds, index: usize) -> Bounds {
        let section = self.type_section_bounds(bounds);
        let padding = 12.0;
        let button_width = (section.size.width - padding * 2.0 - 24.0) / 4.0;
        Bounds::new(
            section.origin.x + padding + index as f32 * (button_width + 8.0),
            section.origin.y + 36.0,
            button_width,
            48.0,
        )
    }

    fn heartbeat_section_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 150.0,
            bounds.size.width,
            80.0,
        )
    }

    fn tick_section_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 230.0,
            bounds.size.width,
            80.0,
        )
    }

    fn footer_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - 50.0,
            bounds.size.width,
            50.0,
        )
    }

    fn save_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let footer = self.footer_bounds(bounds);
        Bounds::new(
            footer.origin.x + footer.size.width - 12.0 - 80.0,
            footer.origin.y + 12.0,
            70.0,
            26.0,
        )
    }

    fn reset_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let footer = self.footer_bounds(bounds);
        Bounds::new(
            footer.origin.x + footer.size.width - 12.0 - 160.0,
            footer.origin.y + 12.0,
            70.0,
            26.0,
        )
    }

    fn schedule_types() -> &'static [ScheduleType] {
        &[
            ScheduleType::Continuous,
            ScheduleType::Interval,
            ScheduleType::Cron,
            ScheduleType::Manual,
        ]
    }
}

impl Component for ScheduleConfig {
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

        let title_run = cx.text.layout(
            "Schedule Configuration",
            Point::new(bounds.origin.x + padding, bounds.origin.y + 16.0),
            theme::font_size::BASE,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        // Status indicator
        let (status_text, status_color) = if self.config.enabled {
            ("Enabled", theme::status::SUCCESS)
        } else {
            ("Disabled", theme::text::MUTED)
        };

        let status_run = cx.text.layout(
            status_text,
            Point::new(
                bounds.origin.x + bounds.size.width - padding - 60.0,
                bounds.origin.y + 18.0,
            ),
            theme::font_size::XS,
            status_color,
        );
        cx.scene.draw_text(status_run);

        // Type section
        let type_section = self.type_section_bounds(&bounds);

        let type_label = cx.text.layout(
            "Schedule Type",
            Point::new(
                type_section.origin.x + padding,
                type_section.origin.y + 12.0,
            ),
            theme::font_size::SM,
            theme::text::SECONDARY,
        );
        cx.scene.draw_text(type_label);

        for (i, stype) in Self::schedule_types().iter().enumerate() {
            let btn = self.type_button_bounds(&bounds, i);
            let is_selected = self.config.schedule_type == *stype;
            let is_hovered = self.hovered_type == Some(*stype);

            let bg = if is_selected {
                theme::accent::PRIMARY.with_alpha(0.2)
            } else if is_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::MUTED
            };

            let border_color = if is_selected {
                theme::accent::PRIMARY
            } else {
                theme::border::DEFAULT
            };

            cx.scene.draw_quad(
                Quad::new(btn)
                    .with_background(bg)
                    .with_border(border_color, 1.0),
            );

            let text_color = if is_selected {
                theme::accent::PRIMARY
            } else {
                theme::text::SECONDARY
            };

            let type_run = cx.text.layout(
                stype.label(),
                Point::new(btn.origin.x + 8.0, btn.origin.y + 8.0),
                theme::font_size::XS,
                text_color,
            );
            cx.scene.draw_text(type_run);

            // Description under the label
            let desc_run = cx.text.layout(
                &stype.description()[..stype.description().len().min(20)],
                Point::new(btn.origin.x + 4.0, btn.origin.y + 26.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(desc_run);
        }

        // Heartbeat section
        let heartbeat = self.heartbeat_section_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(heartbeat).with_background(theme::bg::APP));

        let heartbeat_label = cx.text.layout(
            "Heartbeat Interval",
            Point::new(heartbeat.origin.x + padding, heartbeat.origin.y + 12.0),
            theme::font_size::SM,
            theme::text::SECONDARY,
        );
        cx.scene.draw_text(heartbeat_label);

        let heartbeat_value = format!(
            "{} {}",
            self.config.heartbeat_interval,
            self.config.heartbeat_unit.label()
        );
        let heartbeat_val_run = cx.text.layout(
            &heartbeat_value,
            Point::new(heartbeat.origin.x + padding, heartbeat.origin.y + 36.0),
            theme::font_size::BASE,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(heartbeat_val_run);

        let heartbeat_total = format!("= {} seconds", self.config.heartbeat_seconds());
        let heartbeat_total_run = cx.text.layout(
            &heartbeat_total,
            Point::new(
                heartbeat.origin.x + padding + 100.0,
                heartbeat.origin.y + 38.0,
            ),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(heartbeat_total_run);

        // Tick section
        let tick = self.tick_section_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(tick).with_background(theme::bg::APP));

        let tick_label = cx.text.layout(
            "Tick Interval",
            Point::new(tick.origin.x + padding, tick.origin.y + 12.0),
            theme::font_size::SM,
            theme::text::SECONDARY,
        );
        cx.scene.draw_text(tick_label);

        let tick_value = format!(
            "{} {}",
            self.config.tick_interval,
            self.config.tick_unit.label()
        );
        let tick_val_run = cx.text.layout(
            &tick_value,
            Point::new(tick.origin.x + padding, tick.origin.y + 36.0),
            theme::font_size::BASE,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(tick_val_run);

        let tick_total = format!("= {} seconds", self.config.tick_seconds());
        let tick_total_run = cx.text.layout(
            &tick_total,
            Point::new(tick.origin.x + padding + 100.0, tick.origin.y + 38.0),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(tick_total_run);

        // Footer with buttons
        let footer = self.footer_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(footer).with_background(theme::bg::MUTED));

        // Reset button
        let reset_btn = self.reset_button_bounds(&bounds);
        let reset_bg = if self.reset_button_hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };
        cx.scene.draw_quad(
            Quad::new(reset_btn)
                .with_background(reset_bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );
        let reset_run = cx.text.layout(
            "Reset",
            Point::new(reset_btn.origin.x + 14.0, reset_btn.origin.y + 7.0),
            theme::font_size::XS,
            theme::text::SECONDARY,
        );
        cx.scene.draw_text(reset_run);

        // Save button
        let save_btn = self.save_button_bounds(&bounds);
        let save_bg = if self.save_button_hovered {
            theme::accent::PRIMARY.with_alpha(0.3)
        } else {
            theme::accent::PRIMARY.with_alpha(0.2)
        };
        cx.scene.draw_quad(
            Quad::new(save_btn)
                .with_background(save_bg)
                .with_border(theme::accent::PRIMARY, 1.0),
        );
        let save_run = cx.text.layout(
            "Save",
            Point::new(save_btn.origin.x + 18.0, save_btn.origin.y + 7.0),
            theme::font_size::XS,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(save_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_type_hovered = self.hovered_type;
                let was_save_hovered = self.save_button_hovered;
                let was_reset_hovered = self.reset_button_hovered;

                self.hovered_type = None;
                self.save_button_hovered = self.save_button_bounds(&bounds).contains(point);
                self.reset_button_hovered = self.reset_button_bounds(&bounds).contains(point);

                for (i, stype) in Self::schedule_types().iter().enumerate() {
                    if self.type_button_bounds(&bounds, i).contains(point) {
                        self.hovered_type = Some(*stype);
                        break;
                    }
                }

                if was_type_hovered != self.hovered_type
                    || was_save_hovered != self.save_button_hovered
                    || was_reset_hovered != self.reset_button_hovered
                {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    // Check save button
                    if self.save_button_bounds(&bounds).contains(point) {
                        if let Some(ref mut callback) = self.on_save {
                            callback(self.config.clone());
                        }
                        return EventResult::Handled;
                    }

                    // Check reset button
                    if self.reset_button_bounds(&bounds).contains(point) {
                        if let Some(ref mut callback) = self.on_reset {
                            callback();
                        }
                        return EventResult::Handled;
                    }

                    // Check type selection
                    for (i, stype) in Self::schedule_types().iter().enumerate() {
                        if self.type_button_bounds(&bounds, i).contains(point) {
                            self.config.schedule_type = *stype;
                            return EventResult::Handled;
                        }
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
        (Some(400.0), Some(360.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_data_creation() {
        let config = ScheduleData::new(ScheduleType::Interval)
            .heartbeat(60, IntervalUnit::Seconds)
            .tick(10, IntervalUnit::Minutes)
            .enabled(true);

        assert_eq!(config.schedule_type, ScheduleType::Interval);
        assert_eq!(config.heartbeat_interval, 60);
        assert_eq!(config.tick_interval, 10);
        assert!(config.enabled);
    }

    #[test]
    fn test_interval_conversion() {
        assert_eq!(IntervalUnit::Seconds.to_seconds(30), 30);
        assert_eq!(IntervalUnit::Minutes.to_seconds(5), 300);
        assert_eq!(IntervalUnit::Hours.to_seconds(2), 7200);
        assert_eq!(IntervalUnit::Days.to_seconds(1), 86400);
    }

    #[test]
    fn test_schedule_type_labels() {
        assert_eq!(ScheduleType::Continuous.label(), "Continuous");
        assert_eq!(ScheduleType::Interval.label(), "Interval");
        assert_eq!(ScheduleType::Cron.label(), "Cron");
        assert_eq!(ScheduleType::Manual.label(), "Manual");
    }

    #[test]
    fn test_heartbeat_seconds() {
        let config = ScheduleData::default().heartbeat(5, IntervalUnit::Minutes);

        assert_eq!(config.heartbeat_seconds(), 300);
    }

    #[test]
    fn test_schedule_config() {
        let config = ScheduleData::new(ScheduleType::Cron).cron("0 * * * *");

        let schedule = ScheduleConfig::new(config);
        assert_eq!(schedule.config.schedule_type, ScheduleType::Cron);
        assert_eq!(schedule.config.cron_expression, "0 * * * *");
    }
}
