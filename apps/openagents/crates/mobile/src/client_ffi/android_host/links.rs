//! Account-fenced review of links classified by the existing Android link contract.
use super::*;
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use serde::Serialize;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Serialize)]
enum Intent {
    Confirm,
    Close,
}
struct Review {
    instance: String,
    load_started: bool,
    revision: u64,
    title: String,
    body: String,
    confirm: Option<String>,
    loading: bool,
    busy: bool,
    terminal: bool,
    pending: Option<ValidatedView<Intent>>,
    applied: Option<ValidatedView<Intent>>,
}
impl Review {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            instance: format!(
                "android-link-review-{}",
                NEXT.fetch_add(1, Ordering::Relaxed)
            ),
            load_started: false,
            revision: 0,
            title: "Open link".into(),
            body: "Checking this link…".into(),
            confirm: None,
            loading: true,
            busy: false,
            terminal: false,
            pending: None,
            applied: None,
        }
    }
    fn frame(&mut self) -> CoreResult<String> {
        self.revision = self.revision.checked_add(1).ok_or(CoreError::Closed)?;
        let mut children = vec![
            node(
                "title",
                Element::Text {
                    value: self.title.clone(),
                    role: TextRole::Heading,
                },
            ),
            node(
                "body",
                Element::Text {
                    value: self.body.clone(),
                    role: TextRole::Body,
                },
            ),
        ];
        if let Some(label) = &self.confirm {
            children.push(button(
                "confirm",
                label,
                !self.busy && !self.loading && !self.terminal,
                Intent::Confirm,
            ));
        }
        children.push(button("close", "Close", true, Intent::Close));
        let view = View::new_v3(
            &self.instance,
            self.revision,
            node(
                "root",
                Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            ),
        )
        .validate()
        .map_err(|_| CoreError::Closed)?;
        let bytes = view.to_json().map_err(|_| CoreError::Closed)?;
        self.pending = Some(view);
        Ok(String::from_utf8(bytes).map_err(|_| CoreError::Closed)?)
    }
    fn ready(&mut self, title: &str, body: String, confirm: &str) {
        self.loading = false;
        self.title = title.into();
        self.body = body;
        self.confirm = Some(confirm.into());
    }
    fn finish(&mut self, message: &str) {
        self.loading = false;
        self.busy = false;
        self.terminal = true;
        self.confirm = None;
        self.body = message.into();
        self.applied = None;
        self.pending = None;
    }
    fn activate(&mut self, instance: String, revision: u64, key: String) -> CoreResult<Intent> {
        let intent = *self
            .applied
            .as_ref()
            .ok_or(CoreError::Closed)?
            .activate(&Activation {
                instance,
                revision,
                node: key,
            })
            .map_err(|_| CoreError::Closed)?;
        if matches!(intent, Intent::Confirm) {
            if self.loading || self.busy || self.terminal || self.confirm.is_none() {
                return Err(CoreError::Closed);
            }
            self.busy = true;
            self.applied = None;
            self.pending = None;
        }
        Ok(intent)
    }
}
fn node(key: &str, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}
fn button(key: &str, label: &str, enabled: bool, intent: Intent) -> Node<Intent> {
    node(
        key,
        Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut: None,
            intent,
        },
    )
}
fn checked_label(value: String, limit: usize) -> CoreResult<String> {
    if value.is_empty() || value.len() > limit || value.chars().any(char::is_control) {
        return Err(CoreError::Closed);
    }
    Ok(value)
}

