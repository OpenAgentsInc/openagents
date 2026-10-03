//! `OPENAGENTS_TASKS` moves the task store, and with it the worktrees and
//! target slots the background runner looks at (#10290). Its own test
//! binary, since it sets a process-wide variable.

use std::path::Path;

#[test]
fn the_runner_finds_coders_folders_where_openagents_tasks_puts_them() {
    let home = tempfile::tempdir().unwrap();
    let store = home.path().join("pool/tasks");
    std::fs::create_dir_all(&store).unwrap();
    // SAFETY: this binary has one test and no other threads read the
    // environment while it runs.
    unsafe {
        std::env::set_var("HOME", home.path());
        std::env::set_var(background::paths::STORE_VAR, &store);
    }
    assert_eq!(background::task_store(home.path()), store);
    let layout = background::Layout::from_env().unwrap();
    let beside = store.canonicalize().unwrap();
    let beside = beside.parent().unwrap();
    assert_eq!(layout.store, beside.join("tasks"));
    assert_eq!(layout.worktrees(), beside.join("worktrees"));
    assert_eq!(layout.targets(), beside.join("targets"));
    assert_ne!(layout.worktrees(), layout.openagents.join("worktrees"));

    // Unset, it is the default store under the home.
    unsafe { std::env::remove_var(background::paths::STORE_VAR) };
    assert_eq!(
        background::task_store(Path::new("/h")),
        Path::new("/h/.openagents/tasks")
    );
}
