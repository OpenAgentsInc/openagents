use super::*;
use lab_kit::{LabKind, Refusal, Report};

fn version(content: Value, number: u64) -> Version {
    Version {
        document: "film-fixture".into(),
        number,
        parent: number.checked_sub(1).filter(|n| *n > 0),
        author: "fixture".into(),
        summary: "Synthetic film".into(),
        created_unix_ms: 1,
        schema: "cut.v1".into(),
        content_sha256: lab_kit::digest(&content),
        content,
        report: Report::default(),
        assets: vec![],
    }
}

fn fixture() -> Timeline {
    Timeline::new(
        version(
            json!({"fps":30,"shots":[
        {"name":"a","src":"a.mp4","dur":4},
        {"name":"b","src":"b.mp4","dur":4},
        {"name":"c","src":"c.mp4","dur":4}],
        "voices":[{"src":"voice.wav","at":4.5}],
        "captions":[{"text":"Hi","at":5,"dur":1,"shot":"b"}]}),
            1,
        ),
        json!({"voice.wav":{"duration":1.5}}),
    )
    .unwrap()
}

#[test]
fn only_cards_with_an_editable_video_cut_offer_the_editor() {
    let mut card = video_lab::VideoKind::card(fixture().version(), &[], &[]);
    assert!(has_timeline(&card));
    card.detail["shots"] = json!([]);
    assert!(!has_timeline(&card));
    card.detail["shots"] = json!([{"name":"one"}]);
    card.kind = "photo".into();
    assert!(!has_timeline(&card));
}

