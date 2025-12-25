use std::cell::RefCell;
use std::rc::Rc;

use autopilot::parallel::AgentStatus;
use wgpui::components::{Button, ButtonVariant, TextInput};
use wgpui::{
    Bounds, Component, EventContext, EventResult, InputEvent, PaintContext, Text, theme,
};

use crate::backend::BackendCommand;
use crate::state::AppState;
use crate::views::fit_text;

pub struct ParallelView {
    state: Rc<RefCell<AppState>>,
    count_input: TextInput,
    start_button: Button,
    stop_button: Button,
}

struct ControlLayout {
    input: Bounds,
    button: Bounds,
    height: f32,
}

impl ParallelView {
    pub fn new(
        state: Rc<RefCell<AppState>>,
        command_tx: std::sync::mpsc::Sender<BackendCommand>,
    ) -> Self {
        let count_value = Rc::new(RefCell::new("3".to_string()));
        let count_input = TextInput::new()
            .value(count_value.borrow().clone())
            .placeholder("Agents")
            .on_change({
                let count_value = count_value.clone();
                move |value| {
                    *count_value.borrow_mut() = value.to_string();
                }
            });

        let start_tx = command_tx.clone();
        let stop_tx = command_tx;
        let start_state = state.clone();
        let start_count = count_value.clone();

        let start_button = Button::new("Start")
            .variant(ButtonVariant::Primary)
            .on_click(move || {
                let raw = start_count.borrow().trim().to_string();
                let mut count = raw.parse::<usize>().unwrap_or(1).max(1);
                let max_agents = start_state.borrow().parallel_platform.max_agents;
                if max_agents > 0 {
                    count = count.min(max_agents);
                }
                let _ = start_tx.send(BackendCommand::StartParallel { count });
            });

        let stop_button = Button::new("Stop")
            .variant(ButtonVariant::Danger)
            .on_click(move || {
                let _ = stop_tx.send(BackendCommand::StopParallel);
            });

        Self {
            state,
            count_input,
            start_button,
            stop_button,
        }
    }

    fn control_layout(
        &self,
        bounds: Bounds,
        y: f32,
        button_size: (Option<f32>, Option<f32>),
    ) -> ControlLayout {
        let padding = theme::spacing::MD;
        let gap = theme::spacing::SM;
        let max_width = (bounds.size.width - padding * 2.0).max(0.0);

        let input_height = self.count_input.size_hint().1.unwrap_or(30.0);
        let input_width = 110.0_f32.min(max_width);

        let (button_w, button_h) = button_size;
        let button_width = button_w.unwrap_or(100.0).min(max_width);
        let button_height = button_h.unwrap_or(input_height);

        let same_row = max_width >= input_width + gap + button_width;
        let input_bounds = Bounds::new(
            bounds.origin.x + padding,
            y,
            if same_row { input_width } else { max_width },
            input_height,
        );

        let (button_x, button_y, button_row_height) = if same_row {
            (
                input_bounds.origin.x + input_bounds.size.width + gap,
                y,
                input_height,
            )
        } else {
            (
                input_bounds.origin.x,
                y + input_height + gap,
                input_height + gap + button_height,
            )
        };

        let button_bounds = Bounds::new(button_x, button_y, button_width, button_height);
        let height = if same_row {
            input_height
        } else {
            button_row_height
        };

        ControlLayout {
            input: input_bounds,
            button: button_bounds,
            height,
        }
    }
}

