# Briefing: #10283 Terminal: keep background-watcher notices ("Disk almost full" etc.) out of the transcript; show them in a watchers view

## The issue

Owner: "i dont want to see 'Disk almost full' etc notifications in the transcript of the terminal, hide those somewhere else like user must hit left to see background watchers or something". Background rule notices (disk cleanup, other watchers from crates/background) must not be inserted into the chat transcript. Put them in a background-watchers view the user opens on purpose (e.g. Left arrow from an empty input, or a key listed in the help/status line), with an unobtrusive indicator (a small count in the status line) when something new happened. Watcher start-up lists, run results and escalations all go there; nothing about background watchers appears in the transcript unless the user asks in chat. Snapshot tests for both.

## Change plan

1. Goal: Terminal: keep background-watcher notices ("Disk almost full" etc.) out of the transcript; show them in a watchers view
2. Change the behavior where it lives, most likely in `crates/background/src/run.rs`, `docs/background/2026-10-02-background-processes.md`.
3. Add or update a test that pins the new behavior, next to the existing ones in `crates/background/src/tests.rs`.
4. Run `check:background`, then `test:background`, then `fmt`; stop when they pass.

## Files to change

### `crates/background/src/run.rs`

Why: has `Disk almost full`

```
--- lines 459-489 of 605 ---
  459                  Outcome::Deleted | Outcome::Removed | Outcome::Trashed
  460              )
  461          })
  462          .collect();
  463      let free = observation.iter().map(|o| o.free_after).min()?;
  464      let low = plan
  465          .volumes
  466          .iter()
  467          .zip(observation)
  468          .any(|(volume, o)| o.free_after < volume.start && volume.needed > 0);
  469      if emergency {
  470          return Some(format!(
  471              "Disk almost full ({} free). Cleaned everything allowed: {}.",
  472              bytes(free),
  473              bytes(freed)
  474          ));
  475      }
  476      let mut line = String::new();
  477      if !done.is_empty() {
  478          let mut counts: Vec<(Class, usize)> = Vec::new();
  479          for action in &done {
  480              match counts.iter_mut().find(|(class, _)| *class == action.class) {
  481                  Some((_, count)) => *count += 1,
  482                  None => counts.push((action.class, 1)),
  483              }
  484          }
  485          let parts: Vec<String> = counts
  486              .iter()
  487              .map(|(class, count)| format!("{count} {}", class.noun(*count)))
  488              .collect();
  489          line = if freed > 0 || counts.iter().any(|(class, _)| *class != Class::Judged) {
```

### `crates/background/src/tests.rs`

Why: has `Disk almost full`

```
--- lines 598-628 of 1018 ---
  598      run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
  599      assert!(trash.exists());
  600      let volumes = Fixed {
  601          free: GB,
  602          total: 1_000 * GB,
  603      };
  604      let env = Env {
  605          volumes: &volumes,
  606          ..env
  607      };
  608      let report = run::run(&env, &disk(), Cause::Threshold, false, false).unwrap();
  609      assert!(!trash.parent().unwrap().exists());
  610      assert!(report.notice.unwrap().starts_with("Disk almost full"));
  611  }
  612  
  613  /// A home with one ended task's target on a low volume, checked for
  614  /// `cause` under `rule`: whether the target was deleted, and the run's
  615  /// recorded trigger.
  616  fn checked(rule: &Rule, cause: Cause) -> (bool, Option<Cause>) {
  617      let home = Home::new();
  618      let ended = home.layout.targets().join("openagents-aaaa-1111");
  619      target(&ended, 4096);
  620      home.task("aaaa", &home.layout.worktrees().join("a"), &ended, true);
  621      let facts = home.facts();
  622      let volumes = low();
  623      let env = env(&home, &facts, &volumes, &Idle);
  624      let report = crate::runner::check(&env, rule, cause).map(Result::unwrap);
  625      let trigger = report.and_then(|report| report.record).map(|r| r.trigger);
  626      (!ended.exists(), trigger)
  627  }
  628  
```

### `docs/background/2026-10-02-background-processes.md`

Why: has `Disk almost full`

