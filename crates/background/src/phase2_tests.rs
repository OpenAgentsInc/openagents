//! Phase 2: rules compiled from conversation with a stand-in Jev, golden
//! rules for the spec's examples, and the general engine's triggers and
//! conditions with an injected clock, file events, and task endings.
//! Nothing here reads or changes the real home or asks a live model.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use crate::compile::{self, Compiled, Context, Kind};
use crate::engine::{self, Answer, Clock, Event, Judge, Question, StepOutcome};
use crate::inuse::{Processes, Snapshot};
use crate::paths::Layout;
use crate::plan::{Env, TaskFact};
use crate::rule::{Action, Class, Condition, GB, Level, Origin, Rule, TaskOutcome, Trigger, disk};
use crate::run::Cause;
use crate::store;
use crate::volume::{Space, Volumes};

/// A stand-in for Jev: each question's answer is the option it names at
/// the probability it names; any other question gets no answer.
struct Stand(Vec<(&'static str, &'static str, f64)>);

impl Judge for Stand {
    fn ask(
        &self,
        _state: &str,
        questions: &[(String, Question)],
    ) -> Result<BTreeMap<String, Answer>, String> {
        let mut answers = BTreeMap::new();
        for (id, question) in questions {
            let Some((_, pick, p)) = self.0.iter().find(|(q, _, _)| q == id) else {
                continue;
            };
            let answer = match question {
                Question::Noul(_) => Answer {
                    noul: Some(*p),
                    choice: Vec::new(),
                },
                Question::Choice { options, .. } => {
                    assert!(
                        options.iter().any(|(name, _)| name == pick),
                        "{id} has no option {pick}"
                    );
                    Answer {
                        noul: None,
                        choice: options
                            .iter()
                            .map(|(name, _)| {
                                let share = if name == pick {
                                    *p
                                } else {
                                    (1.0 - p) / (options.len() as f64 - 1.0).max(1.0)
                                };
                                (name.clone(), share)
                            })
                            .collect(),
                    }
                }
            };
            answers.insert(id.clone(), answer);
        }
        Ok(answers)
    }
}

struct Fixed {
    free: u64,
    total: u64,
}

impl Volumes for Fixed {
    fn space(&self, path: &Path) -> std::io::Result<Space> {
        use std::os::unix::fs::MetadataExt;
        Ok(Space {
            device: std::fs::metadata(path)?.dev(),
            free: self.free,
            total: self.total,
        })
    }
}

struct Idle;

impl Processes for Idle {
    fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(Snapshot::default())
    }
}

/// 2026-10-02 10:00 UTC, in a UTC zone.
const TEN_AM: u64 = 1_790_935_200;

fn clock(now: u64) -> Clock {
    Clock { now, offset: 0 }
}

struct Scratch {
    _dir: tempfile::TempDir,
    layout: Layout,
}

fn scratch() -> Scratch {
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::new(dir.path(), None).unwrap();
    std::fs::create_dir_all(&layout.openagents).unwrap();
    std::fs::create_dir_all(dir.path().join("work")).unwrap();
    Scratch { _dir: dir, layout }
}

fn context(project: Option<&Path>) -> Context {
    Context {
        thread: "thread-1".into(),
        project: project.map(Path::to_owned),
        clock: clock(TEN_AM),
        new_only: false,
    }
}

fn compiled(layout: &Layout, message: &str, stand: Stand) -> Compiled {
    let home = layout.home.clone();
    compile::compile(
        layout,
        message,
        &context(Some(&home.join("work/openagents"))),
        &stand,
    )
    .unwrap()
}

fn drafted(compiled: Compiled) -> compile::Draft {
    match compiled {
        Compiled::Draft(draft) => *draft,
        other => panic!("not a draft: {other:?}"),
    }
}

fn define(what: &'static str) -> Stand {
    Stand(vec![
        ("intent", "define", 0.95),
        ("rule", "new", 0.9),
        ("what", what, 0.92),
        ("when", "unstated", 0.8),
        ("part_of_day", "unstated", 0.8),
    ])
}

fn edit_disk(change: &'static str) -> Stand {
    Stand(vec![
        ("intent", "edit", 0.93),
        ("rule", "disk", 0.95),
        ("change", change, 0.9),
        ("class", "worktrees", 0.85),
    ])
}

// Golden compiled rules for the spec's examples.

