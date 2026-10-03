## Prepared source context

Source commit: `e11b84c3adde42755c32fb478cc1895d13149042`. These are complete syntax declarations selected from a bounded candidate pool. They are starting points, not a complete dependency graph or a proof that the requirements are covered. Imports, callers, other modules, macros, cfg behavior, and unselected tests may need inspection. Read additional source whenever this material is insufficient.

### crates/coder/src/task/owner/tests.rs:1048-1076 — the_owner_process_waits_out_a_store_busy_past_the_lock_wait

File SHA-256: `db37ddf8491974043f0199f2e535987ccacad01be788704c1fbcec6190842b68`

```rust
#[tokio::test]
async fn the_owner_process_waits_out_a_store_busy_past_the_lock_wait() {
    let (root, _workspace, grant) = fixture();
    let dir = root.path().join("store");
    let bytes = serde_json::to_vec(&grant).unwrap();
    // Another process's slow disk sync holds the store past the five
    // seconds a device-facing open waits.
    let holder = hold_store(&dir, LOCK_WAIT + Duration::from_secs(2));
    let cancel = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: "cancel-busy".into(),
        task_id: "task-one".into(),
        expected_revision: Some(1),
        action: Action::Cancel {
            reason: "Stop.".into(),
        },
    };
    assert!(matches!(
        Store::open_waiting(&dir, Duration::from_millis(100))
            .unwrap()
            .apply(&serde_json::to_vec(&cancel).unwrap()),
        Err(Error::Busy)
    ));
    // The task's own process waits it out and runs the task, where it
    // used to fail at admission.
    let task = execute(&dir, &bytes).await.unwrap();
    holder.join().unwrap();
    assert_eq!(task.execution, Execution::Finished);
}
```

### crates/coder/src/task/local.rs:2711-2776 — tests::a_follower_waits_out_a_store_another_process_holds_then_continues

File SHA-256: `402fa03f4b08e46f13b884208541de2705fa306452b33522fb9c1bf331c314e9`

```rust
    /// A follower whose store another process holds past the open's wait
    /// waits it out and then continues, instead of ending with "another
    /// process holds the task store lock" (#10049's chat issue flow, which
    /// released its claim on a transient busy store). Only a store busy
    /// past the reader's limit is a read failure, and it never touches the
    /// task.
    #[test]
    fn a_follower_waits_out_a_store_another_process_holds_then_continues() {
        use std::time::{Duration, Instant};
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        let run = local(dir.path(), both).with_launcher(Box::new(Held));
        let record = run
            .start(
                &top,
                "Fix the parser",
                "Fix the parser.",
                Some(&"4c".repeat(16)),
            )
            .unwrap();
        let mut follow = run.follow(&record.task, None, None);
        follow.reading =
            super::super::Reading::within(Duration::from_millis(50), Duration::from_secs(30));
        let (lines, state) = follow.poll().unwrap();
        assert_eq!(state, State::Running);
        assert!(!lines.is_empty(), "the turn's start is followed");
        let before = Store::open(run.store())
            .unwrap()
            .show(&record.task)
            .unwrap();

        // Another process writes the task for well past the open's wait.
        // Reads take no lock (#10231), so the follower never waits on it.
        let store = run.store().to_path_buf();
        let task = record.task.clone();
        let (held_tx, held_rx) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let held = Store::open(&store).unwrap().lock_task(&task).unwrap();
            held_tx.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(600));
            drop(held);
        });
        held_rx.recv().unwrap();
        let started = Instant::now();
        let mut polled = 0;
        while started.elapsed() < Duration::from_millis(400) {
            let began = Instant::now();
            let (lines, state) = follow.poll().expect("a writer never stops a reader");
            assert!(began.elapsed() < Duration::from_millis(200));
            assert_eq!(state, State::Running);
            assert!(lines.is_empty());
            polled += 1;
        }
        assert!(polled > 0);
        holder.join().unwrap();

        // Released: the follower reads again, and the task is untouched.
        let (_, state) = follow.poll().expect("the follower continues");
        assert_eq!(state, State::Running);
        let after = Store::open(run.store())
            .unwrap()
            .show(&record.task)
            .unwrap();
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.status, before.status);
    }
```

### crates/microcoder/src/repository/tests.rs:1770-1794 — a_store_another_process_holds_is_not_a_stop

File SHA-256: `8f6a39107e6e353f2ab87f2f327b966bc675f511b03f21abc1f7283caac7c9c4`