```
--- lines 282-312 of 726 ---
  282  5. **Age.** Lock and fingerprint modification times.
  283  
  284  Phase 1 needs no model. Phase 3 adds one judgment for directories no class
  285  covers (below).
  286  
  287  ### What the user sees
  288  
  289  Notifications are one or two plain lines, sent only when something happens:
  290  
  291  - `Freed 84 GB: 3 build caches from ended tasks.`
  292  - `Freed 41 GB: 2 old agent build folders, 6 finished worktrees. 312 GB free.`
  293  - `Disk still low: 22 GB free. Largest not cleaned: ~/.openagents/pylon (47 GB), not a known cache.`
  294  - `Disk almost full (9 GB free). Cleaned everything allowed: 12 GB.`
  295  
  296  A check that finds enough free space sends nothing. The terminal status line
  297  shows `disk ok · 312 GB free` only in `/background`.
  298  
  299  ### Changing it in conversation
  300  
  301  Each request becomes a typed edit to the `disk` rule. The user sees the
  302  change and the dry run before it applies.
  303  
  304  | The user says | The edit |
  305  | --- | --- |
  306  | "only keep 2 agent target dirs" | Class 2, agent target directories: `keep: 2` (the two most recently used stay, regardless of age). |
  307  | "never touch ~/.openagents/pylon" | Add `~/.openagents/pylon` to the deny list. |
  308  | "keep 200 GB free" | Start threshold `200 GB`; stop threshold raised to stay above it. |
  309  | "clean old build caches first" | No change: that is already the order. The reply says so. |
  310  | "don't delete worktrees, just tell me" | Class 3 set to report only. |
  311  | "pause disk cleanup until tomorrow" | `enabled: false` with a resume time. |
  312  
```

## Similar past changes

### d7f0ab8c6c Disk cleanup keeps a worktree that holds ignored files outside build caches

```diff
diff --git a/crates/background/src/tests.rs b/crates/background/src/tests.rs
index ad3fa5c396..3332dae146 100644
--- a/crates/background/src/tests.rs
+++ b/crates/background/src/tests.rs
@@ -346,4 +346,45 @@ fn unsaved_worktrees_stay_and_a_clean_pushed_one_goes_and_comes_back() {
 }
 
+#[test]
+fn ignored_files_keep_a_worktree_and_ignored_caches_do_not() {
+    let home = Home::new();
+    let (repo, trees) = repo(&home, &["env", "private", "cache"]);
+    let [env_tree, private, cache] = [&trees[0], &trees[1], &trees[2]];
+    std::fs::write(
+        repo.join(".git/info/exclude"),
+        ".env\nprivate/\ntarget/\nnode_modules/\n",
+    )
+    .unwrap();
+    std::fs::write(env_tree.join(".env"), "SECRET=1").unwrap();
+    std::fs::create_dir_all(private.join("private")).unwrap();
+    std::fs::write(private.join("private/data"), "mine").unwrap();
+    std::fs::create_dir_all(cache.join("target/debug")).unwrap();
+    std::fs::write(cache.join("target/debug/out"), "built").unwrap();
+    std::fs::create_dir_all(cache.join("web/node_modules/x")).unwrap();
+    std::fs::write(cache.join("web/node_modules/x/index.js"), "").unwrap();
+    let facts = home.facts();
+    let volumes = low();
+    let env = env(&home, &facts, &volumes, &Idle);
+    let dry = run::run(&env, &disk(), Cause::Manual, true, true).unwrap();
+    let reasons: Vec<(PathBuf, String)> = dry
+        .plan
+        .kept
+        .iter()
+        .map(|kept| (kept.path.clone(), kept.why.clone()))
+        .collect();
+    assert!(
+        reasons.contains(&(env_tree.clone(), "holds ignored files: .env".into())),
+        "{reasons:?}"
+    );
+    assert!(
+        reasons.contains(&(private.clone(), "holds ignored files: private/".into())),
+        "{reasons:?}"
+    );
+    let real = run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
+    assert!(env_tree.join(".env").exists());
+    assert!(private.join("private/data").exists());
+    assert!(!cache.exists(), "{:?} {reasons:?}", real.record);
+}
+
 #[test]
 fn symlinks_are_not_followed_and_other_volumes_are_refused() {
```

## Checks (run them with the `run_check` tool)

- `check:background`: `cargo check -p background --tests --message-format short` (compile background and its tests)
- `test:background`: `cargo test -p background [FILTER]` (run background's tests (pass a test-name filter to run fewer))
- `fmt:background`: `cargo fmt -p background` (format background)

## Repo rules

- Product code is Rust. Do not add TypeScript.
- The check for a change is `cargo test -p` for the crates you edited plus `cargo fmt`. Do not run clippy, release gates, or other crates' tests.
- Never put machine talk (internal words like retained, projection, canonical, digest, lane) in text a user sees; say what happened in plain words. Each surface's tests run the `oa-copy` guard over user-visible text.
- When a test fails only because a checked-in generated file is stale, regenerate it.
- No new INVARIANTS rows, design notes, or long docs for a small change.
- Fix stale or false user-facing copy you touch in the same change.
