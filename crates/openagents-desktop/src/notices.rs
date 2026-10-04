//! Desktop notifications for Coder's work: when a chat's Coder asks a
//! question, asks for approval, finishes, or fails while the window is not
//! in front, the desktop says so once.
//!
//! This decides *when*; the platform delivers (on Linux the desktop portal,
//! else `org.freedesktop.Notifications`; on macOS the notification center,
//! through [`deliver`] and a [`Center`]; on Windows a toast, the same way,
//! under [`WINDOWS_APP_ID`]). A chat seen for the first time is
//! only recorded, so opening the app on finished work notifies nothing, and
//! a status that does not change notifies nothing again. Nothing here reads
//! a message: the notice says what Coder is doing, under the chat's title.
//!
//! Clicking a notice opens its chat: the notice carries [`OPEN_ACTION`] with
//! the chat's ID, and the window runs the shared command registry's switch
//! entry for that chat ([`open_command`]), the same command the palette's
//! "Switch to" row runs.

use std::collections::BTreeMap;

/// What a chat's Coder is doing, as far as a notice cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Working, starting, stopped by the person, or nothing yet.
    Working,
    /// Waiting for an answer.
    Question,
    /// Waiting for approval to go ahead.
    Approval,
    /// The work finished.
    Finished,
    /// The work failed.
    Failed,
}

impl Status {
    /// The notice's text, `None` for a status that asks nothing of the
    /// person.
    #[must_use]
    pub fn body(self) -> Option<&'static str> {
        match self {
            Status::Working => None,
            Status::Question => Some("Coder asked a question"),
            Status::Approval => Some("Coder asked for approval"),
            Status::Finished => Some("Coder finished"),
            Status::Failed => Some("Coder stopped with an error"),
        }
    }

    /// The attention activity this status reports, for the sound a change
    /// plays ([`openagents_chat_app::cues`]).
    #[must_use]
    pub fn activity(self) -> openagents_chat_app::attention::Activity {
        use openagents_chat_app::attention::Activity;
        match self {
            Status::Working => Activity::Working,
            Status::Question | Status::Approval => Activity::AwaitingInput,
            Status::Finished => Activity::Completed,
            Status::Failed => Activity::Failed,
        }
    }
}

/// The prefix of a Coder notice's [`Notice::id`]; the rest is the chat's ID.
const PREFIX: &str = "coder-";

/// The action a notice carries for a click on its body.
pub const OPEN_ACTION: &str = "open";

/// The notification server's name for a click on a notice's body.
pub const SERVER_DEFAULT: &str = "default";

/// The chat a click reported by the desktop portal opens: notice `id`'s
/// chat when `action` is [`OPEN_ACTION`].
#[must_use]
pub fn portal_click<'a>(id: &'a str, action: &str) -> Option<&'a str> {
    (action == OPEN_ACTION).then(|| chat_of(id)).flatten()
}

/// Whether a notification server's `ActionInvoked` `action` opens the
/// notice's chat: its body ([`SERVER_DEFAULT`]) or its Open button.
#[must_use]
pub fn server_click(action: &str) -> bool {
    action == SERVER_DEFAULT || action == OPEN_ACTION
}

/// macOS's name for a click on a notice's body
/// (`UNNotificationDefaultActionIdentifier`).
pub const MAC_DEFAULT_ACTION: &str = "com.apple.UNNotificationDefaultActionIdentifier";

/// The chat a click reported by macOS's notification center opens: the
/// notice `id`'s chat when `action` is a click on its body
/// ([`MAC_DEFAULT_ACTION`]), not a dismissal.
#[must_use]
pub fn mac_click<'a>(id: &'a str, action: &str) -> Option<&'a str> {
    (action == MAC_DEFAULT_ACTION)
        .then(|| chat_of(id))
        .flatten()
}

/// The AppUserModelID Windows shows this app's toasts under. The MSI puts
/// it on the Start menu shortcut (`System.AppUserModel.ID`) and registers
/// it under `HKCU\Software\Classes\AppUserModelId`, and the app claims it
/// at startup (`SetCurrentProcessExplicitAppUserModelID`); all three must
/// match (`scripts/desktop/package-windows.sh` and `.ps1`).
pub const WINDOWS_APP_ID: &str = "OpenAgents.Desktop";

