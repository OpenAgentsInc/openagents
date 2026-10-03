//! The WoW task grades deterministic outcomes in addition to attempted actions.
use atif::document::Outcome;
use coderbench::{Check, Ending, Observed, Task, Workspace, tasks_dir};

#[test]
fn northshire_requires_successful_critics_and_a_closed_episode() {
    let task = Task::load(&tasks_dir().join("wow-northshire-first-quests/task.json")).unwrap();
    let mut run = Observed {
        checks: task
            .grade
            .checks
            .iter()
            .map(|name| Check {
                name: name.clone(),
                outcome: Outcome::Completed,
            })
            .collect(),
        order: task.grade.path.clone(),
        workspace: Some(Workspace::default()),
        ending: Ending::Other("ended".into()),
        closed: true,
        ..Default::default()
    };
    assert!(task.judge(&run).passed());
    let check = run
        .checks
        .iter_mut()
        .find(|c| c.name == "voyager:verify:kobold-camp-cleanup")
        .unwrap();
    check.outcome = Outcome::Failed;
    assert!(!task.judge(&run).passed());
    run.checks
        .retain(|c| !c.name.starts_with("voyager:verify:"));
    assert!(!task.judge(&run).passed());
    run.ending = Ending::Other("interrupted".into());
    assert!(!task.judge(&run).passed());
}
