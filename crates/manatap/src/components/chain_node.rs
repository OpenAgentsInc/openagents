use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem};

// Layout constants
const CARD_PADDING: f32 = 12.0;
const STATUS_DOT_SIZE: f32 = 8.0;
const FONT_SIZE_TITLE: f32 = 14.0;
const FONT_SIZE_BODY: f32 = 12.0;
const FONT_SIZE_META: f32 = 10.0;
const FONT_SIZE_DESC: f32 = 11.0;
const CORNER_RADIUS: f32 = 6.0;
const LINE_HEIGHT: f32 = 16.0;

// Colors
const PENDING_COLOR: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.4,
    a: 1.0,
};
const RUNNING_COLOR: Hsla = Hsla {
    h: 210.0,
    s: 0.8,
    l: 0.5,
    a: 1.0,
};
const COMPLETE_COLOR: Hsla = Hsla {
    h: 140.0,
    s: 0.7,
    l: 0.4,
    a: 1.0,
};
const FAILED_COLOR: Hsla = Hsla {
    h: 0.0,
    s: 0.8,
    l: 0.5,
    a: 1.0,
};
const CARD_BG: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.12,
    a: 1.0,
};
const BORDER_COLOR: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.25,
    a: 1.0,
};
const TEXT_PRIMARY: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.95,
    a: 1.0,
};
const TEXT_MUTED: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.6,
    a: 1.0,
};
const TEXT_ACCENT: Hsla = Hsla {
    h: 180.0,
    s: 0.6,
    l: 0.6,
    a: 1.0,
};
const TEXT_DESC: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.5,
    a: 1.0,
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NodeState {
    Pending,
    Running,
    Complete,
    Failed,
}

impl NodeState {
    pub fn color(&self) -> Hsla {
        match self {
            NodeState::Pending => PENDING_COLOR,
            NodeState::Running => RUNNING_COLOR,
            NodeState::Complete => COMPLETE_COLOR,
            NodeState::Failed => FAILED_COLOR,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            NodeState::Pending => "pending",
            NodeState::Running => "running",
            NodeState::Complete => "complete",
            NodeState::Failed => "failed",
        }
    }
}

pub struct ChainNode {
    pub name: String,
    pub description: String,
    pub state: NodeState,
    pub inputs: Vec<(String, String)>,
    pub outputs: Vec<(String, String)>,
    pub tokens: Option<u32>,
    pub cost_msats: Option<u64>,
    pub duration_ms: Option<u64>,
    pub progress_message: Option<String>,
}

impl ChainNode {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            state: NodeState::Pending,
            inputs: Vec::new(),
            outputs: Vec::new(),
            tokens: None,
            cost_msats: None,
            duration_ms: None,
            progress_message: None,
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn with_state(mut self, state: NodeState) -> Self {
        self.state = state;
        self
    }

    pub fn with_input(mut self, field: &str, value: &str) -> Self {
        self.inputs.push((field.to_string(), value.to_string()));
        self
    }

    pub fn with_output(mut self, field: &str, value: &str) -> Self {
        self.outputs.push((field.to_string(), value.to_string()));
        self
    }

    pub fn with_metrics(mut self, tokens: u32, cost: u64, duration: u64) -> Self {
        self.tokens = Some(tokens);
        self.cost_msats = Some(cost);
        self.duration_ms = Some(duration);
        self
    }

    pub fn with_progress(mut self, message: &str) -> Self {
        self.progress_message = Some(message.to_string());
        self
    }

