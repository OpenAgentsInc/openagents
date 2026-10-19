//! Retained Android account, navigation, and chat ownership. Activities borrow these handles.
mod links;
mod notifications;
pub use notifications::AndroidNotificationOpen;
use notifications::PendingTap;
mod wallet;
use super::*;
pub use links::CoreAndroidLinkReview;
use openagents_account::openagents::Deployment;
use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
pub use wallet::{AndroidWalletFrame, CoreAndroidWallet};

#[uniffi::export(with_foreign)]
pub trait AndroidHostListener: Send + Sync {
    fn on_android_host_changed(&self);
}
struct Events(Arc<dyn AndroidHostListener>);
impl AccountListener for Events {
    fn on_account_changed(&self, _: AccountSnapshot) {
        self.0.on_android_host_changed();
    }
}
impl CloudChatListener for Events {
    fn on_cloud_chat_changed(&self, _: CloudChatSnapshot) {
        self.0.on_android_host_changed();
    }
}
impl ClientListener for Events {
    fn on_event(&self, _: ClientEvent) {
        self.0.on_android_host_changed();
    }
}
impl SharingListener for Events {
    fn on_sharing_changed(&self) {
        self.0.on_android_host_changed();
    }
}
impl VoiceAgentListener for Events {
    fn on_voice_agent_changed(&self) {
        self.0.on_android_host_changed();
    }
}
impl VoiceUsageListener for Events {
    fn on_voice_usage_changed(&self) {
        self.0.on_android_host_changed();
    }
}
impl crate::wallet::WalletListener for Events {
    fn on_wallet_changed(&self) {
        self.0.on_android_host_changed();
    }
}
impl SavedHistoryListener for Events {
    fn on_saved_history_changed(&self) {
        self.0.on_android_host_changed();
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, uniffi::Enum)]
pub enum AndroidHostSurface {
    Sessions,
    Settings,
    Recovery,
    Wallet,
    VoiceCapsule,
    VoiceScreen,
    VoiceAgent,
    VoiceUsage,
    Sharing,
    LinkReview,
    Composer,
}
impl AndroidHostSurface {
    fn scope(self) -> &'static str {
        match self {
            Self::Sessions => "android:sessions",
            Self::Settings => "android:settings",
            Self::Recovery => "android:recovery",
            Self::Wallet => "android:wallet",
            Self::VoiceCapsule => "android:voice-capsule",
            Self::VoiceScreen => "android:voice-screen",
            Self::VoiceAgent => "android:voice-agent",
            Self::VoiceUsage => "android:voice-usage",
            Self::Sharing => "android:sharing",
            Self::LinkReview => "android:link-review",
            Self::Composer => "android:composer",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidHostRoute {
    Chat,
    Sessions,
    Settings,
    Recovery,
    Voice,
    LinkReview,
    Wallet,
    VoiceSettings,
    Sharing,
}
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct AndroidTranscriptAnchor {
    pub row: u64,
    pub inside: f32,
    pub following: bool,
}
impl AndroidTranscriptAnchor {
    fn valid(&self) -> bool {
        self.inside.is_finite() && (0.0..=1_000_000.0).contains(&self.inside)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidHostContent {
    Cloud,
    Live,
    Saved,
    Indexed,
}
#[derive(Clone, uniffi::Enum)]
pub enum AndroidTranscriptEffect {
    Handled,
    Copy {
        text: String,
    },
    Media {
        row: u64,
        widget: u32,
        kind: crate::layout::display::WidgetKind,
    },
    Unavailable,
}
#[derive(uniffi::Record)]
pub struct AndroidVoiceSettings {
    pub generation: u64,
    pub picker: Arc<CoreVoiceAgent>,
    pub usage: Arc<CoreVoiceUsage>,
}
#[derive(Clone, uniffi::Record)]
pub struct AndroidHostTextPoint {
    pub row: u64,
    pub utf16: u32,
}
#[derive(Clone, uniffi::Record)]
pub struct AndroidHostSelection {
    pub anchor: AndroidHostTextPoint,
    pub focus: AndroidHostTextPoint,
}
#[derive(Clone, uniffi::Record)]
pub struct AndroidHostSnapshot {
    pub generation: u64,
    pub route: AndroidHostRoute,
    pub content_revision: u64,
    pub selected_chat: Option<String>,
    pub source: AndroidHostContent,
    pub signed_in: bool,
    pub notice: Option<String>,
    pub composer: Option<Arc<CoreAndroidComposer>>,
    pub sessions: Option<Arc<CoreAndroidSessions>>,
    pub client: Option<Arc<CoreClient>>,
    pub history: Option<Arc<CoreSavedHistory>>,
    pub account: Option<Arc<CoreAccount>>,
    pub anchor: Option<AndroidTranscriptAnchor>,
    pub recovery: Option<Arc<CoreAndroidRecovery>>,
    pub selection: Option<AndroidHostSelection>,
    pub active_voice: Option<Arc<VoiceCall>>,
    pub can_start_voice: bool,
    pub link_review: Option<Arc<CoreAndroidLinkReview>>,
    pub pending_notification: bool,
}
enum Content {
    Cloud(Arc<CoreCloudChat>),
    Live {
        id: String,
        client: Arc<CoreClient>,
        session: Arc<SessionHandle>,
    },
    Saved {
        id: String,
        history: Arc<CoreSavedHistory>,
    },
    Indexed {
        row: SavedIndexedRow,
        history: Arc<CoreSavedHistory>,
        input: crate::layout::TranscriptInput,
    },
}
struct State {
    pending_notification: Option<PendingTap>,
    selected_notification: Option<String>,
    wallet: Option<Arc<CoreAndroidWallet>>,
    pending_link: Option<(Option<u64>, String)>,
    link_review: Option<Arc<CoreAndroidLinkReview>>,
    voice_notice: Option<String>,
    active_voice: Option<Arc<VoiceCall>>,
    selection: Option<AndroidHostSelection>,
    foreground: bool,
    semantics: BTreeMap<AndroidHostSurface, Arc<CoreAccessibilitySurface>>,
    navigation_revision: u64,
    anchor: Option<AndroidTranscriptAnchor>,
    generation: u64,
    content_revision: u64,
    route: AndroidHostRoute,
    sessions: Option<Arc<CoreAndroidSessions>>,
    client: Option<Arc<CoreClient>>,
    history: Option<Arc<CoreSavedHistory>>,
    content: Content,
    composer: Option<Arc<CoreAndroidComposer>>,
}
struct NavigationStamp {
    generation: u64,
    content: u64,
    request: u64,
}
impl NavigationStamp {
    fn accepts(&self, state: &State) -> bool {
        self.generation == state.generation
            && self.content == state.content_revision
            && self.request == state.navigation_revision
    }
}
impl State {
    fn voice_target(&self) -> Option<(Arc<CoreClient>, String)> {
        let Content::Live { client, id, .. } = &self.content else {
            return None;
        };
        let row = client.session_row(id.clone())?;
        client
            .devices()
            .iter()
            .find(|device| voice_device_matches(&row.device_id, device))?;
        Some((client.clone(), row.device_id))
    }
    fn apply_visibility(&self) {
        let visible = self.foreground && self.route == AndroidHostRoute::Chat;
        if let Content::Live { session, .. } = &self.content {
            session.set_view_attached(visible);
        }
        if let Some(history) = &self.history {
            history.follow(visible && matches!(self.content, Content::Saved { .. }));
        }
    }
    fn retire_content(&self) {
        if let Content::Live { session, .. } = &self.content {
            session.set_view_attached(false);
        }
        if let Content::Saved { history, .. } = &self.content {
            history.follow(false);
        }
    }
    fn begin_navigation(&mut self) -> CoreResult<NavigationStamp> {
        self.pending_notification = None;
        self.navigation_revision = self
            .navigation_revision
            .checked_add(1)
            .ok_or(CoreError::Closed)?;
        Ok(NavigationStamp {
            generation: self.generation,
            content: self.content_revision,
            request: self.navigation_revision,
        })
    }
}
#[derive(uniffi::Object)]
pub struct CoreAndroidHost {
    account: Option<Arc<CoreAccount>>,
    current_storage: Option<Arc<dyn AndroidSecretStorage>>,
    wallet_core: Mutex<Option<Arc<crate::wallet::CoreWallet>>>,
    recovery: Option<Arc<CoreAndroidRecovery>>,
    config: CoreConfig,
    events: Arc<Events>,
    state: Mutex<State>,
    started: AtomicBool,
    start_failed: AtomicBool,
}
impl CoreAndroidHost {
    fn refresh(&self, state: &mut State) -> CoreResult<()> {
        let Some(account) = &self.account else {
            return Ok(());
        };
        let snapshot = account.snapshot();
        if snapshot.generation != state.generation {
            state.selected_notification = None;
            if state
                .pending_notification
                .as_ref()
                .is_some_and(|tap| !tap.accepts(snapshot.generation))
            {
                state.pending_notification = None;
            }
            if let Some(wallet) = state.wallet.take() {
                wallet.redact();
            }
            if let Some(review) = state.link_review.take() {
                review.retire();
            }
            if state
                .pending_link
                .as_ref()
                .is_some_and(|(expected, _)| expected.is_some())
            {
                state.pending_link = None;
            }
            if let Some(call) = state.active_voice.take() {
                call.stop();
            }
            state.voice_notice = None;
            if let Some(composer) = state.composer.take() {
                composer.dispose();
            }
            if let Some(sessions) = state.sessions.take() {
                sessions.close();
            }
            if let Some(history) = state.history.take() {
                history.shutdown();
            }
            for (_, surface) in std::mem::take(&mut state.semantics) {
                surface.close();
            }
            state.retire_content();
            state.client = None;
            let sessions = CoreAndroidSessions::new(account.clone(), self.events.clone());
            let cloud = sessions.default_chat()?;
            state.content = Content::Cloud(cloud.clone());
            state.composer = Some(CoreAndroidComposer::cloud(cloud, Some(account.clone())));
            if let Some(composer) = &state.composer {
                composer.bind_listener(self.events.0.clone());
            }
            state.sessions = Some(sessions);
            state.anchor = None;
            state.selection = None;
            state.generation = snapshot.generation;
            state.content_revision = state
                .content_revision
                .checked_add(1)
                .ok_or(CoreError::Closed)?;
            if state.route != AndroidHostRoute::Recovery {
                state.route = AndroidHostRoute::Chat;
            }
        }
        if matches!(snapshot.state, AccountState::SignedIn { .. }) && state.client.is_none() {
            let core = account.connect(self.config.clone(), self.events.clone())?;
            state
                .sessions
                .as_ref()
                .ok_or(CoreError::Closed)?
                .attach_client(core.clone())?;
            state.history = Some(account.saved_history(self.events.clone())?);
            if let Some(composer) = &state.composer {
                if matches!(state.content, Content::Cloud(_)) {
                    composer.bind_drafts(core.composer_drafts.clone())?;
                }
            }
            state.client = Some(core);
        }
        if state.route != AndroidHostRoute::Recovery
            && matches!(snapshot.state, AccountState::SignedIn { .. })
        {
            if let Some(client) = state.client.clone() {
                if let Some((expected, raw)) = state.pending_link.take() {
                    if expected.is_none_or(|generation| generation == state.generation) {
                        state.link_review = Some(CoreAndroidLinkReview::new(
                            account.clone(),
                            client,
                            raw,
                            self.events.0.clone(),
                        )?);
                        state.route = AndroidHostRoute::LinkReview;
                        state.apply_visibility();
                    }
                }
            }
        }
        Ok(())
    }
    fn replace(
        &self,
        state: &mut State,
        content: Content,
        composer: Option<Arc<CoreAndroidComposer>>,
    ) -> CoreResult<()> {
        let revision = state
            .content_revision
            .checked_add(1)
            .ok_or(CoreError::Closed)?;
        if let Some(composer) = &composer {
            composer.bind_listener(self.events.0.clone());
            if matches!(&content, Content::Cloud(_)) {
                if let Some(client) = &state.client {
                    composer.bind_drafts(client.composer_drafts.clone())?;
                }
            }
        }
        if let Some(previous) = state.composer.take() {
            previous.dispose();
        }
        if let Some(review) = state.link_review.take() {
            review.retire();
        }
        if let Some(wallet) = &state.wallet {
            wallet.redact();
        }
        state.retire_content();
        state.content = content;
        state.pending_notification = None;
        state.selected_notification = None;
        state.anchor = None;
        state.selection = None;
        state.composer = composer;
        state.content_revision = revision;
        state.route = AndroidHostRoute::Chat;
        state.apply_visibility();
        Ok(())
    }
}
#[uniffi::export]
impl CoreAndroidHost {
    #[uniffi::constructor]
    pub fn production(
        data_dir: String,
        device_id: String,
        device_name: String,
        version: String,
        storage: Arc<dyn AndroidSecretStorage>,
        listener: Arc<dyn AndroidHostListener>,
    ) -> CoreResult<Arc<Self>> {
        let events = Arc::new(Events(listener));
        let deployment = Deployment::production();
        let account = CoreAccount::new_with_secure_storage(
            data_dir.clone(),
            deployment.api_base().to_string(),
            deployment.website().to_string(),
            events.clone(),
            storage.clone(),
        )?;
        let generation = account.snapshot().generation;
        let sessions = CoreAndroidSessions::new(account.clone(), events.clone());
        let cloud = sessions.default_chat()?;
        let composer = CoreAndroidComposer::cloud(cloud.clone(), Some(account.clone()));
        let onboarding = CoreOnboarding::new(data_dir.clone());
        let route = if onboarding.should_show() {
            AndroidHostRoute::Recovery
        } else {
            AndroidHostRoute::Chat
        };
        let recovery = CoreAndroidRecovery::new(account.clone(), onboarding);
        composer.bind_listener(events.0.clone());
        Ok(Arc::new(Self {
            account: Some(account),
            current_storage: Some(storage),
            wallet_core: Mutex::new(None),
            recovery: Some(recovery),
            config: CoreConfig {
                edge_url: production_edge_url(),
                data_dir,
                device_id,
                device_name,
                platform: "android".into(),
                app_version: version,
            },
            events,
            state: Mutex::new(State {
                pending_notification: None,
                selected_notification: None,
                wallet: None,
                pending_link: None,
                link_review: None,
                voice_notice: None,
                active_voice: None,
                selection: None,
                foreground: true,
                semantics: BTreeMap::new(),
                navigation_revision: 0,
                anchor: None,
                generation,
                content_revision: 1,
                route,
                sessions: Some(sessions),
                client: None,
                history: None,
                content: Content::Cloud(cloud),
                composer: Some(composer),
            }),
            started: AtomicBool::new(false),
            start_failed: AtomicBool::new(false),
        }))
    }
    #[uniffi::constructor]
    pub fn demo(listener: Arc<dyn AndroidHostListener>) -> Arc<Self> {
        let events = Arc::new(Events(listener));
        let cloud = CoreCloudChat::demo(events.clone());
        let composer = CoreAndroidComposer::cloud(cloud.clone(), None);
        composer.bind_listener(events.0.clone());
        Arc::new(Self {
            account: None,
            current_storage: None,
            wallet_core: Mutex::new(None),
            recovery: None,
            config: CoreConfig {
                edge_url: String::new(),
                data_dir: String::new(),
                device_id: String::new(),
                device_name: String::new(),
                platform: "android".into(),
                app_version: String::new(),
            },
            events,
            state: Mutex::new(State {
                pending_notification: None,
                selected_notification: None,
                wallet: None,
                pending_link: None,
                link_review: None,
                voice_notice: None,
                active_voice: None,
                selection: None,
                foreground: true,
                semantics: BTreeMap::new(),
                navigation_revision: 0,
                anchor: None,
                generation: 0,
                content_revision: 1,
                route: AndroidHostRoute::Chat,
                sessions: None,
                client: None,
                history: None,
                content: Content::Cloud(cloud),
                composer: Some(composer),
            }),
            started: AtomicBool::new(false),
            start_failed: AtomicBool::new(false),
        })
    }
    /// Called by the Application once. Activity recreation never starts another identity flow.
    pub fn start(&self) {
        if !self.started.swap(true, Ordering::AcqRel) {
            if let Some(recovery) = &self.recovery {
                // Recovery owns the once-only identity-first start and existing seed policy.
                if recovery.start(self.config.device_name.clone()).is_err() {
                    self.start_failed.store(true, Ordering::Release);
                    self.events.0.on_android_host_changed();
                }
            }
        }
    }
    pub fn snapshot(&self) -> CoreResult<AndroidHostSnapshot> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        let account = self.account.as_ref().map(|a| a.snapshot());
        let signed_in = account
            .as_ref()
            .is_none_or(|a| matches!(a.state, AccountState::SignedIn { .. }));
        let notice = match account.map(|a| a.state) {
            Some(AccountState::Failed { message, .. }) => Some(message),
            Some(AccountState::SignedOut { notice }) => notice,
            Some(AccountState::Starting | AccountState::Restoring) => Some("Signing in…".into()),
            Some(AccountState::Waiting { .. }) => Some("Finish signing in to continue.".into()),
            _ => None,
        }
        .or_else(|| {
            self.start_failed
                .load(Ordering::Acquire)
                .then(|| "Couldn't start the app. Reopen it to try again.".into())
        });
        Ok(AndroidHostSnapshot {
            generation: s.generation,
            route: s.route,
            content_revision: s.content_revision,
            selected_chat: match &s.content {
                Content::Cloud(c) => c.snapshot().chat,
                Content::Live { id, .. } | Content::Saved { id, .. } => Some(id.clone()),
                Content::Indexed { row, .. } => Some(row.key.clone()),
            },
            source: match &s.content {
                Content::Cloud(_) => AndroidHostContent::Cloud,
                Content::Live { .. } => AndroidHostContent::Live,
                Content::Saved { .. } => AndroidHostContent::Saved,
                Content::Indexed { .. } => AndroidHostContent::Indexed,
            },
            signed_in,
            notice: notice.or_else(|| s.voice_notice.clone()),
            composer: s.composer.clone(),
            sessions: s.sessions.clone(),
            client: s.client.clone(),
            history: s.history.clone(),
            account: self.account.clone(),
            anchor: s.anchor.clone(),
            recovery: self.recovery.clone(),
            selection: s.selection.clone(),
            active_voice: s.active_voice.clone(),
            can_start_voice: s.active_voice.is_none() && s.voice_target().is_some(),
            link_review: s.link_review.clone(),
            pending_notification: s.pending_notification.is_some(),
        })
    }
    /// Android uses the same initial session-notification choices as the other phone shell.
    pub fn default_push_prefs(&self) -> PushPrefs {
        PushPrefs {
            done: true,
            input: true,
            failed: true,
        }
    }
    pub fn offer_notification_tap(&self, link: String, request: String) -> CoreResult<bool> {
        let Some(account) = &self.account else {
            return Ok(false);
        };
        let generation = account.snapshot();
        let generation = matches!(generation.state, AccountState::SignedIn { .. })
            .then_some(generation.generation);
        let Some(tap) = PendingTap::new(link, request, generation) else {
            return Ok(false);
        };
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        state.begin_navigation()?;
        state.pending_notification = Some(tap);
        drop(state);
        self.events.0.on_android_host_changed();
        Ok(true)
    }
    pub async fn resolve_notification_tap(&self) -> CoreResult<AndroidNotificationOpen> {
        let (client, request, link, navigation) = {
            let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
            self.refresh(&mut state)?;
            let Some(client) = state.client.clone() else {
                return Ok(AndroidNotificationOpen {
                    retry_after_ms: None,
                });
            };
            let generation = state.generation;
            let Some(tap) = state.pending_notification.as_mut() else {
                return Ok(AndroidNotificationOpen {
                    retry_after_ms: None,
                });
            };
            if !tap.accepts(generation) || tap.attempts >= 10 {
                state.pending_notification = None;
                return Ok(AndroidNotificationOpen {
                    retry_after_ms: None,
                });
            }
            tap.attempts += 1;
            let request = tap.request.clone();
            let link = tap.link.clone();
            (
                client,
                request,
                link,
                NavigationStamp {
                    generation: state.generation,
                    content: state.content_revision,
                    request: state.navigation_revision,
                },
            )
        };
        let note = client.owner_notification().await?;
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if !navigation.accepts(&state)
            || !state
                .pending_notification
                .as_ref()
                .is_some_and(|tap| tap.request == request && tap.link == link)
        {
            return Err(CoreError::Closed);
        }
        let Some(note) = note else {
            return Ok(AndroidNotificationOpen {
                retry_after_ms: Some(1000),
            });
        };
        if !state
            .pending_notification
            .as_ref()
            .is_some_and(|tap| tap.matches(&note))
        {
            state.pending_notification = None;
            return Err(CoreError::NotFound {
                message: "This notification is no longer available.".into(),
            });
        }
        // Hydration may lag a cold wake. Only an already authorized session can open.
        let session = match client.open_session(note.chat_id.clone()) {
            Ok(session) => session,
            Err(_) => {
                return Ok(AndroidNotificationOpen {
                    retry_after_ms: Some(1000),
                });
            }
        };
        let composer = CoreAndroidComposer::live(session.clone());
        self.replace(
            &mut state,
            Content::Live {
                id: note.chat_id,
                client,
                session,
            },
            Some(composer),
        )?;
        state.selected_notification = Some(request);
        drop(state);
        self.events.0.on_android_host_changed();
        Ok(AndroidNotificationOpen {
            retry_after_ms: None,
        })
    }
    pub fn sharing(&self) -> CoreResult<Arc<CoreSharing>> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        self.account
            .as_ref()
            .ok_or(CoreError::Closed)?
            .project_sharing(self.events.clone())
    }
    pub fn voice_settings(&self) -> CoreResult<AndroidVoiceSettings> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        let account = self.account.as_ref().ok_or(CoreError::Closed)?;
        let client = state.client.as_ref().ok_or(CoreError::Closed)?;
        let picker = account.voice_agent(self.events.clone());
        let usage = client.voice_usage(self.events.clone());
        usage.track(selected_voice_host(&state), state.active_voice.clone());
        Ok(AndroidVoiceSettings {
            generation: state.generation,
            picker,
            usage,
        })
    }
    pub fn sync_voice_usage(&self, generation: u64, usage: Arc<CoreVoiceUsage>) -> CoreResult<()> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if state.generation != generation {
            return Err(CoreError::Closed);
        }
        usage.track(selected_voice_host(&state), state.active_voice.clone());
        Ok(())
    }
    pub async fn activate_voice_agent(
        &self,
        generation: u64,
        picker: Arc<CoreVoiceAgent>,
        instance: String,
        revision: u64,
        key: String,
    ) -> CoreResult<()> {
        {
            let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
            self.refresh(&mut state)?;
            if state.generation != generation {
                return Err(CoreError::Closed);
            }
        }
        picker.activate(instance, revision, key).await?;
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if state.generation != generation {
            return Err(CoreError::Closed);
        }
        Ok(())
    }
    pub fn activate_voice_usage(
        &self,
        generation: u64,
        usage: Arc<CoreVoiceUsage>,
        instance: String,
        revision: u64,
        key: String,
    ) -> CoreResult<()> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if state.generation != generation {
            return Err(CoreError::Closed);
        }
        usage.activate_view(instance, revision, key)
    }
    /// The underlying device wallet is shared across native mounts; account wrappers are fenced.
    pub fn wallet(
        &self,
        retired: Arc<dyn AndroidRetiredWalletSeed>,
    ) -> CoreResult<Arc<CoreAndroidWallet>> {
        let account = self.account.clone().ok_or(CoreError::Closed)?;
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if !matches!(account.snapshot().state, AccountState::SignedIn { .. }) {
            return Err(CoreError::Closed);
        }
        if let Some(wallet) = &state.wallet {
            return Ok(wallet.clone());
        }
        let storage = self.current_storage.clone().ok_or(CoreError::Closed)?;
        let mut shared = self.wallet_core.lock().map_err(|_| CoreError::Closed)?;
        let core = shared
            .get_or_insert_with(|| {
                crate::wallet::CoreWallet::android(
                    std::path::Path::new(&self.config.data_dir)
                        .join("spark")
                        .to_string_lossy()
                        .into_owned(),
                    storage,
                    retired,
                    self.events.clone(),
                )
            })
            .clone();
        let wallet = CoreAndroidWallet::new(account, core);
        state.wallet = Some(wallet.clone());
        Ok(wallet)
    }
    /// One bounded pending link can survive initial sign-in, without moving codes into Kotlin state.
    pub fn offer_app_link(&self, raw: String) -> CoreResult<bool> {
        let Some(account) = &self.account else {
            return Ok(false);
        };
        if dispatch_android_app_link(raw.clone()).is_none() {
            return Ok(false);
        }
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        state.begin_navigation()?;
        if let Some(review) = state.link_review.take() {
            review.retire();
        }
        let expected = matches!(account.snapshot().state, AccountState::SignedIn { .. })
            .then_some(state.generation);
        state.pending_link = Some((expected, raw));
        drop(state);
        self.events.0.on_android_host_changed();
        Ok(true)
    }
    pub fn dismiss_link_review(&self, review: Arc<CoreAndroidLinkReview>) -> CoreResult<bool> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if !state
            .link_review
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &review))
        {
            return Ok(false);
        }
        review.retire();
        state.link_review = None;
        if state.route == AndroidHostRoute::LinkReview {
            state.route = AndroidHostRoute::Chat;
        }
        state.apply_visibility();
        drop(state);
        self.events.0.on_android_host_changed();
        Ok(true)
    }
    /// Permission callbacks carry the account/content that requested the microphone.
    pub fn start_host_voice(
        &self,
        generation: u64,
        content_revision: u64,
        media: Arc<dyn VoiceMediaListener>,
        listener: Arc<dyn VoiceSessionListener>,
    ) -> CoreResult<Arc<VoiceCall>> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if state.generation != generation
            || state.content_revision != content_revision
            || state.active_voice.is_some()
            || !state.foreground
        {
            return Err(CoreError::Closed);
        }
        let (client, device) = state.voice_target().ok_or(CoreError::Closed)?;
        state.begin_navigation()?;
        if let Some(review) = state.link_review.take() {
            review.retire();
        }
        let call = client.start_voice(device, None, media, listener);
        state.voice_notice = None;
        state.active_voice = Some(call.clone());
        state.route = AndroidHostRoute::Voice;
        state.apply_visibility();
        drop(state);
        self.events.0.on_android_host_changed();
        Ok(call)
    }
    /// A late terminal callback cannot clear a replacement call or account.
    pub fn voice_ended(
        &self,
        generation: u64,
        call: Arc<VoiceCall>,
        reason: Option<VoiceEndReason>,
    ) -> CoreResult<bool> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if state.generation != generation
            || !state
                .active_voice
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &call))
        {
            return Ok(false);
        }
        state.active_voice = None;
        state.voice_notice = reason.map(|reason| voice_end_notice(reason).into());
        if state.route == AndroidHostRoute::Voice {
            state.route = AndroidHostRoute::Chat;
        }
        state.apply_visibility();
        drop(state);
        self.events.0.on_android_host_changed();
        Ok(true)
    }
    /// Resolves a widget identity against the current Rust display, never a caller-supplied payload.
    pub fn transcript_widget(
        &self,
        view: Arc<crate::layout::TranscriptView>,
        generation: u64,
        content_revision: u64,
        frame_revision: u64,
        row: u64,
        widget: u32,
    ) -> CoreResult<AndroidTranscriptEffect> {
        use crate::layout::display::WidgetKind;
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        let frame = view.frame();
        if state.generation != generation
            || state.content_revision != content_revision
            || frame.revision() != frame_revision
        {
            return Err(CoreError::Closed);
        }
        let display = frame
            .index_of(row)
            .and_then(|i| frame.display(i))
            .ok_or(CoreError::Closed)?;
        let widget = display
            .widgets
            .into_iter()
            .find(|item| item.id == widget)
            .ok_or(CoreError::Closed)?;
        Ok(match widget.kind {
            WidgetKind::CopyCode => match widget.payload {
                Some(text) if text.len() <= 64 * 1024 => AndroidTranscriptEffect::Copy { text },
                _ => AndroidTranscriptEffect::Unavailable,
            },
            WidgetKind::Disclosure { .. } | WidgetKind::Chevron { .. } => {
                view.toggle(row);
                AndroidTranscriptEffect::Handled
            }
            kind @ (WidgetKind::Image { .. } | WidgetKind::LabMedia { .. }) => {
                AndroidTranscriptEffect::Media {
                    row,
                    widget: widget.id,
                    kind,
                }
            }
            WidgetKind::ToolToggle { detail, open } => {
                view.toggle_detail(row, detail, open);
                AndroidTranscriptEffect::Handled
            }
            _ => AndroidTranscriptEffect::Unavailable,
        })
    }
    /// Only a link in the current visible Rust rows may reach the platform browser.
    pub fn transcript_link(
        &self,
        view: Arc<crate::layout::TranscriptView>,
        generation: u64,
        content_revision: u64,
        frame_revision: u64,
        top: f32,
        height: f32,
        url: String,
    ) -> CoreResult<String> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        let frame = view.frame();
        if state.generation != generation
            || state.content_revision != content_revision
            || frame.revision() != frame_revision
            || !top.is_finite()
            || top < 0.0
            || !height.is_finite()
            || !(0.0..=100_000.0).contains(&height)
            || !browser_link(&url)
        {
            return Err(CoreError::Closed);
        }
        let found = frame
            .rows_in(top, top + height)
            .into_iter()
            .take(256)
            .any(|row| {
                frame
                    .display(row.index)
                    .is_some_and(|display| display.links.iter().any(|link| link.url == url))
            });
        if found {
            Ok(url)
        } else {
            Err(CoreError::Closed)
        }
    }
    pub fn save_text_selection(
        &self,
        view: Arc<crate::layout::TranscriptView>,
        generation: u64,
        content_revision: u64,
        frame_revision: u64,
        selection: Option<AndroidHostSelection>,
    ) -> CoreResult<()> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if state.generation != generation || state.content_revision != content_revision {
            return Err(CoreError::Closed);
        }
        let frame = view.frame();
        if frame.revision() != frame_revision {
            return Err(CoreError::Closed);
        }
        if let Some(selection) = &selection {
            for point in [&selection.anchor, &selection.focus] {
                let row = frame
                    .index_of(point.row)
                    .and_then(|index| frame.display(index))
                    .ok_or(CoreError::Closed)?;
                if !selection_boundary(&row.text, point.utf16 as usize) {
                    return Err(CoreError::Closed);
                }
            }
        }
        state.selection = selection;
        Ok(())
    }
    pub fn save_transcript_anchor(
        &self,
        generation: u64,
        content_revision: u64,
        anchor: AndroidTranscriptAnchor,
    ) -> CoreResult<()> {
        if !anchor.valid() {
            return Err(CoreError::InvalidArgument {
                message: "The scroll position could not be saved.".into(),
            });
        }
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if state.generation != generation || state.content_revision != content_revision {
            return Err(CoreError::Closed);
        }
        state.anchor = Some(anchor);
        Ok(())
    }
    pub fn set_foreground(&self, active: bool) -> CoreResult<()> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        state.foreground = active;
        if let Some(client) = &state.client {
            if active {
                client.on_foreground();
            } else {
                client.on_background();
            }
        }
        if let Some(history) = &state.history {
            history.set_foreground(active);
        }
        state.apply_visibility();
        Ok(())
    }
    pub fn accessibility_surface(
        &self,
        surface: AndroidHostSurface,
    ) -> CoreResult<Arc<CoreAccessibilitySurface>> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if let Some(existing) = state.semantics.get(&surface) {
            return Ok(existing.clone());
        }
        let semantics = CoreAccessibilitySurface::new(
            self.account.clone().ok_or(CoreError::Closed)?,
            surface.scope().into(),
        )?;
        state.semantics.insert(surface, semantics.clone());
        Ok(semantics)
    }
    pub fn account(&self) -> Option<Arc<CoreAccount>> {
        self.account.clone()
    }
    pub fn client(&self) -> CoreResult<Option<Arc<CoreClient>>> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        Ok(s.client.clone())
    }
    pub fn sessions(&self) -> CoreResult<Option<Arc<CoreAndroidSessions>>> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        Ok(s.sessions.clone())
    }
    pub fn history(&self) -> CoreResult<Option<Arc<CoreSavedHistory>>> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        Ok(s.history.clone())
    }
    pub fn composer(&self) -> CoreResult<Option<Arc<CoreAndroidComposer>>> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        Ok(s.composer.clone())
    }
    /// Binds Rust sources directly to the shared layout worker, without transcript rows crossing FFI.
    pub fn attach_transcript(
        &self,
        view: Arc<crate::layout::TranscriptView>,
        generation: u64,
        content_revision: u64,
    ) -> CoreResult<()> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        if s.generation != generation || s.content_revision != content_revision {
            return Err(CoreError::Closed);
        }
        match &s.content {
            Content::Cloud(chat) => {
                if view.attach_cloud(chat.clone()) {
                    Ok(())
                } else {
                    Err(CoreError::Closed)
                }
            }
            Content::Live { id, client, .. } => {
                if view.attach(client.clone(), id.clone()) {
                    Ok(())
                } else {
                    Err(CoreError::Closed)
                }
            }
            Content::Saved { history, .. } => history.render_transcript(view),
            Content::Indexed { input, .. } => {
                view.clear();
                view.set_input(input.clone());
                Ok(())
            }
        }
    }
    /// Refresh a saved reader without detaching a live/cloud layout subscription.
    pub fn refresh_transcript(
        &self,
        view: Arc<crate::layout::TranscriptView>,
        generation: u64,
        content_revision: u64,
    ) -> CoreResult<()> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        if s.generation != generation || s.content_revision != content_revision {
            return Err(CoreError::Closed);
        }
        if let Content::Saved { history, .. } = &s.content {
            history.render_transcript(view)?;
        }
        Ok(())
    }
    pub fn selected_session(&self) -> CoreResult<Option<Arc<SessionHandle>>> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        Ok(match &s.content {
            Content::Live { session, .. } => Some(session.clone()),
            _ => None,
        })
    }
    pub fn navigate(&self, route: AndroidHostRoute) -> CoreResult<()> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        s.begin_navigation()?;
        if self.account.is_none() && route != AndroidHostRoute::Chat {
            return Err(CoreError::Closed);
        }
        if route == AndroidHostRoute::Voice && s.active_voice.is_none() {
            return Err(CoreError::Closed);
        }
        if route == AndroidHostRoute::LinkReview && s.link_review.is_none() {
            return Err(CoreError::Closed);
        }
        if route != AndroidHostRoute::LinkReview {
            if let Some(review) = s.link_review.take() {
                review.retire();
            }
        }
        if route != AndroidHostRoute::Wallet {
            if let Some(wallet) = &s.wallet {
                wallet.redact();
            }
        }
        if route == AndroidHostRoute::Wallet
            && !matches!(
                self.account.as_ref().map(|a| a.snapshot().state),
                Some(AccountState::SignedIn { .. })
            )
        {
            return Err(CoreError::Closed);
        }
        if route == AndroidHostRoute::VoiceSettings && s.client.is_none() {
            return Err(CoreError::Closed);
        }
        s.route = route;
        s.apply_visibility();
        drop(s);
        self.events.0.on_android_host_changed();
        Ok(())
    }
    pub fn select_default_chat(&self) -> CoreResult<()> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        s.begin_navigation()?;
        let cloud = s
            .sessions
            .as_ref()
            .ok_or(CoreError::Closed)?
            .default_chat()?;
        let same = matches!(&s.content, Content::Cloud(c) if Arc::ptr_eq(c, &cloud));
        if !same {
            let composer = CoreAndroidComposer::cloud(cloud.clone(), self.account.clone());
            self.replace(&mut s, Content::Cloud(cloud), Some(composer))?;
        }
        s.route = AndroidHostRoute::Chat;
        s.apply_visibility();
        drop(s);
        self.events.0.on_android_host_changed();
        Ok(())
    }
    /// Preserve the exact cloud object returned by this account's session action.
    pub fn select_cloud_chat(&self, chat: Arc<CoreCloudChat>) -> CoreResult<()> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        state.begin_navigation()?;
        let owned = state
            .sessions
            .as_ref()
            .ok_or(CoreError::Closed)?
            .default_chat()?;
        if !Arc::ptr_eq(&chat, &owned) {
            return Err(CoreError::Closed);
        }
        let composer = CoreAndroidComposer::cloud(chat.clone(), self.account.clone());
        self.replace(&mut state, Content::Cloud(chat), Some(composer))?;
        drop(state);
        self.events.0.on_android_host_changed();
        Ok(())
    }
    pub fn select_session(&self, id: String) -> CoreResult<()> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        s.begin_navigation()?;
        if !matches!(&s.content, Content::Live { id: current, .. } if current == &id) {
            let client = s.client.clone().ok_or(CoreError::Closed)?;
            let session = client.open_session(id.clone())?;
            let composer = CoreAndroidComposer::live(session.clone());
            self.replace(
                &mut s,
                Content::Live {
                    id,
                    client,
                    session,
                },
                Some(composer),
            )?;
        }
        s.route = AndroidHostRoute::Chat;
        s.apply_visibility();
        drop(s);
        self.events.0.on_android_host_changed();
        Ok(())
    }
    pub async fn select_indexed_chat(&self, key: String) -> CoreResult<()> {
        let (history, navigation) = {
            let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
            self.refresh(&mut state)?;
            (
                state.history.clone().ok_or(CoreError::Closed)?,
                state.begin_navigation()?,
            )
        };
        let (row, input) = history.indexed_layout(key).await?;
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut state)?;
        if !navigation.accepts(&state) {
            return Err(CoreError::Closed);
        }
        self.replace(
            &mut state,
            Content::Indexed {
                row,
                history,
                input,
            },
            None,
        )?;
        drop(state);
        self.events.0.on_android_host_changed();
        Ok(())
    }
    /// The native view reports its first painted rows; Rust supplies the authorized read-marker identity.
    pub async fn transcript_mounted(&self, generation: u64, revision: u64) -> CoreResult<()> {
        let indexed = {
            let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
            self.refresh(&mut state)?;
            if state.generation != generation
                || state.content_revision != revision
                || !state.foreground
                || state.route != AndroidHostRoute::Chat
            {
                return Err(CoreError::Closed);
            }
            match &state.content {
                Content::Indexed { row, history, .. } => row
                    .activity_revision
                    .as_ref()
                    .map(|activity| (history.clone(), row.key.clone(), activity.clone())),
                Content::Live { client, id, .. } => {
                    client.mark_seen(id.clone());
                    None
                }
                _ => None,
            }
        };
        let notification = {
            let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
            self.refresh(&mut state)?;
            if state.generation != generation || state.content_revision != revision {
                return Err(CoreError::Closed);
            }
            state
                .client
                .clone()
                .zip(state.selected_notification.clone())
        };
        if let Some((client, request)) = notification {
            client.read_owner_notification(request.clone()).await?;
            let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
            self.refresh(&mut state)?;
            if state.generation == generation
                && state.content_revision == revision
                && state.selected_notification.as_deref() == Some(&request)
            {
                state.selected_notification = None;
            }
        }
        if let Some((history, key, activity)) = indexed {
            history.mark_indexed_seen(key, activity).await?;
        }
        Ok(())
    }
    pub fn select_saved_chat(&self, id: String) -> CoreResult<()> {
        let mut s = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.refresh(&mut s)?;
        s.begin_navigation()?;
        let history = s.history.clone().ok_or(CoreError::Closed)?;
        history.action(SavedHistoryAction::Open { id: id.clone() })?;
        history.follow(true);
        self.replace(&mut s, Content::Saved { id, history }, None)?;
        drop(s);
        self.events.0.on_android_host_changed();
        Ok(())
    }
}

