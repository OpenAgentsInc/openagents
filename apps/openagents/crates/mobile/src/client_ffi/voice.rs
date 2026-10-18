//! Bounded platform callbacks; no PCM or provider credentials cross UniFFI.
//! The call state is reduced here (`openagents_voice_session::VoiceView`), so the
//! platform renders typed snapshots exactly as the desktop reads them.
use super::*;
use crate::orb::VoiceOrb;
use crate::voice_ui::{self, VoiceIntent, VoiceUiAction};
use openagents_proto::voice::{
    VoiceChatContext, VoicePhase, VoiceRejection, VoiceRole, VoiceWork, remote::Sdp,
};
use openagents_voice_session::{VoiceMediaEndpoint, VoiceView};
use std::collections::HashMap;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

static NEXT_VOICE_VIEW_INSTANCE: AtomicU64 = AtomicU64::new(1);

struct CallPresentation {
    instance: String,
    revision: u64,
    state: VoiceCallState,
    closed: bool,
    capsule: Option<rust_native::ValidatedView<VoiceIntent>>,
    screen: Option<rust_native::ValidatedView<VoiceIntent>>,
}

fn new_call_presentation() -> Arc<Mutex<CallPresentation>> {
    let id = NEXT_VOICE_VIEW_INSTANCE.fetch_add(1, Ordering::Relaxed);
    Arc::new(Mutex::new(CallPresentation {
        instance: format!("voice-call-{id}"),
        revision: 0,
        state: call_state(&VoiceView::new()),
        closed: false,
        capsule: None,
        screen: None,
    }))
}

fn publish_call_state(presentation: &Mutex<CallPresentation>, state: VoiceCallState) -> bool {
    let mut current = presentation.lock().unwrap();
    if current.closed {
        return false;
    }
    current.state = state;
    current.capsule = None;
    current.screen = None;
    true
}

