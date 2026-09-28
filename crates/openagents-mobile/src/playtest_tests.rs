use super::*;
use secp256k1::Secp256k1;

const WORLD: [u8; 32] = [3; 32];
const TRIAGE: [u8; 32] = [4; 32];

#[derive(Default)]
struct Fake {
    sent: Mutex<Vec<Event>>,
    refuse: bool,
}

impl Relay for Fake {
    fn publish(&self, wrap: &Event, _auth: &SecretKey) -> Result<(), String> {
        if self.refuse {
            return Err("offline".into());
        }
        self.sent.lock().unwrap().push(wrap.clone());
        Ok(())
    }
}

fn triage_hex() -> String {
    SecretKey::from_byte_array(TRIAGE)
        .unwrap()
        .x_only_public_key(&Secp256k1::new())
        .0
        .to_string()
}

fn world() -> SecretKey {
    SecretKey::from_byte_array(WORLD).unwrap()
}

fn form(tab: Tab, route: Route) -> Form {
    Form {
        app_version: "1.0.0".into(),
        build: "16".into(),
        device: "iPhone17,1".into(),
        os_version: "26.0".into(),
        tab,
        route,
        kind: Kind::Confusing,
        happened: "I couldn't find the Gym.".into(),
        expected: "A sign.".into(),
        steps: "Spawn, look around.".into(),
        quote: true,
        include_task: false,
        include_session: false,
        session_digest: String::new(),
        screenshot: None,
    }
}

fn jpeg() -> Screenshot {
    Screenshot {
        jpeg_base64: "/9j/AAAA".into(),
        width: 10,
        height: 20,
    }
}

fn setup(relay: Arc<Fake>, triage: Option<&str>) -> (Playtest, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Cache::open(dir.path(), &world()).unwrap();
    (Playtest::new(Some(store), relay, triage), dir)
}

#[test]
fn a_report_is_sealed_to_the_triage_key_and_listed_with_its_code() {
    let relay = Arc::new(Fake::default());
    let key = triage_hex();
    let (mut playtest, _dir) = setup(relay.clone(), Some(&key));
    let mut filed = form(Tab::Verse, Route::Gym);
    filed.screenshot = Some(jpeg());
    let packet = playtest.send(filed, &world(), None, Platform::Ios);
    assert!(packet.error.is_none(), "{:?}", packet.error);
    playtest.wait();
    let sent = relay.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    let opened = report::open(&sent[0], &SecretKey::from_byte_array(TRIAGE).unwrap()).unwrap();
    assert_eq!(opened.report.happened, "I couldn't find the Gym.");
    assert_eq!(opened.report.context.build_label(), "1.0.0 (16)");
    assert!(opened.report.screenshot.is_some());
    // Signed by the world key.
    let world_hex = world().x_only_public_key(&Secp256k1::new()).0.to_string();
    assert_eq!(opened.tester, world_hex);
    let listed = playtest.reports(None);
    assert_eq!(listed.reports.len(), 1);
    let row = &listed.reports[0];
    assert_eq!(row.status, Status::Sent);
    assert_eq!(row.code.as_deref(), Some(opened.code.as_str()));
    assert_eq!(row.place, "verse/gym");
    assert_eq!(row.build, "1.0.0 (16)");
}

#[test]
fn without_the_triage_key_a_report_waits_on_the_phone_and_sends_later() {
    let relay = Arc::new(Fake::default());
    let dir = tempfile::tempdir().unwrap();
    {
        let store = Cache::open(dir.path(), &world()).unwrap();
        let mut playtest = Playtest::new(Some(store), relay.clone(), None);
        let packet = playtest.send(form(Tab::Coder, Route::Chat), &world(), None, Platform::Ios);
        assert!(!packet.triage_ready);
        assert_eq!(packet.sent.unwrap().status, Status::Waiting);
        playtest.wait();
    }
    assert!(relay.sent.lock().unwrap().is_empty());
    // A later build carries the key: My reports sends what waited.
    let store = Cache::open(dir.path(), &world()).unwrap();
    let key = triage_hex();
    let mut playtest = Playtest::new(Some(store), relay.clone(), Some(&key));
    let _ = playtest.reports(Some(&world()));
    playtest.wait();
    assert_eq!(relay.sent.lock().unwrap().len(), 1);
    assert_eq!(playtest.reports(None).reports[0].status, Status::Sent);
}

