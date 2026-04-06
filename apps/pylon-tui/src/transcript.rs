use ratatui::text::{Line, Text};

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

impl TranscriptRole {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Assistant => "assistant",
            Self::User => "user",
        }
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
        let mut lines = vec![Line::from(format!(
            "[{}] {}",
            self.role.label(),
            self.title
        ))];
        for line in &self.body {
            lines.push(Line::from(format!("  {line}")));
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

    fn render_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from(format!(
            "[active {}] {}",
            self.role.label(),
            self.title
        ))];
        for line in &self.body {
            lines.push(Line::from(format!("  {line}")));
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
        let mut lines = Vec::new();
        append_entry_lines(&mut lines, &self.entries);
        if let Some(active_turn) = &self.active_turn {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.extend(active_turn.render_lines());
        }
        Text::from(lines)
    }
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
}