    pub fn height(&self, _width: f32, _text: &mut TextSystem, _scale: f32) -> f32 {
        let mut h = CARD_PADDING * 2.0 + 20.0; // Header

        // Description line
        if !self.description.is_empty() {
            h += 14.0;
        }

        if !self.inputs.is_empty() {
            h += 14.0; // "INPUT:" label
            h += self.inputs.len() as f32 * LINE_HEIGHT;
        }

        if !self.outputs.is_empty() {
            h += 14.0; // "OUTPUT:" label
            h += self.outputs.len() as f32 * LINE_HEIGHT;
        }

        if self.state == NodeState::Running && self.progress_message.is_some() {
            h += LINE_HEIGHT;
        }

        if self.state == NodeState::Complete && self.tokens.is_some() {
            h += 4.0 + 1.0 + 8.0 + 14.0; // separator + metrics line
        }

        h
    }

    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text: &mut TextSystem, _scale: f32) {
        let x = bounds.origin.x;
        let y = bounds.origin.y;
        let w = bounds.size.width;

        // Card background with colored border
        scene.draw_quad(
            Quad::new(bounds)
                .with_background(CARD_BG)
                .with_border(self.state.color(), 1.0)
                .with_corner_radius(CORNER_RADIUS),
        );

        // Status dot
        let dot_x = x + CARD_PADDING;
        let dot_y = y + CARD_PADDING + 3.0;
        scene.draw_quad(
            Quad::new(Bounds::new(dot_x, dot_y, STATUS_DOT_SIZE, STATUS_DOT_SIZE))
                .with_background(self.state.color())
                .with_corner_radius(STATUS_DOT_SIZE / 2.0),
        );

        // Signature name
        let name_run = text.layout_mono(
            &self.name,
            Point::new(dot_x + STATUS_DOT_SIZE + 8.0, y + CARD_PADDING),
            FONT_SIZE_TITLE,
            TEXT_PRIMARY,
        );
        scene.draw_text(name_run);

        // State badge (right side)
        let state_text = self.state.label();
        let state_w = text.measure(state_text, FONT_SIZE_BODY);
        let badge_x = x + w - CARD_PADDING - state_w - 12.0;
        let badge_color = self.state.color();
        scene.draw_quad(
            Quad::new(Bounds::new(
                badge_x,
                y + CARD_PADDING - 2.0,
                state_w + 12.0,
                18.0,
            ))
            .with_background(Hsla {
                a: 0.2,
                ..badge_color
            })
            .with_corner_radius(3.0),
        );
        let badge_run = text.layout_mono(
            state_text,
            Point::new(badge_x + 6.0, y + CARD_PADDING),
            FONT_SIZE_BODY,
            badge_color,
        );
        scene.draw_text(badge_run);

        let mut content_y = y + CARD_PADDING + 24.0;

        // Description (below name)
        if !self.description.is_empty() {
            let desc_run = text.layout_mono(
                &self.description,
                Point::new(dot_x + STATUS_DOT_SIZE + 8.0, content_y - 6.0),
                FONT_SIZE_DESC,
                TEXT_DESC,
            );
            scene.draw_text(desc_run);
            content_y += 14.0;
        }

        // Inputs section
        if !self.inputs.is_empty() {
            let label_run = text.layout_mono(
                "INPUT:",
                Point::new(x + CARD_PADDING, content_y),
                FONT_SIZE_META,
                TEXT_MUTED,
            );
            scene.draw_text(label_run);
            content_y += 14.0;

            for (field, value) in &self.inputs {
                let line = format!("  {} = {}", field, truncate(value, 50));
                let line_run = text.layout_mono(
                    &line,
                    Point::new(x + CARD_PADDING, content_y),
                    FONT_SIZE_BODY,
                    TEXT_ACCENT,
                );
                scene.draw_text(line_run);
                content_y += LINE_HEIGHT;
            }
        }

        // Outputs section
        if !self.outputs.is_empty() {
            let label_run = text.layout_mono(
                "OUTPUT:",
                Point::new(x + CARD_PADDING, content_y),
                FONT_SIZE_META,
                TEXT_MUTED,
            );
            scene.draw_text(label_run);
            content_y += 14.0;

            for (field, value) in &self.outputs {
                let line = format!("  {} = {}", field, truncate(value, 50));
                let line_run = text.layout_mono(
                    &line,
                    Point::new(x + CARD_PADDING, content_y),
                    FONT_SIZE_BODY,
                    TEXT_PRIMARY,
                );
                scene.draw_text(line_run);
                content_y += LINE_HEIGHT;
            }
        }

        // Progress message for running state
        if self.state == NodeState::Running {
            if let Some(msg) = &self.progress_message {
                let msg_run = text.layout_mono(
                    msg,
                    Point::new(x + CARD_PADDING, content_y),
                    FONT_SIZE_BODY,
                    RUNNING_COLOR,
                );
                scene.draw_text(msg_run);
                content_y += LINE_HEIGHT;
            }
        }

        // Metrics footer for complete state
        if self.state == NodeState::Complete && self.tokens.is_some() {
            content_y += 4.0;
            // Separator line
            scene.draw_quad(
                Quad::new(Bounds::new(
                    x + CARD_PADDING,
                    content_y,
                    w - CARD_PADDING * 2.0,
                    1.0,
                ))
                .with_background(BORDER_COLOR),
            );
            content_y += 8.0;

            let metrics = format!(
                "tokens: {} | cost: {} msats | {}ms",
                self.tokens.unwrap_or(0),
                self.cost_msats.unwrap_or(0),
                self.duration_ms.unwrap_or(0)
            );
            let metrics_run = text.layout_mono(
                &metrics,
                Point::new(x + CARD_PADDING, content_y),
                FONT_SIZE_META,
                TEXT_MUTED,
            );
            scene.draw_text(metrics_run);
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