#[test]
fn a_failed_send_is_kept_and_sent_again_from_my_reports() {
    let refusing = Arc::new(Fake {
        refuse: true,
        ..Fake::default()
    });
    let key = triage_hex();
    let dir = tempfile::tempdir().unwrap();
    {
        let store = Cache::open(dir.path(), &world()).unwrap();
        let mut playtest = Playtest::new(Some(store), refusing, Some(&key));
        let _ = playtest.send(form(Tab::Verse, Route::Home), &world(), None, Platform::Ios);
        playtest.wait();
        let row = &playtest.reports(None).reports[0];
        assert_eq!(row.status, Status::Failed);
        assert_eq!(row.error.as_deref(), Some("offline"));
    }
    let relay = Arc::new(Fake::default());
    let store = Cache::open(dir.path(), &world()).unwrap();
    let mut playtest = Playtest::new(Some(store), relay.clone(), Some(&key));
    let _ = playtest.reports(Some(&world()));
    playtest.wait();
    assert_eq!(relay.sent.lock().unwrap().len(), 1);
}

#[test]
fn no_screenshot_is_taken_from_the_wallet_or_a_key_screen() {
    let relay = Arc::new(Fake::default());
    let key = triage_hex();
    let (mut playtest, _dir) = setup(relay.clone(), Some(&key));
    for (tab, route) in [
        (Tab::Wallet, Route::Home),
        (Tab::Wallet, Route::Send),
        (Tab::Account, Route::Identity),
        (Tab::Account, Route::Trainer),
    ] {
        assert!(!playtest.draft(tab, route, None).screenshot_allowed);
        let mut filed = form(tab, route);
        filed.screenshot = Some(jpeg());
        let packet = playtest.send(filed, &world(), None, Platform::Ios);
        assert!(
            packet.error.unwrap().contains("never sent from the Wallet"),
            "{tab:?} {route:?}"
        );
    }
    playtest.wait();
    assert!(relay.sent.lock().unwrap().is_empty());
    assert!(
        playtest
            .draft(Tab::Verse, Route::Gym, None)
            .screenshot_allowed
    );
}

#[test]
fn the_task_id_is_attached_only_from_coder_when_ticked() {
    let (playtest_relay, key) = (Arc::new(Fake::default()), triage_hex());
    let (mut playtest, _dir) = setup(playtest_relay.clone(), Some(&key));
    assert_eq!(
        playtest
            .draft(Tab::Coder, Route::Chat, Some("task-1".into()))
            .task
            .as_deref(),
        Some("task-1")
    );
    assert!(
        playtest
            .draft(Tab::Verse, Route::Home, Some("task-1".into()))
            .task
            .is_none()
    );
    let mut filed = form(Tab::Coder, Route::Chat);
    let _ = playtest.send(
        filed.clone(),
        &world(),
        Some("task-1".into()),
        Platform::Ios,
    );
    filed.happened = "Second.".into();
    filed.include_task = true;
    let _ = playtest.send(filed, &world(), Some("task-1".into()), Platform::Ios);
    playtest.wait();
    let triage = SecretKey::from_byte_array(TRIAGE).unwrap();
    let tasks: Vec<Option<String>> = playtest_relay
        .sent
        .lock()
        .unwrap()
        .iter()
        .map(|wrap| report::open(wrap, &triage).unwrap().report.task)
        .collect();
    assert!(tasks.contains(&None) && tasks.contains(&Some("task-1".into())));
}