fn close_call_presentation(presentation: &Mutex<CallPresentation>) {
    let mut current = presentation.lock().unwrap();
    current.closed = true;
    current.capsule = None;
    current.screen = None;
}
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, uniffi::Enum)]
pub enum VoiceMediaOperation {
    Prepare,
    Offer,
    ApplyAnswer,
    SetMuted,
    Levels,
    Close,
}
#[derive(Clone, uniffi::Record)]
pub struct VoiceMediaRequest {
    pub request_id: u64,
    pub operation: VoiceMediaOperation,
    pub sdp: Option<String>,
    pub muted: bool,
}
/// Why a platform media operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum VoiceMediaFailure {
    PermissionDenied,
    Unavailable,
}
#[uniffi::export(with_foreign)]
pub trait VoiceMediaListener: Send + Sync {
    fn on_request(&self, request: VoiceMediaRequest);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum VoiceCallPhase {
    Connecting,
    Active,
    Ending,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum VoiceCallWork {
    Idle,
    /// The orchestrator's Codex turn (or a delegation) is running.
    Working,
    /// Codex asked the user something; answer it in the transcript.
    AwaitingInput,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum VoiceSpeaker {
    User,
    Assistant,
}
/// The currently selected Android output route. Other platforms can report
/// their closest equivalent; this is state, not a command to the audio device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, uniffi::Enum)]
pub enum VoiceAudioRoute {
    #[default]
    Unknown,
    Earpiece,
    Speaker,
    WiredHeadset,
    Bluetooth,
}
/// Audio focus as reported by the platform audio session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, uniffi::Enum)]
pub enum VoiceAudioFocus {
    #[default]
    Gained,
    Lost,
    LostTransient,
    LostTransientCanDuck,
}
/// Connectivity relevant to the active voice media session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, uniffi::Enum)]
pub enum VoiceNetworkState {
    #[default]
    Available,
    Lost,
}
/// Ephemeral audio state owned by the active call and reported by the host OS.
#[derive(Clone, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct VoiceAudioState {
    pub route: VoiceAudioRoute,
    pub focus: VoiceAudioFocus,
    pub network: VoiceNetworkState,
    pub proximity_near: bool,
}
/// Everything a call screen shows. Ephemeral: never log or persist captions.
#[derive(Clone, Debug, uniffi::Record)]
pub struct VoiceCallState {
    pub phase: VoiceCallPhase,
    pub orb: VoiceOrb,
    /// Full id of the host's orchestrator chat (its canonical transcript).
    pub chat_id: Option<String>,
    pub work: VoiceCallWork,
    pub muted: bool,
    /// The assistant's voice is playing.
    pub speaking: bool,
    pub caption: String,
    /// Set once the caption is a final segment; live partials have none.
    pub caption_speaker: Option<VoiceSpeaker>,
    /// The utterance the caption belongs to: a change is a new speaker turn.
    pub caption_item: Option<String>,
    /// Normalized 0…1 peaks (microphone is 0 while muted).
    pub microphone: f32,
    pub speaker: f32,
    /// Styles the host offers, once it reports them.
    pub voices: Vec<String>,
}
/// Why a call ended without the user hanging up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum VoiceEndReason {
    VoiceAgentRequired,
    MicrophoneDenied,
    AudioUnavailable,
    /// Codex on the host must be signed in with ChatGPT.
    SignInRequired,
    UsageUnavailable,
    /// The host already has a voice call.
    Busy,
    HostUnavailable,
    /// The host's OpenAgents is too old or has remote voice disabled.
    HostIncompatible,
    ConnectionLost,
}
impl From<VoiceRejection> for VoiceEndReason {
    fn from(reason: VoiceRejection) -> Self {
        use VoiceRejection::*;
        match reason {
            VoiceAgentRequired => Self::VoiceAgentRequired,
            MicrophonePermissionDenied => Self::MicrophoneDenied,
            DeviceUnavailable | MicrophoneMetadataMissing => Self::AudioUnavailable,
            ChatgptRequired => Self::SignInRequired,
            IncludedUsageUnavailable | CreditExclusionUnverified => Self::UsageUnavailable,
            Busy => Self::Busy,
            RemoteHost => Self::HostUnavailable,
            Disabled
            | Unsupported
            | WrongHarness
            | NativeRuntimeUnavailable
            | AudioFormatUnverified
            | DuplexUnverified => Self::HostIncompatible,
            InvalidLease | StaleGeneration | Overflow | Protocol => Self::ConnectionLost,
        }
    }
}
#[uniffi::export(with_foreign)]
pub trait VoiceSessionListener: Send + Sync {
    fn on_voice_state(&self, state: VoiceCallState);
    /// Terminal, exactly once. `None` is an orderly end.
    fn on_voice_closed(&self, reason: Option<VoiceEndReason>);
}

/// Hosts advertising this capability accept client-media voice calls.
#[uniffi::export]
pub fn voice_host_capability() -> String {
    openagents_proto::voice::remote::CAPABILITY.into()
}
/// Codex voice styles to offer before a host reports its own.
#[uniffi::export]
pub fn default_voice_styles() -> Vec<String> {
    openagents_proto::voice::DEFAULT_VOICES
        .iter()
        .map(|voice| (*voice).to_owned())
        .collect()
}

fn call_state(view: &VoiceView) -> VoiceCallState {
    VoiceCallState {
        phase: match view.phase {
            VoicePhase::Active => VoiceCallPhase::Active,
            VoicePhase::Stopping | VoicePhase::Closed | VoicePhase::Failed => {
                VoiceCallPhase::Ending
            }
            VoicePhase::Checking | VoicePhase::Starting => VoiceCallPhase::Connecting,
        },
        orb: VoiceOrb::from_state(view.orb_state()),
        chat_id: view.chat_id().map(str::to_owned),
        work: match view.work() {
            VoiceWork::Idle => VoiceCallWork::Idle,
            VoiceWork::Working => VoiceCallWork::Working,
            VoiceWork::AwaitingInput => VoiceCallWork::AwaitingInput,
        },
        muted: view.muted(),
        speaking: view.playing(),
        caption: view.caption().to_owned(),
        caption_speaker: view.caption_role().map(|role| match role {
            VoiceRole::User => VoiceSpeaker::User,
            VoiceRole::Assistant => VoiceSpeaker::Assistant,
        }),
        caption_item: view.caption_item().map(str::to_owned),
        microphone: view.microphone_level(),
        speaker: view.speaker_level(),
        voices: view
            .snapshot
            .as_ref()
            .map(|s| s.voices.clone())
            .unwrap_or_default(),
    }
}

struct Reply {
    sdp: Option<String>,
    microphone: u16,
    speaker: u16,
}
struct PlatformMedia {
    listener: Arc<dyn VoiceMediaListener>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Reply, VoiceRejection>>>>,
    next: AtomicU64,
    closed: AtomicBool,
}
impl PlatformMedia {
    async fn request(
        &self,
        operation: VoiceMediaOperation,
        sdp: Option<String>,
        muted: bool,
    ) -> Result<Reply, VoiceRejection> {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().unwrap();
            if self.closed.load(Ordering::Acquire) {
                return Err(VoiceRejection::InvalidLease);
            }
            if pending.len() >= 4 {
                return Err(VoiceRejection::Overflow);
            }
            pending.insert(id, tx);
        }
        struct Pending<'a>(&'a PlatformMedia, u64);
        impl Drop for Pending<'_> {
            fn drop(&mut self) {
                self.0.pending.lock().unwrap().remove(&self.1);
            }
        }
        let _pending = Pending(self, id);
        self.listener.on_request(VoiceMediaRequest {
            request_id: id,
            operation,
            sdp,
            muted,
        });
        tokio::time::timeout(std::time::Duration::from_secs(30), rx)
            .await
            .map_err(|_| VoiceRejection::DeviceUnavailable)?
            .map_err(|_| VoiceRejection::DeviceUnavailable)?
    }
}
#[async_trait::async_trait]
impl VoiceMediaEndpoint for PlatformMedia {
    async fn prepare(&self) -> Result<(), VoiceRejection> {
        self.request(VoiceMediaOperation::Prepare, None, true)
            .await
            .map(|_| ())
    }
    async fn offer(&self) -> Result<Sdp, VoiceRejection> {
        Sdp::new(
            self.request(VoiceMediaOperation::Offer, None, true)
                .await?
                .sdp
                .ok_or(VoiceRejection::Protocol)?,
        )
    }
    async fn apply_answer(&self, answer: Sdp) -> Result<(), VoiceRejection> {
        self.request(
            VoiceMediaOperation::ApplyAnswer,
            Some(answer.expose().into()),
            true,
        )
        .await
        .map(|_| ())
    }
    async fn set_muted(&self, muted: bool) -> Result<(), VoiceRejection> {
        self.request(VoiceMediaOperation::SetMuted, None, muted)
            .await
            .map(|_| ())
    }
    async fn levels(&self) -> Result<(u16, u16), VoiceRejection> {
        let r = self
            .request(VoiceMediaOperation::Levels, None, true)
            .await?;
        Ok((r.microphone, r.speaker))
    }
    fn close(&self) {
        let pending = {
            let mut pending = self.pending.lock().unwrap();
            if self.closed.swap(true, Ordering::AcqRel) {
                return;
            }
            std::mem::take(&mut *pending)
        };
        for (_, tx) in pending {
            let _ = tx.send(Err(VoiceRejection::InvalidLease));
        }
        self.listener.on_request(VoiceMediaRequest {
            request_id: 0,
            operation: VoiceMediaOperation::Close,
            sdp: None,
            muted: true,
        });
    }
}

