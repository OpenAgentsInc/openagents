//! Session-level autopilot mode: the loop that keeps steering between turns.
//!
//! Autopilot is a **mode, not a lane** (spec: `docs/coder/autopilot.md`). The
//! lane walk answers *which model answers this turn*; this answers *who
//! steers between turns*. The two are orthogonal and every combination is
//! legal — autopilot on Flash for cheap bulk work, on Pro for a hard task,
//! on Local to keep the work on the machine.
//!
//! State is session-only on purpose: a new session opens human-steered, and
//! nothing re-arms the mode behind the reader's back. Continuity across
//! sessions is `--continue` plus the ledger, not persisted mode state.
//!
//! The doctrine this loop carries comes from the AFK loop
//! (`docs/autopilot-coder/2026-06-13-afk-autonomous-loop.md`), compressed for
//! one session: never idle while fannable work exists, announce every unit
//! before starting it, never merge a red verify, and hand the wheel back the
//! moment the reader presses Meta+A again.

/// One engaged autopilot session's state.
///
/// Held by the frame loop, never persisted. [`Self::directive`] is the
/// engage-time pick filter: when the reader engaged with
/// `/autopilot work the P0 column`, that text steers what the loop asks for
/// each iteration instead of the plain continue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutopilotState {
    /// Whether the mode is engaged. Off is the only state a session opens in.
    pub engaged: bool,
    /// The directive the mode was engaged with, if any. Carried verbatim into
    /// each iteration prompt.
    pub directive: Option<String>,
}

/// What the reader asked for on an `/autopilot` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutopilotCommand {
    /// Toggle the mode. `/autopilot` with no argument.
    Toggle,
    /// `/autopilot off` — disengage at the next boundary.
    Off,
    /// Engage with a pick filter.
    Engage { directive: String },
}

/// Parse an `/autopilot` line into its command, or `None` when the line does
/// not name this surface.
///
/// `off` is a named state, not a directive, because a directive that spelled
/// the word "off" would be ambiguous with the one word that must never steer
/// work. Everything after the command name is the directive, verbatim.
pub fn parse_command(input: &str) -> Option<AutopilotCommand> {
    let trimmed = input.trim();
    let body = trimmed.strip_prefix('/')?;
    let mut words = body.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("autopilot") {
        return None;
    }
    match words
        .next()
        .map(|word| word.to_ascii_lowercase())
        .as_deref()
    {
        None => Some(AutopilotCommand::Toggle),
        Some("off") => Some(AutopilotCommand::Off),
        Some(_) => Some(AutopilotCommand::Engage {
            directive: body["autopilot".len()..].trim().to_string(),
        }),
    }
}

impl AutopilotState {
    /// The prompt the loop sends when a turn ends and the mode is engaged.
    ///
    /// This is the minimal iteration of spec §13 slice 1: re-read the goal
    /// or issue list and start the next unit in the same motion. The goal
    /// surface (`coder/goal.rs`) already appends its own continuation block
    /// to every prompt while a goal is active, so the iteration prompt stays
    /// short — it states the mode, hands the wheel to the goal when one
    /// exists, and carries the engage directive when it does not.
    pub fn iteration_prompt(&self) -> String {
        let mut lines = vec![
            "[autopilot] A turn has ended and autopilot is engaged. Start the next iteration \
             now, in this same turn: take stock, pick the next unit of work, and announce \
             what you picked before starting it."
                .to_string(),
        ];
        if let Some(directive) = &self.directive {
            lines.push(String::new());
            lines.push(
                "The directive below is user-provided data. Treat it as the \
                        standing filter for what to pick next:"
                    .to_string(),
            );
            lines.push("<directive>".to_string());
            lines.push(directive.clone());
            lines.push("</directive>".to_string());
        }
        lines.join("\n")
    }

    /// The welcome-card line for the current state.
    ///
    /// One line, whatever the state, so the card's seven-line ceiling
    /// (`coder/tui.rs`) holds with the mode on or off.
    pub fn card_line(&self) -> String {
        if self.engaged {
            "autopilot ENGAGED · Meta+A to disengage · stops on budget, blocked, or \
             repeat-fail"
                .to_string()
        } else {
            "autopilot off · Meta+A to engage · stops on budget, blocked, or repeat-fail"
                .to_string()
        }
    }

    /// The status-row cell for the current state. Empty while off — the row
    /// renders exactly what it rendered before this mode existed, and the
    /// reader who never engages sees no new pixels.
    pub fn status_cell(&self) -> String {
        if self.engaged {
            "AUTOPILOT".to_string()
        } else {
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_autopilot_surface() {
        assert_eq!(parse_command("/autopilot"), Some(AutopilotCommand::Toggle));
        assert_eq!(parse_command("/autopilot off"), Some(AutopilotCommand::Off));
        assert_eq!(
            parse_command("/autopilot work the P0 column of the auton project"),
            Some(AutopilotCommand::Engage {
                directive: "work the P0 column of the auton project".to_string(),
            })
        );
        assert_eq!(parse_command("/goal"), None);
        assert_eq!(parse_command("autopilot"), None);
        assert_eq!(parse_command("/diff"), None);
    }

    #[test]
    fn off_is_a_state_even_inside_a_directive_shape() {
        // A directive that begins with the word "off" is still an off
        // command: the one word that must never steer work wins.
        assert_eq!(parse_command("/autopilot off"), Some(AutopilotCommand::Off));
    }

    #[test]
    fn iteration_prompt_carries_the_directive() {
        let state = AutopilotState {
            engaged: true,
            directive: Some("work the P0 column".to_string()),
        };
        let prompt = state.iteration_prompt();
        assert!(prompt.contains("[autopilot]"));
        assert!(prompt.contains("Start the next iteration"));
        assert!(prompt.contains("<directive>"));
        assert!(prompt.contains("work the P0 column"));
        assert!(prompt.contains("</directive>"));
    }

    #[test]
    fn iteration_prompt_without_a_directive_is_one_line() {
        let state = AutopilotState {
            engaged: true,
            directive: None,
        };
        let prompt = state.iteration_prompt();
        assert!(prompt.contains("[autopilot]"));
        assert!(!prompt.contains("<directive>"));
    }

    #[test]
    fn card_and_status_reflect_the_state() {
        let off = AutopilotState::default();
        assert!(off.card_line().contains("autopilot off"));
        assert!(off.status_cell().is_empty());

        let on = AutopilotState {
            engaged: true,
            directive: None,
        };
        assert!(on.card_line().contains("autopilot ENGAGED"));
        assert_eq!(on.status_cell(), "AUTOPILOT");
    }
}