```rust
#[tokio::test]
async fn a_store_another_process_holds_is_not_a_stop() {
    let (_root, store, grant) = fixture();
    // Holding the store spends the lock wait from the turn's wall time; the
    // fixture's eight seconds would leave too little for the run under a
    // loaded test machine, where it would end as stopped by its deadline.
    let mut grant = task::owner::Grant::parse(&grant).unwrap();
    grant.wall_seconds = 60;
    let grant = serde_json::to_vec(&grant).unwrap();
    let host = Host::admit(&store, &grant).await.unwrap();
    assert!(!host.cancelled());
    // Hold the store past the lock wait, as a slow disk sync can.
    let held = Store::open(&store).unwrap();
    assert!(!host.cancelled(), "a busy store is not a stop request");
    drop(held);
    assert!(!host.cancelled());
    let task = run(
        host,
        &generator("printf output > result.txt"),
        &JudgeFixture,
    )
    .await
    .unwrap();
    assert_eq!(task.execution, task::Execution::Finished);
}
```

### crates/coder/src/task/owner/tests.rs:489-548 — a_dead_owners_live_process_still_holds_its_project_and_is_never_killed

File SHA-256: `db37ddf8491974043f0199f2e535987ccacad01be788704c1fbcec6190842b68`

```rust
#[tokio::test]
async fn a_dead_owners_live_process_still_holds_its_project_and_is_never_killed() {
    let (root, _workspace, grant) = fixture();
    let dir = root.path().join("store");
    // The owner died after recording its effect intent, and a process the
    // run recorded still runs.
    OWNER_FAULT.with(|fault| fault.set(Some("after_intent")));
    assert!(
        execute(&dir, &serde_json::to_vec(&grant).unwrap())
            .await
            .is_err()
    );
    let mut child = {
        let mut command = std::process::Command::new("/bin/sleep");
        command.arg("30");
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        command.spawn().unwrap()
    };
    {
        let mut store = Store::open(&dir).unwrap();
        let owner = acquired(&store, "task-one");
        store
            .record(
                &owner,
                Event::Spawned {
                    process_id: child.id(),
                },
                1,
            )
            .unwrap();
    }
    let next = second_task(&dir, &grant);
    let next_bytes = serde_json::to_vec(&next).unwrap();
    assert!(matches!(
        execute(&dir, &next_bytes).await,
        Err(Error::WorkspaceBusy)
    ));
    assert_eq!(Store::open(&dir).unwrap().settle("task-one").unwrap(), None);
    // Recovery leaves it unknown while the process runs, and kills nothing.
    assert_eq!(recovered(&dir, "task-one").status, Status::Unknown);
    assert!(
        child.try_wait().unwrap().is_none(),
        "the process was killed"
    );
    // Once it has ended, the next start ends the unknown run and runs.
    child.kill().unwrap();
    child.wait().unwrap();
    let task = executed(&dir, &next_bytes).await;
    assert_eq!(task.execution, Execution::Finished);
    let ended = Store::open(&dir).unwrap().show("task-one").unwrap();
    assert_eq!(ended.status, Status::Finished);
    assert_eq!(ended.execution, Execution::Failed);
    let run = ended.run.as_ref().unwrap();
    assert_eq!(run.epoch, 2);
    assert_eq!(
        run.recovery_reason.as_deref(),
        Some("owner_lost_effects_not_replayed")
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
}
```

### crates/coder/src/task.rs:713-718 — Store::open_for_owner

File SHA-256: `fd4c43866798b722df0e87b89305ef67ee7515ca4a7bda4c9d92b4c492bb5047`

```rust
    /// [`Store::open`] for a task's own owner process (its launcher, its
    /// admission, and every record it makes while it runs), waiting up to
    /// [`OWNER_LOCK_WAIT`] for a busy task instead of failing it.
    pub fn open_for_owner(dir: &Path) -> Result<Self, Error> {
        Self::open_waiting(dir, OWNER_LOCK_WAIT)
    }
```

### crates/nostr-relay/src/store/mod.rs:1528-1548 — Store::events_after

File SHA-256: `6baac83a1fa983c7f11d699787a51b33340d325c801049e2c5a917559d7f2320`

```rust
    /// Read a bounded, stable catch-up page through a previously sampled
    /// high-water mark.
    pub async fn events_after(
        &self,
        after: i64,
        through: i64,
        now: u64,
        limit: usize,
    ) -> Result<Vec<StoredEvent>, StoreError> {
        self.ensure_current()?;
        let now = pg_i64(now, "now")?;
        let limit = pg_limit(limit)?;
        let rows = self
            .client
            .query(
                &self.statements.events_after,
                &[&after, &through, &now, &limit],
            )
            .await?;
        rows.into_iter().map(decode_event_row).collect()
    }
```

### crates/nostr-relay/src/store/mod.rs:1342-1435 — Store::process_identity_archive

File SHA-256: `6baac83a1fa983c7f11d699787a51b33340d325c801049e2c5a917559d7f2320`