/// The toast group every Coder notice is posted under on Windows.
pub const WINDOWS_GROUP: &str = "coder";

/// The longest toast tag Windows accepts.
const WINDOWS_TAG_MAX: usize = 64;

/// The toast tag for notice `id`: the ID itself (`coder-<chat>`), so a
/// newer toast for a chat replaces the older one. An ID longer than Windows
/// allows becomes `coder-` and a stable hash of it, still one tag a chat.
#[must_use]
pub fn windows_tag(id: &str) -> String {
    if id.len() <= WINDOWS_TAG_MAX {
        return id.to_owned();
    }
    // FNV-1a, 64-bit: stable across runs and builds.
    let hash = id.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{PREFIX}{hash:016x}")
}

fn xml_escaped(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            // XML 1.0 has no other control characters.
            c if c.is_control() && !matches!(c, '\t' | '\n' | '\r') => {}
            c => escaped.push(c),
        }
    }
    escaped
}

/// The toast XML for `notice`: its title and body and nothing else, the
/// notice's ID as the activation arguments (`launch`), foreground
/// activation, and a sound only when Coder waits for the person.
#[must_use]
pub fn windows_toast_xml(notice: &Notice) -> String {
    let audio = if notice.urgent {
        ""
    } else {
        "<audio silent=\"true\"/>"
    };
    format!(
        "<toast launch=\"{}\" activationType=\"foreground\"><visual><binding template=\"ToastGeneric\">\
         <text>{}</text><text>{}</text></binding></visual>{audio}</toast>",
        xml_escaped(&notice.id),
        xml_escaped(&notice.title),
        xml_escaped(&notice.body),
    )
}

/// The chat a toast click opens: the activation `arguments` (the toast's
/// `launch`, the notice's ID) name a Coder notice's chat.
#[must_use]
pub fn windows_click(arguments: &str) -> Option<&str> {
    chat_of(arguments)
}

/// Windows's `NotificationSetting` for this app as a [`Permission`]:
/// `Enabled` (0) is granted; turned off for the app (1), for the user (2),
/// by group policy (3), or by the manifest (4) is denied. Windows never
/// asks the person, so the first notice shows at once when on.
#[must_use]
pub fn windows_permission(setting: i32) -> Permission {
    match setting {
        0 => Permission::Granted,
        1..=4 => Permission::Denied,
        _ => Permission::Unavailable,
    }
}

/// The chat a notice's ID names, if it is a Coder notice.
#[must_use]
pub fn chat_of(id: &str) -> Option<&str> {
    id.strip_prefix(PREFIX).filter(|chat| !chat.is_empty())
}

/// The shared command (`openagents_chat_app::commands::registry` key) a
/// click on a chat's notice runs: switch to that chat.
#[must_use]
pub fn open_command(chat: &str) -> String {
    format!("switch-{chat}")
}

/// One notification to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// Stable for the chat, so a newer notice replaces an older one.
    pub id: String,
    pub title: String,
    pub body: String,
    /// Coder waits for the person.
    pub urgent: bool,
}

impl Notice {
    /// The chat this notice opens when clicked.
    #[must_use]
    pub fn chat(&self) -> Option<&str> {
        chat_of(&self.id)
    }
}

/// Whether the person lets this app show notifications.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Permission {
    Granted,
    /// Turned off for this app in the system's settings, or declined when
    /// asked.
    Denied,
    /// No notification service to ask (e.g. not running from an app bundle).
    Unavailable,
}

/// How a notice went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Delivery {
    Shown,
    Denied,
    Unavailable,
    /// The service refused the notice, with its reason.
    Failed(String),
}

/// A system notification center that asks the person before it shows
/// anything (macOS's `UNUserNotificationCenter`). Both calls may answer on
/// another thread.
pub trait Center {
    /// Asks the person the first time; after that answers from their
    /// setting without asking again.
    fn authorize(&self, then: Box<dyn FnOnce(Permission) + Send>);
    /// Shows `notice` under its ID (a newer notice with the same ID
    /// replaces the older one), with its title and body and nothing else.
    fn post(&self, notice: &Notice, then: Box<dyn FnOnce(Result<(), String>) + Send>);
}