#[derive(uniffi::Object)]
pub struct VoiceCall {
    pub(super) host_device_id: String,
    media: Arc<PlatformMedia>,
    cancel: CancellationToken,
    muted: watch::Sender<bool>,
    context: watch::Sender<Option<VoiceChatContext>>,
    audio: Mutex<VoiceAudioState>,
    presentation: Arc<Mutex<CallPresentation>>,
}
impl Drop for VoiceCall {
    fn drop(&mut self) {
        close_call_presentation(&self.presentation);
        self.media.close();
        self.cancel.cancel();
    }
}
#[uniffi::export]
impl VoiceCall {
    /// Update presentation context; only the active call may publish it.
    pub fn update_context(&self, chat_id: Option<String>, title: Option<String>) {
        if self.cancel.is_cancelled() {
            return;
        }
        let context = chat_id.and_then(|id| VoiceChatContext::new(id, title));
        self.context.send_if_modified(|current| {
            if *current == context {
                false
            } else {
                *current = context;
                true
            }
        });
    }
    pub fn set_muted(&self, muted: bool) {
        if !self.cancel.is_cancelled() {
            self.muted.send_replace(muted);
            let mut presentation = self.presentation.lock().unwrap();
            if !presentation.closed {
                presentation.state.muted = muted;
                presentation.capsule = None;
                presentation.screen = None;
            }
        }
    }
    /// Record the current platform route while the call is active.
    pub fn update_audio_route(&self, route: VoiceAudioRoute) {
        self.update_audio_state(|state| state.route = route);
    }
    /// Record focus changes, including interruptions and ducking.
    pub fn update_audio_focus(&self, focus: VoiceAudioFocus) {
        self.update_audio_state(|state| state.focus = focus);
    }
    /// Record connectivity changes without creating a second reconnect path.
    pub fn update_network_state(&self, network: VoiceNetworkState) {
        self.update_audio_state(|state| state.network = network);
    }
    /// Record proximity so the platform can select its near/far audio behavior.
    pub fn update_proximity(&self, near: bool) {
        self.update_audio_state(|state| state.proximity_near = near);
    }
    /// Read the latest platform state for the active call.
    pub fn audio_state(&self) -> VoiceAudioState {
        self.audio.lock().unwrap().clone()
    }
    pub fn stop(&self) {
        close_call_presentation(&self.presentation);
        self.media.close();
        self.cancel.cancel();
    }