```rust
    pub async fn process_identity_archive(
        &mut self,
        event: &Event,
        request: &IdentityArchiveRequest,
        consent: &str,
        now: u64,
        signer: &RelaySigner,
    ) -> Result<bool, StoreError> {
        self.ensure_current()?;
        let statements = self.statements.clone();
        let transaction = self.client.transaction().await?;
        let kind = i32::from(event.kind);
        if transaction
            .query_opt(
                &statements.accept_block_command,
                &[&event.id, &event.pubkey, &kind],
            )
            .await?
            .is_none()
        {
            transaction.commit().await?;
            return Ok(false);
        }

        let changed = if request.archive {
            transaction
                .query_opt(
                    &statements.upsert_archived_identity,
                    &[
                        &request.target,
                        &request.reason,
                        &request.replaced_by,
                        &consent,
                        &event.pubkey,
                        &event.id,
                    ],
                )
                .await?
                .is_some()
        } else {
            transaction
                .query_opt(&statements.delete_archived_identity, &[&request.target])
                .await?
                .is_some()
        };
        if changed {
            transaction.query_one(&statements.ingest_lock, &[]).await?;
            let mut delta_tags = vec![
                Tag::new(vec!["-".into()]),
                Tag::new(vec!["p".into(), request.target.clone()]),
                Tag::new(vec!["consent".into(), consent.into(), event.pubkey.clone()]),
                Tag::new(vec!["e".into(), event.id.clone()]),
            ];
            if let Some(reason) = &request.reason {
                delta_tags.push(Tag::new(vec!["reason".into(), reason.clone()]));
            }
            if let Some(replaced_by) = &request.replaced_by {
                delta_tags.push(Tag::new(vec!["replaced-by".into(), replaced_by.clone()]));
            }
            let delta_kind = if request.archive {
                IDENTITY_ARCHIVED_KIND
            } else {
                IDENTITY_UNARCHIVED_KIND
            };
            let delta = signer.sign(now, delta_kind, delta_tags, event.content.clone());
            insert_internal_regular_event(&transaction, &statements, &delta).await?;

            let archived = transaction
                .query(&statements.list_archived_identities, &[])
                .await?
                .into_iter()
                .map(|row| row.get::<_, String>(0))
                .collect::<Vec<_>>();
            let mut snapshot_tags = vec![Tag::new(vec!["-".into()])];
            snapshot_tags.extend(
                archived
                    .into_iter()
                    .map(|pubkey| Tag::new(vec!["p".into(), pubkey])),
            );
            insert_internal_replaceable_event(
                &transaction,
                &statements,
                signer,
                IDENTITY_ARCHIVE_LIST_KIND,
                "",
                now,
                snapshot_tags,
                String::new(),
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(changed)
    }
```

### crates/nostr-relay/src/store/mod.rs:1437-1512 — Store::process_dm_visibility

File SHA-256: `6baac83a1fa983c7f11d699787a51b33340d325c801049e2c5a917559d7f2320`

```rust
    pub async fn process_dm_visibility(
        &mut self,
        event: &Event,
        channel: &str,
        hidden: bool,
        now: u64,
        signer: &RelaySigner,
    ) -> Result<bool, StoreError> {
        self.ensure_current()?;
        let statements = self.statements.clone();
        let transaction = self.client.transaction().await?;
        if transaction
            .query_opt(&statements.group_member, &[&channel, &event.pubkey])
            .await?
            .is_none()
        {
            transaction.commit().await?;
            return Err(StoreError::Management(
                "DM visibility actor is not a member of the target group".into(),
            ));
        }
        let kind = i32::from(event.kind);
        if transaction
            .query_opt(
                &statements.accept_block_command,
                &[&event.id, &event.pubkey, &kind],
            )
            .await?
            .is_none()
        {
            transaction.commit().await?;
            return Ok(false);
        }
        let changed = if hidden {
            transaction
                .query_opt(&statements.insert_dm_hidden, &[&event.pubkey, &channel])
                .await?
                .is_some()
        } else {
            transaction
                .query_opt(&statements.delete_dm_hidden, &[&event.pubkey, &channel])
                .await?
                .is_some()
        };
        if changed {
            transaction.query_one(&statements.ingest_lock, &[]).await?;
            let hidden_groups = transaction
                .query(&statements.list_dm_hidden, &[&event.pubkey])
                .await?
                .into_iter()
                .map(|row| row.get::<_, String>(0))
                .collect::<Vec<_>>();
            let mut tags = vec![
                Tag::new(vec!["d".into(), event.pubkey.clone()]),
                Tag::new(vec!["p".into(), event.pubkey.clone()]),
            ];
            tags.extend(
                hidden_groups
                    .into_iter()
                    .map(|group| Tag::new(vec!["h".into(), group])),
            );
            insert_internal_replaceable_event(
                &transaction,
                &statements,
                signer,
                DM_VISIBILITY_KIND,
                &event.pubkey,
                now,
                tags,
                String::new(),
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(changed)
    }
```
