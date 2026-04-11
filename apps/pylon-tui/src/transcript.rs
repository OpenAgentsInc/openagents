use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

#[allow(
    dead_code,
    reason = "Assistant-stream wiring lands in the next Pylon chat issue."
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptRole {
    System,
    Assistant,
    User,
}

fn transcript_system_label_style() -> Style {
    Style::default()
        .fg(Color::Rgb(0x8b, 0xc7, 0xff))
        .add_modifier(Modifier::BOLD)
}

fn transcript_user_label_style() -> Style {
    Style::default()
        .fg(Color::Rgb(0xc5, 0xe7, 0xff))
        .add_modifier(Modifier::BOLD)
}

fn transcript_assistant_label_style(active: bool) -> Style {
    if active {
        Style::default()
            .fg(Color::Rgb(0xf8, 0xf4, 0xe3))
            .bg(Color::Rgb(0x1c, 0x3b, 0x55))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Rgb(0xf8, 0xf4, 0xe3))
            .add_modifier(Modifier::BOLD)
    }
}

fn transcript_system_title_style() -> Style {
    Style::default().fg(Color::Rgb(0x9b, 0xd6, 0xff))
}

fn transcript_user_title_style() -> Style {
    Style::default().fg(Color::Rgb(0xa9, 0xd8, 0xff))
}

fn transcript_assistant_title_style(active: bool) -> Style {
    if active {
        Style::default()
            .fg(Color::Rgb(0xf8, 0xf4, 0xe3))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(0xd8, 0xec, 0xff))
    }
}

fn transcript_meta_style() -> Style {
    Style::default().fg(Color::Rgb(0x7d, 0xb9, 0xe6))
}

fn transcript_system_body_style() -> Style {
    Style::default().fg(Color::Rgb(0x9b, 0xd6, 0xff))
}

fn transcript_user_body_style() -> Style {
    Style::default().fg(Color::Rgb(0xb2, 0xe1, 0xff))
}

fn transcript_assistant_body_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::Rgb(0xf8, 0xf4, 0xe3))
    } else {
        Style::default().fg(Color::Rgb(0xe7, 0xf3, 0xff))
    }
}

fn transcript_role_label(role: TranscriptRole, active: bool) -> &'static str {
    match (role, active) {
        (TranscriptRole::Assistant, true) => "[active assistant]",
        _ => match role {
            TranscriptRole::System => "[system]",
            TranscriptRole::Assistant => "[assistant]",
            TranscriptRole::User => "[user]",
        },
    }
}

fn transcript_role_label_style(role: TranscriptRole, active: bool) -> Style {
    match role {
        TranscriptRole::System => transcript_system_label_style(),
        TranscriptRole::User => transcript_user_label_style(),
        TranscriptRole::Assistant => transcript_assistant_label_style(active),
    }
}

fn transcript_title_style(role: TranscriptRole, active: bool) -> Style {
    match role {
        TranscriptRole::System => transcript_system_title_style(),
        TranscriptRole::User => transcript_user_title_style(),
        TranscriptRole::Assistant => transcript_assistant_title_style(active),
    }
}

fn transcript_body_style(role: TranscriptRole, active: bool) -> Style {
    match role {
        TranscriptRole::System => transcript_system_body_style(),
        TranscriptRole::User => transcript_user_body_style(),
        TranscriptRole::Assistant => transcript_assistant_body_style(active),
    }
}

fn split_assistant_title(title: &str) -> (String, Vec<String>) {
    let mut parts = title
        .split("  ")
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return (String::new(), Vec::new());
    }
    let primary = parts.remove(0);
    (primary, parts)
}

fn active_cursor(phase: usize) -> &'static str {
    match phase % 4 {
        0 => " ▍",
        1 => " ▌",
        2 => " ▊",
        _ => " ▋",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptEntry {
    role: TranscriptRole,
    title: String,
    body: Vec<String>,
}

impl TranscriptEntry {
    #[must_use]
    pub fn new(role: TranscriptRole, title: impl Into<String>, body: Vec<String>) -> Self {
        Self {
            role,
            title: title.into(),
            body,
        }
    }

    fn render_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![render_transcript_header(self.role, self.title.as_str(), false, 0)];
        for line in &self.body {
            lines.push(render_transcript_body_line(
                self.role,
                line.as_str(),
                false,
                false,
                0,
            ));
        }
        lines
    }
}

#[allow(
    dead_code,
    reason = "Active streamed assistant turns land in the next Pylon chat issue."
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveTurn {
    role: TranscriptRole,
    title: String,
    body: Vec<String>,
}

#[allow(
    dead_code,
    reason = "Active streamed assistant turns land in the next Pylon chat issue."
)]
impl ActiveTurn {
    #[must_use]
    pub fn new(role: TranscriptRole, title: impl Into<String>, body: Vec<String>) -> Self {
        Self {
            role,
            title: title.into(),
            body,
        }
    }