#[test]
fn keep_my_disk_above_50_gb_is_a_disk_rule_of_its_own() {
    let s = scratch();
    let draft = drafted(compiled(
        &s.layout,
        "keep my disk above 50 GB free; clear old build caches first",
        define("free_space"),
    ));
    assert_eq!(draft.kind, Kind::Define);
    let rule = &draft.rule;
    assert_eq!(rule.id, "keep-free-50gb");
    assert_eq!(rule.name, "Keep 50 GB free");
    assert!(
        rule.enabled,
        "a rule asked for in conversation is on once confirmed"
    );
    assert_eq!(
        rule.origin,
        Origin::Conversation {
            thread: "thread-1".into(),
            message: "keep my disk above 50 GB free; clear old build caches first".into(),
        }
    );
    assert_eq!(
        rule.goal.start,
        Level {
            bytes: 50 * GB,
            percent: 0
        }
    );
    assert_eq!(
        rule.goal.stop,
        Level {
            bytes: 60 * GB,
            percent: 0
        }
    );
    assert_eq!(rule.goal.emergency.bytes, 10 * GB);
    assert_eq!(rule.actions, disk().actions);
    assert_eq!(rule.triggers, disk().triggers);
    rule.validate().unwrap();
    let card = compile::card(&draft).join("\n");
    assert!(
        card.contains("New background rule: Keep 50 GB free"),
        "{card}"
    );
    assert!(card.contains("If: free space is below 50 GB."), "{card}");
}

#[test]
fn only_keep_2_agent_target_dirs_edits_the_disk_rule() {
    let s = scratch();
    let draft = drafted(compiled(
        &s.layout,
        "only keep 2 agent target dirs",
        edit_disk("keep_count"),
    ));
    assert_eq!(draft.kind, Kind::Edit);
    assert_eq!(draft.rule.classes.keep, 2);
    let mut expected = disk();
    expected.classes.keep = 2;
    // Asking for the rule to work a way turns the off built-in rule on.
    expected.enabled = true;
    assert_eq!(draft.rule, expected);
    let diff = compile::diff(draft.before.as_ref().unwrap(), &draft.rule);
    assert_eq!(
        diff,
        vec![
            "- Status: off.",
            "+ Keeps: the 2 most recently used agent build folders.",
            "+ Status: on.",
        ]
    );
}

#[test]
fn an_unsure_intent_is_settled_only_by_agreeing_sure_readings() {
    let s = scratch();
    // A sure new action over a sure "no listed rule" is a new rule.
    let new = Stand(vec![
        ("intent", "define", 0.5),
        ("rule", "new", 0.9),
        ("what", "task_failed", 0.95),
    ]);
    let draft = drafted(compiled(
        &s.layout,
        "tell me whenever a coder run fails",
        new,
    ));
    assert_eq!(draft.kind, Kind::Define);
    // A sure change of a sure listed rule is an edit.
    let edit = Stand(vec![
        ("intent", "define", 0.4),
        ("rule", "disk", 0.9),
        ("change", "keep_count", 0.9),
    ]);
    let draft = drafted(compiled(&s.layout, "only keep 2 agent target dirs", edit));
    assert_eq!(draft.kind, Kind::Edit);
    // Readings that do not agree still ask.
    let unsure = Stand(vec![
        ("intent", "define", 0.5),
        ("rule", "new", 0.5),
        ("what", "task_failed", 0.95),
    ]);
    assert!(matches!(
        compiled(&s.layout, "runs failing", unsure),
        Compiled::Question { .. }
    ));
}

#[test]
fn never_touch_a_folder_adds_it_to_the_deny_list() {
    let s = scratch();
    let draft = drafted(compiled(
        &s.layout,
        "never touch ~/.openagents/pylon",
        edit_disk("deny_path"),
    ));
    assert_eq!(draft.rule.safety.deny, vec!["~/.openagents/pylon"]);
}

#[test]
fn keep_200_gb_free_raises_start_and_stop() {
    let s = scratch();
    let draft = drafted(compiled(
        &s.layout,
        "keep 200 GB free",
        edit_disk("free_level"),
    ));
    let goal = &draft.rule.goal;
    assert_eq!(
        goal.start,
        Level {
            bytes: 200 * GB,
            percent: 0
        }
    );
    assert_eq!(
        goal.stop,
        Level {
            bytes: 240 * GB,
            percent: 0
        }
    );
    assert!(goal.emergency.bytes <= goal.start.bytes);
    draft.rule.validate().unwrap();
}

#[test]
fn clean_old_build_caches_first_is_already_the_order() {
    let s = scratch();
    let compiled = compiled(
        &s.layout,
        "clean old build caches first",
        edit_disk("already"),
    );
    let Compiled::Unchanged { text } = compiled else {
        panic!("{compiled:?}");
    };
    assert!(text.starts_with("Disk cleanup already does that"), "{text}");
}

#[test]
fn dont_delete_worktrees_just_tell_me_reports_them_only() {
    let s = scratch();
    let draft = drafted(compiled(
        &s.layout,
        "don't delete worktrees, just tell me",
        edit_disk("report_only"),
    ));
    assert_eq!(draft.rule.classes.report_only, vec![Class::Worktrees]);
}

