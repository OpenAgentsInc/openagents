//! Native controls project the shared board state and dispatch shared commands.
use super::{control, text};
use crate::model::Intent;
use coder_mobile::verse_surface::Command;
use gym_leaderboard::view::{Page, ReferenceView, TextView};
use rust_native::{Node, TextRole};
use verse::gym_results::Action;

#[derive(Default)]
pub struct Projection {
    pub nodes: Vec<Node<Intent>>,
    pub commands: Vec<Command>,
    serial: usize,
}

impl Projection {
    fn words(&mut self, value: impl AsRef<str>) {
        // Split long verified output instead of exceeding the semantic text bound.
        let chars: Vec<char> = value.as_ref().chars().collect();
        for chunk in chars.chunks(8000) {
            self.serial += 1;
            self.nodes.push(text(
                &format!("grid-panel-text-{}", self.serial),
                chunk.iter().collect::<String>(),
                TextRole::Body,
            ));
        }
    }

    fn button(&mut self, label: &str, command: Command) {
        let key = format!("panel-{}", self.commands.len());
        self.commands.push(command);
        self.nodes.push(control(&key, label));
    }

    fn action(&mut self, label: &str, action: Action) {
        self.button(label, Command::Results(action));
    }

    fn close(&mut self) {
        self.button("Close board", Command::Close);
    }

    pub fn gym(&mut self, view: verse::gym::BoardView) {
        self.close();
        self.words(format!(
            "Gym · {}{}",
            view.status,
            if view.stale {
                " · saved observation"
            } else {
                ""
            }
        ));
        self.words(format!("World public key: {}", view.public_key));
        self.nodes
            .push(control("copy-key", "Copy world public key"));
        self.nodes
            .push(control("connect", "Open Gym connection file"));
        if let Some(error) = view.error {
            self.words(error);
        }
        for notice in view.notices {
            self.words(notice);
        }
        if let Some(run) = view.selected_run {
            self.words(format!("{} · {:?} · {:?}\nProgress: {} / {}\nCost: {} USD · elapsed: {} ms\nSource: {}\nOrigin: {}", run.title, run.category, run.status, known(run.completed), known(run.total), known(run.cost_usd), known(run.elapsed_ms), run.source, run.provenance));
            for metric in run.metrics {
                self.words(format!(
                    "{} ({})\n{}",
                    metric.name,
                    metric.unit,
                    metric
                        .points
                        .iter()
                        .map(|p| format!("{}: {}", p.step, p.value))
                        .collect::<Vec<_>>()
                        .join(" · ")
                ));
            }
            self.button("Back to runs", Command::Back);
        } else if let Some(recipe) = view.selected_recipe {
            self.words(format!("{}\n{}\nRevision: {}\nBudget: {} ms · {} starts\nSpend limit: {} USD · hard cap: {}", recipe.title, recipe.detail, recipe.revision, recipe.budget.wall_ms, recipe.budget.max_starts, known(recipe.budget.spend_limit_usd), recipe.budget.spend_enforced));
            self.button("Confirm and launch this recipe", Command::Launch);
            self.button("Back to recipes", Command::Back);
        } else {
            for run in view.runs {
                self.button(
                    &format!("{} · {:?}", run.title, run.status),
                    Command::Run(run.id),
                );
            }
            for recipe in view.recipes {
                self.button(
                    &format!("Review {}", recipe.title),
                    Command::Recipe(recipe.id),
                );
            }
        }
        if let Some(launch) = view.launch {
            self.words(format!("Launch {} · {}", launch.request_id, launch.phase));
            if let Some(error) = launch.error {
                self.words(error);
                self.button("Retry this launch", Command::RetryLaunch);
            }
            if let Some(receipt) = launch.receipt {
                self.words(format!(
                    "Run {} · {:?} · exit {}",
                    receipt.run_id,
                    receipt.status,
                    known(receipt.exit_code)
                ));
            }
        }
    }

    fn reference(&mut self, reference: ReferenceView) {
        self.words(format!(
            "Reference: {}\n{}\n{}",
            reference.name, reference.rule, reference.conditions
        ));
    }

