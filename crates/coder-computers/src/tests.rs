use crate::authority::{Action, Denial, check};
use crate::model::*;
use crate::service::{ComputersService, Result as ServiceResult, Unavailable};
use crate::synthetic;
use crate::synthetic::{
    APPROVAL_CODE, EXPIRED_INVITATION, Synthetic, device, key as synthetic_key,
};
use crate::{Computers, InputPurpose, Intent, MAX_INPUT_BYTES, Outcome, Refusal, Screen};
use coder_access::protocol::{DeviceState, OriginKind};
use coder_access::{Code, Error, Right, Rights};
use coder_link::{BlockReason, Failure, Freshness, Moment, Phase, Stage, StaleCause, Status};
use coder_reach::hints::Class;
use coder_reach::presence::{ClientProfile, Presence, VersionRange};
use rust_native::{Activation, Element, Node, TextRole, ValidatedView};
use std::sync::{Arc, Mutex};

const NOW: u64 = 1_790_000_000;
fn now() -> u64 {
    NOW
}

fn caps(platform: Platform) -> Capabilities {
    Capabilities {
        platform,
        camera: platform == Platform::Phone,
    }
}

fn open(platform: Platform) -> Computers {
    Computers::new(
        Box::new(Synthetic::fixture(platform, now)),
        caps(platform),
        "computers:test",
    )
    .unwrap()
}

fn view(computers: &Computers) -> &ValidatedView<Intent> {
    computers.view().unwrap()
}

fn walk<'a>(node: &'a Node<Intent>, out: &mut Vec<&'a Node<Intent>>) {
    out.push(node);
    if let Element::Stack { children, .. } | Element::List { children, .. } = &node.element {
        for child in children {
            walk(child, out);
        }
    }
}

fn nodes(computers: &Computers) -> Vec<&Node<Intent>> {
    let mut out = Vec::new();
    walk(&view(computers).view().root, &mut out);
    out
}

fn find<'a>(computers: &'a Computers, key: &str) -> Option<&'a Node<Intent>> {
    nodes(computers).into_iter().find(|node| node.key == key)
}