    /// Current Rust-owned compact call view. A new revision replaces its prior activation tree.
    pub fn capsule_view(&self) -> Result<String, CoreError> {
        let mut current = self.presentation.lock().unwrap();
        if current.closed || self.cancel.is_cancelled() {
            return Err(CoreError::Closed);
        }
        current.revision =
            current
                .revision
                .checked_add(1)
                .ok_or_else(|| CoreError::InvalidArgument {
                    message: "Couldn't refresh the call controls.".into(),
                })?;
        let view = voice_ui::call_capsule(&current.instance, current.revision, &current.state)
            .map_err(|_| CoreError::InvalidArgument {
                message: "Couldn't open the call controls.".into(),
            })?;
        let json = serde_json::to_string(view.view()).map_err(|_| CoreError::InvalidArgument {
            message: "Couldn't open the call controls.".into(),
        })?;
        current.capsule = Some(view);
        Ok(json)
    }

    /// Current Rust-owned full call view. A new revision replaces its prior activation tree.
    pub fn screen_view(&self) -> Result<String, CoreError> {
        let mut current = self.presentation.lock().unwrap();
        if current.closed || self.cancel.is_cancelled() {
            return Err(CoreError::Closed);
        }
        current.revision =
            current
                .revision
                .checked_add(1)
                .ok_or_else(|| CoreError::InvalidArgument {
                    message: "Couldn't refresh the call screen.".into(),
                })?;
        let view = voice_ui::call_screen(&current.instance, current.revision, &current.state)
            .map_err(|_| CoreError::InvalidArgument {
                message: "Couldn't open the call screen.".into(),
            })?;
        let json = serde_json::to_string(view.view()).map_err(|_| CoreError::InvalidArgument {
            message: "Couldn't open the call screen.".into(),
        })?;
        current.screen = Some(view);
        Ok(json)
    }