#[test]
fn pause_until_tomorrow_pauses_until_midnight() {
    let s = scratch();
    let stand = Stand(vec![
        ("intent", "pause", 0.94),
        ("rule", "disk", 0.95),
        ("pause_for", "day", 0.88),
    ]);
    let draft = drafted(compiled(
        &s.layout,
        "pause disk cleanup until tomorrow",
        stand,
    ));
    assert_eq!(draft.kind, Kind::Pause);
    assert!(draft.rule.enabled);
    assert_eq!(draft.rule.paused_until, Some(TEN_AM - 10 * 3600 + 86_400));
}

#[test]
fn every_morning_pull_main_keeps_the_project_up_to_date() {
    let s = scratch();
    let stand = Stand(vec![
        ("intent", "define", 0.9),
        ("rule", "new", 0.9),
        ("what", "git_update", 0.9),
        ("when", "daily", 0.9),
        ("part_of_day", "morning", 0.9),
    ]);
    let draft = drafted(compiled(
        &s.layout,
        "every morning pull main on CoderOS",
        stand,
    ));
    let rule = &draft.rule;
    assert_eq!(rule.id, "update-openagents");
    assert_eq!(rule.triggers, vec![Trigger::Daily { at: "09:00".into() }]);
    assert_eq!(
        rule.actions,
        vec![Action::GitFastForward {
            repo: "~/work/openagents".into(),
            branch: None,
        }]
    );
    // A time in the words wins over the part of the day.
    let stand = Stand(vec![
        ("intent", "define", 0.9),
        ("what", "git_update", 0.9),
        ("when", "daily", 0.9),
        ("part_of_day", "morning", 0.9),
    ]);
    let draft = drafted(compiled(
        &s.layout,
        "pull ~/work/psionic every day at 7:30am",
        stand,
    ));
    assert_eq!(
        draft.rule.triggers,
        vec![Trigger::Daily { at: "07:30".into() }]
    );
    assert_eq!(
        draft.rule.actions,
        vec![Action::GitFastForward {
            repo: "~/work/psionic".into(),
            branch: None,
        }]
    );
}

#[test]
fn tell_me_when_a_coder_run_fails_reads_each_ended_task() {
    let s = scratch();
    let draft = drafted(compiled(
        &s.layout,
        "tell me when a Coder run fails",
        define("task_failed"),
    ));
    let rule = &draft.rule;
    assert_eq!(rule.id, "notify-run-failed");
    assert_eq!(rule.triggers, vec![Trigger::TaskEnded]);
    assert_eq!(
        rule.conditions,
        vec![Condition::TaskOutcome {
            outcomes: vec![TaskOutcome::Failed]
        }]
    );
    assert!(!rule.cleans());
    assert_eq!(rule.cooldown_secs, 0);
}

// Below the setting, or missing a field: one question, never a rule.

#[test]
fn an_unsure_reading_asks_instead_of_guessing() {
    let s = scratch();
    let unsure = Stand(vec![
        ("intent", "define", 0.45),
        ("what", "free_space", 0.9),
    ]);
    let Compiled::Question { text, .. } = compiled(&s.layout, "disk stuff", unsure) else {
        panic!("not a question");
    };
    assert!(text.contains("background rule"), "{text}");
    // A sure intent but an unsure action.
    let unsure_what = Stand(vec![("intent", "define", 0.9), ("what", "free_space", 0.5)]);
    assert!(matches!(
        compiled(&s.layout, "keep things tidy", unsure_what),
        Compiled::Question { .. }
    ));
    // A sure edit of a rule the reading cannot name.
    let unsure_rule = Stand(vec![
        ("intent", "edit", 0.9),
        ("rule", "disk", 0.4),
        ("change", "keep_count", 0.9),
    ]);
    let Compiled::Question { text, .. } = compiled(&s.layout, "keep 2", unsure_rule) else {
        panic!("not a question");
    };
    assert!(text.starts_with("Which rule do you mean"), "{text}");
    // No size in the words.
    let Compiled::Question { text, .. } = compiled(
        &s.layout,
        "keep my disk from filling up",
        define("free_space"),
    ) else {
        panic!("not a question");
    };
    assert!(text.contains("How much free space"), "{text}");
    // Something no built-in action does.
    let Compiled::Question { text, .. } = compiled(
        &s.layout,
        "run my backup script every night",
        define("unsupported"),
    ) else {
        panic!("not a question");
    };
    assert!(text.contains("never another command"), "{text}");
    // One-off.
    let once = Stand(vec![("intent", "none", 0.9)]);
    assert!(matches!(
        compiled(&s.layout, "clean my disk now", once),
        Compiled::Question { .. }
    ));
}

// Drafts: saved only once confirmed; an edit applies only to the rule it
// was drafted against.