fn selected_voice_host(state: &State) -> Option<String> {
    match &state.content {
        Content::Live { client, id, .. } => client.session_row(id.clone()).map(|row| row.device_id),
        _ => None,
    }
}

fn voice_end_notice(reason: VoiceEndReason) -> &'static str {
    match reason {
        VoiceEndReason::MicrophoneDenied => "Allow microphone access to make a call.",
        VoiceEndReason::SignInRequired => "Sign in on your computer to use voice.",
        VoiceEndReason::Busy => "Your computer is already in a call.",
        VoiceEndReason::VoiceAgentRequired => "Choose a voice agent on your computer first.",
        VoiceEndReason::UsageUnavailable => "Voice usage is unavailable. Try again later.",
        VoiceEndReason::HostIncompatible => "Update OpenAgents on your computer to use voice.",
        VoiceEndReason::HostUnavailable => {
            "Your computer is unavailable. Try again when it is online."
        }
        VoiceEndReason::AudioUnavailable => {
            "Audio is unavailable. Check your microphone and try again."
        }
        VoiceEndReason::ConnectionLost => "The call disconnected. Try again.",
    }
}

fn voice_device_matches(selected: &str, device: &DeviceView) -> bool {
    device.id == selected
        && device.online
        && device.is_execution_host
        && device
            .capabilities
            .iter()
            .any(|capability| capability == &voice_host_capability())
}