impl Component for ParallelView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let line_height = theme::font_size::SM * 1.5;
        let line_height_xs = theme::font_size::XS * 1.5;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        let running_agents = state
            .agents
            .iter()
            .filter(|agent| !matches!(agent.status, AgentStatus::Stopped))
            .count();

        let header_lines = [
            format!("Platform: {}", state.parallel_platform.platform),
            format!("Max agents: {}", state.parallel_platform.max_agents),
            format!(
                "Limits: {} RAM / {} CPU",
                state.parallel_platform.memory_limit, state.parallel_platform.cpu_limit
            ),
            format!("Active agents: {}", running_agents),
        ];

        let mut y = bounds.origin.y + padding;
        for line in header_lines {
            let line = fit_text(cx, &line, theme::font_size::SM, available_width);
            let mut text = Text::new(line)
                .font_size(theme::font_size::SM)
                .color(theme::text::PRIMARY);
            text.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    bounds.size.width - padding * 2.0,
                    line_height,
                ),
                cx,
            );
            y += line_height;
        }

        y += theme::spacing::SM;
        let target_label = fit_text(cx, "Target agents", theme::font_size::XS, available_width);
        let mut target_text = Text::new(target_label)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        target_text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height_xs,
            ),
            cx,
        );
        y += line_height_xs + theme::spacing::XS;

        let running = running_agents > 0;
        let button_size = if running {
            self.stop_button.size_hint()
        } else {
            self.start_button.size_hint()
        };
        let controls = self.control_layout(bounds, y, button_size);
        self.count_input.paint(controls.input, cx);
        let active_button = if running {
            &mut self.stop_button
        } else {
            &mut self.start_button
        };
        active_button.paint(controls.button, cx);
        y += controls.height + theme::spacing::SM;

        let max_y = bounds.origin.y + bounds.size.height - padding;

        let issues_header = format!("Open issues: {}", state.open_issues.len());
        let issues_header = fit_text(cx, &issues_header, theme::font_size::XS, available_width);
        let mut issues_text = Text::new(issues_header)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        issues_text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height_xs,
            ),
            cx,
        );
        y += line_height_xs;

        for issue in state.open_issues.iter().take(5) {
            if y + line_height_xs > max_y {
                break;
            }
            let line = format!("#{} {} ({})", issue.number, issue.title, issue.priority);
            let line = fit_text(cx, &line, theme::font_size::XS, available_width);
            let mut text = Text::new(line)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            text.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    bounds.size.width - padding * 2.0,
                    line_height_xs,
                ),
                cx,
            );
            y += line_height_xs;
        }

        y += theme::spacing::SM;
        let agents_header = format!("Agents detected: {}", state.agents.len());
        let agents_header = fit_text(cx, &agents_header, theme::font_size::XS, available_width);
        let mut agents_text = Text::new(agents_header)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        agents_text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height_xs,
            ),
            cx,
        );
        y += line_height_xs;

        for agent in state.agents.iter().take(6) {
            if y + line_height_xs > max_y {
                break;
            }
            let issue = agent
                .current_issue
                .map(|num| format!("#{}", num))
                .unwrap_or_else(|| "-".to_string());
            let line = format!("{}  {}  {}", agent.id, agent.status, issue);
            let line = fit_text(cx, &line, theme::font_size::XS, available_width);
            let mut text = Text::new(line)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            text.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    bounds.size.width - padding * 2.0,
                    line_height_xs,
                ),
                cx,
            );
            y += line_height_xs;
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let line_height = theme::font_size::SM * 1.5;
        let line_height_xs = theme::font_size::XS * 1.5;
        let mut y = bounds.origin.y + padding + line_height * 4.0;
        y += theme::spacing::SM + line_height_xs + theme::spacing::XS;

        let running_agents = state
            .agents
            .iter()
            .filter(|agent| !matches!(agent.status, AgentStatus::Stopped))
            .count();
        drop(state);

        let running = running_agents > 0;
        let button_size = if running {
            self.stop_button.size_hint()
        } else {
            self.start_button.size_hint()
        };
        let controls = self.control_layout(bounds, y, button_size);

        let mut handled = false;

        match event {
            InputEvent::KeyDown { .. } | InputEvent::KeyUp { .. } => {
                handled = matches!(
                    self.count_input.event(event, controls.input, cx),
                    EventResult::Handled
                );
            }
            InputEvent::MouseMove { .. }
            | InputEvent::MouseDown { .. }
            | InputEvent::MouseUp { .. } => {
                let input_result = self.count_input.event(event, controls.input, cx);
                let button_result = if running {
                    self.stop_button.event(event, controls.button, cx)
                } else {
                    self.start_button.event(event, controls.button, cx)
                };
                handled =
                    matches!(input_result, EventResult::Handled)
                        || matches!(button_result, EventResult::Handled);
            }
            _ => {}
        }

        if handled {
            EventResult::Handled
        } else {
            EventResult::Ignored
        }
    }
}