#[test]
fn the_session_log_records_only_while_on_and_leaves_only_as_previewed() {
    let relay = Arc::new(Fake::default());
    let key = triage_hex();
    let (mut playtest, _dir) = setup(relay.clone(), Some(&key));
    // Off: nothing is recorded, and nothing can be attached.
    playtest.screen(Tab::Verse, Route::Gym);
    playtest.observe(true, true, false);
    let draft = playtest.draft(Tab::Verse, Route::Gym, None);
    assert!(!draft.session_on && draft.session_lines.is_empty());
    let mut filed = form(Tab::Verse, Route::Gym);
    filed.include_session = true;
    filed.session_digest = draft.session_digest;
    assert!(
        playtest
            .send(filed, &world(), None, Platform::Ios)
            .error
            .is_some()
    );
    // On: structural events only.
    playtest.set_session(true, Tab::Verse, Route::Home);
    playtest.screen(Tab::Verse, Route::Gym);
    playtest.observe(true, true, false);
    playtest.observe(true, true, false);
    playtest.lifecycle(false);
    let draft = playtest.draft(Tab::Verse, Route::Gym, None);
    let codes: Vec<&str> = draft
        .session_lines
        .iter()
        .map(|l| l.rsplit(' ').next().unwrap())
        .collect();
    assert_eq!(
        codes,
        [
            "started",
            "screen",
            "notice",
            "wallet-error",
            "background",
            "report-opened"
        ]
    );
    // A log that changed after the preview isn't attached.
    playtest.screen(Tab::Verse, Route::Results);
    let mut filed = form(Tab::Verse, Route::Gym);
    filed.include_session = true;
    filed.session_digest = draft.session_digest.clone();
    let refused = playtest.send(filed.clone(), &world(), None, Platform::Ios);
    assert!(refused.error.unwrap().contains("changed since you looked"));
    // The previewed log goes, exactly as shown.
    let draft = playtest.draft(Tab::Verse, Route::Results, None);
    filed.session_digest = draft.session_digest.clone();
    filed.route = Route::Results;
    let packet = playtest.send(filed, &world(), None, Platform::Ios);
    assert!(packet.error.is_none());
    playtest.wait();
    let triage = SecretKey::from_byte_array(TRIAGE).unwrap();
    let sent = relay.sent.lock().unwrap();
    let opened = report::open(&sent[0], &triage).unwrap();
    let attached = opened.report.session.unwrap();
    assert_eq!(session::digest(&attached), draft.session_digest);
    assert_eq!(attached.len(), draft.session_lines.len());
    // Turning it off stops recording; clearing deletes the events.
    playtest.set_session(false, Tab::Verse, Route::Home);
    let before = playtest.reports(None).session.events;
    playtest.screen(Tab::Coder, Route::Chat);
    assert_eq!(playtest.reports(None).session.events, before);
    playtest.clear_session();
    assert_eq!(playtest.reports(None).session.events, 0);
}

#[test]
fn reports_and_the_session_survive_a_relaunch_and_sent_bodies_are_erased() {
    let relay = Arc::new(Fake::default());
    let key = triage_hex();
    let dir = tempfile::tempdir().unwrap();
    let digest;
    {
        let store = Cache::open(dir.path(), &world()).unwrap();
        let mut playtest = Playtest::new(Some(store), relay.clone(), Some(&key));
        playtest.set_session(true, Tab::Account, Route::Home);
        let packet = playtest.send(form(Tab::Verse, Route::Home), &world(), None, Platform::Ios);
        playtest.wait();
        digest = lock(&playtest.inner).saved[0].digest.clone();
        assert!(packet.sent.is_some());
    }
    let store = Cache::open(dir.path(), &world()).unwrap();
    assert!(
        store
            .read::<Option<Report>>(&body_key(&digest))
            .unwrap()
            .flatten()
            .is_none()
    );
    let mut playtest = Playtest::new(Some(store), relay, Some(&key));
    let packet = playtest.reports(None);
    assert!(packet.session.on);
    assert_eq!(packet.reports[0].status, Status::Sent);
}