#[test]
fn native_gestures_match_web_intents_and_preserve_linked_offsets() {
    let mut timeline = fixture();
    for gesture in [
        Gesture::Link {
            id: "audio:0".into(),
            shot: Some("b".into()),
        },
        Gesture::Move {
            id: "video:b".into(),
            at: 0.,
        },
        Gesture::Speed {
            id: "video:a".into(),
            value: 2.,
        },
        Gesture::Text {
            id: "text:0".into(),
            value: "Go".into(),
        },
        Gesture::Duration {
            id: "text:0".into(),
            value: 1.5,
        },
        Gesture::Dissolve {
            id: "video:a".into(),
            enabled: true,
        },
    ] {
        let base = timeline.version().clone();
        let web = video_lab::timeline::request(&json!({"action":"edit", "document":base.content,
            "command":gesture.command(),"media":timeline.media}))
        .unwrap();
        let request = timeline.begin("me", gesture).unwrap();
        assert_eq!(request.base, base.number);
        assert_eq!(
            timeline.version().number,
            base.number,
            "a preview is not a saved version"
        );
        assert_eq!(timeline.document(), &web["document"]);
        let draft = video_lab::VideoKind::apply(
            &base,
            &request.change,
            &request.summary,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(timeline.document(), &draft.content);
        assert!(timeline.saving());
        let saved = version(draft.content, base.number + 1);
        timeline.finish(EditResult::Created(saved)).unwrap();
        assert!(!timeline.saving());
    }
    assert_eq!(timeline.document()["shots"][0]["name"], "b");
    assert_eq!(timeline.document()["shots"][1]["name"], "a");
    assert_eq!(timeline.document()["shots"][1]["dur"], 2.0);
    assert_eq!(timeline.document()["shots"][1]["speed"], 2.0);
    assert_eq!(timeline.document()["shots"][1]["xfade"], 0.5);
    assert_eq!(timeline.document()["voices"][0]["at"], 0.5);
    assert_eq!(timeline.document()["captions"][0]["at"], 1.0);
    assert_eq!(timeline.document()["captions"][0]["text"], "Go");
    assert_eq!(timeline.document()["captions"][0]["dur"], 1.5);
}

#[test]
fn local_refusal_leaves_the_saved_document_and_controls_unchanged() {
    let mut timeline = fixture();
    let before = timeline.document().clone();
    for gesture in [
        Gesture::Text {
            id: "text:0".into(),
            value: " ".into(),
        },
        Gesture::Speed {
            id: "audio:0".into(),
            value: 2.,
        },
        Gesture::Dissolve {
            id: "video:a".into(),
            enabled: true,
        },
        Gesture::Link {
            id: "audio:0".into(),
            shot: Some("missing".into()),
        },
        Gesture::Move {
            id: "audio:0".into(),
            at: -1.,
        },
    ] {
        assert!(timeline.begin("me", gesture).is_err());
        assert_eq!(timeline.document(), &before);
        assert!(!timeline.saving());
        assert!(timeline.editable());
    }
}

#[test]
fn refusal_conflict_and_lost_response_cannot_advance_or_repeat_a_save() {
    for outcome in 0..3 {
        let mut timeline = fixture();
        let before = timeline.document().clone();
        timeline
            .begin(
                "me",
                Gesture::Text {
                    id: "text:0".into(),
                    value: "Go".into(),
                },
            )
            .unwrap();
        assert!(
            timeline
                .begin(
                    "me",
                    Gesture::Move {
                        id: "video:b".into(),
                        at: 0.
                    }
                )
                .is_err()
        );
        match outcome {
            0 => timeline
                .finish(EditResult::Refused(Refusal::new(
                    "fixture",
                    "Choose a shorter caption.",
                )))
                .unwrap(),
            1 => timeline.finish(EditResult::Stale).unwrap(),
            _ => timeline.unknown(),
        }
        assert_eq!(timeline.version().number, 1);
        assert_eq!(timeline.document(), &before);
        assert_eq!(timeline.editable(), outcome == 0);
        assert_eq!(timeline.requires_reload, outcome != 0);
        assert!(!timeline.saving());
        assert!(oa_copy::violations(timeline.notice.as_deref().unwrap(), &[]).is_empty());
    }
}

#[test]
fn acknowledgements_must_match_the_exact_document_parent_and_content_hash() {
    for mutation in 0..4 {
        let mut timeline = fixture();
        timeline
            .begin(
                "me",
                Gesture::Text {
                    id: "text:0".into(),
                    value: "Go".into(),
                },
            )
            .unwrap();
        let mut saved = version(timeline.document().clone(), 2);
        match mutation {
            0 => saved.document = "other-film".into(),
            1 => saved.parent = Some(10),
            2 => saved.number = 10,
            _ => saved.content_sha256 = "0".repeat(64),
        }
        assert!(timeline.finish(EditResult::Created(saved)).is_err());
        assert_eq!(timeline.version().number, 1);
        assert!(timeline.requires_reload);
        assert!(!timeline.editable());
    }
}

#[test]
fn arrows_move_one_frame_or_one_second_and_scrubbing_stays_on_the_film() {
    let mut timeline = fixture();
    timeline.step(1., false);
    assert!((timeline.playhead - 1. / 30.).abs() < 1e-9);
    timeline.step(1., true);
    assert!((timeline.playhead - 31. / 30.).abs() < 1e-9);
    timeline.step(-1., true);
    timeline.step(-1., false);
    assert_eq!(timeline.playhead, 0.);
    timeline.seek(1000.);
    assert_eq!(timeline.playhead, timeline.duration());
    timeline.seek(-1.);
    assert_eq!(timeline.playhead, 0.);
    timeline.seek(f64::NAN);
    assert_eq!(timeline.playhead, 0.);
    timeline.select("video:b");
    timeline.select("missing");
    assert_eq!(timeline.selected.as_deref(), Some("video:b"));
}

#[test]
fn unlink_and_frame_snapping_use_the_shared_timeline_rules() {
    let mut timeline = fixture();
    timeline
        .begin(
            "me",
            Gesture::Link {
                id: "text:0".into(),
                shot: None,
            },
        )
        .unwrap();
    let saved = version(timeline.document().clone(), 2);
    timeline.finish(EditResult::Created(saved)).unwrap();
    timeline
        .begin(
            "me",
            Gesture::Move {
                id: "text:0".into(),
                at: 1.02,
            },
        )
        .unwrap();
    assert!((timeline.document()["captions"][0]["at"].as_f64().unwrap() - 31. / 30.).abs() < 1e-9);
    assert!(
        timeline
            .clips()
            .iter()
            .find(|c| c.id == "text:0")
            .unwrap()
            .link
            .is_none()
    );
}

#[test]
fn selected_clip_keys_match_the_webs_frame_and_second_move_commands() {
    let mut timeline = fixture();
    timeline
        .begin(
            "me",
            Gesture::Link {
                id: "text:0".into(),
                shot: None,
            },
        )
        .unwrap();
    timeline
        .finish(EditResult::Created(version(timeline.document().clone(), 2)))
        .unwrap();
    timeline.select("text:0");
    for (direction, second) in [(1., false), (-1., true)] {
        let gesture = timeline.nudge(direction, second).unwrap();
        let web = video_lab::timeline::request(
            &json!({"action":"edit","document":timeline.version().content,
            "media":timeline.media,"command":gesture.command()}),
        )
        .unwrap();
        let before = timeline
            .clips()
            .into_iter()
            .find(|clip| clip.id == "text:0")
            .unwrap()
            .at;
        let next = timeline.version().number + 1;
        timeline.begin("me", gesture).unwrap();
        assert_eq!(timeline.document(), &web["document"]);
        let after = timeline
            .clips()
            .into_iter()
            .find(|clip| clip.id == "text:0")
            .unwrap()
            .at;
        assert!(
            (after - before - direction * if second { 1. } else { 1. / timeline.fps() }).abs()
                < 1e-9
        );
        timeline
            .finish(EditResult::Created(version(
                timeline.document().clone(),
                next,
            )))
            .unwrap();
    }
}

#[test]
fn drag_previews_follow_linked_clips_without_saving_or_locking_the_editor() {
    let timeline = fixture();
    let before = timeline.document().clone();
    let preview = timeline
        .preview(&Gesture::Move {
            id: "video:b".into(),
            at: 0.,
        })
        .unwrap();
    assert_eq!(
        preview.clips.iter().find(|c| c.id == "text:0").unwrap().at,
        1.
    );
    assert_eq!(timeline.document(), &before);
    assert_eq!(timeline.version().number, 1);
    assert!(timeline.editable());
    assert!(
        timeline
            .preview(&Gesture::Move {
                id: "video:b".into(),
                at: 9.
            })
            .is_ok()
    );
    assert!(
        timeline
            .preview(&Gesture::Text {
                id: "text:0".into(),
                value: "".into()
            })
            .is_err()
    );
    assert_eq!(timeline.document(), &before);
}

mod wire {
    use super::*;
    use axum::{
        Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get,
    };
    use openagents::{Client, CredentialRecord, CredentialStore, Deployment, StorageError};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
    };

    struct Volatile;
    impl CredentialStore for Volatile {
        fn load(&self, _: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
            Ok(None)
        }
        fn save(&self, _: &CredentialRecord) -> Result<(), StorageError> {
            Ok(())
        }
        fn clear(&self, _: &Deployment) -> Result<(), StorageError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct Server {
        calls: AtomicUsize,
        outcome: AtomicU8,
        pause: AtomicBool,
        entered: tokio::sync::Notify,
        release: tokio::sync::Notify,
        last: Mutex<Option<Value>>,
        job_down: AtomicBool,
        job_pending: AtomicBool,
        job_done: AtomicBool,
        events: AtomicUsize,
        job_keys: Mutex<Vec<String>>,
    }

    fn job(server: &Server) -> lab_kit::JobState {
        let spec = video_lab::VideoKind::jobs_for(fixture().version(), &Want::Card).remove(0);
        let status = if server.job_pending.load(Ordering::SeqCst)
            && !server.job_done.load(Ordering::SeqCst)
        {
            JobStatus::Queued
        } else {
            JobStatus::Done
        };
        lab_kit::JobState {
            id: "job_fixture".into(),
            spec,
            status,
            message: None,
            estimate_seconds: None,
            artifacts: vec![],
        }
    }

    async fn edit(
        State(server): State<Arc<Server>>,
        Json(request): Json<EditDocument>,
    ) -> axum::response::Response {
        server.calls.fetch_add(1, Ordering::SeqCst);
        *server.last.lock().unwrap() = Some(serde_json::to_value(&request).unwrap());
        server.entered.notify_one();
        if server.pause.load(Ordering::SeqCst) {
            server.release.notified().await;
        }
        match server.outcome.load(Ordering::SeqCst) {
            1 => (
                StatusCode::CONFLICT,
                Json(json!({"error":{"code":"stale_base","message":"Reload this film."}})),
            )
                .into_response(),
            2 => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error":{"code":"timeline","message":"Choose a shorter caption."}})),
            )
                .into_response(),
            3 => (
                StatusCode::FORBIDDEN,
                Json(json!({"error":{"code":"forbidden","message":"This film is no longer available."}})),
            ).into_response(),
            _ => {
                let base = fixture().version().clone();
                let draft = video_lab::VideoKind::apply(
                    &base,
                    &request.change,
                    &request.summary,
                    &Default::default(),
                )
                .unwrap();
                Json(version(draft.content, 2)).into_response()
            }
        }
    }

    struct Fixture {
        controller: Arc<crate::Controller>,
        source: Source,
        state: Arc<Server>,
        task: tokio::task::JoinHandle<()>,
        _root: tempfile::TempDir,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.source.close();
            self.task.abort();
        }
    }

    async fn setup() -> Fixture {
        let state = Arc::new(Server::default());
        let router = Router::new()
            .route("/v1/account", get(|| async { Json(json!({"account":{"id":"acct_film_fixture","label":"Fixture"},"workspaces":[]})) }))
            .route("/v1/labs/documents/{id}", get(|State(state): State<Arc<Server>>| async move {
                state.calls.fetch_add(1, Ordering::SeqCst);
                Json(json!({"id":"film-fixture","kind":"video","schema":"cut.v1","title":"Synthetic film","created_unix_ms":1,"latest":1}))
            }))
            .route("/v1/labs/documents/{id}/versions/{n}", get(|State(state): State<Arc<Server>>| async move {
                state.calls.fetch_add(1, Ordering::SeqCst);
                Json(fixture().version().clone())
            }))
            .route("/v1/labs/documents/{id}/versions", axum::routing::post(edit))
            .route("/v1/labs/documents/{id}/versions/{n}/card", get(|State(state): State<Arc<Server>>| async move {
                state.calls.fetch_add(1, Ordering::SeqCst);
                let jobs = if state.job_keys.lock().unwrap().is_empty() { vec![] } else { vec![job(&state)] };
                Json(video_lab::VideoKind::card(fixture().version(), &jobs, &[]))
            }))
            .route("/v1/labs/artifacts/{id}", get(|State(state): State<Arc<Server>>| async move {
                state.calls.fetch_add(1, Ordering::SeqCst);
                (StatusCode::NOT_FOUND, Json(json!({"error":{"code":"not_found","message":"This picture is no longer available."}})))
            }))
            .route("/v1/labs/jobs", axum::routing::post(|State(state): State<Arc<Server>>, headers: axum::http::HeaderMap, Json(request): Json<CreateJob>| async move {
                state.job_keys.lock().unwrap().push(headers["idempotency-key"].to_str().unwrap().into());
                if state.job_down.load(Ordering::SeqCst) {
                    return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error":{"code":"unavailable","message":"Try again."}}))).into_response();
                }
                assert_eq!(request.version, fixture().version().reference());
                Json(job(&state)).into_response()

            }))
            .route("/v1/labs/jobs/{id}", get(|State(state): State<Arc<Server>>| async move { Json(job(&state)) }))
            .route("/v1/labs/documents/{id}/events", get(|State(state): State<Arc<Server>>, axum::extract::Query(query): axum::extract::Query<std::collections::BTreeMap<String,String>>| async move {
                state.events.fetch_add(1, Ordering::SeqCst);
                if query.get("since").is_some_and(|cursor| cursor != "0") {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
                state.job_done.store(true, Ordering::SeqCst);
                let event = json!({"cursor":1,"document":"film-fixture","at_unix_ms":1,"change":{"job":"job_fixture"}});
                ([("content-type", "text/event-stream")], format!("id: 1\ndata: {}\n\n", json!({"data":[event]})))
            })).with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client =
            Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
        let root = tempfile::tempdir().unwrap();
        let controller =
            crate::Controller::new(client, Arc::new(Volatile), root.path().join("account"));
        controller
            .import_credential(format!("oa_sk_{}", "1".repeat(64)))
            .await;
        let lease = controller.lease().unwrap();
        let labs = lease.client().labs(lease.session()).unwrap();
        let document = labs.document_ref("me", "film-fixture").unwrap();
        let source = Source::new(lease, document).unwrap();
        Fixture {
            controller,
            source,
            state,
            task,
            _root: root,
        }
    }

    #[tokio::test]
    async fn native_save_sends_the_expected_base_and_handles_typed_server_outcomes() {
        let f = setup().await;
        for outcome in [0, 1, 2] {
            f.state.outcome.store(outcome, Ordering::SeqCst);
            let (mut timeline, _) = f.source.load().await.unwrap();
            let request = timeline
                .begin(
                    f.source.space(),
                    Gesture::Text {
                        id: "text:0".into(),
                        value: "Go".into(),
                    },
                )
                .unwrap();
            let result = f.source.save(request).await.unwrap();
            timeline.finish(result).unwrap();
            assert_eq!(timeline.version().number, if outcome == 0 { 2 } else { 1 });
            assert_eq!(timeline.requires_reload, outcome == 1);
            let body = f.state.last.lock().unwrap().clone().unwrap();
            assert_eq!(body["space"], "me");
            assert_eq!(body["base"], 1);
            assert_eq!(
                body["change"]["intent"]["command"],
                json!({"op":"text","id":"text:0","value":"Go"})
            );
        }
    }

    #[tokio::test]
    async fn windows_cancel_independently_and_account_changes_stop_all_document_calls() {
        let f = setup().await;
        let a = f.source.fork();
        let b = f.source.fork();
        a.close();
        assert!(!a.current());
        assert!(b.current());
        assert!(f.source.current());
        let (mut timeline, mut card) = f.source.load().await.unwrap();
        let calls = f.state.calls.load(Ordering::SeqCst);
        assert!(f.source.media(&card, "not-in-this-card").await.is_none());
        card.document = "different-film".into();
        assert!(f.source.media(&card, "any-file").await.is_none());
        assert_eq!(f.state.calls.load(Ordering::SeqCst), calls);
        let lease = f.controller.lease().unwrap();
        let document = f.source.document.clone();
        let request = timeline
            .begin(
                f.source.space(),
                Gesture::Text {
                    id: "text:0".into(),
                    value: "Go".into(),
                },
            )
            .unwrap();
        f.controller.invalidate();
        assert!(!b.current());
        assert!(!f.source.current());
        tokio::time::timeout(Duration::from_secs(1), f.source.closed())
            .await
            .unwrap();
        assert!(Source::new(lease, document).is_none());
        assert!(f.source.load().await.is_err());
        assert!(f.source.save(request).await.is_err());
        assert!(f.source.media(&card, "any-file").await.is_none());
        assert_eq!(f.state.calls.load(Ordering::SeqCst), calls);
    }

    #[tokio::test]
    async fn a_save_in_flight_cannot_return_a_previous_accounts_document() {
        let f = setup().await;
        let (mut timeline, _) = f.source.load().await.unwrap();
        f.state.pause.store(true, Ordering::SeqCst);
        let request = timeline
            .begin(
                f.source.space(),
                Gesture::Text {
                    id: "text:0".into(),
                    value: "Go".into(),
                },
            )
            .unwrap();
        let source = f.source.clone();
        let pending = tokio::spawn(async move { source.save(request).await });
        tokio::time::timeout(Duration::from_secs(5), f.state.entered.notified())
            .await
            .unwrap();
        f.controller.invalidate();
        assert!(
            tokio::time::timeout(Duration::from_secs(5), pending)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        f.state.release.notify_one();
    }

    #[tokio::test]
    async fn retrying_a_preview_keeps_its_request_key_after_an_unknown_outcome() {
        let f = setup().await;
        let (timeline, _) = f.source.load().await.unwrap();
        f.state.job_down.store(true, Ordering::SeqCst);
        let (cards, _) = tokio::sync::mpsc::channel(4);
        assert!(f.source.render(timeline.version(), cards).await.is_err());
        f.state.job_down.store(false, Ordering::SeqCst);
        let (cards, mut updates) = tokio::sync::mpsc::channel(4);
        f.source.render(timeline.version(), cards).await.unwrap();
        assert_eq!(updates.recv().await.unwrap().version, 1);
        let keys = f.state.job_keys.lock().unwrap();
        assert!(keys.len() >= 2);
        assert!(keys.iter().all(|key| key == &keys[0]));
    }

    #[tokio::test]
    async fn loss_of_document_access_closes_the_editor_and_stops_further_reads() {
        let f = setup().await;
        let (mut timeline, _) = f.source.load().await.unwrap();
        f.state.outcome.store(3, Ordering::SeqCst);
        let request = timeline
            .begin(
                f.source.space(),
                Gesture::Text {
                    id: "text:0".into(),
                    value: "Go".into(),
                },
            )
            .unwrap();
        assert!(f.source.save(request).await.is_err());
        assert!(!f.source.current());
        tokio::time::timeout(Duration::from_secs(1), f.source.closed())
            .await
            .unwrap();
        let calls = f.state.calls.load(Ordering::SeqCst);
        assert!(f.source.load().await.is_err());
        assert_eq!(f.state.calls.load(Ordering::SeqCst), calls);
        assert!(f.controller.lease().unwrap().is_current());
    }

    #[tokio::test]
    async fn a_running_preview_follows_its_event_and_publishes_the_finished_card() {
        let f = setup().await;
        f.state.job_pending.store(true, Ordering::SeqCst);
        let (timeline, _) = f.source.load().await.unwrap();
        let (cards, mut updates) = tokio::sync::mpsc::channel(4);
        tokio::time::timeout(
            Duration::from_secs(10),
            f.source.render(timeline.version(), cards),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(updates.recv().await.unwrap().state, Some(JobStatus::Queued));
        assert_eq!(updates.recv().await.unwrap().state, Some(JobStatus::Done));
        assert!(f.state.events.load(Ordering::SeqCst) >= 1);
        assert!(f.source.current());
    }

    #[tokio::test]
    async fn a_missing_picture_keeps_the_film_open_but_a_missing_document_closes_it() {
        let f = setup().await;
        let (_, mut card) = f.source.load().await.unwrap();
        card.poster = Some("missing-poster".into());
        assert!(f.source.media(&card, "missing-poster").await.is_none());
        assert!(f.source.current());
        assert!(f.source.load().await.is_ok());
        f.source.document_failed(labs::Error::Refused {
            status: 404,
            refusal: Refusal::new("not_found", "This film is no longer available."),
            retry_after: None,
        });
        assert!(!f.source.current());
    }

    #[tokio::test]
    async fn an_idle_editor_closes_when_its_account_controller_disappears() {
        let f = setup().await;
        let source = f.source.fork();
        let watching = source.clone();
        let closed = tokio::spawn(async move { watching.closed().await });
        tokio::task::yield_now().await;
        drop(f);
        assert!(!source.current());
        tokio::time::timeout(Duration::from_secs(3), closed)
            .await
            .unwrap()
            .unwrap();
        assert!(source.cancel.is_cancelled());
    }
}