/// Shows `notice` through `center`: asks for permission first (so the
/// person is asked on the first notice, never at launch), and posts only
/// once it is granted. `done` hears how it went.
pub fn deliver<C: Center + Send + Sync + 'static>(
    center: std::sync::Arc<C>,
    notice: Notice,
    done: impl FnOnce(Delivery) + Send + 'static,
) {
    let poster = center.clone();
    center.authorize(Box::new(move |permission| match permission {
        Permission::Granted => poster.post(
            &notice,
            Box::new(move |posted| {
                done(match posted {
                    Ok(()) => Delivery::Shown,
                    Err(reason) => Delivery::Failed(reason),
                });
            }),
        ),
        Permission::Denied => done(Delivery::Denied),
        Permission::Unavailable => done(Delivery::Unavailable),
    }));
}

/// The prefix of a background rule's notice ID.
const BACKGROUND: &str = "background-";

/// The last status seen for each chat, and the newest background notice.
#[derive(Debug, Default)]
pub struct Notices {
    seen: BTreeMap<String, Status>,
    /// When the newest background notice seen was sent; `None` before the
    /// first look.
    background: Option<u64>,
}

impl Notices {
    /// Records the newest background notice (`(when, line)`, from the
    /// rules the host runs) and returns it as a notice when it is newer
    /// than the last one seen, while the window is not `focused`. The
    /// first look only records, so opening the app notifies nothing.
    pub fn observe_background(
        &mut self,
        latest: Option<(u64, String)>,
        focused: bool,
    ) -> Option<Notice> {
        let at = latest.as_ref().map_or(0, |(at, _)| *at);
        let before = self
            .background
            .replace(at.max(self.background.unwrap_or(0)))?;
        let (at, line) = latest?;
        (!focused && at > before).then(|| Notice {
            id: format!("{BACKGROUND}{at}"),
            title: "Background".into(),
            body: line,
            urgent: false,
        })
    }

