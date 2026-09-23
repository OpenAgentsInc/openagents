//! The Gym's read-only Terminal-Bench views.

use crate::terminal_bench::{Attempt, ComparisonGroup, Records};
use crate::tui::{DASH, ladder_from_environment, show};
use coder_terminal::{Intensity, Ladder, frame, rail};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Overview,
    Comparison,
    Attempt,
    Evidence,
    History,
    Guide,
    Components,
    Requirements,
}

impl View {
    pub const ALL: [Self; 8] = [
        Self::Overview,
        Self::Comparison,
        Self::Attempt,
        Self::Evidence,
        Self::History,
        Self::Guide,
        Self::Components,
        Self::Requirements,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Comparison => "comparison",
            Self::Attempt => "attempt",
            Self::Evidence => "evidence",
            Self::History => "history",
            Self::Guide => "runbooks",
            Self::Components => "components",
            Self::Requirements => "requirements",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Comparison => 1,
            Self::Attempt => 2,
            Self::Evidence => 3,
            Self::History => 4,
            Self::Guide => 5,
            Self::Components => 6,
            Self::Requirements => 7,
        }
    }
    pub fn from_digit(digit: char) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|view| (b'1' + view.index() as u8) as char == digit)
    }
}

pub struct App {
    records: Records,
    groups: Vec<ComparisonGroup>,
    view: View,
    cursor: [usize; 8],
    selected_group: usize,
    selected_attempt: usize,
    history_order: Vec<usize>,
    ladder: Ladder,
    /// Coder One's components: isolated runs beside episode invocations.
    components: Option<crate::coder_components::Report>,
    /// Coder One's requirement maps.
    requirements: Option<crate::coder_requirements::Report>,
}

impl App {
    pub fn new(records: Records) -> Self {
        let groups = ComparisonGroup::from_records(&records);
        let mut history_order: Vec<usize> = (0..records.attempts.len()).collect();
        history_order.sort_by(|&left, &right| {
            records.attempts[right]
                .started_at
                .cmp(&records.attempts[left].started_at)
        });
        Self {
            records,
            groups,
            view: View::Overview,
            cursor: [0; 8],
            selected_group: 0,
            selected_attempt: 0,
            history_order,
            ladder: ladder_from_environment(),
            components: None,
            requirements: None,
        }
    }

    /// Adds the Requirements view's report.
    #[must_use]
    pub fn with_requirements(mut self, report: crate::coder_requirements::Report) -> Self {
        self.requirements = Some(report);
        self
    }

    /// Adds the Components view's report.
    #[must_use]
    pub fn with_components(mut self, report: crate::coder_components::Report) -> Self {
        self.components = Some(report);
        self
    }

    pub fn view(&self) -> View {
        self.view
    }
    pub fn open(&mut self, view: View) {
        self.view = view;
    }
    pub fn next(&mut self) {
        self.view = View::ALL[(self.view.index() + 1) % View::ALL.len()];
    }
    pub fn previous(&mut self) {
        self.view = View::ALL[(self.view.index() + View::ALL.len() - 1) % View::ALL.len()];
    }
    pub fn down(&mut self) {
        self.cursor[self.view.index()] = (self.cursor() + 1).min(self.length().saturating_sub(1));
    }
    pub fn up(&mut self) {
        self.cursor[self.view.index()] = self.cursor().saturating_sub(1);
    }
    pub fn home(&mut self) {
        self.cursor[self.view.index()] = 0;
    }
    pub fn end(&mut self) {
        self.cursor[self.view.index()] = self.length().saturating_sub(1);
    }
    pub fn cursor(&self) -> usize {
        self.cursor[self.view.index()].min(self.length().saturating_sub(1))
    }
    pub fn length(&self) -> usize {
        match self.view {
            View::Overview => self.groups.len(),
            View::Comparison => self
                .groups
                .get(self.selected_group)
                .map_or(0, |g| g.attempts.len()),
            View::Attempt => self.records.attempts.len(),
            View::Evidence => self.current().map_or(0, |a| a.evidence.len()),
            View::History => self.records.attempts.len(),
            View::Guide => 0,
            // The components report scrolls line by line.
            View::Components => self.components().len(),
            View::Requirements => self.requirements().len(),
        }
    }
    pub fn inspect(&mut self) {
        match self.view {
            View::Overview => {
                self.selected_group = self.cursor();
                self.cursor[View::Comparison.index()] = 0;
                self.view = View::Comparison;
            }
            View::Comparison => {
                if let Some(index) = self
                    .groups
                    .get(self.selected_group)
                    .and_then(|g| g.attempts.get(self.cursor()))
                {
                    self.selected_attempt = *index;
                    self.view = View::Attempt;
                }
            }
            View::History => {
                if let Some(index) = self.history_order.get(self.cursor()) {
                    self.selected_attempt = *index;
                    self.view = View::Attempt;
                }
            }
            View::Attempt => {
                self.view = View::Evidence;
            }
            View::Evidence => {}
            View::Guide | View::Components | View::Requirements => {}
        }
    }
    fn current(&self) -> Option<&Attempt> {
        self.records.attempts.get(self.selected_attempt)
    }