#[test]
fn a_draft_is_saved_only_when_applied() {
    let s = scratch();
    let draft = drafted(compiled(
        &s.layout,
        "tell me when a Coder run fails",
        define("task_failed"),
    ));
    compile::save_draft(&s.layout, &draft).unwrap();
    assert!(store::load(&s.layout, "notify-run-failed").is_err());
    assert!(compile::drafted(&s.layout, "thread-1", TEN_AM));
    let (_, saved) = compile::apply(&s.layout, "thread-1", TEN_AM + 5).unwrap();
    let saved = saved.unwrap();
    assert_eq!(store::load(&s.layout, "notify-run-failed").unwrap(), saved);
    assert!(!compile::drafted(&s.layout, "thread-1", TEN_AM));
    let ids: Vec<String> = store::list(&s.layout)
        .into_iter()
        .filter_map(Result::ok)
        .map(|rule| rule.id)
        .filter(|id| id == "disk" || crate::rule::built_in(id).is_none())
        .collect();
    assert_eq!(ids, vec!["disk", "notify-run-failed"]);
    // A second rule of the same kind gets its own id.
    let again = drafted(compiled(
        &s.layout,
        "tell me when a Coder run fails",
        define("task_failed"),
    ));
    assert_eq!(again.rule.id, "notify-run-failed-2");
    // Removing it is its own draft, and only rules made here are removed.
    let remove = Stand(vec![
        ("intent", "remove", 0.9),
        ("rule", "notify-run-failed", 0.9),
    ]);
    let draft = drafted(compiled(&s.layout, "get rid of the run alerts", remove));
    assert_eq!(draft.kind, Kind::Remove);
    compile::save_draft(&s.layout, &draft).unwrap();
    compile::apply(&s.layout, "thread-1", TEN_AM + 9).unwrap();
    assert!(store::load(&s.layout, "notify-run-failed").is_err());
    let remove_disk = Stand(vec![("intent", "remove", 0.9), ("rule", "disk", 0.9)]);
    let draft = drafted(compiled(&s.layout, "remove disk cleanup", remove_disk));
    assert_eq!(draft.kind, Kind::Pause);
    assert!(!draft.rule.enabled);
    assert!(
        draft
            .note
            .unwrap()
            .contains("turned off rather than removed")
    );
}

#[test]
fn an_edit_drafted_against_an_older_rule_is_refused() {
    let s = scratch();
    let draft = drafted(compiled(
        &s.layout,
        "only keep 2 agent target dirs",
        edit_disk("keep_count"),
    ));
    compile::save_draft(&s.layout, &draft).unwrap();
    let mut changed = disk();
    changed.classes.idle_days = 5;
    store::save(&s.layout, &changed).unwrap();
    let refused = compile::apply(&s.layout, "thread-1", TEN_AM).unwrap_err();
    assert!(refused.contains("changed since"), "{refused}");
    // A stale draft is refused too.
    compile::save_draft(&s.layout, &draft).unwrap();
    assert!(compile::apply(&s.layout, "thread-1", TEN_AM + 2 * 86_400).is_err());
}

#[test]
fn a_conversation_rule_cleans_only_where_the_host_does() {
    let s = scratch();
    let mut rule = drafted(compiled(&s.layout, "keep 50 GB free", define("free_space"))).rule;
    rule.safety.allow.push("~/Documents".into());
    assert!(rule.validate().unwrap_err().contains("outside"));
    let mut plugin = disk();
    plugin.id = "update".into();
    plugin.actions = vec![Action::GitFastForward {
        repo: "~/work/x".into(),
        branch: None,
    }];
    let installed = crate::plugins::Installed {
        id: "k:s".into(),
        slug: "s".into(),
        name: "S".into(),
        summary: String::new(),
        version: "1".into(),
        dir: s.layout.home.clone(),
        background: vec!["update".into()],
        classes: Vec::new(),
        enabled: true,
    };
    assert!(
        crate::plugins::admit(plugin, &installed)
            .unwrap_err()
            .contains("cannot update a Git checkout")
    );
}

// Bounded fields.

#[test]
fn bounded_fields_read_sizes_counts_durations_times_and_paths() {
    use compile::fields::{count, date, duration, paths, percent, size, time_of_day};
    assert_eq!(size("keep 50 GB free"), Some(50 * GB));
    assert_eq!(size("above 50gb"), Some(50 * GB));
    assert_eq!(size("1.5 TB"), Some(1_500 * GB));
    assert_eq!(size("keep 2 agent dirs"), None);
    assert_eq!(percent("below 10%"), Some(10));
    assert_eq!(count("only keep 2 agent target dirs"), Some(2));
    assert_eq!(count("keep three of them"), Some(3));
    assert_eq!(count("keep 50 GB"), None);
    assert_eq!(duration("every 10 minutes"), Some(600));
    assert_eq!(duration("every hour"), Some(3600));
    assert_eq!(duration("for 2h"), Some(7200));
    assert_eq!(duration("every morning"), None);
    assert_eq!(time_of_day("at 7am"), Some("07:00".into()));
    assert_eq!(time_of_day("at 7:30 pm"), Some("19:30".into()));
    assert_eq!(time_of_day("at 19:05"), Some("19:05".into()));
    assert_eq!(time_of_day("at 12am"), Some("00:00".into()));
    assert_eq!(time_of_day("keep 2"), None);
    assert_eq!(date("until 2026-10-03"), Some(1_790_985_600));
    assert_eq!(
        paths("never touch ~/.openagents/pylon, or /tmp/x."),
        vec!["~/.openagents/pylon", "/tmp/x"]
    );
}

