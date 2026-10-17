use coder_sync::server::notifications::{Category, chat_title, notification_for, parse_prefs};
use openagents_doc::RegistryRow;
use openagents_proto::view::SESSION_STALE_MS;
use serde_json::{Value, json};
const NOW: i64 = 1_800_000_000_000;
fn session(status: &str, turn: Option<&str>, at: i64) -> RegistryRow {
    let mut fields = json!({"chatId":"c1","status":status,"updatedAt":at});
    if let Some(turn) = turn {
        fields["lastCompletedTurn"] = json!(turn);
    }
    serde_json::from_value(
        json!({"kind":"sessions","id":"c1","seq":1,"deleted":false,"fields":fields,"clocks":{}}),
    )
    .unwrap()
}
#[test]
fn completed_turn_notifies_once() {
    let before = session("working", None, NOW);
    let done = session("idle", Some("t1"), NOW);
    assert_eq!(
        notification_for(Some(&before), &done, NOW),
        Some(Category::Done)
    );
    assert_eq!(notification_for(Some(&done), &done, NOW), None);
}
#[test]
fn interrupted_or_expired_activity_is_silent() {
    for turn in [None, Some("old")] {
        assert_eq!(
            notification_for(
                Some(&session("working", turn, NOW)),
                &session("idle", turn, NOW),
                NOW
            ),
            None
        );
    }
}
#[test]
fn run_error_notifies_once_without_completion() {
    let failed = session("errored", Some("failed"), NOW);
    assert_eq!(
        notification_for(Some(&session("working", Some("old"), NOW)), &failed, NOW),
        Some(Category::Failed)
    );
    assert_eq!(notification_for(Some(&failed), &failed, NOW), None);
}
#[test]
fn question_notifies_once() {
    let asking = session("awaitingInput", Some("t1"), NOW);
    assert_eq!(
        notification_for(Some(&session("working", Some("t1"), NOW)), &asking, NOW),
        Some(Category::Input)
    );
    assert_eq!(notification_for(Some(&asking), &asking, NOW), None);
}
#[test]
fn coalesced_working_states_keep_completions() {
    let a = session("working", None, NOW);
    let b = session("working", Some("first"), NOW);
    let c = session("idle", Some("second"), NOW);
    assert_eq!(notification_for(Some(&a), &b, NOW), Some(Category::Done));
    assert_eq!(notification_for(Some(&b), &b, NOW), None);
    assert_eq!(notification_for(Some(&b), &c, NOW), Some(Category::Done));
}
#[test]
fn stale_completion_and_question_are_silent() {
    let a = session("working", None, NOW);
    assert_eq!(
        notification_for(
            Some(&a),
            &session("idle", Some("old"), NOW - SESSION_STALE_MS - 1),
            NOW
        ),
        None
    );
    assert_eq!(
        notification_for(Some(&a), &session("awaitingInput", None, NOW - 60_000), NOW),
        None
    );
}
#[test]
fn first_or_revived_row_only_sets_baseline() {
    assert_eq!(
        notification_for(None, &session("errored", Some("t"), NOW), NOW),
        None
    );
    let mut deleted = session("idle", None, NOW);
    deleted.deleted = true;
    assert_eq!(
        notification_for(Some(&deleted), &session("idle", Some("t"), NOW), NOW),
        None
    );
    assert_eq!(
        notification_for(Some(&session("working", None, NOW)), &deleted, NOW),
        None
    );
}
fn chat(fields: Value) -> RegistryRow {
    serde_json::from_value(json!({"kind":"chats","id":"c1","seq":1,"fields":fields,"clocks":{}}))
        .unwrap()
}
#[test]
fn titles_fall_back_and_archived_or_side_chats_are_skipped() {
    assert_eq!(
        chat_title(Some(&chat(json!({"title":"Fix login"})))),
        Some("Fix login".into())
    );
    assert_eq!(
        chat_title(Some(&chat(json!({})))),
        Some("New session".into())
    );
    assert_eq!(chat_title(None), Some("New session".into()));
    assert_eq!(chat_title(Some(&chat(json!({"parentChatId":"p"})))), None);
    assert_eq!(chat_title(Some(&chat(json!({"archived":true})))), None);
    let mut deleted = chat(json!({}));
    deleted.deleted = true;
    assert_eq!(chat_title(Some(&deleted)), None);
}
#[test]
fn prefs_default_on_and_keep_desktop_words() {
    assert_eq!(
        serde_json::to_value(parse_prefs(&Value::Null)).unwrap(),
        json!({"done":true,"input":true,"failed":true})
    );
    assert_eq!(
        serde_json::to_value(parse_prefs(&json!({"done":false}))).unwrap(),
        json!({"done":false,"input":true,"failed":true})
    );
    assert_eq!(Category::Done.body(), "Run finished");
    assert_eq!(Category::Input.body(), "Waiting on your input");
    assert_eq!(Category::Failed.body(), "Run failed");
}