    /// Records each chat's `(id, title, status)` and returns the notices
    /// to show: chats whose status changed to one that asks for the
    /// person, while the window is not `focused`. Chats no longer listed
    /// are forgotten.
    pub fn observe(
        &mut self,
        chats: impl IntoIterator<Item = (String, String, Status)>,
        focused: bool,
    ) -> Vec<Notice> {
        let mut notices = vec![];
        let mut seen = BTreeMap::new();
        for (id, title, status) in chats {
            let before = self.seen.get(&id).copied();
            if !focused
                && before.is_some_and(|before| before != status)
                && let Some(body) = status.body()
            {
                notices.push(Notice {
                    id: format!("{PREFIX}{id}"),
                    title: if title.trim().is_empty() {
                        "OpenAgents".into()
                    } else {
                        title
                    },
                    body: body.into(),
                    urgent: matches!(status, Status::Question | Status::Approval),
                });
            }
            seen.insert(id, status);
        }
        self.seen = seen;
        notices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(status: Status) -> Vec<(String, String, Status)> {
        vec![("c1".into(), "Fix the login bug".into(), status)]
    }

    #[test]
    fn a_new_background_notice_notifies_once_and_the_first_look_only_records() {
        let mut notices = Notices::default();
        let line = |at: u64| Some((at, format!("Freed {at} GB.")));
        assert_eq!(notices.observe_background(line(10), false), None);
        assert_eq!(notices.observe_background(line(10), false), None);
        let shown = notices.observe_background(line(20), false).unwrap();
        assert_eq!(shown.title, "Background");
        assert_eq!(shown.body, "Freed 20 GB.");
        assert!(!shown.urgent);
        assert_eq!(shown.chat(), None, "it opens no chat");
        // In front, it is only recorded.
        assert_eq!(notices.observe_background(line(30), true), None);
        assert_eq!(notices.observe_background(line(30), false), None);
        assert_eq!(notices.observe_background(None, false), None);
    }

    #[test]
    fn a_change_that_asks_for_the_person_notifies_once_while_the_window_is_away() {
        let mut notices = Notices::default();
        assert!(notices.observe(chat(Status::Working), false).is_empty());
        let asked = notices.observe(chat(Status::Approval), false);
        assert_eq!(
            asked,
            [Notice {
                id: "coder-c1".into(),
                title: "Fix the login bug".into(),
                body: "Coder asked for approval".into(),
                urgent: true,
            }]
        );
        // The same status again: nothing.
        assert!(notices.observe(chat(Status::Approval), false).is_empty());
        assert!(notices.observe(chat(Status::Working), false).is_empty());
        let done = notices.observe(chat(Status::Finished), false);
        assert_eq!(done[0].body, "Coder finished");
        assert!(!done[0].urgent);
    }

    #[test]
    fn nothing_notifies_while_the_window_is_in_front_or_for_a_chat_first_seen() {
        let mut notices = Notices::default();
        // Opening the app on finished work.
        assert!(notices.observe(chat(Status::Finished), false).is_empty());
        assert!(notices.observe(chat(Status::Working), true).is_empty());
        // In front: recorded, not shown, and not shown later either.
        assert!(notices.observe(chat(Status::Question), true).is_empty());
        assert!(notices.observe(chat(Status::Question), false).is_empty());
        let failed = notices.observe(chat(Status::Failed), false);
        assert_eq!(failed[0].body, "Coder stopped with an error");
        // A chat that goes away is forgotten, and comes back unannounced.
        assert!(notices.observe(vec![], false).is_empty());
        assert!(notices.observe(chat(Status::Approval), false).is_empty());
    }

    #[test]
    fn a_notice_opens_its_chat_through_the_shared_switch_command() {
        let mut notices = Notices::default();
        notices.observe(chat(Status::Working), false);
        let done = notices.observe(chat(Status::Finished), false);
        assert_eq!(done[0].chat(), Some("c1"));
        assert_eq!(open_command("c1"), "switch-c1");
        let registry = openagents_chat_app::commands::registry(
            &[openagents_chat::basic_chats::Summary {
                id: "c1".into(),
                title: "Fix the login bug".into(),
                started: 1,
                updated: 1,
                coder: None,
                archived: false,
                pinned: false,
                named: false,
            }],
            None,
            false,
        );
        let entry = registry
            .iter()
            .find(|entry| entry.key == open_command("c1"))
            .expect("the registry switches to the notice's chat");
        assert_eq!(
            entry.action,
            openagents_chat_app::commands::Action::Switch("c1".into())
        );
        assert_eq!(portal_click("coder-c1", OPEN_ACTION), Some("c1"));
        assert_eq!(portal_click("coder-c1", "dismiss"), None);
        assert_eq!(portal_click("other-c1", OPEN_ACTION), None);
        assert!(server_click("default") && server_click("open"));
        assert!(!server_click("close"));
        assert_eq!(chat_of("coder-"), None);
        assert_eq!(chat_of("other-c1"), None);
    }

    /// A stand-in notification center that records what it was asked.
    struct MockCenter {
        permission: Permission,
        refuse: Option<String>,
        calls: std::sync::Mutex<Vec<String>>,
    }

    impl MockCenter {
        fn new(permission: Permission, refuse: Option<&str>) -> std::sync::Arc<Self> {
            std::sync::Arc::new(Self {
                permission,
                refuse: refuse.map(str::to_owned),
                calls: std::sync::Mutex::default(),
            })
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Center for MockCenter {
        fn authorize(&self, then: Box<dyn FnOnce(Permission) + Send>) {
            self.calls.lock().unwrap().push("authorize".into());
            then(self.permission.clone());
        }

        fn post(&self, notice: &Notice, then: Box<dyn FnOnce(Result<(), String>) + Send>) {
            self.calls.lock().unwrap().push(format!(
                "post {} | {} | {}",
                notice.id, notice.title, notice.body
            ));
            then(self.refuse.clone().map_or(Ok(()), Err));
        }
    }

    fn delivered(center: &std::sync::Arc<MockCenter>, notice: Notice) -> Delivery {
        let (tx, rx) = std::sync::mpsc::channel();
        deliver(center.clone(), notice, move |delivery| {
            tx.send(delivery).unwrap();
        });
        rx.recv().unwrap()
    }

    #[test]
    fn the_center_asks_on_the_first_notice_and_posts_only_the_title_and_status() {
        let mut notices = Notices::default();
        notices.observe(chat(Status::Working), false);
        let center = MockCenter::new(Permission::Granted, None);
        // Nothing is asked before there is a notice to show.
        assert!(center.calls().is_empty());
        let asked = notices.observe(chat(Status::Question), false).remove(0);
        assert_eq!(delivered(&center, asked), Delivery::Shown);
        assert_eq!(
            center.calls(),
            [
                "authorize",
                "post coder-c1 | Fix the login bug | Coder asked a question"
            ]
        );
    }

    #[test]
    fn a_declined_or_missing_center_shows_nothing() {
        let notice = || Notice {
            id: "coder-c1".into(),
            title: "Fix the login bug".into(),
            body: "Coder finished".into(),
            urgent: false,
        };
        let denied = MockCenter::new(Permission::Denied, None);
        assert_eq!(delivered(&denied, notice()), Delivery::Denied);
        assert_eq!(denied.calls(), ["authorize"]);
        let missing = MockCenter::new(Permission::Unavailable, None);
        assert_eq!(delivered(&missing, notice()), Delivery::Unavailable);
        assert_eq!(missing.calls(), ["authorize"]);
        let refused = MockCenter::new(Permission::Granted, Some("no"));
        assert_eq!(delivered(&refused, notice()), Delivery::Failed("no".into()));
    }

    #[test]
    fn a_click_on_a_mac_notice_opens_its_chat_and_a_dismissal_does_not() {
        assert_eq!(mac_click("coder-c1", MAC_DEFAULT_ACTION), Some("c1"));
        assert_eq!(
            mac_click(
                "coder-c1",
                "com.apple.UNNotificationDismissActionIdentifier"
            ),
            None
        );
        assert_eq!(mac_click("openagents-test", MAC_DEFAULT_ACTION), None);
        assert_eq!(mac_click("coder-", MAC_DEFAULT_ACTION), None);
    }

    #[test]
    fn a_windows_toast_says_only_the_title_and_status_and_carries_the_notice_id() {
        let mut notices = Notices::default();
        let chat = |status| vec![("c1".to_owned(), "Fix <login> & \"auth\"".to_owned(), status)];
        notices.observe(chat(Status::Working), false);
        let asked = notices.observe(chat(Status::Question), false).remove(0);
        assert_eq!(
            windows_toast_xml(&asked),
            "<toast launch=\"coder-c1\" activationType=\"foreground\"><visual>\
             <binding template=\"ToastGeneric\"><text>Fix &lt;login&gt; &amp; &quot;auth&quot;</text>\
             <text>Coder asked a question</text></binding></visual></toast>"
        );
        let finished = Notice {
            urgent: false,
            ..asked.clone()
        };
        assert!(windows_toast_xml(&finished).ends_with("<audio silent=\"true\"/></toast>"));
        // A control character in a title never reaches the XML.
        let odd = Notice {
            title: "a\u{1}b".into(),
            ..asked
        };
        assert!(windows_toast_xml(&odd).contains("<text>ab</text>"));
    }

    #[test]
    fn a_newer_windows_toast_for_a_chat_replaces_the_older_one() {
        // The tag is the notice's ID, the same for every notice of a chat.
        let mut notices = Notices::default();
        notices.observe(chat(Status::Working), false);
        let asked = notices.observe(chat(Status::Approval), false).remove(0);
        notices.observe(chat(Status::Working), false);
        let done = notices.observe(chat(Status::Finished), false).remove(0);
        assert_eq!(windows_tag(&asked.id), "coder-c1");
        assert_eq!(windows_tag(&asked.id), windows_tag(&done.id));
        // A chat ID too long for a tag still has one tag, its own.
        let long = format!("coder-{}", "x".repeat(80));
        let other = format!("coder-{}", "y".repeat(80));
        assert!(windows_tag(&long).len() <= 64);
        assert_eq!(windows_tag(&long), windows_tag(&long.clone()));
        assert_ne!(windows_tag(&long), windows_tag(&other));
        assert_eq!(windows_tag(&"c".repeat(64)), "c".repeat(64));
    }

    #[test]
    fn a_click_on_a_windows_toast_opens_its_chat() {
        let mut notices = Notices::default();
        notices.observe(chat(Status::Working), false);
        let done = notices.observe(chat(Status::Finished), false).remove(0);
        // The activation arguments are the toast's `launch`, its ID.
        assert!(windows_toast_xml(&done).starts_with("<toast launch=\"coder-c1\""));
        assert_eq!(windows_click(&done.id), Some("c1"));
        assert_eq!(windows_click("openagents-test"), None);
        assert_eq!(windows_click(""), None);
        assert_eq!(windows_click("coder-"), None);
    }

    #[test]
    fn windows_notifications_turned_off_show_nothing() {
        assert_eq!(windows_permission(0), Permission::Granted);
        for off in 1..=4 {
            assert_eq!(windows_permission(off), Permission::Denied);
        }
        assert_eq!(windows_permission(9), Permission::Unavailable);
        // Off in Windows's settings: the center is asked and posts nothing.
        let notice = Notice {
            id: "coder-c1".into(),
            title: "Fix the login bug".into(),
            body: "Coder finished".into(),
            urgent: false,
        };
        let off = MockCenter::new(windows_permission(1), None);
        assert_eq!(delivered(&off, notice.clone()), Delivery::Denied);
        assert_eq!(off.calls(), ["authorize"]);
        // Not installed from the MSI (no AppUserModelID): nothing either.
        let unregistered = MockCenter::new(Permission::Unavailable, None);
        assert_eq!(delivered(&unregistered, notice), Delivery::Unavailable);
        assert_eq!(unregistered.calls(), ["authorize"]);
    }

    #[test]
    fn both_installers_give_the_shortcut_the_app_id_the_app_claims() {
        let sh = include_str!("../../../scripts/desktop/package-windows.sh");
        let ps1 = include_str!("../../../scripts/desktop/package-windows.ps1");
        assert!(sh.contains(&format!("app_id=\"{WINDOWS_APP_ID}\"")));
        assert!(sh.contains(r"ShortcutAppId\tStartMenuShortcut\tSystem.AppUserModel.ID\t%s"));
        assert!(sh.contains(r"Software\\Classes\\AppUserModelId\\$app_id"));
        assert!(ps1.contains(&format!("$AppId = \"{WINDOWS_APP_ID}\"")));
        assert!(
            ps1.contains(r#"<ShortcutProperty Key="System.AppUserModel.ID" Value="$AppId" />"#)
        );
        assert!(ps1.contains(r"Software\Classes\AppUserModelId\$AppId"));
    }

    #[test]
    fn a_status_change_plays_its_cue_once() {
        use openagents_chat_app::cues::{Cue, Cues};
        let mut cues = Cues::default();
        let mut observe = |status: Status| {
            cues.observe([("c1".to_owned(), status.activity())])
                .into_iter()
                .map(|(_, cue)| cue)
                .collect::<Vec<_>>()
        };
        assert!(observe(Status::Working).is_empty());
        assert_eq!(observe(Status::Question), [Cue::Request]);
        // A question that becomes an approval still waits: no second cue.
        assert!(observe(Status::Approval).is_empty());
        assert!(observe(Status::Working).is_empty());
        assert_eq!(observe(Status::Finished), [Cue::Done]);
        assert!(observe(Status::Finished).is_empty());
        assert!(observe(Status::Working).is_empty());
        assert_eq!(observe(Status::Failed), [Cue::Attention]);
    }

    #[test]
    fn an_untitled_chat_is_named_for_the_app() {
        let mut notices = Notices::default();
        let untitled = |status| vec![("c2".to_owned(), "  ".to_owned(), status)];
        notices.observe(untitled(Status::Working), false);
        assert_eq!(
            notices.observe(untitled(Status::Question), false)[0].title,
            "OpenAgents"
        );
    }
}