// The engine: triggers with an injected clock and file events.

#[test]
fn daily_runs_once_after_its_time_and_catches_up_after_sleep() {
    // First sight only sets the baseline.
    assert!(!engine::daily_due("09:00", None, clock(TEN_AM)));
    // Ran yesterday at 09:00: today's 09:00 has passed.
    assert!(engine::daily_due(
        "09:00",
        Some(TEN_AM - 86_400 - 3600),
        clock(TEN_AM)
    ));
    // Ran at 09:00 today: not again until tomorrow.
    assert!(!engine::daily_due(
        "09:00",
        Some(TEN_AM - 3600),
        clock(TEN_AM)
    ));
    assert!(!engine::daily_due(
        "11:00",
        Some(TEN_AM - 3600),
        clock(TEN_AM)
    ));
    // Asleep for three days: due once now, then marked.
    assert!(engine::daily_due(
        "09:00",
        Some(TEN_AM - 3 * 86_400),
        clock(TEN_AM)
    ));
    assert!(!engine::daily_due(
        "09:00",
        Some(TEN_AM),
        clock(TEN_AM + 60)
    ));
    // A zone east of UTC: 09:00 local is 07:00 UTC.
    let east = Clock {
        now: TEN_AM,
        offset: 2 * 3600,
    };
    assert_eq!(east.last(9 * 60), TEN_AM - 3 * 3600);
    assert_eq!(east.minute(), 12 * 60);
}

#[test]
fn intervals_are_jittered_by_at_most_a_tenth() {
    let mut seen = std::collections::BTreeSet::new();
    for (n, id) in ["disk", "a", "b", "keep-free-50gb", "notify"]
        .iter()
        .enumerate()
    {
        let at = engine::next_interval(id, 300, TEN_AM + n as u64);
        let wait = at - (TEN_AM + n as u64);
        assert!((300..=330).contains(&wait), "{wait}");
        seen.insert(wait);
    }
    assert!(seen.len() > 1, "rules started together spread out");
}

#[test]
fn file_events_fire_on_change_not_on_first_sight() {
    let s = scratch();
    let watched = s.layout.home.join("work/watched.txt");
    std::fs::write(&watched, "one").unwrap();
    let mut rule = disk();
    rule.id = "watch".into();
    rule.triggers = vec![Trigger::FsEvent {
        paths: vec!["~/work/watched.txt".into()],
    }];
    let rules = vec![rule];
    assert!(engine::poll_files(&s.layout, &rules).is_empty());
    assert!(engine::poll_files(&s.layout, &rules).is_empty());
    std::fs::write(&watched, "two, longer").unwrap();
    assert_eq!(
        engine::poll_files(&s.layout, &rules),
        vec![("watch".to_owned(), vec!["~/work/watched.txt".to_owned()])]
    );
    assert!(engine::poll_files(&s.layout, &rules).is_empty());
    std::fs::remove_file(&watched).unwrap();
    assert_eq!(engine::poll_files(&s.layout, &rules).len(), 1);
    // The pure comparison.
    let before = BTreeMap::from([("a".to_owned(), engine::Seen::default())]);
    let now = BTreeMap::from([
        (
            "a".to_owned(),
            engine::Seen {
                exists: true,
                size: 1,
                modified: 1,
            },
        ),
        ("b".to_owned(), engine::Seen::default()),
    ]);
    assert_eq!(engine::changed(&before, &now), vec!["a"]);
}

// Conditions.

fn notify_rule(conditions: Vec<Condition>) -> Rule {
    let mut rule = disk();
    rule.id = "notify".into();
    rule.name = "Notify".into();
    rule.origin = Origin::Conversation {
        thread: "t".into(),
        message: "m".into(),
    };
    rule.enabled = true;
    rule.triggers = vec![Trigger::TaskEnded, Trigger::Interval { every_secs: 300 }];
    rule.conditions = conditions;
    rule.actions = vec![Action::Notify {
        text: "A Coder run {outcome}: task {task}. {free} free.".into(),
    }];
    rule.cooldown_secs = 0;
    rule
}

fn ended(id: &str, failed: bool) -> TaskFact {
    TaskFact {
        id: id.into(),
        ended: true,
        failed,
        ..TaskFact::default()
    }
}