fn browser_link(url: &str) -> bool {
    url.len() <= 8192
        && (url.starts_with("https://") || url.starts_with("http://"))
        && !url.chars().any(|c| c.is_control() || c.is_whitespace())
}

fn selection_boundary(text: &str, offset: usize) -> bool {
    let mut at = 0;
    for c in text.chars() {
        if at == offset {
            return true;
        }
        at += c.len_utf16();
        if at > offset {
            return false;
        }
    }
    at == offset
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Listener;
    impl AndroidHostListener for Listener {
        fn on_android_host_changed(&self) {}
    }
    #[test]
    fn voice_target_requires_the_selected_online_execution_host_and_capability() {
        let mut device = DeviceView {
            id: "selected".into(),
            name: "Fixture computer".into(),
            platform: "linux".into(),
            online: true,
            last_seen_ms: None,
            version: None,
            capabilities: vec![voice_host_capability()],
            is_execution_host: true,
            is_self: false,
            session_count: 0,
        };
        assert!(voice_device_matches("selected", &device));
        assert!(!voice_device_matches("another", &device));
        device.online = false;
        assert!(!voice_device_matches("selected", &device));
        device.online = true;
        device.is_execution_host = false;
        assert!(!voice_device_matches("selected", &device));
        device.is_execution_host = true;
        device.capabilities.clear();
        assert!(!voice_device_matches("selected", &device));
    }
    #[test]
    fn demo_and_unselected_cloud_do_not_advertise_or_open_voice() {
        let owner = CoreAndroidHost::demo(Arc::new(Listener));
        assert!(!owner.snapshot().unwrap().can_start_voice);
        assert!(owner.snapshot().unwrap().active_voice.is_none());
        assert!(owner.navigate(AndroidHostRoute::Voice).is_err());
    }
    #[test]
    fn browser_links_are_bounded_and_do_not_launch_platform_commands() {
        assert!(browser_link("https://example.invalid/path?q=1"));
        for url in [
            "intent://example",
            "javascript:alert(1)",
            "file:///secret",
            "https://example.invalid/\n",
        ] {
            assert!(!browser_link(url));
        }
        assert!(!browser_link(&format!(
            "https://example.invalid/{}",
            "x".repeat(8192)
        )));
    }
    #[test]
    fn selection_offsets_cannot_split_utf16_surrogates() {
        assert!(selection_boundary("a🙂z", 1));
        assert!(!selection_boundary("a🙂z", 2));
        assert!(selection_boundary("a🙂z", 3));
        assert!(!selection_boundary("a🙂z", usize::MAX));
    }
    #[test]
    fn remount_borrows_the_same_demo_owner_and_editor() {
        let owner = CoreAndroidHost::demo(Arc::new(Listener));
        owner.start();
        owner.start();
        let before = owner.snapshot().unwrap();
        owner.navigate(AndroidHostRoute::Chat).unwrap();
        let after = owner.snapshot().unwrap();
        assert!(Arc::ptr_eq(
            before.composer.as_ref().unwrap(),
            after.composer.as_ref().unwrap()
        ));
        assert!(before.account.is_none());
        assert!(after.client.is_none());
        assert_eq!(before.content_revision, after.content_revision);
    }
    #[test]
    fn a_late_indexed_load_cannot_replace_a_newer_navigation_intent() {
        let owner = CoreAndroidHost::demo(Arc::new(Listener));
        let stamp = owner.state.lock().unwrap().begin_navigation().unwrap();
        assert!(stamp.accepts(&owner.state.lock().unwrap()));
        owner.navigate(AndroidHostRoute::Chat).unwrap();
        assert!(!stamp.accepts(&owner.state.lock().unwrap()));
    }
    #[test]
    fn bookmark_rejects_stale_content_and_unbounded_coordinates() {
        let owner = CoreAndroidHost::demo(Arc::new(Listener));
        let snapshot = owner.snapshot().unwrap();
        let anchor = AndroidTranscriptAnchor {
            row: 7,
            inside: 12.0,
            following: false,
        };
        owner
            .save_transcript_anchor(
                snapshot.generation,
                snapshot.content_revision,
                anchor.clone(),
            )
            .unwrap();
        assert!(
            owner
                .save_transcript_anchor(
                    snapshot.generation + 1,
                    snapshot.content_revision,
                    anchor.clone()
                )
                .is_err()
        );
        assert!(
            owner
                .save_transcript_anchor(
                    snapshot.generation,
                    snapshot.content_revision + 1,
                    anchor.clone()
                )
                .is_err()
        );
        for inside in [f32::NAN, f32::INFINITY, -1.0, 1_000_001.0] {
            assert!(
                owner
                    .save_transcript_anchor(
                        snapshot.generation,
                        snapshot.content_revision,
                        AndroidTranscriptAnchor {
                            inside,
                            ..anchor.clone()
                        }
                    )
                    .is_err()
            );
        }
        assert_eq!(owner.snapshot().unwrap().anchor, Some(anchor));
    }
}