fn text_of(computers: &Computers, key: &str) -> String {
    match find(computers, key).map(|node| &node.element) {
        Some(Element::Text { value, .. }) => value.clone(),
        Some(Element::Button { label, .. }) => label.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

fn all_text(computers: &Computers) -> String {
    nodes(computers)
        .into_iter()
        .filter_map(|node| match &node.element {
            Element::Text { value, .. } => Some(value.as_str()),
            Element::Button { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn event(computers: &Computers, node: &str) -> Activation {
    let current = view(computers).view();
    Activation {
        instance: current.instance.clone(),
        revision: current.revision,
        node: node.into(),
    }
}

thread_local! {
    /// The intent kinds this test thread resolved from a current view.
    static PRESSED: std::cell::RefCell<std::collections::BTreeSet<String>> =
        std::cell::RefCell::default();
}

fn press(computers: &mut Computers, node: &str) -> Result<Outcome, Refusal> {
    let activation = event(computers, node);
    if let Ok(intent) = view(computers).activate(&activation) {
        let kind = serde_json::to_value(intent).unwrap()["kind"]
            .as_str()
            .unwrap()
            .to_owned();
        PRESSED.with(|pressed| pressed.borrow_mut().insert(kind));
    }
    computers.activate(&activation)
}

fn enabled(computers: &Computers, key: &str) -> bool {
    match find(computers, key).map(|node| &node.element) {
        Some(Element::Button { enabled, .. }) => *enabled,
        other => panic!("{key} is not a button: {other:?}"),
    }
}

/// A service that returns a snapshot the test controls and records calls.
#[derive(Clone)]
struct Fixed(Arc<Mutex<FixedState>>);
struct FixedState {
    snapshot: Snapshot,
    calls: Vec<String>,
    fail: Option<Error>,
}
impl Fixed {
    fn new(snapshot: Snapshot) -> Self {
        Self(Arc::new(Mutex::new(FixedState {
            snapshot,
            calls: Vec::new(),
            fail: None,
        })))
    }
    fn calls(&self) -> Vec<String> {
        self.0.lock().unwrap().calls.clone()
    }
    fn effect(&mut self, call: String) -> ServiceResult<()> {
        let mut state = self.0.lock().unwrap();
        state.calls.push(call);
        match state.fail.clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
impl ComputersService for Fixed {
    fn snapshot(&mut self) -> ServiceResult<Snapshot> {
        Ok(self.0.lock().unwrap().snapshot.clone())
    }
    fn set_enabled(&mut self, host: &str, enabled: bool) -> ServiceResult<()> {
        self.effect(format!("set_enabled {host} {enabled}"))
    }
    fn retry_now(&mut self, host: &str) -> ServiceResult<()> {
        self.effect(format!("retry_now {host}"))
    }
    fn forget(&mut self, host: &str) -> ServiceResult<()> {
        self.effect(format!("forget {host}"))
    }
    fn redeem_invitation(&mut self, _: &str) -> ServiceResult<String> {
        self.effect("redeem".into()).map(|()| "aa".repeat(32))
    }
    fn approve_enrollment(
        &mut self,
        host: &str,
        _: &str,
        code: &str,
        rights: &Rights,
        _: u64,
    ) -> ServiceResult<()> {
        self.effect(format!("approve {host} {code} {}", rights.to_list()))
    }
    fn deny_enrollment(&mut self, host: &str, _: &str) -> ServiceResult<()> {
        self.effect(format!("deny {host}"))
    }
    fn connect_ssh(&mut self, destination: &str) -> ServiceResult<()> {
        self.effect(format!("ssh {destination}"))
    }
    fn run_without_local_host(&mut self) -> ServiceResult<()> {
        self.effect("client_only".into())
    }
    fn refresh_devices(&mut self, host: &str) -> ServiceResult<()> {
        self.effect(format!("refresh_devices {host}"))
    }
    fn create_invitation(
        &mut self,
        host: &str,
        rights: &Rights,
        expiry: u64,
    ) -> ServiceResult<CreatedInvitation> {
        self.effect(format!("invite {host} {} {expiry}", rights.to_list()))?;
        Ok(CreatedInvitation {
            invitation: "i".into(),
            code: "coder-host:x".into(),
            rights: rights.clone(),
            expires_at: NOW + 600,
        })
    }
    fn cancel_invitation(&mut self, host: &str, _: &str) -> ServiceResult<()> {
        self.effect(format!("cancel {host}"))
    }
    fn revoke(&mut self, host: &str, device: &str) -> ServiceResult<()> {
        self.effect(format!("revoke {host} {device}"))
    }
    fn complete_first_run(&mut self) -> ServiceResult<()> {
        self.effect("first_run".into())
    }
    fn import_owner_key(&mut self, secret: &str) -> ServiceResult<()> {
        self.effect(format!("owner_key {secret}"))
    }
    fn list_in_directory(&mut self, host: &str, label: &str) -> ServiceResult<()> {
        self.effect(format!("list {host} {label}"))
    }
    fn answer_ssh_prompt(&mut self, id: u64, answer: Option<&str>) -> ServiceResult<()> {
        self.effect(format!("ssh_answer {id} {}", answer.is_some()))
    }
    fn edit_listing(
        &mut self,
        host: &str,
        revision: u64,
        change: &ListingChange,
    ) -> ServiceResult<()> {
        self.effect(format!("edit {host} {revision} {change:?}"))
    }
    fn remove_from_directory(&mut self, host: &str, revision: u64) -> ServiceResult<()> {
        self.effect(format!("delist {host} {revision}"))
    }
    fn keep_directory(&mut self, revision: u64) -> ServiceResult<()> {
        self.effect(format!("keep {revision}"))
    }
    fn remove_ssh(&mut self, host: &str) -> ServiceResult<()> {
        self.effect(format!("ssh_remove {host}"))
    }
}

/// Directory editing and SSH removal: every new intent and state.
mod edit;

fn link(phase: Phase) -> Status {
    Status {
        phase,
        freshness: Freshness::Unknown,
        enabled: true,
        wanted: true,
        network_available: true,
        step: 0,
        last_failure: None,
        connection: None,
    }
}

fn host(enrollment: Enrollment, link: Option<Status>) -> HostRecord {
    HostRecord {
        key: "ab".repeat(32),
        label: "Desk".into(),
        enrollment,
        link,
        route: Some(Class::Tailnet),
        compatibility: Compatibility::Compatible,
        listing: None,
        delisted: false,
        ssh: None,
        tunnel: None,
        presence: None,
        devices: DeviceList::NotLoaded,
        enrollments: Vec::new(),
        workspaces: None,
        watchers: None,
        background: None,
    }
}

fn enrolled(rights: Rights) -> Enrollment {
    Enrollment::Enrolled {
        grant: "g".into(),
        rights,
        epoch: 1,
        expires_at: NOW + 1_000,
    }
}

#[test]
fn every_status_is_derived_and_worded() {
    let connected = |freshness| Status {
        freshness,
        ..link(Phase::Connected)
    };
    let off = Status {
        enabled: false,
        ..link(Phase::Available)
    };
    let backoff = Status {
        last_failure: Some(Failure::Timeout),
        ..link(Phase::Backoff { until: Moment(5) })
    };
    let all = Rights::all();
    let cases: Vec<(HostRecord, HostStatus, &str)> = vec![
        (
            host(
                enrolled(all.clone()),
                Some(connected(Freshness::Current { as_of: Moment(1) })),
            ),
            HostStatus::Online {
                data: DataState::Current,
            },
            "Online over your tailnet. Up to date.",
        ),
        (
            host(
                enrolled(all.clone()),
                Some(connected(Freshness::Stale {
                    as_of: None,
                    cause: StaleCause::Syncing,
                })),
            ),
            HostStatus::Online {
                data: DataState::CatchingUp,
            },
            "Catching up",
        ),
        (
            host(
                enrolled(all.clone()),
                Some(connected(Freshness::Stale {
                    as_of: Some(Moment(1)),
                    cause: StaleCause::SubscriptionFailed,
                })),
            ),
            HostStatus::Online {
                data: DataState::UpdatesFailed,
            },
            "Updates failed",
        ),
        (
            host(
                enrolled(all.clone()),
                Some(link(Phase::Connecting(Stage::Establishing))),
            ),
            HostStatus::Connecting {
                stage: Stage::Establishing,
            },
            "Connecting.",
        ),
        (
            host(
                enrolled(all.clone()),
                Some(link(Phase::Connecting(Stage::Probing))),
            ),
            HostStatus::Connecting {
                stage: Stage::Probing,
            },
            "checking the connection",
        ),
        (
            host(
                enrolled(all.clone()),
                Some(link(Phase::Connecting(Stage::Replacing))),
            ),
            HostStatus::Connecting {
                stage: Stage::Replacing,
            },
            "replacing the connection",
        ),
        (
            host(enrolled(all.clone()), Some(off)),
            HostStatus::Offline {
                cause: OfflineCause::SwitchedOff,
            },
            "switched off",
        ),
        (
            host(enrolled(all.clone()), Some(link(Phase::Available))),
            HostStatus::Offline {
                cause: OfflineCause::NotConnected,
            },
            "not connected",
        ),
        (
            host(enrolled(all.clone()), None),
            HostStatus::Offline {
                cause: OfflineCause::NotConnected,
            },
            "not connected",
        ),
        (
            host(enrolled(all.clone()), Some(link(Phase::Offline))),
            HostStatus::Offline {
                cause: OfflineCause::NoNetwork,
            },
            "no network",
        ),
        (
            host(enrolled(all.clone()), Some(backoff)),
            HostStatus::Offline {
                cause: OfflineCause::Retrying {
                    failure: Some(Failure::Timeout),
                },
            },
            "timed out. Retrying automatically.",
        ),
        (
            host(
                enrolled(all.clone()),
                Some(link(Phase::Blocked(BlockReason::Authentication))),
            ),
            HostStatus::Offline {
                cause: OfflineCause::Refused,
            },
            "Blocked: the computer didn't accept",
        ),
        (
            host(
                enrolled(all.clone()),
                Some(link(Phase::Blocked(BlockReason::Configuration))),
            ),
            HostStatus::Offline {
                cause: OfflineCause::Misconfigured,
            },
            "Blocked: this device's settings",
        ),
        (
            HostRecord {
                compatibility: Compatibility::HostOutOfDate,
                ..host(enrolled(all.clone()), Some(link(Phase::Connected)))
            },
            HostStatus::OutOfDate {
                side: OutOfDate::Host,
            },
            "Out of date: the computer runs an older Coder",
        ),
        (
            HostRecord {
                compatibility: Compatibility::ClientOutOfDate,
                ..host(enrolled(all.clone()), None)
            },
            HostStatus::OutOfDate {
                side: OutOfDate::ThisApp,
            },
            "Update this app",
        ),
        (
            host(
                enrolled(all.clone()),
                Some(link(Phase::Blocked(BlockReason::Incompatible))),
            ),
            HostStatus::OutOfDate {
                side: OutOfDate::Unknown,
            },
            "share no protocol version",
        ),
        (
            host(Enrollment::NotEnrolled, None),
            HostStatus::NotEnrolled {
                cause: NotEnrolledCause::NoAccess,
            },
            "Not enrolled: this device has no access",
        ),
        (
            host(Enrollment::Expired { at: NOW - 1 }, None),
            HostStatus::NotEnrolled {
                cause: NotEnrolledCause::Expired,
            },
            "access expired",
        ),
        (
            host(
                Enrollment::Enrolled {
                    grant: "g".into(),
                    rights: all.clone(),
                    epoch: 1,
                    expires_at: NOW,
                },
                Some(link(Phase::Connected)),
            ),
            HostStatus::NotEnrolled {
                cause: NotEnrolledCause::Expired,
            },
            "access expired",
        ),
        (
            host(Enrollment::Revoked, Some(link(Phase::Connected))),
            HostStatus::Revoked,
            "Revoked: this computer removed",
        ),
        (
            host(
                enrolled(all),
                Some(link(Phase::Blocked(BlockReason::Revoked))),
            ),
            HostStatus::Revoked,
            "Revoked",
        ),
    ];
    for (record, expected, words) in cases {
        assert_eq!(HostStatus::derive(&record, NOW), expected, "{words}");
        let line = crate::project::status_line(&record, NOW);
        assert!(line.contains(words), "{line} lacks {words}");
    }
}

#[test]
fn compatibility_names_the_side_that_is_behind() {
    let presence = |protocol, min, max| Presence {
        v: "openagents.host-presence.v1".into(),
        requires: vec![],
        host: "ab".repeat(32),
        owner: "cd".repeat(32),
        generation: 1,
        protocol,
        compatibility: VersionRange { min, max },
        capabilities: vec![],
        observed_at: NOW,
        telemetry: None,
        meta: None,
    };
    let client = |protocol, min, max| ClientProfile {
        protocol,
        accepts: VersionRange { min, max },
    };
    assert_eq!(
        Compatibility::judge(&presence(1, 1, 1), &client(1, 1, 1)),
        Compatibility::Compatible
    );
    assert_eq!(
        Compatibility::judge(&presence(1, 1, 1), &client(2, 2, 2)),
        Compatibility::HostOutOfDate
    );
    assert_eq!(
        Compatibility::judge(&presence(2, 2, 2), &client(1, 1, 1)),
        Compatibility::ClientOutOfDate
    );
}

#[test]
fn the_fixture_shows_all_six_statuses_with_reasons() {
    let mut computers = open(Platform::Phone);
    assert_eq!(computers.screen(), &Screen::FirstRun);
    press(&mut computers, "first-run-continue").unwrap();
    assert_eq!(computers.screen(), &Screen::Computers);
    let text = all_text(&computers);
    for words in [
        "Studio Mac",
        "Online over the local network. Up to date.",
        "Connecting.",
        "Offline: the computer didn't answer. Retrying automatically.",
        "Out of date: the computer runs an older Coder",
        "Not enrolled: this device has no access",
        "Revoked: this computer removed this device's access",
        "Offline: switched off. It stays in your list.",
    ] {
        assert!(text.contains(words), "missing {words}\n{text}");
    }
}

/// Every disabled control is followed by a nonempty reason.
fn assert_reasons(computers: &Computers) {
    let root = &view(computers).view().root;
    let mut stacks = Vec::new();
    walk(root, &mut stacks);
    for node in stacks {
        let Element::Stack { children, .. } = &node.element else {
            continue;
        };
        for (index, child) in children.iter().enumerate() {
            if let Element::Button { enabled: false, .. } = child.element {
                let next = children
                    .get(index + 1)
                    .unwrap_or_else(|| panic!("{} has no reason", child.key));
                assert_eq!(next.key, format!("{}-reason", child.key));
                match &next.element {
                    Element::Text {
                        value,
                        role: TextRole::Status,
                    } => {
                        assert!(
                            value.starts_with("Unavailable: ") && value.len() > 16,
                            "{value}"
                        )
                    }
                    other => panic!("{other:?}"),
                }
            }
        }
    }
}

fn screens(computers: &Computers) -> Vec<Screen> {
    let mut screens = vec![Screen::Computers, Screen::Add, Screen::Activity];
    screens.extend(
        computers
            .snapshot()
            .hosts
            .iter()
            .map(|host| Screen::Access {
                host: host.key.clone(),
            }),
    );
    // Every enrolled host has its own screen; a host only the directory
    // lists has no connection to open.
    screens.extend(
        computers
            .snapshot()
            .hosts
            .iter()
            .filter(|host| !host.directory_only())
            .map(|host| Screen::Host {
                host: host.key.clone(),
            }),
    );
    screens
}

fn show(computers: &mut Computers, screen: Screen) {
    // Navigation goes through a real control when one exists; the Access
    // screen of each host is reached from its row.
    let key = match &screen {
        Screen::Computers => "tab-computers".to_owned(),
        Screen::Add => "tab-add".to_owned(),
        Screen::Activity => "tab-activity".to_owned(),
        Screen::Access { host } => {
            if computers.screen() != &Screen::Computers {
                press(computers, "tab-computers").unwrap();
            }
            let index = computers
                .snapshot()
                .hosts
                .iter()
                .position(|record| &record.key == host)
                .unwrap();
            format!("host-{index}-access")
        }
        Screen::Host { host } | Screen::Order { host } => {
            if computers.screen() != &Screen::Computers {
                press(computers, "tab-computers").unwrap();
            }
            let index = computers
                .snapshot()
                .hosts
                .iter()
                .position(|record| &record.key == host)
                .unwrap();
            if matches!(screen, Screen::Host { .. }) {
                format!("host-{index}-open")
            } else {
                press(computers, &format!("host-{index}-open")).unwrap();
                "host-order".to_owned()
            }
        }
        Screen::FirstRun => unreachable!(),
    };
    press(computers, &key).unwrap();
    assert_eq!(computers.screen(), &screen);
}

#[test]
fn disabled_controls_carry_reasons_on_every_screen_and_platform() {
    for platform in [Platform::Phone, Platform::Desktop, Platform::Terminal] {
        let mut computers = open(platform);
        assert_reasons(&computers);
        press(&mut computers, "first-run-continue").unwrap();
        for screen in screens(&computers) {
            show(&mut computers, screen);
            assert_reasons(&computers);
        }
    }
    let mut unavailable = Computers::new(
        Box::new(Unavailable::new(device(), LocalHost::NotSupported, now)),
        caps(Platform::Phone),
        "computers:none",
    )
    .unwrap();
    assert_reasons(&unavailable);
    // A phone never offers SSH.
    assert!(find(&unavailable, "ssh-connect").is_none());
    for key in [
        "invite-scan",
        "invite-paste",
        "local-client-only",
        "first-run-continue",
    ] {
        assert!(!enabled(&unavailable, key), "{key}");
        assert!(
            text_of(&unavailable, &format!("{key}-reason")).contains("can't reach computers yet")
        );
    }
    assert_eq!(
        press(&mut unavailable, "invite-paste"),
        Err(Refusal::Disabled)
    );
}

#[test]
fn stale_disabled_and_inert_activations_change_nothing() {
    let fixed = Fixed::new(
        Synthetic::fixture(Platform::Desktop, now)
            .snapshot()
            .unwrap(),
    );
    let mut computers =
        Computers::new(Box::new(fixed.clone()), caps(Platform::Desktop), "c:1").unwrap();
    press(&mut computers, "first-run-continue").unwrap();
    let old = event(&computers, "host-0-switch");
    // Any newer revision makes the captured activation stale.
    press(&mut computers, "computers-refresh").unwrap();
    let before = fixed.calls();
    assert_eq!(computers.activate(&old), Err(Refusal::Stale));
    assert_eq!(fixed.calls(), before);
    assert_eq!(
        computers.notice().unwrap().text,
        "The screen changed. Check it and try again."
    );
    let mut foreign = event(&computers, "host-0-switch");
    foreign.instance = "c:other".into();
    assert_eq!(computers.activate(&foreign), Err(Refusal::Stale));
    assert_eq!(
        press(&mut computers, "host-0-label"),
        Err(Refusal::NotInteractive)
    );
    assert_eq!(
        press(&mut computers, "no-such-node"),
        Err(Refusal::NotInteractive)
    );
    // Lab box is not enrolled: its Forget works, its Access screen denies.
    press(&mut computers, "host-4-access").unwrap();
    assert!(!enabled(&computers, "devices-refresh"));
    assert_eq!(
        press(&mut computers, "devices-refresh"),
        Err(Refusal::Disabled)
    );
    assert_eq!(fixed.calls(), before);
}

#[test]
fn a_host_refusal_is_shown_and_grants_nothing() {
    let fixed = Fixed::new(
        Synthetic::fixture(Platform::Desktop, now)
            .snapshot()
            .unwrap(),
    );
    let mut computers =
        Computers::new(Box::new(fixed.clone()), caps(Platform::Desktop), "c:2").unwrap();
    press(&mut computers, "first-run-continue").unwrap();
    press(&mut computers, "host-0-access").unwrap();
    fixed.0.lock().unwrap().fail = Some(Error::missing(Right::AccessAdmin));
    let refused = press(&mut computers, "share-create").unwrap_err();
    assert!(matches!(refused, Refusal::Failed(ref error) if error.code == Code::MissingRight));
    assert!(
        computers
            .notice()
            .unwrap()
            .text
            .contains("\"Manage access\"")
    );
    assert!(find(&computers, "share-code").is_none());
}

#[test]
fn every_intent_runs_through_its_check() {
    let mut computers = open(Platform::Desktop);
    // First run: continue.
    assert_eq!(
        press(&mut computers, "first-run-continue"),
        Ok(Outcome::ContinueOnboarding)
    );
    // Show, refresh.
    press(&mut computers, "tab-activity").unwrap();
    assert_eq!(text_of(&computers, "activity-title"), "Activity");
    press(&mut computers, "activity-refresh").unwrap();
    press(&mut computers, "tab-computers").unwrap();
    // Switch off, then on, without forgetting.
    press(&mut computers, "host-1-switch").unwrap();
    assert!(text_of(&computers, "host-1-status").starts_with("Offline: switched off"));
    assert_eq!(text_of(&computers, "host-1-switch"), "Switch on");
    press(&mut computers, "host-1-switch").unwrap();
    assert_eq!(text_of(&computers, "host-1-status"), "Connecting.");
    assert_eq!(computers.snapshot().hosts.len(), 7);
    // Try now on the retrying host.
    assert!(text_of(&computers, "host-2-status").contains("Retrying"));
    press(&mut computers, "host-2-retry").unwrap();
    assert_eq!(text_of(&computers, "host-2-status"), "Connecting.");
    // Forget asks first; Keep cancels; confirm forgets.
    press(&mut computers, "host-6-forget").unwrap();
    press(&mut computers, "host-6-forget-no").unwrap();
    assert!(find(&computers, "host-6-forget-confirm").is_none());
    press(&mut computers, "host-6-forget").unwrap();
    assert!(text_of(&computers, "host-6-forget-confirm").contains("keeps this device's access"));
    press(&mut computers, "host-6-forget-yes").unwrap();
    assert_eq!(computers.snapshot().hosts.len(), 6);

    // Add: paste, scan, codes, SSH, and no local host.
    press(&mut computers, "tab-add").unwrap();
    assert_eq!(
        press(&mut computers, "invite-paste"),
        Ok(Outcome::InputRequested)
    );
    let input = computers.input().unwrap().clone();
    assert_eq!(
        (input.purpose, input.scan),
        (InputPurpose::Invitation, false)
    );
    assert_eq!(
        computers.submit("wrong-token", "coder-host:x"),
        Err(Refusal::Stale)
    );
    assert!(matches!(
        computers.submit(&input.token, "coder-pair:abc"),
        Err(Refusal::Input(_))
    ));
    assert!(matches!(
        computers.submit(&input.token, "hello"),
        Err(Refusal::Input(_))
    ));
    assert!(matches!(
        computers.submit(&input.token, EXPIRED_INVITATION),
        Err(Refusal::Failed(ref e)) if e.code == Code::Expired
    ));
    assert!(computers.notice().unwrap().text.starts_with("It expired."));
    computers
        .submit(&input.token, "  coder-host:new  ")
        .unwrap();
    assert!(computers.input().is_none());
    assert_eq!(computers.screen(), &Screen::Computers);
    assert_eq!(computers.notice().unwrap().text, "Added New computer.");
    // A used token is stale.
    assert_eq!(
        computers.submit(&input.token, "coder-host:again"),
        Err(Refusal::Stale)
    );

    press(&mut computers, "tab-add").unwrap();
    // The desktop fixture has no camera.
    assert!(!enabled(&computers, "invite-scan"));
    press(&mut computers, "invite-paste").unwrap();
    let token = computers.input().unwrap().token.clone();
    press(&mut computers, "input-cancel").unwrap();
    assert!(computers.input().is_none());
    assert_eq!(computers.cancel_input(&token), Err(Refusal::Stale));

    press(&mut computers, "approve-4-0-code").unwrap();
    let code = computers.input().unwrap().clone();
    assert_eq!(code.purpose, InputPurpose::ApprovalCode);
    assert!(matches!(
        computers.submit(&code.token, "12"),
        Err(Refusal::Input(_))
    ));
    assert!(matches!(
        computers.submit(&code.token, "AAAA-BBBB"),
        Err(Refusal::Failed(ref e)) if e.code == Code::WrongCode
    ));
    // The request stays open after a wrong code; lowercase and O/I/L normalize.
    computers
        .submit(&code.token, &APPROVAL_CODE.to_lowercase())
        .unwrap();
    assert!(
        computers.snapshot().hosts[4]
            .enrollment
            .rights(NOW)
            .is_some()
    );

    press(&mut computers, "ssh-connect").unwrap();
    let ssh = computers.input().unwrap().clone();
    assert_eq!(ssh.purpose, InputPurpose::SshDestination);
    for bad in ["-oProxyCommand=x", "a b", ""] {
        assert!(
            matches!(computers.submit(&ssh.token, bad), Err(Refusal::Input(_))),
            "{bad}"
        );
    }
    computers.submit(&ssh.token, "me@box").unwrap();
    assert!(
        computers
            .snapshot()
            .hosts
            .iter()
            .any(|host| host.label == "me@box")
    );

    press(&mut computers, "local-client-only").unwrap();
    assert_eq!(computers.snapshot().local_host, LocalHost::ClientOnly);
    assert!(!enabled(&computers, "local-client-only"));

    // Access: devices, rights, invitations, and revocation.
    show(
        &mut computers,
        Screen::Access {
            host: synthetic_key(0xa1),
        },
    );
    press(&mut computers, "devices-refresh").unwrap();
    assert!(text_of(&computers, "devices-as-of").contains("Checked just now"));
    assert!(text_of(&computers, "device-0-label").starts_with("This device. Active"));
    assert!(text_of(&computers, "device-1-label").contains("last seen 2 h ago"));
    assert!(text_of(&computers, "device-2-label").contains("last seen: unknown"));
    assert!(!enabled(&computers, "device-0-revoke"));
    assert!(text_of(&computers, "device-0-revoke-reason").contains("device you're using"));
    assert_eq!(
        text_of(&computers, "share-right-access_admin"),
        "Manage access: not included"
    );
    press(&mut computers, "share-right-access_admin").unwrap();
    press(&mut computers, "share-right-terminal").unwrap();
    assert_eq!(
        text_of(&computers, "share-right-access_admin"),
        "Manage access: included"
    );
    press(&mut computers, "share-create").unwrap();
    assert!(text_of(&computers, "share-code").starts_with("coder-host:"));
    assert!(text_of(&computers, "share-code-detail").contains("Manage access"));
    assert!(!text_of(&computers, "share-code-detail").contains("Open terminals"));
    press(&mut computers, "share-cancel").unwrap();
    assert!(find(&computers, "share-code").is_none());
    press(&mut computers, "share-create").unwrap();
    press(&mut computers, "share-done").unwrap();
    assert!(find(&computers, "share-code").is_none());
    press(&mut computers, "device-1-revoke").unwrap();
    press(&mut computers, "device-1-revoke-no").unwrap();
    press(&mut computers, "device-1-revoke").unwrap();
    press(&mut computers, "device-1-revoke-yes").unwrap();
    assert!(text_of(&computers, "device-1-label").contains("Revoked"));
    assert!(find(&computers, "device-1-revoke").is_none());

    // Deny on a fresh fixture.
    let mut other = open(Platform::Terminal);
    press(&mut other, "approve-4-0-deny").unwrap();
    assert!(other.snapshot().hosts[4].enrollments.is_empty());
    assert!(text_of(&other, "approve-empty").contains("No computers are waiting"));

    let mut phone = open(Platform::Phone);
    assert_eq!(
        press(&mut phone, "invite-scan"),
        Ok(Outcome::InputRequested)
    );
    assert!(phone.input().unwrap().scan);

    // The owner key, then an owner action on the directory.
    let fixed = Fixed::new(
        Synthetic::fixture(Platform::Desktop, now)
            .snapshot()
            .unwrap(),
    );
    let mut owner =
        Computers::new(Box::new(fixed.clone()), caps(Platform::Desktop), "c:owner").unwrap();
    press(&mut owner, "first-run-continue").unwrap();
    press(&mut owner, "directory-owner-key").unwrap();
    let asked = owner.input().unwrap().clone();
    assert_eq!(asked.purpose, InputPurpose::OwnerKey);
    assert!(asked.secret);
    owner.submit(&asked.token, "  ab  ").unwrap();
    assert_eq!(fixed.calls().last().unwrap(), "owner_key ab");
    fixed.0.lock().unwrap().snapshot.directory = DirectoryState::Current {
        revision: Some(4),
        as_of: NOW,
    };
    press(&mut owner, "computers-refresh").unwrap();
    assert!(find(&owner, "directory-owner-key").is_none());
    press(&mut owner, "host-0-list").unwrap();
    let asked = owner.input().unwrap().clone();
    assert_eq!(asked.purpose, InputPurpose::DirectoryLabel);
    assert!(!asked.secret);
    assert!(matches!(
        owner.submit(&asked.token, &"x".repeat(65)),
        Err(Refusal::Input(_))
    ));
    owner.submit(&asked.token, "Studio").unwrap();
    let key = synthetic_key(0xa1);
    assert_eq!(fixed.calls().last().unwrap(), &format!("list {key} Studio"));

    // Every intent variant was resolved from a real control above.
    let pressed = PRESSED.with(|pressed| pressed.borrow().clone());
    for kind in [
        "show",
        "refresh",
        "set_enabled",
        "retry_now",
        "forget",
        "confirm_forget",
        "scan_invitation",
        "paste_invitation",
        "enter_code",
        "deny",
        "connect_ssh",
        "run_without_host",
        "refresh_devices",
        "toggle_right",
        "create_invitation",
        "cancel_invitation",
        "dismiss_invitation",
        "revoke",
        "confirm_revoke",
        "cancel",
        "continue_onboarding",
        "import_owner_key",
        "list_in_directory",
    ] {
        assert!(pressed.contains(kind), "{kind} was never pressed");
    }
    assert_eq!(pressed.len(), 23);
}

#[test]
fn every_control_refuses_a_stale_revision() {
    for platform in [Platform::Phone, Platform::Desktop] {
        let fixed = Fixed::new(Synthetic::fixture(platform, now).snapshot().unwrap());
        let mut computers =
            Computers::new(Box::new(fixed.clone()), caps(platform), "c:stale").unwrap();
        let mut screens_seen = 0;
        loop {
            let buttons: Vec<String> = nodes(&computers)
                .into_iter()
                .filter(|node| matches!(node.element, Element::Button { .. }))
                .map(|node| node.key.clone())
                .collect();
            assert!(!buttons.is_empty());
            for key in &buttons {
                for offset in [-1_i64, 1] {
                    // Each refusal draws a new revision, so read it every time.
                    let current = view(&computers).view();
                    let stale = Activation {
                        instance: current.instance.clone(),
                        revision: current.revision.checked_add_signed(offset).unwrap(),
                        node: key.clone(),
                    };
                    assert_eq!(computers.activate(&stale), Err(Refusal::Stale), "{key}");
                }
            }
            assert!(
                fixed.calls().is_empty(),
                "a stale activation reached the service"
            );
            screens_seen += 1;
            match screens_seen {
                1 => press(&mut computers, "first-run-continue")
                    .map(|_| ())
                    .unwrap_or(()),
                2 => press(&mut computers, "tab-add").map(|_| ()).unwrap(),
                3 => press(&mut computers, "tab-activity").map(|_| ()).unwrap(),
                4 => {
                    press(&mut computers, "tab-computers").unwrap();
                    press(&mut computers, "host-0-access").map(|_| ()).unwrap()
                }
                _ => break,
            }
            fixed.0.lock().unwrap().calls.clear();
        }
        assert_eq!(screens_seen, 5);
    }
}

#[test]
fn invitation_rights_only_narrow() {
    let mut snapshot = Synthetic::fixture(Platform::Desktop, now)
        .snapshot()
        .unwrap();
    snapshot.first_run_complete = true;
    snapshot.owner = false;
    // An administrator who lacks `terminal` cannot share it.
    let rights = Rights::new([Right::Observe, Right::AccessRead, Right::AccessAdmin]).unwrap();
    snapshot.hosts[0].enrollment = enrolled(rights.clone());
    let caps = caps(Platform::Desktop);
    let key = snapshot.hosts[0].key.clone();
    assert_eq!(
        check(
            &snapshot,
            caps,
            Action::IncludeRight {
                host: &key,
                right: Right::Terminal
            }
        ),
        Err(Denial::RightNotHeld(Right::Terminal))
    );
    let wider = Rights::new([Right::Observe, Right::Terminal]).unwrap();
    assert_eq!(
        check(
            &snapshot,
            caps,
            Action::Invite {
                host: &key,
                rights: Some(&wider)
            }
        ),
        Err(Denial::RightNotHeld(Right::Terminal))
    );
    assert_eq!(
        check(
            &snapshot,
            caps,
            Action::Invite {
                host: &key,
                rights: None
            }
        ),
        Err(Denial::NoRightsChosen)
    );
    let fixed = Fixed::new(snapshot);
    let mut computers = Computers::new(Box::new(fixed.clone()), caps, "c:3").unwrap();
    press(&mut computers, "host-0-access").unwrap();
    // The default draft is the standard rights this device holds.
    assert_eq!(
        text_of(&computers, "share-right-observe"),
        "View sessions and tasks: included"
    );
    assert!(!enabled(&computers, "share-right-terminal"));
    press(&mut computers, "share-right-observe").unwrap();
    assert!(!enabled(&computers, "share-create"));
    assert!(text_of(&computers, "share-create-reason").contains("Choose at least one right"));
    press(&mut computers, "share-right-observe").unwrap();
    press(&mut computers, "share-create").unwrap();
    // The grant expiry never outlives this device's own grant.
    assert_eq!(
        fixed.calls().last().unwrap(),
        &format!("invite {key} observe {}", NOW + 1_000)
    );
}

#[test]
fn approval_is_limited_to_approvers_and_narrows_rights() {
    let mut snapshot = Synthetic::fixture(Platform::Desktop, now)
        .snapshot()
        .unwrap();
    snapshot.first_run_complete = true;
    snapshot.owner = false;
    let key = snapshot.hosts[4].key.clone();
    let caps = caps(Platform::Desktop);
    let action = Action::Approve {
        host: &key,
        enrollment: "synthetic-enrollment",
    };
    assert_eq!(check(&snapshot, caps, action), Err(Denial::NotApprover));
    snapshot.hosts[4].enrollment =
        enrolled(Rights::new([Right::Observe, Right::AccessAdmin]).unwrap());
    assert_eq!(check(&snapshot, caps, action), Ok(()));
    assert_eq!(
        check(
            &snapshot,
            caps,
            Action::Approve {
                host: &key,
                enrollment: "gone"
            }
        ),
        Err(Denial::UnknownRequest)
    );
    let fixed = Fixed::new(snapshot);
    let mut computers = Computers::new(Box::new(fixed.clone()), caps, "c:4").unwrap();
    press(&mut computers, "tab-add").unwrap();
    press(&mut computers, "approve-4-0-code").unwrap();
    let token = computers.input().unwrap().token.clone();
    computers.submit(&token, "7k4m 9qxz").unwrap();
    // The request asked for standard rights; this admin holds only observe.
    assert_eq!(
        fixed.calls().last().unwrap(),
        &format!("approve {key} 7K4M9QXZ observe")
    );
}

#[test]
fn ssh_and_local_host_follow_the_platform() {
    let phone = open(Platform::Phone);
    // A phone can't run ssh, so it doesn't offer SSH at all.
    assert!(find(&phone, "ssh").is_none());
    assert!(find(&phone, "ssh-connect").is_none());
    let snapshot = phone.snapshot().clone();
    assert_eq!(
        check(&snapshot, caps(Platform::Phone), Action::ConnectSsh),
        Err(Denial::SshNeedsComputer)
    );
    assert!(!enabled(&phone, "local-client-only"));
    assert!(text_of(&phone, "local-client-only-reason").contains("never run a host"));
    assert!(enabled(&phone, "invite-scan"));
    let terminal = open(Platform::Terminal);
    assert!(enabled(&terminal, "ssh-connect"));
    let mut snapshot = Synthetic::fixture(Platform::Desktop, now)
        .snapshot()
        .unwrap();
    snapshot.local_host = LocalHost::Running {
        host: "ee".repeat(32),
    };
    assert_eq!(
        check(&snapshot, caps(Platform::Desktop), Action::RunWithoutHost),
        Err(Denial::LocalHostRunning)
    );
}

#[test]
fn first_run_needs_one_enrolled_computer() {
    let mut computers = Computers::new(
        Box::new(Synthetic::empty(Platform::Phone, now)),
        caps(Platform::Phone),
        "c:5",
    )
    .unwrap();
    assert_eq!(computers.screen(), &Screen::FirstRun);
    assert!(find(&computers, "tabs").is_none());
    assert_eq!(
        text_of(&computers, "first-run-count"),
        "No computers added yet."
    );
    assert!(!enabled(&computers, "first-run-continue"));
    assert!(text_of(&computers, "first-run-continue-reason").contains("Add at least one computer"));
    press(&mut computers, "invite-scan").unwrap();
    assert!(computers.input().unwrap().scan);
    let token = computers.input().unwrap().token.clone();
    computers.submit(&token, "coder-host:first").unwrap();
    // First run stays until the person continues.
    assert_eq!(computers.screen(), &Screen::FirstRun);
    assert_eq!(text_of(&computers, "first-run-count"), "1 computer added.");
    assert_eq!(
        press(&mut computers, "first-run-continue"),
        Ok(Outcome::ContinueOnboarding)
    );
    assert!(computers.snapshot().first_run_complete);
}

#[test]
fn activity_keeps_the_newest_summary_and_shows_attention_first() {
    let mut computers = open(Platform::Phone);
    press(&mut computers, "first-run-continue").unwrap();
    press(&mut computers, "tab-activity").unwrap();
    assert_eq!(
        text_of(&computers, "activity-count"),
        "3 items need your attention."
    );
    // The superseded running summary for the same task is gone.
    assert_eq!(
        text_of(&computers, "activity-0-subject"),
        "Build server. Task, waiting."
    );
    assert_eq!(text_of(&computers, "activity-0-headline"), "Deploy preview");
    assert!(
        text_of(&computers, "activity-0-time")
            .starts_with("Needs your approval. Updated 2 min ago.")
    );
    // Build server is connecting, so its summary is marked as last known.
    assert!(text_of(&computers, "activity-0-stale").contains("isn't online now"));
    assert_eq!(text_of(&computers, "activity-1-headline"), "Session failed");
    assert!(find(&computers, "activity-1-stale").is_none());
    let all = all_text(&computers);
    assert!(!all.contains("private detail"));
    assert!(!all.contains("/Users/me"));
    assert!(all.contains("Back up [redacted]"));
    assert!(find(&computers, "activity-4-subject").is_none());
}

#[test]
fn created_invitations_stay_out_of_debug_output() {
    let created = CreatedInvitation {
        invitation: "i".into(),
        code: "coder-host:secret".into(),
        rights: Rights::standard(),
        expires_at: 1,
    };
    assert!(!format!("{created:?}").contains("secret"));
}

#[test]
fn views_round_trip_and_revoked_devices_cannot_be_revoked_again() {
    let mut computers = open(Platform::Desktop);
    press(&mut computers, "first-run-continue").unwrap();
    let bytes = view(&computers).to_json().unwrap();
    let decoded = rust_native::View::<Intent>::from_json(&bytes).unwrap();
    assert_eq!(decoded.view(), view(&computers).view());
    let snapshot = computers.snapshot().clone();
    let key = &snapshot.hosts[0].key;
    let DeviceList::Loaded { devices, .. } = &snapshot.hosts[0].devices else {
        panic!()
    };
    assert_eq!(devices[2].state, DeviceState::Revoked);
    assert_eq!(devices[2].origin, OriginKind::Invitation);
    assert_eq!(
        check(
            &snapshot,
            caps(Platform::Desktop),
            Action::Revoke {
                host: key,
                device: &devices[2].device
            }
        ),
        Err(Denial::NotActive)
    );
    assert_eq!(
        check(
            &snapshot,
            caps(Platform::Desktop),
            Action::Revoke {
                host: key,
                device: &device()
            }
        ),
        Err(Denial::ThisDevice)
    );
}

#[test]
fn the_terminal_adapter_draws_and_activates_the_same_tree() {
    use coder_terminal::native::{Focus, render};
    use crossterm::event::{KeyCode, KeyEvent};
    let mut computers = open(Platform::Terminal);
    let ladder = coder_terminal::Ladder::new(coder_terminal::Colors::None);
    let drawn = render(view(&computers).view(), ladder, None);
    let lines: Vec<String> = drawn
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect()
        })
        .collect();
    assert!(lines.contains(&"Connect a computer".to_owned()));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("[ Connect over SSH ]"))
    );
    // The terminal has no camera: scanning shows as disabled with its reason.
    assert!(
        lines
            .iter()
            .any(|line| line.contains("( Scan invitation )"))
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("( Scan invitation )  Unavailable: This device can't scan."))
    );
    // Tab to Continue and press Enter.
    let mut focus = Focus::new(view(&computers).view(), None);
    while focus.current() != Some("first-run-continue") {
        focus.handle(KeyEvent::from(KeyCode::Tab));
    }
    let activation = focus.handle(KeyEvent::from(KeyCode::Enter)).unwrap();
    assert_eq!(
        computers.activate(&activation),
        Ok(Outcome::ContinueOnboarding)
    );
    // The same key press against the old revision is stale.
    assert_eq!(computers.activate(&activation), Err(Refusal::Stale));
    let drawn = render(view(&computers).view(), ladder, None);
    let text: Vec<String> = drawn
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect()
        })
        .collect();
    assert!(
        text.iter()
            .any(|line| line.contains("Revoked: this computer removed"))
    );
    assert!(drawn.unsupported.contains("style.background"));
}

#[test]
fn a_created_invitation_shows_as_a_locally_rendered_qr_code() {
    for platform in [Platform::Phone, Platform::Terminal] {
        let mut computers = open(platform);
        press(&mut computers, "first-run-continue").unwrap();
        assert_eq!(computers.invitation_qr(), None);
        press(&mut computers, "host-0-access").unwrap();
        press(&mut computers, "share-create").unwrap();
        let modules = computers.invitation_qr().expect("a QR code");
        assert_eq!(
            modules,
            crate::qr::modules(&text_of(&computers, "share-code")).unwrap()
        );
        match platform {
            // A phone draws the modules natively and the tree says so.
            Platform::Phone => {
                assert!(find(&computers, "share-qr").is_none());
                assert!(find(&computers, "share-qr-hint").is_some());
            }
            // A terminal draws the code in the tree.
            _ => assert_eq!(text_of(&computers, "share-qr"), crate::qr::text(&modules)),
        }
        press(&mut computers, "share-done").unwrap();
        assert_eq!(computers.invitation_qr(), None);
    }
}

#[test]
fn application_lifecycle_reaches_every_supervisor() {
    let mut service = Synthetic::fixture(Platform::Phone, now);
    service.application(false).unwrap();
    service.application(true).unwrap();
    assert_eq!(
        service.calls,
        vec![
            "application false".to_owned(),
            "application true".to_owned()
        ]
    );
    // The connected host probed its connection on return from a short
    // background, and the fixture's probe answered at once.
    let snapshot = service.snapshot().unwrap();
    let studio = snapshot.host(&crate::synthetic::key(0xa1)).unwrap();
    assert_eq!(
        studio.link.map(|status| status.phase),
        Some(Phase::Connected)
    );
    let mut computers = open(Platform::Phone);
    computers.set_active(false).unwrap();
    computers.set_active(true).unwrap();
}

fn listed(key: &str, label: &str, weight: u32, enrollment: Enrollment) -> HostRecord {
    HostRecord {
        key: key.into(),
        label: label.into(),
        listing: Some(Listing {
            weight,
            added_at: NOW - 100,
        }),
        ..host(enrollment, None)
    }
}

fn received(host: &str, cpu_utilization_pct: u8) -> coder_reach::presence::Received {
    coder_reach::presence::Received {
        presence: Presence {
            v: "openagents.host-presence.v1".into(),
            requires: vec![],
            host: host.into(),
            owner: "cd".repeat(32),
            generation: 1,
            protocol: coder_reach::PROTOCOL_VERSION,
            compatibility: VersionRange {
                min: coder_reach::PROTOCOL_VERSION,
                max: coder_reach::PROTOCOL_VERSION,
            },
            capabilities: vec![],
            observed_at: NOW,
            telemetry: Some(coder_reach::presence::Telemetry {
                cpu_count: 8,
                cpu_utilization_pct,
                memory_available_pct: 50,
            }),
            meta: None,
        },
        received_at: NOW,
    }
}

fn directory_snapshot() -> Snapshot {
    let mut snapshot = Synthetic::empty(Platform::Desktop, now).snapshot().unwrap();
    let online = Some(Status {
        freshness: Freshness::Current { as_of: Moment(1) },
        ..link(Phase::Connected)
    });
    let studio = HostRecord {
        link: online,
        presence: Some(received(&"a1".repeat(32), 50)),
        ..listed(&"a1".repeat(32), "Studio", 300, enrolled(Rights::all()))
    };
    let laptop = HostRecord {
        key: "a2".repeat(32),
        label: "Computer a2a2a2a2".into(),
        link: online,
        presence: Some(received(&"a2".repeat(32), 50)),
        ..host(enrolled(Rights::all()), None)
    };
    let build = listed(&"a3".repeat(32), "Build box", 0, Enrollment::NotEnrolled);
    snapshot.hosts = vec![studio, laptop, build];
    snapshot.first_run_complete = true;
    snapshot.directory = DirectoryState::Current {
        revision: Some(2),
        as_of: NOW - 30,
    };
    snapshot
}

#[test]
fn directory_hosts_show_labels_weights_and_unenrolled_rows() {
    let fixed = Fixed::new(directory_snapshot());
    let mut computers =
        Computers::new(Box::new(fixed.clone()), caps(Platform::Desktop), "c:dir").unwrap();
    assert_eq!(text_of(&computers, "host-0-label"), "Studio");
    assert_eq!(
        text_of(&computers, "host-0-directory"),
        "In your directory. Weight 300."
    );
    // An enrolled host the directory doesn't list keeps its own label and
    // offers the owner action.
    assert!(find(&computers, "host-1-directory").is_none());
    assert!(enabled(&computers, "host-1-list"));
    assert!(find(&computers, "host-0-list").is_none());
    // A listed host this device holds no grant for: not enrolled, with no
    // connection to switch, retry, or forget.
    assert_eq!(text_of(&computers, "host-2-label"), "Build box");
    assert!(
        text_of(&computers, "host-2-status").starts_with(
            "Not enrolled: it's in your directory, and this device has no access yet."
        )
    );
    assert_eq!(
        text_of(&computers, "host-2-directory"),
        "In your directory. Weight 0: not used for new work."
    );
    for control in ["switch", "retry", "forget", "list"] {
        assert!(find(&computers, &format!("host-2-{control}")).is_none());
    }
    let snapshot = computers.snapshot().clone();
    let build = "a3".repeat(32);
    for action in [
        Action::SetEnabled { host: &build },
        Action::RetryNow { host: &build },
        Action::Forget { host: &build },
    ] {
        assert_eq!(
            check(&snapshot, caps(Platform::Desktop), action),
            Err(Denial::NotEnrolled)
        );
    }
    assert_eq!(
        text_of(&computers, "directory-status"),
        "Your directory, revision 2, read just now."
    );

    // A later revision relabels the host.
    fixed.0.lock().unwrap().snapshot.hosts[0].label = "Studio Mac".into();
    computers.refresh().unwrap();
    assert_eq!(text_of(&computers, "host-0-label"), "Studio Mac");

    // Owner actions need a directory this device read.
    for (state, denial) in [
        (DirectoryState::NoOwnerKey, Denial::NotOwner),
        (DirectoryState::Loading, Denial::DirectoryNotRead),
        (
            DirectoryState::Failed { revision: Some(2) },
            Denial::DirectoryNotRead,
        ),
        (
            DirectoryState::Conflict { revision: 2 },
            Denial::DirectoryConflict,
        ),
    ] {
        let mut snapshot = directory_snapshot();
        snapshot.directory = state;
        assert_eq!(
            check(
                &snapshot,
                caps(Platform::Desktop),
                Action::ListInDirectory {
                    host: &"a2".repeat(32)
                }
            ),
            Err(denial)
        );
    }
    assert_eq!(
        check(
            &directory_snapshot(),
            caps(Platform::Desktop),
            Action::ListInDirectory {
                host: &"a1".repeat(32)
            }
        ),
        Err(Denial::AlreadyListed)
    );
}

#[test]
fn directory_weights_reach_placement() {
    let client = ClientProfile {
        protocol: coder_reach::PROTOCOL_VERSION,
        accepts: VersionRange {
            min: coder_reach::PROTOCOL_VERSION,
            max: coder_reach::PROTOCOL_VERSION,
        },
    };
    let snapshot = directory_snapshot();
    let studio = "a1".repeat(32);
    let laptop = "a2".repeat(32);
    let build = "a3".repeat(32);
    // Equal telemetry: the directory's weight of 300 beats the local weight
    // of 100 an unlisted host gets.
    assert_eq!(snapshot.place(&client), Some(studio.as_str()));
    let scores: Vec<_> = snapshot.assess_placement(&client);
    assert_eq!(
        scores,
        vec![
            coder_reach::placement::Assessment::Eligible {
                host: &studio,
                score: 300 * 8 * 50 * 50
            },
            coder_reach::placement::Assessment::Eligible {
                host: &laptop,
                score: u128::from(LOCAL_WEIGHT) * 8 * 50 * 50
            },
            coder_reach::placement::Assessment::Skipped {
                host: &build,
                reason: coder_reach::placement::Skip::NotAdmitted
            },
        ]
    );
    // A new revision that sets the weight to zero excludes the host.
    let mut zero = directory_snapshot();
    zero.hosts[0].listing = Some(Listing {
        weight: 0,
        added_at: NOW - 100,
    });
    assert_eq!(zero.place(&client), Some(laptop.as_str()));
}

#[test]
fn owner_key_import_is_offered_on_every_platform_and_needs_a_computer() {
    let mut snapshot = directory_snapshot();
    snapshot.directory = DirectoryState::NoOwnerKey;
    for platform in [Platform::Desktop, Platform::Terminal, Platform::Phone] {
        assert_eq!(
            check(&snapshot, caps(platform), Action::ImportOwnerKey),
            Ok(())
        );
    }
    let mut phone = Computers::new(
        Box::new(Fixed::new(snapshot.clone())),
        caps(Platform::Phone),
        "c:phone",
    )
    .unwrap();
    assert!(find(&phone, "directory-owner-key").is_some());
    assert!(text_of(&phone, "directory-status").contains("Enter your owner key"));
    // The phone asks for a masked value, bounded by the shared contract.
    press(&mut phone, "directory-owner-key").unwrap();
    let asked = phone.input().unwrap().clone();
    assert_eq!(asked.purpose, InputPurpose::OwnerKey);
    assert!(asked.secret && !asked.scan);
    assert_eq!(asked.validate(), Ok(()));
    let json = serde_json::to_value(&asked).unwrap();
    assert_eq!(json["secret"], true);
    assert_eq!(json["purpose"], "owner_key");
    snapshot.hosts.clear();
    assert_eq!(
        check(&snapshot, caps(Platform::Phone), Action::ImportOwnerKey),
        Err(Denial::OwnerKeyNeedsComputer)
    );
    assert_eq!(
        check(
            &directory_snapshot(),
            caps(Platform::Phone),
            Action::ImportOwnerKey
        ),
        Err(Denial::OwnerKeyHeld)
    );
}

#[test]
fn a_phone_enters_the_owner_key_the_fixture_grants_name() {
    let mut phone = open(Platform::Phone);
    press(&mut phone, "first-run-continue").unwrap();
    press(&mut phone, "directory-owner-key").unwrap();
    let asked = phone.input().unwrap().clone();
    assert!(asked.secret);
    // A key no held grant names is refused, and the refusal never echoes it.
    let other = "0f".repeat(32);
    let refused = phone.submit(&asked.token, &other).unwrap_err();
    assert!(matches!(&refused, Refusal::Input(reason) if reason.contains("isn't the owner key")));
    assert!(!refused.reason().contains(&other));
    assert!(!phone.notice().unwrap().text.contains(&other));
    // A stale token and an oversized value are refused before the service.
    assert_eq!(
        phone.submit("c:other-input-1", &synthetic::owner_secret_hex()),
        Err(Refusal::Stale)
    );
    assert!(matches!(
        phone.submit(&asked.token, &"0".repeat(MAX_INPUT_BYTES + 1)),
        Err(Refusal::Input(_))
    ));
    let asked = phone.input().unwrap().clone();
    phone
        .submit(
            &asked.token,
            &format!(" {} ", synthetic::owner_secret_hex()),
        )
        .unwrap();
    assert!(phone.input().is_none());
    assert!(find(&phone, "directory-owner-key").is_none());
    assert!(text_of(&phone, "directory-status").contains("Your directory is empty"));
}

#[test]
fn ssh_setup_progress_prompts_and_result_show_on_the_add_screen() {
    let mut snapshot = Synthetic::empty(Platform::Terminal, now)
        .snapshot()
        .unwrap();
    snapshot.first_run_complete = true;
    let fixed = Fixed::new(snapshot);
    let mut computers =
        Computers::new(Box::new(fixed.clone()), caps(Platform::Terminal), "c:ssh").unwrap();
    press(&mut computers, "tab-add").unwrap();
    press(&mut computers, "ssh-connect").unwrap();
    let asked = computers.input().unwrap().clone();
    assert!(!asked.secret);
    computers.submit(&asked.token, "me@box").unwrap();
    assert_eq!(fixed.calls().last().unwrap(), "ssh me@box");
    assert_eq!(
        text_of(&computers, "notice"),
        "Setting up a host on me@box over SSH."
    );

    let set = |stage: SshStage| {
        fixed.0.lock().unwrap().snapshot.ssh = Some(SshAttempt {
            destination: "me@box".into(),
            stage,
        });
    };
    set(SshStage::Starting);
    computers.refresh().unwrap();
    assert_eq!(
        text_of(&computers, "ssh-status"),
        "Setting up a host on me@box."
    );
    assert!(!enabled(&computers, "ssh-connect"));
    assert!(text_of(&computers, "ssh-connect-reason").contains("already running"));

    // A prompt becomes a masked input request with ssh's own words. The
    // answer passes exactly as typed.
    set(SshStage::Prompt {
        id: 7,
        text: "me@box's password:".into(),
    });
    computers.refresh().unwrap();
    let prompt = computers.input().unwrap().clone();
    assert_eq!(prompt.purpose, InputPurpose::SshPassword);
    assert!(prompt.secret);
    assert_eq!(prompt.prompt, "me@box's password:");
    // Refreshing keeps the same request.
    computers.refresh().unwrap();
    assert_eq!(computers.input().unwrap().token, prompt.token);
    computers.submit(&prompt.token, " pass word ").unwrap();
    assert_eq!(fixed.calls().last().unwrap(), "ssh_answer 7 true");
    assert!(computers.input().is_none());

    // Cancelling a prompt refuses it, so ssh fails rather than wait.
    set(SshStage::Prompt {
        id: 8,
        text: "Enter passphrase:".into(),
    });
    computers.refresh().unwrap();
    let prompt = computers.input().unwrap().clone();
    computers.cancel_input(&prompt.token).unwrap();
    assert_eq!(fixed.calls().last().unwrap(), "ssh_answer 8 false");

    // A prompt the setup stopped waiting on closes.
    set(SshStage::Prompt {
        id: 9,
        text: "Password:".into(),
    });
    computers.refresh().unwrap();
    assert!(computers.input().is_some());
    set(SshStage::Enrolling);
    computers.refresh().unwrap();
    assert!(computers.input().is_none());
    assert_eq!(
        text_of(&computers, "ssh-status"),
        "Adding this device to the host on me@box."
    );
    set(SshStage::Failed {
        reason: "the host didn't start.".into(),
    });
    computers.refresh().unwrap();
    assert_eq!(
        text_of(&computers, "ssh-status"),
        "Couldn't set up a host on me@box: the host didn't start."
    );
    assert!(enabled(&computers, "ssh-connect"));

    // A client without a host release can't offer SSH.
    fixed.0.lock().unwrap().snapshot.ssh_ready = false;
    computers.refresh().unwrap();
    assert!(!enabled(&computers, "ssh-connect"));
    assert_eq!(
        text_of(&computers, "ssh-connect-reason"),
        "Unavailable: This app has no Coder release to install over SSH."
    );
}

#[test]
fn the_invitation_help_names_the_host_command() {
    let computers = open(Platform::Terminal);
    let body = text_of(&computers, "invite-body");
    assert!(body.contains("`openagents host invite`"), "{body}");
    assert!(!body.contains("coder-access"));
}

/// The index of the Activity row whose headline is `headline`.
fn activity_row(computers: &Computers, prefix: &str, headline: &str) -> usize {
    (0..64)
        .find(|index| {
            find(computers, &format!("{prefix}-{index}-headline")).is_some_and(
                |node| matches!(&node.element, Element::Text { value, .. } if value == headline),
            )
        })
        .unwrap_or_else(|| panic!("no {prefix} row for {headline}"))
}

#[test]
fn a_phone_orders_steers_and_stops_work_on_an_online_host() {
    let mut computers = open(Platform::Phone);
    press(&mut computers, "first-run-continue").unwrap();
    let studio = synthetic_key(0xa1);

    // An offline host offers neither ordering nor a terminal, with reasons.
    show(
        &mut computers,
        Screen::Host {
            host: synthetic_key(0xa3),
        },
    );
    assert!(!enabled(&computers, "host-order"));
    assert!(text_of(&computers, "host-order-reason").contains("offline"));
    assert!(!enabled(&computers, "host-terminal"));

    // The online host names its route and workspaces, and offers both.
    show(
        &mut computers,
        Screen::Host {
            host: studio.clone(),
        },
    );
    assert!(text_of(&computers, "host-status").starts_with("Online over the local network"));
    assert_eq!(
        text_of(&computers, "host-workspaces"),
        "Workspaces: openagents, scratch."
    );
    assert_eq!(
        press(&mut computers, "host-terminal").unwrap(),
        Outcome::Terminal
    );
    assert_eq!(computers.take_terminal().as_deref(), Some(studio.as_str()));
    assert_eq!(computers.take_terminal(), None);

    press(&mut computers, "host-order").unwrap();
    assert_eq!(
        computers.screen(),
        &Screen::Order {
            host: studio.clone()
        }
    );
    assert!(!enabled(&computers, "order-submit"));
    assert_reasons(&computers);
    press(&mut computers, "order-workspace-1").unwrap();
    assert_eq!(text_of(&computers, "order-workspace-1"), "[x] scratch");
    assert_eq!(
        press(&mut computers, "order-prompt").unwrap(),
        Outcome::InputRequested
    );
    let input = computers.input().unwrap().clone();
    assert_eq!(input.purpose, InputPurpose::TaskPrompt);
    assert!(matches!(
        computers.submit(&input.token, "   "),
        Err(Refusal::Input(_))
    ));
    let input = computers.input().unwrap().clone();
    computers
        .submit(
            &input.token,
            "\tFix the flaky parser test\nThen run the suite.",
        )
        .unwrap();
    assert!(text_of(&computers, "order-prompt-text").starts_with("Fix the flaky"));
    assert!(enabled(&computers, "order-submit"));
    press(&mut computers, "order-submit").unwrap();
    assert_eq!(computers.screen(), &Screen::Activity);
    assert!(text_of(&computers, "notice").contains("Sent \"Fix the flaky parser test\""));

    let row = activity_row(&computers, "activity", "Fix the flaky parser test");
    assert!(text_of(&computers, &format!("activity-{row}-subject")).contains("Task, queued"));
    press(&mut computers, &format!("activity-{row}-steer")).unwrap();
    let input = computers.input().unwrap().clone();
    assert_eq!(input.purpose, InputPurpose::SteerPrompt);
    computers
        .submit(&input.token, "Only the parser module.")
        .unwrap();
    assert!(text_of(&computers, "notice").contains("Sent new instructions"));
    let row = activity_row(&computers, "activity", "Fix the flaky parser test");
    assert!(text_of(&computers, &format!("activity-{row}-time")).contains("Revision 2."));

    press(&mut computers, &format!("activity-{row}-cancel")).unwrap();
    assert!(find(&computers, &format!("activity-{row}-cancel-confirm")).is_some());
    press(&mut computers, &format!("activity-{row}-cancel-yes")).unwrap();
    let row = activity_row(&computers, "activity", "Fix the flaky parser test");
    assert!(text_of(&computers, &format!("activity-{row}-subject")).contains("cancelled"));
    assert!(find(&computers, &format!("activity-{row}-steer")).is_none());

    // The host's own screen lists its recent work.
    show(&mut computers, Screen::Host { host: studio });
    activity_row(&computers, "host-task", "Fix the flaky parser test");
    assert_reasons(&computers);
}

#[test]
fn a_host_without_a_workspace_list_takes_a_typed_name() {
    let mut computers = open(Platform::Phone);
    press(&mut computers, "first-run-continue").unwrap();
    let studio = synthetic_key(0xa1);
    show(
        &mut computers,
        Screen::Order {
            host: studio.clone(),
        },
    );
    press(&mut computers, "order-workspace-enter").unwrap();
    let input = computers.input().unwrap().clone();
    assert_eq!(input.purpose, InputPurpose::TaskWorkspace);
    assert!(matches!(
        computers.submit(&input.token, "two\nlines"),
        Err(Refusal::Input(_))
    ));
    let input = computers.input().unwrap().clone();
    computers.submit(&input.token, "elsewhere").unwrap();
    assert_eq!(
        text_of(&computers, "order-workspace-entered"),
        "Workspace: elsewhere."
    );
    press(&mut computers, "order-prompt").unwrap();
    let input = computers.input().unwrap().clone();
    computers.submit(&input.token, "Try it").unwrap();
    // The fixture host refuses a workspace it does not share, and the
    // screen says so instead of leaving.
    assert!(press(&mut computers, "order-submit").is_err());
    assert_eq!(computers.screen(), &Screen::Order { host: studio });
    assert!(text_of(&computers, "notice").contains("refused"));
}

#[test]
fn task_titles_are_one_bounded_line() {
    use crate::controller::task_title;
    assert_eq!(task_title("\n\n  Fix it\nmore"), "Fix it");
    assert_eq!(task_title("a\tb"), "a b");
    assert_eq!(task_title("   "), "Task");
    assert_eq!(task_title(&"x".repeat(300)).chars().count(), 80);
}

#[test]
fn a_native_control_runs_an_intent_through_the_same_check() {
    let mut computers = open(Platform::Phone);
    let host = computers.snapshot().hosts[0].key.clone();
    assert_eq!(
        computers.perform(Intent::Show {
            screen: Screen::Host { host: host.clone() }
        }),
        Ok(Outcome::Updated)
    );
    assert_eq!(computers.screen(), &Screen::Host { host });
    // An unknown host is refused, as a stale button would be, and the
    // refusal shows as the screen's notice.
    assert!(
        computers
            .perform(Intent::Show {
                screen: Screen::Access {
                    host: "unknown".into()
                }
            })
            .is_err()
    );
    assert!(computers.notice().is_some());
    // Confirming a forget nobody asked for is stale.
    let other = computers.snapshot().hosts[1].key.clone();
    assert_eq!(
        computers.perform(Intent::ConfirmForget { host: other }),
        Err(Refusal::Stale)
    );
}

/// Reading a task's change needs `observe`; publishing it is a mutation
/// that needs `operate`, checked before anything reaches the computer
/// (#10067, #10068).
#[test]
fn publishing_a_change_needs_operate_and_reading_it_needs_observe() {
    let mut snapshot = Synthetic::fixture(Platform::Phone, now).snapshot().unwrap();
    let online = snapshot.hosts[0].key.clone();
    let Enrollment::Enrolled { rights, .. } = &mut snapshot.hosts[0].enrollment else {
        panic!("the first host is enrolled");
    };
    *rights = Rights::new([Right::Observe]).unwrap();
    let fixed = Fixed::new(snapshot);
    let mut computers =
        Computers::new(Box::new(fixed.clone()), caps(Platform::Phone), "c:review").unwrap();
    let (task, revision) = ("f".repeat(64), "1".repeat(40));
    assert_eq!(
        computers.publish_task(&online, &task, &revision, &revision, &revision),
        Err(Refusal::Denied(Denial::MissingRight(Right::Operate)))
    );
    // The read passes the grant check and reaches the service, which here
    // reviews nothing.
    assert!(matches!(
        computers.review_task(&online, &task),
        Err(Refusal::Failed(_))
    ));
    assert!(fixed.calls().is_empty(), "nothing was published");
}

/// A host's `background.list` answer names the watchers running: rules on,
/// readable, and not paused, first letter lowercased, as the computer's own
/// terminal and desktop name them.
#[test]
fn background_list_gives_the_newest_notice() {
    let list = serde_json::json!([
        {"name": "Disk cleanup", "enabled": true, "state": {"notice": [100, "Freed 4 GB: 2 old build folders."]}},
        {"name": "Usage", "enabled": true, "state": {"notice": [200, "Today: 3 Coder runs ended (3 finished, 0 failed)."]}},
        {"name": "Quiet", "enabled": true, "state": {}},
    ]);
    assert_eq!(
        crate::model::background_notice(&list),
        Some((
            200,
            "Today: 3 Coder runs ended (3 finished, 0 failed).".into()
        ))
    );
    assert_eq!(
        crate::model::background_notice(&serde_json::json!([])),
        None
    );
    assert_eq!(
        crate::model::background_notice(&serde_json::json!({"queued": "x"})),
        None
    );
}

#[test]
fn background_list_names_the_running_watchers() {
    let list = serde_json::json!([
        {"id": "disk-cleanup", "name": "Disk cleanup", "version": 1, "digest": "d",
         "enabled": true, "state": {}},
        {"id": "logs", "name": "Logs", "version": 1, "digest": "d",
         "enabled": true, "paused_until": 500, "state": {}},
        {"id": "old", "name": "Old", "version": 1, "digest": "d",
         "enabled": true, "paused_until": 50, "state": {}},
        {"id": "off", "name": "Off", "version": 1, "digest": "d",
         "enabled": false, "state": {}},
        {"id": "bad", "name": "bad", "version": 0, "digest": "",
         "enabled": false, "state": {}, "error": "unreadable"},
    ]);
    assert_eq!(
        crate::model::watchers(&list, 100),
        Some(vec!["disk cleanup".to_owned(), "old".to_owned()])
    );
    assert_eq!(
        crate::model::watchers(&serde_json::json!([]), 100),
        Some(vec![])
    );
    assert_eq!(
        crate::model::watchers(&serde_json::json!({"queued": "x"}), 100),
        None
    );
}

#[test]
fn a_phone_takes_a_screenshot_and_copies_files_from_an_online_host() {
    let mut computers = open(Platform::Phone);
    press(&mut computers, "first-run-continue").unwrap();
    let studio = synthetic_key(0xa1);

    // An offline host offers neither, with reasons.
    show(
        &mut computers,
        Screen::Host {
            host: synthetic_key(0xa3),
        },
    );
    assert!(!enabled(&computers, "host-screenshot"));
    assert!(!enabled(&computers, "host-files"));
    assert!(find(&computers, "host-screenshot-reason").is_some());

    show(
        &mut computers,
        Screen::Host {
            host: studio.clone(),
        },
    );
    assert!(find(&computers, "host-capture").is_none());

    // Screenshot shows the picture as an image surface the platform draws.
    assert_eq!(
        press(&mut computers, "host-screenshot").unwrap(),
        Outcome::Updated
    );
    let resource = match find(&computers, "host-capture-image").map(|node| &node.element) {
        Some(Element::Surface { resource, .. }) => resource.clone(),
        other => panic!("{other:?}"),
    };
    assert!(resource.starts_with("image:computer-capture-"));
    assert_eq!(
        computers.capture_image(&resource),
        Some(synthetic::SCREENSHOT)
    );
    assert_eq!(computers.capture_image("image:computer-capture-0"), None);

    // Files asks for a path, refuses one that isn't, and shows text.
    assert_eq!(
        press(&mut computers, "host-files").unwrap(),
        Outcome::InputRequested
    );
    let input = computers.input().unwrap().clone();
    assert_eq!(input.purpose, InputPurpose::FilePath);
    assert!(matches!(
        computers.submit(&input.token, "notes.txt"),
        Err(Refusal::Input(_))
    ));
    computers.submit(&input.token, "~/notes.txt").unwrap();
    assert!(computers.input().is_none());
    assert_eq!(
        text_of(&computers, "host-capture-text"),
        "Buy milk.\nCall the shop.\n"
    );
    assert!(text_of(&computers, "host-capture-title").starts_with("/home/synthetic/notes.txt"));
    assert!(find(&computers, "host-capture-image").is_none());
    assert_eq!(computers.capture_image(&resource), None);

    // Bytes that are neither text nor an image show only their size.
    press(&mut computers, "host-files").unwrap();
    let input = computers.input().unwrap().clone();
    computers.submit(&input.token, "~/data.bin").unwrap();
    assert!(find(&computers, "host-capture-other").is_some());

    // A missing file is refused and keeps the last capture.
    press(&mut computers, "host-files").unwrap();
    let input = computers.input().unwrap().clone();
    assert!(matches!(
        computers.submit(&input.token, "~/gone.txt"),
        Err(Refusal::Failed(_))
    ));
    assert!(find(&computers, "host-capture-other").is_some());

    // Close puts it away.
    computers.cancel_input(&input.token).unwrap();
    press(&mut computers, "host-capture-clear").unwrap();
    assert!(find(&computers, "host-capture").is_none());
    assert!(computers.capture().is_none());
}