    fn output(&mut self, output: TextView) {
        self.words(output.text);
        if let Some(cut) = output.cut {
            self.words(cut);
        }
    }

    pub fn results(&mut self, view: verse::gym_results::ResultsView) {
        self.close();
        self.words(format!("RESULTS · {}", view.status));
        if view.can_back {
            self.action("Back", Action::Back);
        }
        if let Some(replay) = view.replay {
            self.words(replay);
        }
        if let Some(error) = view.error {
            self.words(error);
            self.action("Load again", Action::Retry);
        }
        match view.page {
            Some(Page::Boards(page)) => {
                if let Some(summary) = page.summary {
                    self.words(summary.text);
                }
                for board in page.rows {
                    self.words(board.accessibility);
                    if let Some(note) = board.headline_note {
                        self.words(note);
                    }
                    self.action(&board.title, Action::Board { id: board.id });
                }
                if let Some(footer) = page.footer {
                    self.words(footer);
                }
            }
            Some(Page::Board(page)) => {
                self.words(format!(
                    "{} · {}\n{}\n{}\n{}",
                    page.title, page.benchmark, page.question, page.summary, page.headline
                ));
                if let Some(note) = page.headline_note {
                    self.words(note);
                }
                for label in page.labels {
                    self.words(label.text);
                }
                for tally in page.tallies {
                    self.words(tally.text);
                }
                for spend in page.spend {
                    self.words(spend);
                }
                self.reference(page.reference);
                for caveat in page.caveats {
                    self.words(caveat.text);
                }
                self.action(
                    if page.caveats_open {
                        "Hide caveats"
                    } else {
                        "Show all caveats"
                    },
                    Action::Caveats {
                        open: !page.caveats_open,
                    },
                );
                for filter in page.filters {
                    self.action(
                        &format!(
                            "{} ({}){}",
                            filter.text,
                            filter.count,
                            if filter.selected { " ✓" } else { "" }
                        ),
                        Action::Filter {
                            filter: filter.filter,
                        },
                    );
                }
                for task in page.tasks {
                    self.words(task.accessibility);
                    for attempt in task.attempts {
                        self.action(&attempt.accessibility, Action::Attempt { id: attempt.id });
                    }
                }
            }
            Some(Page::Attempt(page)) => {
                self.words(format!(
                    "{}\n{}\n{} · {}\n{}",
                    page.board_title, page.accessibility, page.series, page.trial, page.misses
                ));
                for line in page
                    .numbers
                    .into_iter()
                    .chain(page.phases)
                    .chain(page.failed_tests)
                {
                    self.words(line);
                }
                self.reference(page.reference);
                for line in [page.how_it_ended, page.jev, page.verifier]
                    .into_iter()
                    .flatten()
                {
                    self.words(line);
                }
                for caveat in page.caveats {
                    self.words(caveat.text);
                }
                if let Some(label) = page.trace {
                    self.action(&label, Action::Trace);
                }
            }
            Some(Page::Trace(page)) => {
                self.words(page.header.accessibility);
                self.words(&page.clock.text);
                self.action(
                    if page.clock.playing {
                        "Pause replay"
                    } else {
                        "Play replay"
                    },
                    Action::Play {
                        playing: !page.clock.playing,
                    },
                );
                self.action("Previous step", Action::Step { forward: false });
                self.action("Next step", Action::Step { forward: true });
                self.action("Start", Action::Seek { fraction: 0.0 });
                self.action("End", Action::Seek { fraction: 1.0 });
                for tab in page.tabs {
                    if tab.available {
                        self.action(tab.text, Action::Tab { tab: tab.tab });
                    }
                }
                if let Some(jev) = page.jev {
                    self.words(format!(
                        "{}\n{}\nKeep threshold: {} · flag threshold: {}",
                        jev.summary, jev.question_set, jev.keep_threshold, jev.flag_threshold
                    ));
                    for line in jev.questions {
                        self.words(line);
                    }
                    for candidate in jev.candidates {
                        self.words(candidate.accessibility);
                    }
                    for requirement in jev.requirements {
                        self.words(requirement.accessibility);
                    }
                }
                if let Some(briefing) = page.briefing {
                    self.output(briefing);
                }
                if let Some(agent) = page.agent {
                    self.words(format!(
                        "Page {} of {} · {}",
                        agent.page + 1,
                        agent.pages,
                        agent.tokens
                    ));
                    if agent.page > 0 {
                        self.action(
                            "Previous page",
                            Action::Page {
                                page: agent.page - 1,
                            },
                        );
                    }
                    if agent.page + 1 < agent.pages {
                        self.action(
                            "Next page",
                            Action::Page {
                                page: agent.page + 1,
                            },
                        );
                    }
                    for step in agent.rows {
                        self.words(format!("{}\n{}", step.accessibility, step.text));
                        if step.expandable {
                            self.action(
                                if step.output.is_some() {
                                    "Collapse output"
                                } else {
                                    "Expand output"
                                },
                                Action::Expand {
                                    index: step.output.is_none().then_some(step.index),
                                },
                            );
                        }
                        if let Some(output) = step.output {
                            self.output(output);
                        }
                        if let Some(cut) = step.cut {
                            self.words(cut);
                        }
                    }
                }
                if let Some(verifier) = page.verifier {
                    self.words(verifier.summary);
                    for test in verifier.tests {
                        self.words(format!("{} · {}", test.name, test.status));
                    }
                    self.output(verifier.output_tail);
                }
            }
            None => {}
        }
    }