// Delivery decisions (#11853): which phones get a wake for a decided
// notification, through the push gateway.
mod delivery {
    use super::{NOW, chat, session};
    use coder_sync::server::{
        notifications::{Category, PushPrefs},
        push::{Note, Recent, Target, notes_for, plan, request_id},
    };
    use openagents_doc::RegistryRow;
    use push_gateway::wire::{Transport, valid_uuid};
    use serde_json::json;
    use std::collections::{BTreeMap, HashSet};

    const SECONDS: u64 = 1_800_000_000;

    fn target(device: &str, prefs: PushPrefs) -> Target {
        Target {
            device: device.into(),
            transport: Transport::Apns,
            grant: format!("pg1_{device}"),
            expires_at: SECONDS + 3_600,
            prefs,
        }
    }
    fn all() -> PushPrefs {
        PushPrefs {
            done: true,
            input: true,
            failed: true,
        }
    }
    fn note(category: Category, marker: &str) -> Note {
        Note {
            chat_id: "c1".into(),
            category,
            marker: marker.into(),
        }
    }
    fn rows(list: &[RegistryRow]) -> BTreeMap<(String, String), RegistryRow> {
        list.iter()
            .map(|r| ((r.kind.clone(), r.id.clone()), r.clone()))
            .collect()
    }

    #[test]
    fn each_kind_reaches_phones_that_want_it() {
        let phones = [
            target("ios-1", all()),
            target(
                "ios-2",
                PushPrefs {
                    done: true,
                    input: false,
                    failed: false,
                },
            ),
        ];
        let mut recent = Recent::default();
        for (category, devices) in [
            (Category::Done, vec!["ios-1", "ios-2"]),
            (Category::Input, vec!["ios-1"]),
            (Category::Failed, vec!["ios-1"]),
        ] {
            let (wakes, _) = plan(
                "acct",
                &[note(category, "t1")],
                &phones,
                &HashSet::new(),
                SECONDS,
                &mut recent,
            );
            assert_eq!(
                wakes.iter().map(|w| w.device.as_str()).collect::<Vec<_>>(),
                devices,
                "{category:?}"
            );
            for wake in &wakes {
                assert_eq!(wake.category, category);
                assert_eq!(wake.grant, format!("pg1_{}", wake.device));
                assert!(valid_uuid(&wake.request_id), "{}", wake.request_id);
            }
        }
    }

    #[test]
    fn quiet_while_the_app_is_open() {
        let present = HashSet::from(["ios-1".to_owned()]);
        let (wakes, log) = plan(
            "acct",
            &[note(Category::Done, "t1")],
            &[target("ios-1", all()), target("ios-2", all())],
            &present,
            SECONDS,
            &mut Recent::default(),
        );
        assert_eq!(
            wakes.iter().map(|w| w.device.as_str()).collect::<Vec<_>>(),
            ["ios-2"]
        );
        assert!(
            log.iter()
                .any(|e| e.device == "ios-1" && e.result == "app open")
        );
    }

    #[test]
    fn the_same_notification_is_sent_once() {
        let mut phone = target("ios-1", all());
        phone.expires_at = SECONDS + 86_400;
        let phones = [phone];
        let mut recent = Recent::default();
        let first = plan(
            "acct",
            &[note(Category::Done, "t1")],
            &phones,
            &HashSet::new(),
            SECONDS,
            &mut recent,
        )
        .0;
        assert_eq!(first.len(), 1);
        let (again, log) = plan(
            "acct",
            &[note(Category::Done, "t1")],
            &phones,
            &HashSet::new(),
            SECONDS + 5,
            &mut recent,
        );
        assert!(again.is_empty());
        assert_eq!(log[0].result, "duplicate");
        // A new turn is news again, and so is the old one after an hour.
        let next = plan(
            "acct",
            &[note(Category::Done, "t2")],
            &phones,
            &HashSet::new(),
            SECONDS + 6,
            &mut recent,
        )
        .0;
        assert_eq!(next.len(), 1);
        assert_ne!(next[0].request_id, first[0].request_id);
        let later = plan(
            "acct",
            &[note(Category::Done, "t1")],
            &phones,
            &HashSet::new(),
            SECONDS + 3_601,
            &mut recent,
        )
        .0;
        assert_eq!(later[0].request_id, first[0].request_id);
    }

    #[test]
    fn request_ids_are_stable_and_distinct() {
        let a = request_id("acct", "ios-1", &note(Category::Done, "t1"));
        assert_eq!(a, request_id("acct", "ios-1", &note(Category::Done, "t1")));
        assert!(valid_uuid(&a));
        for other in [
            request_id("acct2", "ios-1", &note(Category::Done, "t1")),
            request_id("acct", "ios-2", &note(Category::Done, "t1")),
            request_id("acct", "ios-1", &note(Category::Input, "t1")),
            request_id("acct", "ios-1", &note(Category::Done, "t2")),
        ] {
            assert_ne!(a, other);
        }
    }

