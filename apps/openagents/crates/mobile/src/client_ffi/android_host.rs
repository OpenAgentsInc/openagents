//! Retained Android account, navigation, and chat ownership. Activities borrow these handles.
use super::*;
use openagents_account::openagents::Deployment;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

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
impl SavedHistoryListener for Events {
    fn on_saved_history_changed(&self) {
        self.0.on_android_host_changed();
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidHostRoute {
    Chat,
    Sessions,
    Settings,
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
    fn begin_navigation(&mut self) -> CoreResult<NavigationStamp> {
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
    config: CoreConfig,
    events: Arc<Events>,
    state: Mutex<State>,
    started: AtomicBool,
}
impl CoreAndroidHost {
    fn refresh(&self, state: &mut State) -> CoreResult<()> {
        let Some(account) = &self.account else {
            return Ok(());
        };
        let snapshot = account.snapshot();
        if snapshot.generation != state.generation {
            if let Some(composer) = state.composer.take() {
                composer.dispose();
            }
            if let Some(sessions) = state.sessions.take() {
                sessions.close();
            }
            if let Some(history) = state.history.take() {
                history.shutdown();
            }
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
            state.generation = snapshot.generation;
            state.content_revision = state
                .content_revision
                .checked_add(1)
                .ok_or(CoreError::Closed)?;
            state.route = AndroidHostRoute::Chat;
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
        state.content = content;
        state.anchor = None;
        state.composer = composer;
        state.content_revision = revision;
        state.route = AndroidHostRoute::Chat;
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
            storage,
        )?;
        let generation = account.snapshot().generation;
        let sessions = CoreAndroidSessions::new(account.clone(), events.clone());
        let cloud = sessions.default_chat()?;
        let composer = CoreAndroidComposer::cloud(cloud.clone(), Some(account.clone()));
        composer.bind_listener(events.0.clone());
        Ok(Arc::new(Self {
            account: Some(account),
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
                navigation_revision: 0,
                anchor: None,
                generation,
                content_revision: 1,
                route: AndroidHostRoute::Chat,
                sessions: Some(sessions),
                client: None,
                history: None,
                content: Content::Cloud(cloud),
                composer: Some(composer),
            }),
            started: AtomicBool::new(false),
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
        })
    }
    /// Called by the Application once. Activity recreation never starts another identity flow.
    pub fn start(&self) {
        if !self.started.swap(true, Ordering::AcqRel) {
            if let Some(account) = &self.account {
                account.start_identity(self.config.device_name.clone());
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
        };
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
            notice,
            composer: s.composer.clone(),
            sessions: s.sessions.clone(),
            client: s.client.clone(),
            history: s.history.clone(),
            account: self.account.clone(),
            anchor: s.anchor.clone(),
        })
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
        s.route = route;
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
        drop(s);
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
            if state.generation != generation || state.content_revision != revision {
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

#[cfg(test)]
mod tests {
    use super::*;
    struct Listener;
    impl AndroidHostListener for Listener {
        fn on_android_host_changed(&self) {}
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