#[test]
fn conditions_hold_or_say_why_not() {
    let s = scratch();
    let volumes = Fixed {
        free: 20 * GB,
        total: 1_000 * GB,
    };
    let tasks = || -> Result<Vec<TaskFact>, String> {
        Ok(vec![TaskFact {
            id: "running".into(),
            running: true,
            ..TaskFact::default()
        }])
    };
    let env = Env {
        layout: &s.layout,
        facts: Some(&tasks),
        volumes: &volumes,
        processes: &Idle,
        now: TEN_AM,
        kache: None,
    };
    let failed = Event {
        task: Some(ended("aaaabbbbccccdddd", true)),
        paths: Vec::new(),
    };
    let fine = Event {
        task: Some(ended("aaaabbbbccccdddd", false)),
        paths: Vec::new(),
    };
    let on_failure = notify_rule(vec![Condition::TaskOutcome {
        outcomes: vec![TaskOutcome::Failed],
    }]);
    assert!(engine::holds(&env, &on_failure, &failed, clock(TEN_AM), None).is_ok());
    assert!(engine::holds(&env, &on_failure, &fine, clock(TEN_AM), None).is_err());
    assert!(engine::holds(&env, &on_failure, &Event::default(), clock(TEN_AM), None).is_err());
    let low = |gb| {
        notify_rule(vec![Condition::FreeBelow {
            level: Level {
                bytes: gb * GB,
                percent: 0,
            },
        }])
    };
    assert!(engine::holds(&env, &low(30), &Event::default(), clock(TEN_AM), None).is_ok());
    let why = engine::holds(&env, &low(10), &Event::default(), clock(TEN_AM), None).unwrap_err();
    assert_eq!(why, "20 GB free, not below 10 GB");
    let idle = notify_rule(vec![Condition::NoTaskRunning]);
    assert!(engine::holds(&env, &idle, &Event::default(), clock(TEN_AM), None).is_err());
    let night = notify_rule(vec![Condition::TimeBetween {
        from: "22:00".into(),
        to: "06:00".into(),
    }]);
    assert!(engine::holds(&env, &night, &Event::default(), clock(TEN_AM), None).is_err());
    assert!(
        engine::holds(
            &env,
            &night,
            &Event::default(),
            clock(TEN_AM + 13 * 3600),
            None
        )
        .is_ok()
    );
    // Weekdays only (#11177): TEN_AM's day, then the day after.
    let today = clock(TEN_AM).weekday();
    let weekdays = notify_rule(vec![Condition::Weekdays { days: vec![today] }]);
    assert!(weekdays.validate().is_ok());
    assert!(engine::holds(&env, &weekdays, &Event::default(), clock(TEN_AM), None).is_ok());
    assert!(
        engine::holds(
            &env,
            &weekdays,
            &Event::default(),
            clock(TEN_AM + 86_400),
            None
        )
        .is_err()
    );
    assert!(
        notify_rule(vec![Condition::Weekdays { days: vec![7] }])
            .validate()
            .is_err()
    );
    // 1970-01-01 was a Thursday.
    assert_eq!(clock(0).weekday(), 4);
    let exists = notify_rule(vec![Condition::PathExists {
        path: "~/work".into(),
    }]);
    assert!(engine::holds(&env, &exists, &Event::default(), clock(TEN_AM), None).is_ok());
    // A judgment holds only at its threshold, and never without a judge.
    let judged = notify_rule(vec![Condition::Judgment {
        question: "Is this worth telling the user about?".into(),
        setting: engine::JUDGMENT.name.into(),
        threshold: 80,
    }]);
    let yes = Stand(vec![("judgment", "", 0.9)]);
    let no = Stand(vec![("judgment", "", 0.6)]);
    assert!(engine::holds(&env, &judged, &failed, clock(TEN_AM), Some(&yes)).is_ok());
    let why = engine::holds(&env, &judged, &failed, clock(TEN_AM), Some(&no)).unwrap_err();
    assert!(
        why.starts_with("background.judgment: 0.60 below 0.80"),
        "{why}"
    );
    assert!(engine::holds(&env, &judged, &failed, clock(TEN_AM), None).is_err());
}