#[derive(uniffi::Object)]
pub struct CoreAndroidLinkReview {
    account: Arc<CoreAccount>,
    client: Arc<CoreClient>,
    generation: u64,
    raw: String,
    dispatch: AndroidAppLinkDispatch,
    cancel: CancellationToken,
    listener: Arc<dyn AndroidHostListener>,
    state: Mutex<Review>,
}
impl CoreAndroidLinkReview {
    pub(super) fn new(
        account: Arc<CoreAccount>,
        client: Arc<CoreClient>,
        raw: String,
        listener: Arc<dyn AndroidHostListener>,
    ) -> CoreResult<Arc<Self>> {
        let dispatch = dispatch_android_app_link(raw.clone()).ok_or(CoreError::Closed)?;
        Ok(Arc::new(Self {
            generation: account.snapshot().generation,
            account,
            client,
            raw,
            dispatch,
            cancel: CancellationToken::new(),
            listener,
            state: Mutex::new(Review::new()),
        }))
    }
    fn current(&self) -> CoreResult<()> {
        if self.cancel.is_cancelled() || self.account.snapshot().generation != self.generation {
            Err(CoreError::Closed)
        } else {
            Ok(())
        }
    }
    fn code(&self) -> CoreResult<String> {
        self.dispatch.code.clone().ok_or(CoreError::Closed)
    }
}
#[uniffi::export]
impl CoreAndroidLinkReview {
    pub fn id(&self) -> CoreResult<String> {
        self.current()?;
        Ok(self
            .state
            .lock()
            .map_err(|_| CoreError::Closed)?
            .instance
            .clone())
    }
    pub fn snapshot(&self) -> CoreResult<String> {
        self.current()?;
        self.state.lock().map_err(|_| CoreError::Closed)?.frame()
    }
    pub fn acknowledge(&self, instance: String, revision: u64) -> CoreResult<()> {
        self.current()?;
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        let pending = state.pending.as_ref().ok_or(CoreError::Closed)?;
        if pending.view().instance != instance || pending.view().revision != revision {
            return Err(CoreError::Closed);
        }
        state.applied = state.pending.take();
        Ok(())
    }
    /// Preview is read-only. None of these branches approves or accepts the link.
    pub async fn load(&self) -> CoreResult<()> {
        self.current()?;
        {
            let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
            if !state.loading || state.load_started {
                return Ok(());
            }
            state.load_started = true;
        }
        let preview: CoreResult<(&str, String, &str)> = async {
            match self.dispatch.operation {
                AndroidAppLinkOperation::PairComputer => Ok((
                    "Connect computer",
                    "Connect this computer to your account?".into(),
                    "Connect",
                )),
                AndroidAppLinkOperation::ReviewWebsiteSignIn => {
                    let browser =
                        checked_label(self.account.scanned_login(self.code()?).await?, 300)?;
                    Ok((
                        "Sign in to the website",
                        format!("Sign in to {browser} with your account?"),
                        "Sign in",
                    ))
                }
                AndroidAppLinkOperation::ReviewProjectInvitation => {
                    let preview = self
                        .account
                        .preview_project_invitation(self.raw.clone())
                        .await?;
                    let project = checked_label(preview.project_name, 256)?;
                    let role = checked_label(preview.role, 64)?;
                    if preview.already_member {
                        return Ok((
                            "Project invitation",
                            format!("You're already a member of {project}."),
                            "",
                        ));
                    }
                    Ok((
                        "Project invitation",
                        format!("Join {project} as {role}?"),
                        "Join project",
                    ))
                }
            }
        }
        .await;
        self.current()?;
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        if !state.loading {
            return Ok(());
        }
        match preview {
            Ok((title, body, "")) => {
                state.title = title.into();
                state.finish(&body);
            }
            Ok((title, body, confirm)) => state.ready(title, body, confirm),
            Err(_) => state.finish("Couldn't check this link. Open it again to try again."),
        }
        drop(state);
        self.listener.on_android_host_changed();
        Ok(())
    }
    /// Only a current, acknowledged Rust Native confirmation starts a mutation.
    pub async fn activate(&self, instance: String, revision: u64, key: String) -> CoreResult<bool> {
        self.current()?;
        let intent = self
            .state
            .lock()
            .map_err(|_| CoreError::Closed)?
            .activate(instance, revision, key)?;
        if matches!(intent, Intent::Close) {
            self.retire();
            return Ok(true);
        }
        // Recheck time-bounded connect/login codes at the moment of consent.
        if dispatch_android_app_link(self.raw.clone()).is_none() {
            self.state
                .lock()
                .map_err(|_| CoreError::Closed)?
                .finish("This link has expired. Ask for a new one.");
            self.listener.on_android_host_changed();
            return Ok(false);
        }
        self.current()?;
        let result: CoreResult<&str> = async {
            match self.dispatch.operation {
                AndroidAppLinkOperation::PairComputer => {
                    let connected = self.client.connect_computer(self.code()?).await?;
                    self.current()?;
                    self.account
                        .link_computer(self.client.clone(), connected.device_id)
                        .await?;
                    Ok("Computer connected.")
                }
                AndroidAppLinkOperation::ReviewWebsiteSignIn => {
                    self.account.approve_scanned_login(self.code()?).await?;
                    Ok("You're signed in on the website.")
                }
                AndroidAppLinkOperation::ReviewProjectInvitation => {
                    self.account
                        .accept_project_invitation(self.raw.clone())
                        .await?;
                    Ok("You've joined the project.")
                }
            }
        }
        .await;
        self.current()?;
        self.state
            .lock()
            .map_err(|_| CoreError::Closed)?
            .finish(match result {
                Ok(message) => message,
                Err(_) => "Couldn't finish. Open the link again to try again.",
            });
        self.listener.on_android_host_changed();
        Ok(false)
    }
    pub fn retire(&self) {
        self.cancel.cancel();
        if let Ok(mut state) = self.state.lock() {
            state.finish("This link has been closed.");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn mount(state: &mut Review) -> (String, u64) {
        let json: serde_json::Value = serde_json::from_str(&state.frame().unwrap()).unwrap();
        state.applied = state.pending.take();
        (
            json["instance"].as_str().unwrap().into(),
            json["revision"].as_u64().unwrap(),
        )
    }
    #[test]
    fn preview_cannot_confirm_until_ready_and_acknowledged() {
        let mut state = Review::new();
        let (id, revision) = mount(&mut state);
        assert!(state.activate(id, revision, "confirm".into()).is_err());
        state.ready(
            "Project invitation",
            "Join Fixture as member?".into(),
            "Join project",
        );
        state.frame().unwrap();
        assert!(
            state
                .activate("foreign".into(), 1, "confirm".into())
                .is_err()
        );
    }
    #[test]
    fn duplicate_confirmation_and_terminal_actions_are_rejected() {
        let mut state = Review::new();
        state.ready(
            "Connect computer",
            "Connect this computer?".into(),
            "Connect",
        );
        let (id, revision) = mount(&mut state);
        assert!(matches!(
            state
                .activate(id.clone(), revision, "confirm".into())
                .unwrap(),
            Intent::Confirm
        ));
        assert!(state.activate(id, revision, "confirm".into()).is_err());
        state.finish("Computer connected.");
        let (id, revision) = mount(&mut state);
        assert!(
            state
                .activate(id.clone(), revision, "confirm".into())
                .is_err()
        );
        assert!(matches!(
            state.activate(id, revision, "close".into()).unwrap(),
            Intent::Close
        ));
    }
    #[test]
    fn replacement_views_and_reviews_reject_old_confirmation_keys() {
        let mut old = Review::new();
        old.ready(
            "Sign in to the website",
            "Sign in to Fixture browser?".into(),
            "Sign in",
        );
        let (id, revision) = mount(&mut old);
        let (_, replacement_revision) = mount(&mut old);
        assert!(replacement_revision > revision);
        assert!(
            old.activate(id.clone(), revision, "confirm".into())
                .is_err()
        );
        let mut replacement = Review::new();
        replacement.ready(
            "Connect computer",
            "Connect Fixture computer?".into(),
            "Connect",
        );
        let (replacement_id, _) = mount(&mut replacement);
        assert_ne!(id, replacement_id);
        assert!(
            replacement
                .activate(id, replacement_revision, "confirm".into())
                .is_err()
        );
    }
    #[test]
    fn remote_preview_labels_are_bounded() {
        assert!(checked_label("Fixture browser".into(), 300).is_ok());
        assert!(checked_label("x".repeat(301), 300).is_err());
        assert!(checked_label("browser\n".into(), 300).is_err());
    }
}
