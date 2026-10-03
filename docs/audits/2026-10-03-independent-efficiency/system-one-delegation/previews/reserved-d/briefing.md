## Prepared source context

Source commit: `4eee1cbf3ecc844f89d42abc2b7df7be4411c846`. These are complete syntax declarations selected from a bounded candidate pool. They are starting points, not a complete dependency graph or a proof that the requirements are covered. Imports, callers, other modules, macros, cfg behavior, and unselected tests may need inspection. Read additional source whenever this material is insufficient.

### crates/openagents-chat-app/src/host_threads.rs:1734-1761 — tests::a_follow_up_the_computer_took_before_a_crash_is_not_appended_twice

File SHA-256: `83a4553c711f78ae84bf95bbe5df7f39c072d1fafb2f7df8b40deb8adfcc4627`

```rust
    #[test]
    fn a_follow_up_the_computer_took_before_a_crash_is_not_appended_twice() {
        let temp = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        let link: Arc<dyn Link> = fake.clone();
        fake.offline.store(true, Ordering::SeqCst);
        let queued = {
            let threads = HostThreads::default().with_cache(cache(temp.path()));
            threads.open("host", THREAD, link.clone());
            assert!(threads.send("Once"));
            threads.close();
            threads.queued().remove(0)
        };
        // The computer took it, and the app died before it heard back.
        fake.offline.store(false, Ordering::SeqCst);
        link.send("host", THREAD, &queued.request, &queued.text)
            .unwrap();
        assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
        let threads = HostThreads::default().with_cache(cache(temp.path()));
        threads.poll(vec![("host".into(), "Studio Mac".into())], &link);
        until("the outbox empties", || threads.queued().is_empty());
        assert_eq!(fake.sends.load(Ordering::SeqCst), 1, "the send ID held");
        assert_eq!(
            fake.calls.load(Ordering::SeqCst),
            2,
            "sent again, appended once"
        );
    }
```

### crates/coder/src/task/local.rs:3262-3330 — tests::a_start_has_the_host_read_an_engine_again_before_passing_it_over

File SHA-256: `4d89f3da92d8a26f02c0f2d7ea7d5a9e2b6420ed332d88adceef802d6479501d`

```rust
    /// A start that asks for an engine, or would pass one over on an old
    /// reading, has the host read it now first (#10105): the hook runs with
    /// those providers before the choice, and the choice reads what the
    /// host then wrote. Here Claude, at its limit on a reading two minutes
    /// old, reads 5% fresh and runs; without the hook the old reading
    /// passes it over.
    #[test]
    fn a_start_has_the_host_read_an_engine_again_before_passing_it_over() {
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        let old_full = |store: &Path| {
            let now = autostart::unix_now();
            let book = usage::Book {
                schema: usage::SCHEMA.into(),
                entries: vec![usage::Entry {
                    provider: Provider::Claude,
                    attempted_at: now - 120,
                    next_probe_at: now + 300,
                    reading: Some(usage::Reading {
                        provider: Provider::Claude,
                        observed_at: now - 120,
                        windows: vec![usage::Window {
                            window: usage::WindowName::SevenDay,
                            used_fraction: 1.0,
                            resets_at: Some(now + 86_400),
                            length_seconds: None,
                        }],
                        limit_reached: false,
                        plan: None,
                        account: None,
                    }),
                    failure: None,
                }],
            };
            autostart::write_private(
                &store.join(usage::FILE),
                &serde_json::to_vec(&book).unwrap(),
            )
            .unwrap();
        };
        // Without a host to ask: the old reading passes Claude over.
        let alone = local(&dir.path().join("alone"), both).with_launcher(Box::new(Held));
        old_full(alone.store());
        assert_eq!(alone.recheck(Some(Provider::Claude)), [Provider::Claude]);
        let record = alone
            .start_requested(&top, "Fix it", "Fix it.", None, &[], Some(Provider::Claude))
            .unwrap();
        assert_eq!(record.turns[0].provider, "codex");
        // With the host asked first: it reads Claude at 5% and Claude runs.
        let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let store = dir.path().join("hosted").join("tasks");
        let hook = {
            let asked = asked.clone();
            let store = store.clone();
            Box::new(move |providers: &[Provider]| {
                asked.lock().unwrap().push(providers.to_vec());
                reading(&store, Provider::Claude, 0.05);
            })
        };
        let hosted = local(&dir.path().join("hosted"), both)
            .with_launcher(Box::new(Held))
            .with_fresh(hook);
        old_full(hosted.store());
        let record = hosted
            .start_requested(&top, "Fix it", "Fix it.", None, &[], Some(Provider::Claude))
            .unwrap();
        assert_eq!(record.turns[0].provider, "claude");
        assert_eq!(asked.lock().unwrap().as_slice(), [vec![Provider::Claude]]);
    }
```

### crates/coder-delegate/src/issue.rs:1956-1961 — tests::the_word_issue_before_a_number_names_one