#[test]
fn a_notify_rule_runs_on_each_failed_task_and_is_logged() {
    let s = scratch();
    let volumes = Fixed {
        free: 20 * GB,
        total: 1_000 * GB,
    };
    let env = Env {
        layout: &s.layout,
        facts: None,
        volumes: &volumes,
        processes: &Idle,
        now: TEN_AM,
        kache: None,
    };
    let rule = notify_rule(vec![Condition::TaskOutcome {
        outcomes: vec![TaskOutcome::Failed],
    }]);
    assert!(engine::per_task(&rule));
    let failed = Event {
        task: Some(ended("aaaabbbbccccdddd", true)),
        paths: Vec::new(),
    };
    let report =
        crate::runner::check_with(&env, &rule, Cause::TaskEnded, &failed, clock(TEN_AM), None)
            .unwrap()
            .unwrap();
    assert_eq!(
        report.notice.as_deref(),
        Some("A Coder run failed: task aaaabbbbcccc. 20 GB free.")
    );
    let record = report.record.unwrap();
    assert_eq!(record.trigger, Cause::TaskEnded);
    assert_eq!(record.steps.len(), 1);
    assert_eq!(record.steps[0].outcome, StepOutcome::Done);
    assert_eq!(store::read_log(&s.layout).len(), 1);
    // A task that finished fine says nothing.
    let fine = Event {
        task: Some(ended("eeee", false)),
        paths: Vec::new(),
    };
    assert!(
        crate::runner::check_with(&env, &rule, Cause::TaskEnded, &fine, clock(TEN_AM), None)
            .is_none()
    );
    // A trigger the rule does not name does nothing.
    let mut daily_only = rule.clone();
    daily_only.triggers = vec![Trigger::Daily { at: "09:00".into() }];
    assert!(
        crate::runner::check_with(
            &env,
            &daily_only,
            Cause::TaskEnded,
            &failed,
            clock(TEN_AM),
            None
        )
        .is_none()
    );
    assert!(crate::runner::fires(&daily_only, Cause::Daily));
    // A cooldown holds the next one back.
    let mut cooled = rule;
    cooled.cooldown_secs = 3600;
    crate::view::remember(&s.layout, &cooled.id, &report_of(&s.layout));
    assert!(
        crate::runner::check_with(
            &env,
            &cooled,
            Cause::TaskEnded,
            &failed,
            clock(TEN_AM),
            None
        )
        .is_none()
    );
}

/// A finished run's report, to mark a rule as just run.
fn report_of(_layout: &Layout) -> crate::run::Report {
    let mut report = engine::nothing();
    report.record = Some(crate::run::Record::empty("r", "notify"));
    report
}

#[test]
fn a_dry_run_says_what_a_rule_would_do_now() {
    let s = scratch();
    let volumes = Fixed {
        free: 20 * GB,
        total: 1_000 * GB,
    };
    let env = Env {
        layout: &s.layout,
        facts: None,
        volumes: &volumes,
        processes: &Idle,
        now: TEN_AM,
        kache: None,
    };
    let low = notify_rule(vec![Condition::FreeBelow {
        level: Level {
            bytes: 30 * GB,
            percent: 0,
        },
    }]);
    let lines = engine::dry_run(&env, &low, clock(TEN_AM));
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("Now it would tell you: "), "{lines:?}");
    let fine = notify_rule(vec![Condition::FreeBelow {
        level: Level {
            bytes: 10 * GB,
            percent: 0,
        },
    }]);
    assert_eq!(
        engine::dry_run(&env, &fine, clock(TEN_AM)),
        vec!["Now it would do nothing: 20 GB free, not below 10 GB."]
    );
    let failed = notify_rule(vec![Condition::TaskOutcome {
        outcomes: vec![TaskOutcome::Failed],
    }]);
    assert_eq!(
        engine::dry_run(&env, &failed, clock(TEN_AM)),
        vec!["Nothing to try now: it acts when a Coder run ends."]
    );
    // Nothing was recorded.
    assert!(store::read_log(&s.layout).is_empty());
}

// Keeping a checkout up to date: only by fast-forward, only when clean.

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .unwrap();
    assert!(status.status.success(), "git {args:?}: {status:?}");
}

#[test]
fn a_checkout_is_fast_forwarded_only_when_clean_and_not_ahead() {
    let s = scratch();
    let home = &s.layout.home;
    let origin = home.join("origin.git");
    let seed = home.join("seed");
    let checkout = home.join("work/checkout");
    git(
        home,
        &["init", "--bare", "-b", "main", origin.to_str().unwrap()],
    );
    git(
        home,
        &["clone", origin.to_str().unwrap(), seed.to_str().unwrap()],
    );
    std::fs::write(seed.join("a"), "1").unwrap();
    git(&seed, &["add", "a"]);
    git(&seed, &["commit", "-m", "one"]);
    git(&seed, &["push", "origin", "HEAD:main"]);
    git(
        home,
        &[
            "clone",
            origin.to_str().unwrap(),
            checkout.to_str().unwrap(),
        ],
    );
    std::fs::write(seed.join("a"), "2").unwrap();
    git(&seed, &["commit", "-am", "two"]);
    git(&seed, &["push", "origin", "HEAD:main"]);
    // A dry run fetches nothing and changes nothing.
    let dry = engine::fast_forward("~/work/checkout", None, home, true);
    assert_eq!(dry.outcome, StepOutcome::Would, "{dry:?}");
    assert_eq!(std::fs::read_to_string(checkout.join("a")).unwrap(), "1");
    let done = engine::fast_forward("~/work/checkout", None, home, false);
    assert_eq!(done.outcome, StepOutcome::Done, "{done:?}");
    assert_eq!(std::fs::read_to_string(checkout.join("a")).unwrap(), "2");
    let again = engine::fast_forward("~/work/checkout", None, home, false);
    assert_eq!(again.outcome, StepOutcome::Skipped);
    assert!(again.detail.contains("up to date"), "{again:?}");
    // Unsaved work: left as it is.
    std::fs::write(checkout.join("a"), "mine").unwrap();
    let dirty = engine::fast_forward("~/work/checkout", None, home, false);
    assert_eq!(dirty.outcome, StepOutcome::Skipped);
    assert!(dirty.detail.contains("uncommitted"), "{dirty:?}");
    assert_eq!(std::fs::read_to_string(checkout.join("a")).unwrap(), "mine");
    // A commit the remote lacks: left as it is.
    git(&checkout, &["commit", "-am", "local"]);
    let ahead = engine::fast_forward("~/work/checkout", None, home, false);
    assert_eq!(ahead.outcome, StepOutcome::Skipped);
    assert!(ahead.detail.contains("lacks"), "{ahead:?}");
    // Not a checkout.
    let none = engine::fast_forward("~/work", None, home, false);
    assert_eq!(none.outcome, StepOutcome::Skipped);
}