    /// Resolve only the current Rust Native tree. No action payload crosses this boundary.
    pub fn activate_view(
        &self,
        instance: String,
        revision: u64,
        node: String,
    ) -> Result<VoiceUiAction, CoreError> {
        let (should_end, next_muted, action) = {
            let current = self.presentation.lock().unwrap();
            if current.closed || self.cancel.is_cancelled() {
                return Err(CoreError::Closed);
            }
            let activation = rust_native::Activation {
                instance,
                revision,
                node,
            };
            let intent = current
                .capsule
                .as_ref()
                .and_then(|view| view.activate(&activation).ok())
                .or_else(|| {
                    current
                        .screen
                        .as_ref()
                        .and_then(|view| view.activate(&activation).ok())
                })
                .ok_or_else(|| CoreError::InvalidArgument {
                    message: "Those call controls are out of date.".into(),
                })?;
            match intent {
                VoiceIntent::ToggleMute => {
                    (false, Some(!current.state.muted), VoiceUiAction::Handled)
                }
                VoiceIntent::EndCall => (true, None, VoiceUiAction::Handled),
                VoiceIntent::OpenCall => return Ok(VoiceUiAction::OpenCall),
                VoiceIntent::RefreshUsage => {
                    return Err(CoreError::InvalidArgument {
                        message: "That control is not available in a call.".into(),
                    });
                }
            }
        };
        if should_end {
            self.stop();
        } else if let Some(muted) = next_muted {
            self.set_muted(muted);
        }
        Ok(action)
    }
    /// Late native callbacks simply miss the retired request. No resume is possible.
    pub fn complete_media(
        &self,
        request_id: u64,
        failure: Option<VoiceMediaFailure>,
        sdp: Option<String>,
        microphone: u16,
        speaker: u16,
    ) {
        let tx = {
            let mut pending = self.media.pending.lock().unwrap();
            if self.media.closed.load(Ordering::Acquire) {
                return;
            }
            pending.remove(&request_id)
        };
        if let Some(tx) = tx {
            let reply = match failure {
                Some(VoiceMediaFailure::PermissionDenied) => {
                    Err(VoiceRejection::MicrophonePermissionDenied)
                }
                Some(VoiceMediaFailure::Unavailable) => Err(VoiceRejection::DeviceUnavailable),
                None if sdp.as_ref().is_some_and(|s| s.len() > 65_536) => {
                    Err(VoiceRejection::DeviceUnavailable)
                }
                None => Ok(Reply {
                    sdp,
                    microphone,
                    speaker,
                }),
            };
            let _ = tx.send(reply);
        }
    }
}
impl VoiceCall {
    fn update_audio_state(&self, update: impl FnOnce(&mut VoiceAudioState)) {
        if !self.cancel.is_cancelled() && !self.media.closed.load(Ordering::Acquire) {
            update(&mut self.audio.lock().unwrap());
        }
    }
}

/// Reduce owner events and local mute into call snapshots for the platform.
async fn present(
    listener: Arc<dyn VoiceSessionListener>,
    mut events: mpsc::Receiver<openagents_proto::voice::VoiceEvent>,
    muted: watch::Receiver<bool>,
    presentation: Arc<Mutex<CallPresentation>>,
) {
    let mut view = VoiceView::new();
    let initial = call_state(&view);
    if publish_call_state(&presentation, initial.clone()) {
        listener.on_voice_state(initial);
    }
    let mut muted = Some(muted);
    loop {
        let changed = tokio::select! {
            event = events.recv() => match event {
                Some(event) => view.reduce(event, std::time::Instant::now()),
                None => return,
            },
            changed = async { muted.as_mut().unwrap().changed().await }, if muted.is_some() => {
                match changed {
                    Ok(()) => {
                        let value = *muted.as_mut().unwrap().borrow_and_update();
                        view.set_muted(value)
                    }
                    // The call handle is gone; its cancellation ends us next.
                    Err(_) => {
                        muted = None;
                        false
                    }
                }
            }
        };
        if changed {
            let state = call_state(&view);
            if publish_call_state(&presentation, state.clone()) {
                listener.on_voice_state(state);
            }
        }
    }
}