    /// The pylon league: one block per hardware class, worded from the
    /// same league `openagents pylon league` prints.
    fn league(&mut self, view: verse::gym_league::View) {
        self.words(format!(
            "PYLON LEAGUE · {}{}\n{}",
            view.state,
            if view.relay.is_empty() {
                String::new()
            } else {
                format!(
                    " · {} · {} trusted checker{}",
                    view.relay,
                    view.checkers,
                    if view.checkers == 1 { "" } else { "s" }
                )
            },
            view.note
        ));
        if let Some(empty) = view.empty {
            self.words(empty);
        }
        for board in view.boards {
            let lines = board
                .lines
                .into_iter()
                .map(|l| {
                    format!(
                        "{} · {} · pass {} ({})\n{} jobs · median {} · {} · {}{}",
                        l.label,
                        l.model,
                        l.pass,
                        l.checks,
                        l.jobs,
                        l.median,
                        l.cost,
                        l.standing,
                        if l.sigil { " · sigil" } else { "" },
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            self.words(format!("{} · {}\n\n{}", board.class, board.suite, lines));
        }
    }

    pub fn evals(&mut self, view: verse::gym_hall::View, league: Option<verse::gym_league::View>) {
        self.close();
        self.words(format!(
            "EVALS · {}\n{}",
            view.state,
            view.note.replace("on this phone", "on this device")
        ));
        self.words(view.empty);
        for group in view.board.groups {
            self.words(format!(
                "{} · author {} · release {}",
                group.test_set, group.author_tag, group.release
            ));
            let rows = group
                .rows
                .into_iter()
                .map(|r| {
                    format!(
                        "{} · {} · {}\n{}\nTrainer {}{} · {} XP · {}\n{}",
                        r.tool,
                        r.verdict_words,
                        r.verdict,
                        r.headline,
                        r.trainer_tag,
                        if r.mine { " (you)" } else { "" },
                        r.credit_xp,
                        if r.hosted {
                            "hosted"
                        } else {
                            "trainer computer"
                        },
                        r.checks
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            self.words(rows);
            if group.more > 0 {
                self.words(format!(
                    "{} older results outside the board's display limit",
                    group.more
                ));
            }
        }
        if let Some(league) = league {
            self.league(league);
        }
        self.button(
            if view.notes_on {
                "Stop comparing notes"
            } else {
                "Compare notes"
            },
            Command::Notes(!view.notes_on),
        );
        self.words(view.notes_note);
        if view.notes_on {
            self.words(view.notes_empty);
            for note in view.notes {
                self.words(format!(
                    "{}{}: {}",
                    note.author_tag,
                    if note.mine { " (you)" } else { "" },
                    note.text
                ));
            }
        }
    }
}

fn known(value: Option<impl std::fmt::Display>) -> String {
    value.map_or_else(|| "not reported".into(), |v| v.to_string())
}