// Shakeout #10308, #10309: asking to be told is a new rule that deletes
// nothing, and an answer is read with the words it answers.

#[test]
fn tell_me_when_the_disk_is_low_never_edits_the_rule_that_deletes() {
    let s = scratch();
    // Jev reads an edit of the disk rule's level, but also a sure "tell
    // me": the rule that deletes is left alone and a notify rule drafted.
    let stand = Stand(vec![
        ("intent", "edit", 0.8),
        ("rule", "disk", 0.8),
        ("change", "free_level", 0.8),
        ("what", "disk_alert", 0.9),
        ("when", "unstated", 0.8),
        ("part_of_day", "unstated", 0.8),
    ]);
    let draft = drafted(compiled(
        &s.layout,
        "tell me when the disk is under 50 GB",
        stand,
    ));
    assert_eq!(draft.kind, Kind::Define);
    assert_eq!(draft.rule.id, "low-disk-50gb");
    assert!(draft.before.is_none());
    assert!(
        draft
            .rule
            .actions
            .iter()
            .all(|action| matches!(action, Action::Notify { .. })),
        "{:?}",
        draft.rule.actions
    );
}

#[test]
fn a_sure_tell_me_needs_no_keep_happening_question() {
    let s = scratch();
    let stand = Stand(vec![
        ("intent", "define", 0.4),
        ("rule", "disk", 0.9),
        ("what", "disk_alert", 0.9),
        ("when", "unstated", 0.8),
        ("part_of_day", "unstated", 0.8),
    ]);
    let draft = drafted(compiled(
        &s.layout,
        "tell me when the disk is under 50 GB",
        stand,
    ));
    assert_eq!(draft.kind, Kind::Define);
    assert_eq!(draft.rule.id, "low-disk-50gb");
}

#[test]
fn add_only_ever_drafts_a_new_rule() {
    let s = scratch();
    let home = s.layout.home.clone();
    let mut context = context(Some(&home.join("work/openagents")));
    context.new_only = true;
    // Even a sure edit reading drafts a new rule when the words came from
    // `add`.
    let stand = Stand(vec![
        ("intent", "edit", 0.95),
        ("rule", "disk", 0.95),
        ("change", "free_level", 0.9),
        ("what", "free_space", 0.9),
        ("when", "unstated", 0.8),
        ("part_of_day", "unstated", 0.8),
    ]);
    let draft = drafted(compile::compile(&s.layout, "keep 80 GB free", &context, &stand).unwrap());
    assert_eq!(draft.kind, Kind::Define);
    assert_eq!(draft.rule.id, "keep-free-80gb");
    assert!(draft.before.is_none());
}

#[test]
fn an_answer_is_read_with_the_question_it_answers() {
    let s = scratch();
    let pending = compile::Pending {
        message: "tell me when the disk is under 50 GB".into(),
        question: "Should this keep happening on its own as a background rule, or be done \
                   once now?"
            .into(),
        asked: TEN_AM,
    };
    compile::save_pending(&s.layout, "thread-1", &pending).unwrap();
    let taken = compile::take_pending(&s.layout, "thread-1", TEN_AM + 60).unwrap();
    assert_eq!(taken, pending);
    let words = compile::answered(&taken, "keep it as a background rule");
    assert!(
        words.starts_with("tell me when the disk is under 50 GB\n"),
        "{words}"
    );
    assert!(
        words.ends_with("The user answered: keep it as a background rule"),
        "{words}"
    );
    // Taken once; a stale question is forgotten, not answered.
    assert!(compile::take_pending(&s.layout, "thread-1", TEN_AM + 60).is_none());
    compile::save_pending(&s.layout, "thread-1", &pending).unwrap();
    assert!(
        compile::take_pending(&s.layout, "thread-1", TEN_AM + compile::PENDING_SECS + 1).is_none()
    );
}