#[uniffi::export]
impl CoreClient {
    /// Returns immediately. Callbacks are dispatched on the core runtime; the
    /// platform must hop to its media/UI executor and call complete_media.
    pub fn start_voice(
        &self,
        host_device_id: String,
        voice: Option<String>,
        media: Arc<dyn VoiceMediaListener>,
        listener: Arc<dyn VoiceSessionListener>,
    ) -> Arc<VoiceCall> {
        let platform = Arc::new(PlatformMedia {
            listener: media,
            pending: Default::default(),
            next: AtomicU64::new(0),
            closed: AtomicBool::new(false),
        });
        let cancel = self.client.voice_cancellation();
        let (muted, rx) = watch::channel(false);
        let (context, context_rx) = watch::channel(None);
        let presented_mute = rx.clone();
        let presentation = new_call_presentation();
        let handle = Arc::new(VoiceCall {
            host_device_id: host_device_id.clone(),
            media: platform.clone(),
            cancel: cancel.clone(),
            muted,
            context,
            audio: Mutex::new(VoiceAudioState::default()),
            presentation: presentation.clone(),
        });
        let client = self.client.clone();
        zc::runtime::handle().spawn(async move {
            let (events, receiver) = mpsc::channel(32);
            let forward = tokio::spawn(present(
                listener.clone(),
                receiver,
                presented_mute,
                presentation.clone(),
            ));
            let operation = async {
                let control = client
                    .voice_transport(&host_device_id)
                    .await
                    .map_err(|_| VoiceRejection::RemoteHost)?;
                let config = openagents_proto::ChatConfig {
                    harness: openagents_proto::HarnessId::Codex,
                    model: None,
                    reasoning: None,
                    model_options: Default::default(),
                    sandbox: openagents_proto::SandboxLevel::WorkspaceWrite,
                };
                openagents_voice_session::run_with_context(
                    control,
                    platform.clone(),
                    config,
                    voice,
                    cancel.clone(),
                    events,
                    rx,
                    context_rx,
                )
                .await
            };
            let result = tokio::select! {biased;_=cancel.cancelled()=>Ok(()),r=operation=>r};
            platform.close();
            // Every sender is gone: the presenter drains what was queued.
            let _ = forward.await;
            close_call_presentation(&presentation);
            listener.on_voice_closed(result.err().map(VoiceEndReason::from));
        });
        handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_proto::voice::{VoiceEvent, VoiceSnapshot};

    fn test_call() -> VoiceCall {
        VoiceCall {
            host_device_id: "host".into(),
            media: media(Arc::new(Listener(Mutex::new(vec![])))),
            cancel: CancellationToken::new(),
            muted: watch::channel(false).0,
            context: watch::channel(None).0,
            audio: Mutex::new(VoiceAudioState::default()),
            presentation: new_call_presentation(),
        }
    }

    fn activation(view: &str, node: &str) -> (String, u64, String) {
        let view: serde_json::Value = serde_json::from_str(view).unwrap();
        (
            view["instance"].as_str().unwrap().to_owned(),
            view["revision"].as_u64().unwrap(),
            node.to_owned(),
        )
    }

    #[test]
    fn call_view_rejects_stale_and_wrong_view_activations() {
        let call = test_call();
        let old = call.screen_view().unwrap();
        let (instance, revision, _) = activation(&old, "end-call");
        assert!(
            call.activate_view(format!("{instance}-other"), revision, "end-call".into())
                .is_err()
        );
        let mut updated = call.presentation.lock().unwrap().state.clone();
        updated.speaking = true;
        assert!(publish_call_state(&call.presentation, updated));
        assert!(
            call.activate_view(instance.clone(), revision, "end-call".into())
                .is_err()
        );

        let current = call.screen_view().unwrap();
        let (instance, revision, _) = activation(&current, "voice-title");
        assert!(
            call.activate_view(instance, revision, "voice-title".into())
                .is_err()
        );
    }

    #[test]
    fn call_view_rejects_activation_after_call_closes() {
        let call = test_call();
        let current = call.screen_view().unwrap();
        let (instance, revision, _) = activation(&current, "end-call");
        call.stop();
        assert!(matches!(
            call.activate_view(instance, revision, "end-call".into()),
            Err(CoreError::Closed)
        ));
    }

    #[test]
    fn current_call_view_runs_mute_in_rust() {
        let call = test_call();
        let mut state = call.presentation.lock().unwrap().state.clone();
        state.phase = VoiceCallPhase::Active;
        assert!(publish_call_state(&call.presentation, state));
        let current = call.screen_view().unwrap();
        let (instance, revision, _) = activation(&current, "toggle-mute");
        assert_eq!(
            call.activate_view(instance, revision, "toggle-mute".into()),
            Ok(VoiceUiAction::Handled)
        );
        assert!(call.presentation.lock().unwrap().state.muted);
    }

    #[tokio::test]
    async fn voice_context_changes_only_on_the_active_call() {
        let listener = Arc::new(Listener(Mutex::new(vec![])));
        let (context, rx) = watch::channel(None);
        let call = VoiceCall {
            host_device_id: "host".into(),
            media: media(listener),
            cancel: CancellationToken::new(),
            muted: watch::channel(false).0,
            context,
            audio: Mutex::new(VoiceAudioState::default()),
            presentation: new_call_presentation(),
        };
        call.update_context(Some("chat-x".into()), Some("Chat X".into()));
        assert_eq!(rx.borrow().as_ref().unwrap().chat_id, "chat-x");
        call.stop();
        call.update_context(Some("late-chat".into()), None);
        assert_eq!(rx.borrow().as_ref().unwrap().chat_id, "chat-x");
    }
    #[test]
    fn platform_audio_state_is_typed_and_late_updates_are_ignored() {
        let listener = Arc::new(Listener(Mutex::new(vec![])));
        let call = VoiceCall {
            host_device_id: "host".into(),
            media: media(listener),
            cancel: CancellationToken::new(),
            muted: watch::channel(false).0,
            context: watch::channel(None).0,
            audio: Mutex::new(VoiceAudioState::default()),
            presentation: new_call_presentation(),
        };
        call.update_audio_route(VoiceAudioRoute::Bluetooth);
        call.update_audio_focus(VoiceAudioFocus::LostTransient);
        call.update_network_state(VoiceNetworkState::Lost);
        call.update_proximity(true);
        assert_eq!(
            call.audio_state(),
            VoiceAudioState {
                route: VoiceAudioRoute::Bluetooth,
                focus: VoiceAudioFocus::LostTransient,
                network: VoiceNetworkState::Lost,
                proximity_near: true,
            }
        );
        call.stop();
        call.update_audio_route(VoiceAudioRoute::Speaker);
        assert_eq!(call.audio_state().route, VoiceAudioRoute::Bluetooth);
    }
    struct Listener(Mutex<Vec<u64>>);
    impl VoiceMediaListener for Listener {
        fn on_request(&self, r: VoiceMediaRequest) {
            self.0.lock().unwrap().push(r.request_id);
        }
    }
    fn media(listener: Arc<Listener>) -> Arc<PlatformMedia> {
        Arc::new(PlatformMedia {
            listener,
            pending: Default::default(),
            next: AtomicU64::new(0),
            closed: AtomicBool::new(false),
        })
    }
    #[tokio::test]
    async fn close_releases_pending_callbacks_and_rejects_late_replies() {
        let listener = Arc::new(Listener(Mutex::new(vec![])));
        let media = media(listener.clone());
        let (muted, _) = watch::channel(false);
        let call = VoiceCall {
            host_device_id: "host".into(),
            media: media.clone(),
            cancel: CancellationToken::new(),
            muted,
            context: watch::channel(None).0,
            audio: Mutex::new(VoiceAudioState::default()),
            presentation: new_call_presentation(),
        };
        let clone = media.clone();
        let task = tokio::spawn(async move { clone.offer().await });
        tokio::task::yield_now().await;
        call.stop();
        assert!(task.await.unwrap().is_err());
        assert!(media.pending.lock().unwrap().is_empty());
        assert!(call.cancel.is_cancelled());
        call.stop();
        call.complete_media(1, None, Some("late SDP".into()), 0, 0);
        assert!(media.offer().await.is_err());
        drop(call);
        assert_eq!(
            listener
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|id| **id == 0)
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn denied_microphone_is_reported_as_such() {
        let listener = Arc::new(Listener(Mutex::new(vec![])));
        let media = media(listener.clone());
        let (muted, _) = watch::channel(false);
        let call = VoiceCall {
            host_device_id: "host".into(),
            media: media.clone(),
            cancel: CancellationToken::new(),
            muted,
            context: watch::channel(None).0,
            audio: Mutex::new(VoiceAudioState::default()),
            presentation: new_call_presentation(),
        };
        let clone = media.clone();
        let task = tokio::spawn(async move { clone.prepare().await });
        while listener.0.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
        call.complete_media(1, Some(VoiceMediaFailure::PermissionDenied), None, 0, 0);
        let reason = task.await.unwrap().unwrap_err();
        assert_eq!(
            VoiceEndReason::from(reason),
            VoiceEndReason::MicrophoneDenied
        );
    }

    struct States(Mutex<Vec<VoiceCallState>>);
    impl VoiceSessionListener for States {
        fn on_voice_state(&self, state: VoiceCallState) {
            self.0.lock().unwrap().push(state);
        }
        fn on_voice_closed(&self, _: Option<VoiceEndReason>) {}
    }

    #[tokio::test]
    async fn presenter_names_the_orchestrator_and_applies_local_mute_first() {
        let states = Arc::new(States(Mutex::new(vec![])));
        let (events, receiver) = mpsc::channel(8);
        let (mute, muted) = watch::channel(false);
        let presentation = new_call_presentation();
        let task = tokio::spawn(present(states.clone(), receiver, muted, presentation));
        events
            .send(VoiceEvent::Snapshot {
                snapshot: VoiceSnapshot {
                    session_id: "s".into(),
                    chat_id: "voice-orchestrator-full".into(),
                    generation: 1,
                    phase: VoicePhase::Active,
                    muted: false,
                    playing: false,
                    work: VoiceWork::AwaitingInput,
                    reason: None,
                    voice: None,
                    voices: vec!["maple".into()],
                },
            })
            .await
            .unwrap();
        while states.0.lock().unwrap().len() < 2 {
            tokio::task::yield_now().await;
        }
        mute.send_replace(true);
        while states.0.lock().unwrap().len() < 3 {
            tokio::task::yield_now().await;
        }
        drop(events);
        task.await.unwrap();
        let states = states.0.lock().unwrap();
        assert_eq!(states[0].phase, VoiceCallPhase::Connecting);
        assert_eq!(states[0].orb, VoiceOrb::Connecting);
        assert_eq!(
            states[1].chat_id.as_deref(),
            Some("voice-orchestrator-full")
        );
        assert_eq!(states[1].work, VoiceCallWork::AwaitingInput);
        assert_eq!(states[1].orb, VoiceOrb::AwaitingInput);
        assert_eq!(states[1].voices, vec!["maple".to_owned()]);
        assert!(states[2].muted);
    }
}