File SHA-256: `5f3d53f729a3813096e0eed282783c1dfd0a6b54a1462e8d3b7261c82e57e044`

```rust
    #[test]
    fn the_word_issue_before_a_number_names_one() {
        assert_eq!(numbers("work on issue 9597"), [9597]);
        assert_eq!(numbers("Issues 12 and #13"), [12, 13]);
        assert!(numbers("issue9597, issue 12a, tissue 5").is_empty());
    }
```

### crates/coder/src/delegate.rs:2439-2463 — tests::an_unisolated_write_is_refused_before_it_spawns

File SHA-256: `b4875c3cca09dd3ee18306a0584bc5488fe3ce0598cb83a727bb0db7c028d860`

```rust
    /// A delegate that would write into the shared directory, or that asks
    /// a host for a checkout it cannot make, is refused before anything
    /// spawns rather than run in the shared directory.
    #[tokio::test]
    async fn an_unisolated_write_is_refused_before_it_spawns() {
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "stub", "printf 'wrote it\\n'");
        let delegator = Delegator::new(executor(&binary));

        let mut writing = Task::reading("change a file", "a.rs");
        writing.writes = true;
        let refused = delegator.run(writing).await;
        assert_eq!(refused.status, Status::Refused("isolation_required".into()));
        assert!(refused.output.is_empty(), "nothing ran");

        let mut isolated = Task::reading("change a file", "a.rs");
        isolated.isolation = Isolation::Worktree;
        assert!(!delegator.provides(Isolation::Worktree));
        let unavailable = delegator.run(isolated).await;
        assert_eq!(
            unavailable.status,
            Status::Refused("isolation_unavailable".into()),
            "a host with no checkout to branch from refuses rather than sharing one"
        );
    }
```

### crates/coder/src/task/autostart.rs:4160-4236 — tests::a_stale_reading_is_probed_again_before_an_engine_is_passed_over

File SHA-256: `362df1227fbf8ea8761a3da5364e535c9fcbe32e0e328f03f81a2dfd19363ae4`

```rust
    /// A start does not pass an engine over on an old reading: with probes
    /// on, the engine a person asked for, at its limit on a reading two
    /// minutes old and not due by the cache, is read again first, and its
    /// fresh reading under the limit starts it (#10105).
    #[test]
    fn a_stale_reading_is_probed_again_before_an_engine_is_passed_over() {
        use nostr::cj_conversation::Engine;
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CLAUDE_PROBES: AtomicUsize = AtomicUsize::new(0);
        fn calm(provider: Provider) -> Result<usage::Response, usage::Failure> {
            if provider == Provider::Claude {
                CLAUDE_PROBES.fetch_add(1, Ordering::SeqCst);
                return Ok(usage::Response {
                    status: 200,
                    retry_after: None,
                    body: br#"{"five_hour":{"utilization":6.0,"resets_at":null},"seven_day":{"utilization":2.0,"resets_at":null}}"#.to_vec(),
                });
            }
            // Codex well under its limit: an old Claude reading alone would
            // send the start to Codex.
            Ok(usage::Response {
                status: 200,
                retry_after: None,
                body: br#"{"plan_type":"pro","rate_limit":{"primary_window":{"used_percent":5}}}"#
                    .to_vec(),
            })
        }
        let s = setup_with(calm);
        let now = 1_790_572_210;
        CLOCK.with(|clock| clock.set(now));
        probed(90).save(&s.root).unwrap();
        // Claude read at its limit two minutes ago; the cache says not to
        // ask again for five more minutes.
        let full = usage::Book {
            schema: usage::SCHEMA.into(),
            entries: vec![usage::Entry {
                provider: Provider::Claude,
                attempted_at: now - 120,
                next_probe_at: now + 300,
                reading: Some(usage::Reading {
                    provider: Provider::Claude,
                    observed_at: now - 120,
                    windows: vec![usage::Window {
                        window: usage::WindowName::SevenDay,
                        used_fraction: 1.0,
                        resets_at: Some(now + 3 * 86_400),
                        length_seconds: Some(604_800),
                    }],
                    limit_reached: false,
                    plan: None,
                    account: None,
                }),
                failure: None,
            }],
        };
        drop(Store::open(&s.store).unwrap());
        std::fs::write(
            s.store.join(usage::FILE),
            serde_json::to_vec(&full).unwrap(),
        )
        .unwrap();
        assert_eq!(
            recheck(&s.store, &probed(90), &[], now, login),
            [Provider::Claude],
            "a provider at its limit on a reading older than a minute"
        );
        let task = "9".repeat(64);
        let asked = TaskCreate {
            engine: Some(Engine::ClaudeCode),
            ..create("allowed")
        };
        s.inbox.create(&task, "phone", &asked).unwrap();
        assert_eq!(CLAUDE_PROBES.load(Ordering::SeqCst), 1);
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "claude");
        assert!(recheck(&s.store, &probed(90), &[], now, login).is_empty());
    }
```