    #[test]
    fn lapsed_grants_and_no_phones_send_nothing() {
        let mut lapsed = target("ios-1", all());
        lapsed.expires_at = SECONDS;
        let (wakes, log) = plan(
            "acct",
            &[note(Category::Failed, "1")],
            &[lapsed],
            &HashSet::new(),
            SECONDS,
            &mut Recent::default(),
        );
        assert!(wakes.is_empty());
        assert_eq!(log[0].result, "grant expired");
        let (wakes, log) = plan(
            "acct",
            &[note(Category::Failed, "1")],
            &[],
            &HashSet::new(),
            SECONDS,
            &mut Recent::default(),
        );
        assert!(wakes.is_empty());
        assert_eq!(log[0].result, "no targets");
    }

    #[test]
    fn a_batch_yields_one_note_per_transition_with_titles_checked() {
        let before = session("working", None, NOW);
        let original = BTreeMap::from([("c1".to_owned(), Some(before.clone()))]);
        let done = session("idle", Some("t1"), NOW);
        let notes = notes_for(&original, &rows(&[done.clone()]), &|_| None, NOW);
        assert_eq!(notes, [note(Category::Done, "t1")]);
        // A side chat, whether in the batch or already stored, is silent.
        let side = chat(json!({"parentChatId":"p"}));
        assert!(
            notes_for(
                &original,
                &rows(&[done.clone(), side.clone()]),
                &|_| None,
                NOW
            )
            .is_empty()
        );
        assert!(
            notes_for(
                &original,
                &rows(&[done.clone()]),
                &|_| Some(side.clone()),
                NOW
            )
            .is_empty()
        );
        // A row first seen in this batch only sets the baseline.
        let new = BTreeMap::from([("c1".to_owned(), None)]);
        assert!(notes_for(&new, &rows(&[done]), &|_| None, NOW).is_empty());
        // Questions and failures are told apart by when they happened.
        let asking = session("awaitingInput", None, NOW);
        let notes = notes_for(&original, &rows(&[asking]), &|_| None, NOW);
        assert_eq!(notes, [note(Category::Input, &NOW.to_string())]);
    }

    #[test]
    fn grant_bodies_are_checked() {
        use coder_sync::server::push::parse_grant;
        let good = json!({"transport":"apns","grant":"pg1_abc","expiresAt":SECONDS + 10,"prefs":{"done":false}});
        let (transport, grant, at, prefs) = parse_grant(&good, SECONDS).unwrap();
        assert_eq!(
            (transport, grant.as_str(), at),
            (Transport::Apns, "pg1_abc", SECONDS + 10)
        );
        assert!(!prefs.done && prefs.input && prefs.failed);
        for bad in [
            json!({"transport":"sms","grant":"g","expiresAt":SECONDS + 10}),
            json!({"transport":"fcm","grant":"","expiresAt":SECONDS + 10}),
            json!({"transport":"fcm","grant":"has space","expiresAt":SECONDS + 10}),
            json!({"transport":"fcm","grant":"g","expiresAt":SECONDS}),
            json!({"transport":"fcm","grant":"g".repeat(4097),"expiresAt":SECONDS + 10}),
        ] {
            assert!(parse_grant(&bad, SECONDS).is_none(), "{bad}");
        }
    }

    #[test]
    fn push_settings_are_all_or_nothing() {
        use coder_sync::server::push::Settings;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("push.key");
        std::fs::write(&key, "07".repeat(32)).unwrap();
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
        let key = key.to_str().unwrap().to_owned();
        let parse = |pairs: &[(&str, &str)]| {
            let pairs = pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect::<std::collections::HashMap<_, _>>();
            Settings::parse(&|k| pairs.get(k).cloned())
        };
        assert!(parse(&[]).unwrap().is_none());
        assert!(parse(&[("CODER_SYNC_PUSH_GATEWAY", "http://10.0.0.2:8090")]).is_err());
        let full = [
            ("CODER_SYNC_PUSH_GATEWAY", "http://10.0.0.2:8090"),
            ("CODER_SYNC_PUSH_REGISTRATION", "https://push.example.com"),
            ("CODER_SYNC_PUSH_KEY_FILE", key.as_str()),
            ("CODER_SYNC_PUSH_APNS_PROFILE", "app.ios"),
        ];
        let settings = parse(&full).unwrap().unwrap();
        assert_eq!(settings.pubkey().len(), 64);
        assert_eq!(settings.public()["profiles"]["apns"], "app.ios");
        assert_eq!(settings.public()["delegate"], settings.pubkey());
        let mut plain = full;
        plain[1].1 = "http://push.example.com";
        assert!(parse(&plain).is_err());
        let mut no_profile = full.to_vec();
        no_profile.pop();
        assert!(parse(&no_profile).is_err());
        std::fs::set_permissions(&full[2].1, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(parse(&full).is_err());
    }
}