    pub fn to_text(&self, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        self.render(area, &mut buf);
        (0..height)
            .map(|y| {
                buf.content[(usize::from(y) * usize::from(width))
                    ..(usize::from(y + 1) * usize::from(width))]
                    .iter()
                    .map(|c| c.symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn print_height(&self) -> u16 {
        u16::try_from(self.lines().len().saturating_add(3)).unwrap_or(u16::MAX)
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_style(Style::new().bg(self.ladder.background()));
            }
        }
        if area.width < 48 || area.height < 8 {
            buf.set_string(
                area.left(),
                area.top(),
                "window too small",
                self.ladder.style(Intensity::Half),
            );
            return;
        }
        let header = format!(
            "gym / terminal-bench    {} attempts    {} groups    {} read errors",
            self.records.attempts.len(),
            self.groups.len(),
            self.records.errors.len()
        );
        buf.set_string(
            area.left() + 1,
            area.top(),
            clip(&header, usize::from(area.width - 2)),
            self.ladder.style(Intensity::Full),
        );
        let box_area = Rect::new(area.left(), area.top() + 1, area.width, area.height - 1);
        frame(box_area, buf, self.ladder.style(Intensity::ThreeQuarters));
        rail(
            box_area,
            buf,
            0,
            Some((self.view.title(), self.ladder.style(Intensity::Full))),
            Some((
                "local evidence · no inference",
                self.ladder.style(Intensity::Half),
            )),
        );
        let keys = "1-8 view  tab/h/l switch  j/k move  enter inspect  q quit";
        rail(
            box_area,
            buf,
            box_area.height - 1,
            Some((keys, self.ladder.style(Intensity::Quarter))),
            Some(("unknown ≠ 0", self.ladder.style(Intensity::Half))),
        );
        let inner = Rect::new(
            box_area.left() + 2,
            box_area.top() + 1,
            box_area.width - 4,
            box_area.height - 2,
        );
        let lines = self.lines();
        let selected = self.selected_line();
        let room = usize::from(inner.height);
        let scroll = selected
            .filter(|&line| line >= room)
            .map_or(0, |line| line + 1 - room);
        for (offset, line) in lines.iter().skip(scroll).take(room).enumerate() {
            let y = inner.top() + offset as u16;
            if selected == Some(scroll + offset) {
                for x in inner.left() - 1..inner.right() {
                    buf[(x, y)].set_style(Style::new().bg(self.ladder.selection()));
                }
                buf[(inner.left() - 1, y)].set_char('▸').set_style(
                    self.ladder
                        .style(Intensity::Full)
                        .bg(self.ladder.selection()),
                );
            }
            let tone = if selected == Some(scroll + offset) {
                Intensity::Full
            } else {
                Intensity::ThreeQuarters
            };
            buf.set_string(
                inner.left(),
                y,
                clip(line, usize::from(inner.width)),
                self.ladder.style(tone).bg(buf[(inner.left(), y)].bg),
            );
        }
    }

    fn selected_line(&self) -> Option<usize> {
        match self.view {
            View::Overview => Some(6 + self.cursor()),
            View::Comparison => Some(4 + self.cursor()),
            View::History => Some(3 + self.cursor()),
            View::Evidence => Some(2 + self.cursor()),
            View::Attempt | View::Guide => None,
            View::Components | View::Requirements => Some(self.cursor()),
        }
    }

    fn lines(&self) -> Vec<String> {
        match self.view {
            View::Overview => self.overview(),
            View::Comparison => self.comparison(),
            View::Attempt => self.attempt(),
            View::Evidence => self.evidence(),
            View::History => self.history(),
            View::Guide => self.guide(),
            View::Components => self.components(),
            View::Requirements => self.requirements(),
        }
    }

    fn overview(&self) -> Vec<String> {
        let mut lines = vec![
            format!("Sources: {}", self.records.sources.join(" · ")),
            format!("Status: {}", self.records.status_counts().iter().map(|(s,n)| format!("{s} {n}")).collect::<Vec<_>>().join(" · ")),
            format!("Controls: {} oracle/nop attempts; excluded from agent ranking", self.records.attempts.iter().filter(|a| a.is_control()).count()),
            format!("Usage coverage: {}", self.records.attempts.iter().fold(BTreeMap::<&str, usize>::new(), |mut counts, attempt| { *counts.entry(&attempt.usage_coverage).or_default() += 1; counts }).iter().map(|(coverage, count)| format!("{coverage} {count}")).collect::<Vec<_>>().join(" · ")),
            format!("Latest: {}  ·  report: {}", self.records.attempts.iter().filter_map(|a| a.started_at.as_ref()).max().map_or(DASH, String::as_str), self.records.report_label.as_deref().unwrap_or("not loaded")),
            "Task / arm                                    n  reward             status         cost source       evidence".to_owned(),
        ];
        for group in &self.groups {
            let members: Vec<_> = group
                .attempts
                .iter()
                .map(|&i| &self.records.attempts[i])
                .collect();
            let reward = members
                .iter()
                .map(|a| show::number(a.reward, 1))
                .collect::<Vec<_>>()
                .join(",");
            let statuses = members
                .iter()
                .map(|a| a.display_status())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(",");
            let sources = members
                .iter()
                .map(|a| a.cost_provenance.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(",");
            let health = members
                .iter()
                .map(|a| a.evidence_health())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(",");
            lines.push(format!(
                "{:<45} {:>2}  {:<18} {:<14} {:<17} {}",
                clip(&format!("{} / {}", group.task, group.arm), 45),
                members.len(),
                clip(&reward, 18),
                clip(&statuses, 14),
                clip(&sources, 17),
                health
            ));
        }
        lines.extend(
            self.records
                .report_warnings
                .iter()
                .map(|e| format!("REPORT WARNING: {e}")),
        );
        lines.extend(
            self.records
                .errors
                .iter()
                .map(|e| format!("READ ERROR: {e}")),
        );
        lines
    }

    fn comparison(&self) -> Vec<String> {
        let Some(group) = self.groups.get(self.selected_group) else {
            return vec!["No group selected".to_owned()];
        };
        let members: Vec<_> = group
            .attempts
            .iter()
            .map(|&i| &self.records.attempts[i])
            .collect();
        let rewarded = members.iter().filter(|a| a.reward.is_some()).count();
        let sum: f64 = members.iter().filter_map(|a| a.reward).sum();
        let fresh: Vec<_> = members
            .iter()
            .copied()
            .filter(|a| a.kind == "fresh" && a.reward.is_some())
            .collect();
        let binary = fresh.iter().all(|a| matches!(a.reward, Some(0.0 | 1.0)));
        let complete_pin = members.iter().all(|a| a.has_complete_comparison_identity());
        let uncertainty = if fresh.len() >= 3 && binary && complete_pin {
            let successes = fresh.iter().filter(|a| a.reward == Some(1.0)).count();
            let (low, high) = wilson_95(successes, fresh.len());
            format!(
                "{} fresh graded attempts; observed pass fraction {}/{}; Wilson 95% [{low:.2}, {high:.2}]. Small development sample.",
                fresh.len(),
                successes,
                fresh.len()
            )
        } else if !complete_pin {
            format!(
                "{} fresh graded attempts; pin, model, artifact, image state, or host identity is incomplete. No controlled interval.",
                fresh.len()
            )
        } else {
            format!(
                "{} fresh graded attempts; no pass-rate interval. These are development observations.",
                fresh.len()
            )
        };
        let mut lines = vec![
            format!(
                "{} / {}{} · {}",
                group.task,
                group.arm,
                if group.policy.is_some() {
                    format!(" (arms: {})", group.arms.join(", "))
                } else {
                    String::new()
                },
                group.pin
            ),
            format!("{} attempts; {} graded; reward mean {} over graded only", members.len(), rewarded, if rewarded == 0 { DASH.to_owned() } else { format!("{:.3}", sum / rewarded as f64) }),
            uncertainty,
            "Trial                    reward status              setup (cache)     agent/total       cost and source                  tokens in/out        evidence".to_owned(),
        ];
        for attempt in &members {
            lines.push(format!(
                "{:<24} {:>5}  {:<19} {:>7} {:<9} {:>7}/{:<7}  {:<32} {:>7}/{:<7} {}",
                clip(&attempt.trial, 24),
                show::number(attempt.reward, 1),
                clip(attempt.display_status(), 19),
                ms(attempt.phases_ms[1]),
                clip(&format!("({})", attempt.setup_cache_label()), 9),
                ms(attempt.phases_ms[2]),
                ms(attempt.phases_ms[4]),
                clip(&cost(attempt), 32),
                show::whole(attempt.input_tokens),
                show::whole(attempt.output_tokens),
                attempt.evidence_health()
            ));
        }
        if let Some(selected) = group
            .attempts
            .get(self.cursor())
            .and_then(|&index| self.records.attempts.get(index))
        {
            lines.push(format!(
                "Selected {}: model {} · artifact {} · profile {} · kind {}",
                selected.trial,
                show::words(selected.model.as_deref()),
                show::words(selected.artifact.as_deref()),
                selected.profile,
                selected.kind
            ));
            lines.push(format!(
                "Phases: environment {} · install {} · agent {} · verifier {} · total {}",
                ms(selected.phases_ms[0]),
                ms(selected.phases_ms[1]),
                ms(selected.phases_ms[2]),
                ms(selected.phases_ms[3]),
                ms(selected.phases_ms[4])
            ));
            lines.push(format!(
                "Calls: {}",
                selected
                    .counts
                    .iter()
                    .map(|(name, count)| format!("{name} {}", show::whole(*count)))
                    .collect::<Vec<_>>()
                    .join(" · ")
            ));
            for note in selected
                .notes
                .iter()
                .filter(|note| note.starts_with("Counts:"))
            {
                lines.push(note.clone());
            }
        }
        let setup_failures = members.iter().filter(|a| a.is_setup_failure()).count();
        let mut setups = BTreeMap::<&str, Vec<u64>>::new();
        for attempt in &members {
            if let Some(value) = attempt.phases_ms[1] {
                setups
                    .entry(attempt.setup_cache_label())
                    .or_default()
                    .push(value);
            }
        }
        lines.push(format!(
            "Setup (Harbor agent_setup): {} · {setup_failures} setup failures beside {rewarded} graded. Agent is agent_execution; total is trial start to finish.",
            if setups.is_empty() {
                DASH.to_owned()
            } else {
                setups
                    .iter()
                    .map(|(cache, values)| format!("{cache} {}", spread_u64(values)))
                    .collect::<Vec<_>>()
                    .join(" · ")
            }
        ));
        let times: Vec<_> = fresh.iter().filter_map(|a| a.phases_ms[2]).collect();
        let costs: Vec<_> = fresh.iter().filter_map(|a| a.cost_usd).collect();
        lines.push(format!("Observed spread: agent time {} · cost {}. Missing measurements stay out of each range.", spread_u64(&times), spread_f64(&costs)));
        lines.push(
            "Other arms on this task (separate pins and evidence identities stay separate):"
                .to_owned(),
        );
        for other in self
            .groups
            .iter()
            .filter(|other| other.task == group.task && other.arm != group.arm)
        {
            let members: Vec<_> = other
                .attempts
                .iter()
                .map(|&index| &self.records.attempts[index])
                .collect();
            let rewards = members
                .iter()
                .map(|a| show::number(a.reward, 1))
                .collect::<Vec<_>>()
                .join(",");
            let costs = members
                .iter()
                .map(|a| cost(a))
                .collect::<Vec<_>>()
                .join(",");
            let pin = if other.pin == group.pin {
                "same recorded pin"
            } else {
                "different or unknown pin"
            };
            lines.push(format!(
                "  {}  n={}  reward {}  cost {}  {}",
                other.arm,
                members.len(),
                rewards,
                costs,
                pin
            ));
        }
        lines
    }

    fn attempt(&self) -> Vec<String> {
        let Some(a) = self.current() else {
            return vec!["No attempt selected".to_owned()];
        };
        let mut lines = vec![
            format!("{} / {}   {}", a.job, a.trial, a.source),
            format!(
                "Task {}   profile {}   arm {}   attempt kind {}",
                a.task, a.profile, a.arm, a.kind
            ),
            format!("Commit {}", show::words(a.commit.as_deref())),
            format!("Checksum {}", show::words(a.checksum.as_deref())),
            format!("Architecture {}", show::words(a.architecture.as_deref())),
            format!("Host {}", show::words(a.host.as_deref())),
            format!("Image state {}", show::words(a.image_state.as_deref())),
            format!(
                "Agent {}   model {}   artifact {}",
                show::words(a.agent.as_deref()),
                show::words(a.model.as_deref()),
                show::words(a.artifact.as_deref())
            ),
            format!(
                "Reward {}   terminal status {}   started {}",
                show::number(a.reward, 3),
                a.display_status(),
                show::words(a.started_at.as_deref())
            ),
            format!(
                "Time: environment {}   install {}   agent {}   verifier {}   total {}",
                ms(a.phases_ms[0]),
                ms(a.phases_ms[1]),
                ms(a.phases_ms[2]),
                ms(a.phases_ms[3]),
                ms(a.phases_ms[4])
            ),
            format!(
                "Tokens: input {}   cached {}   output {}   coverage {}",
                show::whole(a.input_tokens),
                show::whole(a.cache_tokens),
                show::whole(a.output_tokens),
                a.usage_coverage
            ),
            format!(
                "Cost: {}   {}",
                a.cost_usd.map_or(DASH.to_owned(), |v| format!("${v:.4}")),
                a.cost_provenance
            ),
        ];
        for (name, amount, provenance) in &a.costs {
            lines.push(format!(
                "  {name}: {} ({provenance})",
                amount.map_or(DASH.to_owned(), |v| format!("${v:.4}"))
            ));
        }
        lines.extend(crate::coder_calls::lines(&a.ledger, a.deadline.as_ref()));
        lines.push(format!(
            "Counts: {}",
            a.counts
                .iter()
                .map(|(name, value)| format!("{name} {}", show::whole(*value)))
                .collect::<Vec<_>>()
                .join(" · ")
        ));
        lines.push(format!(
            "Evidence: {} items, {}{}. Press enter to inspect files.",
            a.evidence.len(),
            a.evidence_health(),
            missing_summary(a)
        ));
        match crate::timeline::for_attempt(a) {
            Some(Ok(timeline)) => lines.extend(timeline.lines()),
            Some(Err(error)) => lines.push(format!("Episode timeline unreadable: {error}")),
            None => {
                lines.push("Episode timeline: no trajectory or invocation log retained".to_owned())
            }
        }
        lines.extend(a.notes.iter().map(|note| format!("Note: {note}")));
        lines
    }

    fn evidence(&self) -> Vec<String> {
        let Some(a) = self.current() else {
            return vec!["No attempt selected".to_owned()];
        };
        let mut lines = vec![
            format!("{} / {}", a.job, a.trial),
            "Digest status is checked against retained bytes when a digest exists.".to_owned(),
        ];
        let missing = a.missing_evidence();
        if missing.is_empty() {
            lines.push("Missing: none; every referenced file is present.".to_owned());
        } else {
            lines.push(format!(
                "Missing {}: {}",
                missing.len(),
                missing
                    .iter()
                    .map(|e| e.kind.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        for evidence in &a.evidence {
            let path = evidence
                .path
                .as_ref()
                .map_or(DASH.to_owned(), |p| p.display().to_string());
            lines.push(format!(
                "{:<22} {:<16} {}",
                clip(&evidence.kind, 22),
                evidence.state.label(),
                clip(&path, 100)
            ));
        }
        if let Some(selected) = a.evidence.get(self.cursor()) {
            lines.push(format!(
                "Selected: {} ({})",
                selected.kind,
                selected.state.label()
            ));
            if let Some(path) = &selected.path {
                lines.extend(wrap(&format!("Path: {}", path.display()), 130));
            }
            if let Some(note) = &selected.note {
                lines.extend(wrap(&format!("Original: {note}"), 130));
            }
        }
        lines
    }

    fn history(&self) -> Vec<String> {
        let mut lines = vec![
            "All attempts stay visible, including failed, invalid, and unverifiable runs.".to_owned(),
            "Different pins, hosts, and missing identities require separate comparisons.".to_owned(),
            "Started                   task / arm                                      reward  status              job / trial".to_owned(),
        ];
        for &index in &self.history_order {
            let a = &self.records.attempts[index];
            lines.push(format!(
                "{:<25} {:<47} {:>5}   {:<19} {} / {}",
                clip(a.started_at.as_deref().unwrap_or(DASH), 25),
                clip(&format!("{} / {}", a.task, a.arm), 47),
                show::number(a.reward, 1),
                clip(a.display_status(), 19),
                a.job,
                a.trial
            ));
        }
        lines
    }

    fn components(&self) -> Vec<String> {
        self.components.as_ref().map_or_else(
            || {
                vec![
                    "No component report loaded. Record isolated runs with `coder-one component suite ID`."
                        .to_owned(),
                ]
            },
            |report| report.lines(None),
        )
    }

    fn requirements(&self) -> Vec<String> {
        self.requirements.as_ref().map_or_else(
            || {
                vec![
                    "No requirement maps loaded. Run `coder-one component suite task.requirements`."
                        .to_owned(),
                ]
            },
            |report| report.lines(None),
        )
    }

    fn guide(&self) -> Vec<String> {
        vec![
            "Runbooks and records".to_owned(),
            "  docs/coder/terminal-bench.md                   Harness: doctor, run, resume, inspect, compare".to_owned(),
            "  docs/terminal-bench/runbook.md                 Host, credentials, rate limits, prices, retention".to_owned(),
            "  docs/terminal-bench/coder-one-delegate-runbook.md   Opus and Luna delegate arms".to_owned(),
            "  docs/terminal-bench/README.md                  Every retained result and written analysis".to_owned(),
            "  docs/coder/terminal-bench-contract.md          Headless episode and evidence contract".to_owned(),
            "  docs/gym/terminal-bench-tui.md                  This terminal reader".to_owned(),
            String::new(),
            "Workflow".to_owned(),
            "  1. Run tbench doctor and confirm the task and agent pins.".to_owned(),
            "  2. Run oracle and nop controls before comparing agents.".to_owned(),
            "  3. Give every fresh trial a distinct job name; resume reuses a job.".to_owned(),
            "  4. Run tbench compare, retain evidence, and update the results page.".to_owned(),
            "  5. Return here to inspect rewards, costs, timings, and evidence health.".to_owned(),
            String::new(),
            "Reading rule: a verifier reward is separate from agent status; unknown is never zero.".to_owned(),
            "A price estimate or subscription list price is not an observed bill.".to_owned(),
            "One development trial is an observation, not a pass-rate or a win.".to_owned(),
        ]
    }
}

fn clip(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}
fn missing_summary(attempt: &Attempt) -> String {
    match attempt.missing_evidence().len() {
        0 => String::new(),
        n => format!(" ({n} missing)"),
    }
}

fn ms(value: Option<u64>) -> String {
    value.map_or(DASH.to_owned(), |v| format!("{:.1}s", v as f64 / 1000.0))
}
fn cost(a: &Attempt) -> String {
    a.cost_usd
        .map_or(format!("{DASH} ({})", a.cost_provenance), |v| {
            format!("${v:.4} ({})", a.cost_provenance)
        })
}
fn wrap(text: &str, width: usize) -> Vec<String> {
    text.chars()
        .collect::<Vec<_>>()
        .chunks(width)
        .map(|chunk| chunk.iter().collect())
        .collect()
}
fn spread_u64(values: &[u64]) -> String {
    match (values.iter().min(), values.iter().max()) {
        (Some(min), Some(max)) => format!("{} to {}", ms(Some(*min)), ms(Some(*max))),
        _ => DASH.to_owned(),
    }
}
fn spread_f64(values: &[f64]) -> String {
    if values.is_empty() {
        return DASH.to_owned();
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    format!("${min:.4} to ${max:.4}")
}
fn wilson_95(successes: usize, trials: usize) -> (f64, f64) {
    let p = successes as f64 / trials as f64;
    let z = 1.96_f64;
    let denominator = 1.0 + z * z / trials as f64;
    let center = (p + z * z / (2.0 * trials as f64)) / denominator;
    let radius = z
        * ((p * (1.0 - p) / trials as f64) + z * z / (4.0 * (trials * trials) as f64)).sqrt()
        / denominator;
    ((center - radius).max(0.0), (center + radius).min(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_components_view_shows_isolated_runs_and_episodes() {
        let records = Records::default();
        let report = crate::coder_components::report(None, &records);
        let mut app = App::new(records).with_components(report);
        app.open(View::Components);
        let text = app.to_text(150, 40);
        assert!(text.contains("Coder One components"), "{text}");
        assert!(text.contains("evidence.pack"), "{text}");
        assert!(text.contains("isolated: no recorded runs"), "{text}");
        assert_eq!(View::from_digit('7'), Some(View::Components));
    }

    #[test]
    fn unknown_and_zero_are_distinct_in_the_screen() {
        let mut a = crate::terminal_bench::Records::default();
        let mut unknown = crate::terminal_bench::test_attempt();
        unknown.reward = None;
        unknown.cost_usd = None;
        let mut zero = unknown.clone();
        zero.trial = "zero".to_owned();
        zero.reward = Some(0.0);
        zero.cost_usd = Some(0.0);
        a.attempts.extend([unknown, zero]);
        let mut app = App::new(a);
        app.inspect();
        let text = app.to_text(120, 20);
        assert!(text.contains("—"));
        assert!(text.contains("0.0"));
    }

    #[test]
    fn different_task_pins_do_not_share_a_comparison_group() {
        let mut records = Records::default();
        let mut first = crate::terminal_bench::test_attempt();
        first.checksum = Some("first".to_owned());
        let mut second = first.clone();
        second.trial = "second".to_owned();
        second.checksum = Some("second".to_owned());
        records.attempts = vec![first, second];
        let app = App::new(records);
        assert_eq!(app.groups.len(), 2);
        assert_ne!(app.groups[0].pin, app.groups[1].pin);
    }

    #[test]
    fn history_selection_opens_the_row_it_names() {
        let mut records = Records::default();
        let mut earlier = crate::terminal_bench::test_attempt();
        earlier.trial = "earlier".to_owned();
        earlier.started_at = Some("2026-09-21T00:00:00Z".to_owned());
        let mut later = earlier.clone();
        later.trial = "later".to_owned();
        later.started_at = Some("2026-09-22T00:00:00Z".to_owned());
        records.attempts = vec![earlier, later];
        let mut app = App::new(records);
        app.open(View::History);
        app.inspect();
        assert_eq!(app.view(), View::Attempt);
        assert_eq!(app.current().unwrap().trial, "later");
    }

    #[test]
    fn repeated_runs_without_host_identity_make_no_controlled_claim() {
        let mut records = Records::default();
        for index in 0..3 {
            let mut attempt = crate::terminal_bench::test_attempt();
            attempt.trial = format!("trial-{index}");
            attempt.kind = "fresh".to_owned();
            attempt.reward = Some(1.0);
            attempt.commit = Some("commit".to_owned());
            attempt.checksum = Some("checksum".to_owned());
            attempt.architecture = Some("amd64".to_owned());
            records.attempts.push(attempt);
        }
        let mut app = App::new(records);
        app.inspect();
        let text = app.to_text(150, 25);
        assert!(text.contains("No controlled interval"), "{text}");
        assert!(!text.contains("Wilson 95%"), "{text}");
    }

    #[test]
    fn zero_cost_unknown_cost_and_refusal_keep_their_labels() {
        let mut records = Records::default();
        let mut zero = crate::terminal_bench::test_attempt();
        zero.reward = Some(0.0);
        zero.cost_usd = Some(0.0);
        zero.cost_provenance = "provider_reported".to_owned();
        zero.status = "completed".to_owned();
        let mut refused = zero.clone();
        refused.trial = "refused".to_owned();
        refused.reward = None;
        refused.cost_usd = None;
        refused.cost_provenance = "unknown".to_owned();
        refused.status = "provider_refusal".to_owned();
        records.attempts = vec![zero, refused];
        let mut app = App::new(records);
        let overview = app.overview().join("\n");
        assert!(overview.contains("provider_refusal 1"), "{overview}");
        assert!(overview.contains("task failure 1"), "{overview}");
        app.inspect();
        let comparison = app.comparison().join("\n");
        assert!(
            comparison.contains("$0.0000 (provider_reported)"),
            "{comparison}"
        );
        assert!(comparison.contains("— (unknown)"), "{comparison}");
        assert!(comparison.contains("provider_refusal"), "{comparison}");
    }
}