### crates/openagents-mobile/src/coder_tab_tests.rs:3619-3640 — unsupported_and_oversized_images_are_refused_before_send

File SHA-256: `9413a6f67316c0f881cac4140c030d0a02a09a082ecb53c12bf7e1d1cf8303c0`

```rust
/// An image that isn't PNG or JPEG, or is too large, is refused before
/// anything is sent.
#[test]
fn unsupported_and_oversized_images_are_refused_before_send() {
    let mut drafts = openagents_chat_app::attachments::Drafts::default();
    let mut image = openagents_chat_app::attachments::Image::pixels(1, 1, vec![0; 4]).unwrap();
    image.bytes = std::sync::Arc::new(b"GIF89a not a png".to_vec());
    drafts.add("talk:a", image.clone()).unwrap();
    assert!(
        drafts
            .uploads("talk:a")
            .unwrap_err()
            .contains("PNG or JPEG")
    );
    let mut drafts = openagents_chat_app::attachments::Drafts::default();
    let mut big = b"\x89PNG\r\n\x1a\n".to_vec();
    big.resize(openagents_chat_app::attachments::MAX_IMAGE_BYTES + 1, 0);
    image.bytes = std::sync::Arc::new(big);
    drafts.add("talk:b", image).unwrap();
    assert!(drafts.uploads("talk:b").unwrap_err().contains("8 MiB"));
    assert_eq!(drafts.get("talk:b").len(), 1);
}
```

### crates/coder-mobile/src/verse_app.rs:3956-3973 — tests::remembered_world_relay_is_validated_before_restore

File SHA-256: `96af5bf3286424d6142b6e8879a5e36aef012950dc0ed166cf1f8eaf0bbb93be`

```rust
    #[test]
    fn remembered_world_relay_is_validated_before_restore() {
        for relay in [
            "ws://remote.example.test",
            "wss://example.test/?token=secret",
            "wss://user:pass@example.test",
        ] {
            let config: Config = serde_json::from_value(serde_json::json!({
                "secret_hex": "11".repeat(32), "width": 800, "height": 1200, "scale": 2,
                "world_relay": relay
            }))
            .unwrap();
            let restored = Scene::new(config).unwrap();
            assert!(restored.relay.is_none());
            assert!(restored.error.is_some());
            assert!(restored.session.is_none());
        }
    }
```

### crates/coder/src/delegate.rs:2360-2403 — tests::a_free_slot_refills_before_the_first_task_finishes

File SHA-256: `b4875c3cca09dd3ee18306a0584bc5488fe3ce0598cb83a727bb0db7c028d860`

```rust
    /// A completed later task releases its slot even while the first task waits.
    #[tokio::test]
    async fn a_free_slot_refills_before_the_first_task_finishes() {
        if !boundary_supported() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let marker = state.path().join("third-started");
        let binary = stub(
            dir.path(),
            "refill",
            &format!(
                "for a in \"$@\"; do prompt=\"$a\"; done\n\
                 case \"$prompt\" in\n\
                 first) i=0; while [ ! -f '{}' ] && [ \"$i\" -lt 100 ]; do sleep 0.05; i=$((i+1)); done; test -f '{}' || exit 17 ;;\n\
                 third) touch '{}' ;;\n\
                 esac\nprintf '%s\\n' \"$prompt\"",
                marker.display(),
                marker.display(),
                marker.display()
            ),
        );
        let tasks = ["first", "second", "third"]
            .into_iter()
            .map(|name| {
                Task::reading(name, "a.rs")
                    .expecting(name)
                    .bounded(Bounds::within(Duration::from_secs(10)))
            })
            .collect();
        let results =
            Delegator::new(executor(&binary).under(Policy::empty().granting(state.path())))
                .in_directory(dir.path())
                .bounded_to(2)
                .fan_out(tasks)
                .await;
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(Delegation::answered), "{results:?}");
        assert_eq!(
            results.iter().map(Delegation::answer).collect::<Vec<_>>(),
            ["first", "second", "third"]
        );
    }
```

### crates/coder-delegate/src/issue.rs:1466-1467 — CALLER_BEFORE

File SHA-256: `5f3d53f729a3813096e0eed282783c1dfd0a6b54a1462e8d3b7261c82e57e044`

```rust
/// Lines of a caller shown before and after its call.
const CALLER_BEFORE: usize = 20;
```

### crates/coder/src/task/autostart.rs:4508-4520 — tests::status_rejects_a_probe_flag_before_it_reads_a_login

File SHA-256: `362df1227fbf8ea8761a3da5364e535c9fcbe32e0e328f03f81a2dfd19363ae4`

```rust
    #[test]
    fn status_rejects_a_probe_flag_before_it_reads_a_login() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            cli(&[
                "status".into(),
                "--probe-usage".into(),
                "--root".into(),
                dir.path().to_string_lossy().into_owned(),
            ]),
            2
        );
    }
```