    fn render_lines(&self, phase: usize) -> Vec<Line<'static>> {
        let mut lines = vec![render_transcript_header(self.role, self.title.as_str(), true, phase)];
        for (index, line) in self.body.iter().enumerate() {
            let is_last_line = index + 1 == self.body.len();
            lines.push(render_transcript_body_line(
                self.role,
                line.as_str(),
                true,
                is_last_line,
                phase,
            ));
        }
        lines
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RetainedTranscript {
    entries: Vec<TranscriptEntry>,
    active_turn: Option<ActiveTurn>,
}

impl RetainedTranscript {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_entry(&mut self, entry: TranscriptEntry) {
        self.entries.push(entry);
    }

    #[allow(
        dead_code,
        reason = "Active streamed assistant turns land in the next Pylon chat issue."
    )]
    pub fn set_active_turn(&mut self, turn: ActiveTurn) {
        self.active_turn = Some(turn);
    }

    #[allow(
        dead_code,
        reason = "Active streamed assistant turns land in the next Pylon chat issue."
    )]
    pub fn clear_active_turn(&mut self) {
        self.active_turn = None;
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.active_turn.is_none()
    }

    #[must_use]
    #[allow(
        dead_code,
        reason = "Transcript scroll accounting lands in the next Pylon chat issue."
    )]
    pub fn line_count(&self) -> usize {
        self.as_text().lines.len()
    }

    #[must_use]
    pub fn as_text(&self) -> Text<'static> {
        self.as_text_with_motion(0)
    }

    #[must_use]
    pub fn as_text_with_motion(&self, phase: usize) -> Text<'static> {
        let mut lines = Vec::new();
        append_entry_lines(&mut lines, &self.entries);
        if let Some(active_turn) = &self.active_turn {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.extend(active_turn.render_lines(phase));
        }
        Text::from(lines)
    }
}

fn render_transcript_header(
    role: TranscriptRole,
    title: &str,
    active: bool,
    _phase: usize,
) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            transcript_role_label(role, active).to_string(),
            transcript_role_label_style(role, active),
        ),
        Span::raw(" "),
    ];

    if role == TranscriptRole::Assistant {
        let (primary, metrics) = split_assistant_title(title);
        spans.push(Span::styled(primary, transcript_title_style(role, active)));
        for metric in metrics {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(metric, transcript_meta_style()));
        }
    } else {
        spans.push(Span::styled(
            title.to_string(),
            transcript_title_style(role, active),
        ));
    }

    Line::from(spans)
}

fn render_transcript_body_line(
    role: TranscriptRole,
    body: &str,
    active: bool,
    is_last_line: bool,
    phase: usize,
) -> Line<'static> {
    let mut spans = vec![
        Span::raw("  "),
        Span::styled(body.to_string(), transcript_body_style(role, active)),
    ];
    if active && role == TranscriptRole::Assistant && is_last_line {
        spans.push(Span::styled(
            active_cursor(phase),
            transcript_assistant_label_style(true),
        ));
    }
    Line::from(spans)
}

fn append_entry_lines(lines: &mut Vec<Line<'static>>, entries: &[TranscriptEntry]) {
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            lines.push(Line::from(""));
        }
        lines.extend(entry.render_lines());
    }
}

#[cfg(test)]
mod tests {
    use super::{ActiveTurn, RetainedTranscript, TranscriptEntry, TranscriptRole};

    fn lines_to_text(transcript: &RetainedTranscript) -> String {
        transcript
            .as_text()
            .lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn transcript_renders_entries_and_active_turn() {
        let mut transcript = RetainedTranscript::new();
        transcript.push_entry(TranscriptEntry::new(
            TranscriptRole::User,
            "Prompt",
            vec![String::from("/chat hello")],
        ));
        transcript.set_active_turn(ActiveTurn::new(
            TranscriptRole::Assistant,
            "Reply",
            vec![String::from("hello")],
        ));

        let rendered = lines_to_text(&transcript);
        assert!(rendered.contains("[user] Prompt"));
        assert!(rendered.contains("[active assistant] Reply"));
    }

    #[test]
    fn transcript_motion_adds_live_cursor_to_active_assistant() {
        let mut transcript = RetainedTranscript::new();
        transcript.set_active_turn(ActiveTurn::new(
            TranscriptRole::Assistant,
            "Reply",
            vec![String::from("hello")],
        ));

        let rendered = transcript
            .as_text_with_motion(2)
            .lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("[active assistant] Reply"));
        assert!(rendered.contains("hello ▊"));
    }

    #[test]
    fn assistant_title_keeps_metrics_on_header_line() {
        let transcript = RetainedTranscript {
            entries: vec![TranscriptEntry::new(
                TranscriptRole::Assistant,
                "Local Gemma gemma4:e4b  ttft 0.42s  total 3.18s  27.6 tok/s",
                vec![String::from("hello world")],
            )],
            active_turn: None,
        };

        let rendered = lines_to_text(&transcript);
        assert!(rendered.contains("[assistant] Local Gemma gemma4:e4b  ttft 0.42s  total 3.18s  27.6 tok/s"));
        assert!(rendered.contains("  hello world"));
    }
}
