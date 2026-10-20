//! Browser media and ephemeral signaling to the computer's Codex voice session.
use crate::{
    computer::{self, Computer},
    web::Client,
};
use anyhow::{Result, anyhow};
use futures::{
    Future, StreamExt,
    channel::oneshot,
    future::{Either, select},
};
use openagents_proto::voice::{VoiceEvent, VoiceSnapshot, remote as wire};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};

fn js(_: JsValue) -> anyhow::Error {
    anyhow!("Couldn't connect the call. Check your microphone and try again.")
}

pub async fn sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = JsFuture::from(promise).await;
}
async fn timeout<T>(future: impl Future<Output = Result<T>>, ms: i32) -> Result<T> {
    futures::pin_mut!(future);
    match select(future, Box::pin(sleep(ms))).await {
        Either::Left((result, _)) => result,
        Either::Right(_) => Err(anyhow!("The call stopped responding. Try again.")),
    }
}
fn stop_tracks(stream: &web_sys::MediaStream) {
    for track in stream.get_tracks().iter() {
        track.unchecked_into::<web_sys::MediaStreamTrack>().stop();
    }
}

/// Created on the user's click, so the browser can unlock audio immediately.
pub struct Media {
    peer: web_sys::RtcPeerConnection,
    audio: web_sys::HtmlAudioElement,
    channel: web_sys::RtcDataChannel,
    stream: RefCell<Option<web_sys::MediaStream>>,
    closed: Cell<bool>,
    playback_failed: Cell<bool>,
    muted: Cell<bool>,
    ready: Cell<bool>,
    track: RefCell<Option<Closure<dyn FnMut(web_sys::RtcTrackEvent)>>>,
    context: web_sys::AudioContext,
    microphone: RefCell<Option<(web_sys::MediaStreamAudioSourceNode, web_sys::AnalyserNode)>>,
    speaker: RefCell<Option<(web_sys::MediaStreamAudioSourceNode, web_sys::AnalyserNode)>>,
}
impl Media {
    pub fn new() -> Result<Rc<Self>> {
        let context = web_sys::AudioContext::new().map_err(js)?;
        let _ = context.resume();
        let peer = web_sys::RtcPeerConnection::new().map_err(js)?;
        let channel = peer.create_data_channel("oai-events");
        let audio = web_sys::HtmlAudioElement::new().map_err(js)?;
        audio.set_muted(true);
        let media = Rc::new(Self {
            peer,
            channel,
            audio,
            stream: RefCell::new(None),
            closed: Cell::new(false),
            playback_failed: Cell::new(false),
            muted: Cell::new(false),
            ready: Cell::new(false),
            track: RefCell::new(None),
            context,
            microphone: RefCell::new(None),
            speaker: RefCell::new(None),
        });
        let weak = Rc::downgrade(&media);
        let track = Closure::new(move |event: web_sys::RtcTrackEvent| {
            let Some(media) = weak.upgrade().filter(|m| !m.closed.get()) else {
                return;
            };
            let Ok(stream) = web_sys::MediaStream::new() else {
                media.playback_failed.set(true);
                return;
            };
            stream.add_track(&event.track());
            *media.speaker.borrow_mut() = media.meter(&stream).ok();
            media.audio.set_src_object(Some(&stream));
            match media.audio.play() {
                Ok(playing) => spawn_local(async move {
                    if JsFuture::from(playing).await.is_err() {
                        media.playback_failed.set(true);
                    }
                }),
                Err(_) => media.playback_failed.set(true),
            }
        });
        media.peer.set_ontrack(Some(track.as_ref().unchecked_ref()));
        *media.track.borrow_mut() = Some(track);
        Ok(media)
    }
    fn meter(
        &self,
        stream: &web_sys::MediaStream,
    ) -> Result<(web_sys::MediaStreamAudioSourceNode, web_sys::AnalyserNode)> {
        let source = self
            .context
            .create_media_stream_source(stream)
            .map_err(js)?;
        let analyser = self.context.create_analyser().map_err(js)?;
        analyser.set_fft_size(256);
        source.connect_with_audio_node(&analyser).map_err(js)?;
        Ok((source, analyser))
    }
    pub fn levels(&self) -> (u16, u16) {
        fn peak(
            meter: &RefCell<Option<(web_sys::MediaStreamAudioSourceNode, web_sys::AnalyserNode)>>,
        ) -> u16 {
            let mut bytes = [128; 256];
            if let Some((_, analyser)) = &*meter.borrow() {
                analyser.get_byte_time_domain_data(&mut bytes);
            }
            u16::from(
                bytes
                    .into_iter()
                    .map(|b| b.abs_diff(128))
                    .max()
                    .unwrap_or(0),
            ) * 511
        }
        (
            if self.muted.get() {
                0
            } else {
                peak(&self.microphone)
            },
            peak(&self.speaker),
        )
    }
    pub fn set_muted(&self, muted: bool) {
        self.muted.set(muted);
        if let Some(stream) = &*self.stream.borrow() {
            for track in stream.get_audio_tracks().iter() {
                track
                    .unchecked_into::<web_sys::MediaStreamTrack>()
                    .set_enabled(!muted && self.ready.get() && !self.closed.get());
            }
        }
    }
    pub fn close(&self) {
        if self.closed.replace(true) {
            return;
        }
        if let Some(stream) = self.stream.borrow_mut().take() {
            stop_tracks(&stream);
        }
        self.peer.set_ontrack(None);
        self.channel.close();
        self.peer.close();
        let _ = self.audio.pause();
        self.audio.set_src_object(None);
        self.microphone.borrow_mut().take();
        self.speaker.borrow_mut().take();
        let _ = self.context.close();
    }
    async fn prepare(self: &Rc<Self>) -> Result<()> {
        let constraints = web_sys::MediaStreamConstraints::new();
        constraints.set_audio(&JsValue::TRUE);
        constraints.set_video(&JsValue::FALSE);
        let promise = web_sys::window()
            .ok_or_else(|| anyhow!("No browser window."))?
            .navigator()
            .media_devices()
            .map_err(js)?
            .get_user_media_with_constraints(&constraints)
            .map_err(js)?;
        let (tx, rx) = oneshot::channel();
        let media = self.clone();
        // getUserMedia cannot be aborted. Stop a late grant even after the
        // call task has been cancelled while its permission prompt was open.
        spawn_local(async move {
            let result = async {
                let stream: web_sys::MediaStream = JsFuture::from(promise)
                    .await
                    .map_err(|_| {
                        anyhow!("Allow microphone access in your browser to start a call.")
                    })?
                    .dyn_into()
                    .map_err(js)?;
                if media.closed.get() {
                    stop_tracks(&stream);
                    return Err(anyhow!("Call ended."));
                }
                for track in stream.get_audio_tracks().iter() {
                    let track = track.unchecked_into::<web_sys::MediaStreamTrack>();
                    track.set_enabled(false);
                    media.peer.add_track_0(&track, &stream);
                }
                *media.microphone.borrow_mut() = media.meter(&stream).ok();
                *media.stream.borrow_mut() = Some(stream);
                Ok(())
            }
            .await;
            let _ = tx.send(result);
        });
        rx.await
            .map_err(|_| anyhow!("Couldn't open the microphone."))?
    }
    fn healthy(&self) -> bool {
        !self.closed.get()
            && !self.playback_failed.get()
            && self.peer.connection_state() == web_sys::RtcPeerConnectionState::Connected
            && self.channel.ready_state() == web_sys::RtcDataChannelState::Open
            && self.stream.borrow().as_ref().is_some_and(|s| {
                s.get_audio_tracks().iter().any(|t| {
                    t.unchecked_into::<web_sys::MediaStreamTrack>()
                        .ready_state()
                        == web_sys::MediaStreamTrackState::Live
                })
            })
    }
    async fn offer(&self) -> Result<wire::Sdp> {
        let offer = JsFuture::from(self.peer.create_offer()).await.map_err(js)?;
        let sdp = js_sys::Reflect::get(&offer, &"sdp".into())
            .map_err(js)?
            .as_string()
            .ok_or_else(|| anyhow!("Couldn't start the call."))?;
        let description = web_sys::RtcSessionDescriptionInit::new(web_sys::RtcSdpType::Offer);
        description.set_sdp(&sdp);
        JsFuture::from(self.peer.set_local_description(&description))
            .await
            .map_err(js)?;
        timeout(
            async {
                while self.peer.ice_gathering_state() != web_sys::RtcIceGatheringState::Complete {
                    sleep(50).await;
                }
                let sdp = self
                    .peer
                    .local_description()
                    .ok_or_else(|| anyhow!("Couldn't start the call."))?
                    .sdp();
                wire::Sdp::new(sdp).map_err(|_| anyhow!("Couldn't start the call."))
            },
            15_000,
        )
        .await
    }
    async fn answer(&self, answer: &wire::Sdp) -> Result<()> {
        let description = web_sys::RtcSessionDescriptionInit::new(web_sys::RtcSdpType::Answer);
        description.set_sdp(answer.expose());
        JsFuture::from(self.peer.set_remote_description(&description))
            .await
            .map_err(js)?;
        timeout(
            async {
                loop {
                    match self.peer.connection_state() {
                        web_sys::RtcPeerConnectionState::Connected
                            if self.channel.ready_state() == web_sys::RtcDataChannelState::Open =>
                        {
                            return Ok(());
                        }
                        web_sys::RtcPeerConnectionState::Failed
                        | web_sys::RtcPeerConnectionState::Closed => {
                            return Err(anyhow!("Couldn't connect the call. Try again."));
                        }
                        _ => sleep(50).await,
                    }
                }
            },
            20_000,
        )
        .await
    }
}
impl Drop for Media {
    fn drop(&mut self) {
        self.close();
    }
}

/// One attempt, owned by the window. Local capture stops before cleanup RPC.
pub struct Call {
    pub media: Rc<Media>,
    host: RefCell<Option<Rc<Computer>>>,
    device: String,
    attempt: wire::AttemptKey,
    lease: RefCell<Option<wire::Lease>>,
    stopped: Cell<bool>,
    sequence: Cell<u64>,
    pagehide: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>>,
}
impl Call {
    pub fn with_media(device: String, media: Rc<Media>) -> Rc<Self> {
        let call = Rc::new(Self {
            media,
            host: RefCell::new(None),
            device,
            attempt: wire::AttemptKey::new(),
            lease: RefCell::new(None),
            stopped: Cell::new(false),
            sequence: Cell::new(0),
            pagehide: RefCell::new(None),
        });
        let weak = Rc::downgrade(&call);
        let pagehide = Closure::new(move |_: web_sys::Event| {
            if let Some(call) = weak.upgrade() {
                call.stop();
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("pagehide", pagehide.as_ref().unchecked_ref());
        }
        *call.pagehide.borrow_mut() = Some(pagehide);
        call
    }
    async fn rpc<T: DeserializeOwned>(
        &self,
        method: &str,
        payload: impl Serialize,
        ms: i32,
    ) -> Result<T> {
        let host = self
            .host
            .borrow()
            .clone()
            .ok_or_else(|| anyhow!("Couldn't reach your computer."))?;
        let value = timeout(
            host.call(
                method,
                json!({"targetDeviceId": self.device, "payload": payload}),
            ),
            ms,
        )
        .await?;
        serde_json::from_value(value).map_err(Into::into)
    }
    pub fn stop(&self) {
        self.media.close();
        if self.stopped.replace(true) {
            return;
        }
        let host = self.host.borrow().clone();
        let lease = self.lease.borrow().clone();
        let device = self.device.clone();
        let attempt = self.attempt.clone();
        spawn_local(async move {
            if let Some(host) = host {
                if let Some(lease) = lease {
                    let _ = timeout(
                        host.call(
                            "StopVoiceV2",
                            json!({"targetDeviceId": device, "payload":lease}),
                        ),
                        8_000,
                    )
                    .await;
                }
                let _ = timeout(
                    host.call(
                        "CancelVoiceAttemptV2",
                        json!({"targetDeviceId":device,"payload":{"attemptKey":attempt}}),
                    ),
                    8_000,
                )
                .await;
            }
        });
    }
    /// Emits only events belonging to this lease; never retries a call.
    pub async fn run(
        self: &Rc<Self>,
        client: &Client,
        context: Option<openagents_proto::voice::VoiceChatContext>,
        event: impl FnMut(VoiceEvent),
        waiting: impl FnOnce(),
    ) -> Result<()> {
        let event = RefCell::new(event);
        let host = timeout(computer::connect(client, &self.device, waiting), 610_000).await?;
        if self.stopped.get() {
            return Ok(());
        }
        *self.host.borrow_mut() = Some(host.clone());
        let cap: wire::Capabilities = self.rpc("VoiceCapabilitiesV2", json!({}), 8_000).await?;
        if !cap.client_webrtc || cap.protocol != wire::CAPABILITY {
            return Err(anyhow!("Update OpenAgents on your computer to make calls."));
        }
        self.media.prepare().await?;
        let prepared: wire::Prepared = self
            .rpc(
                "PrepareVoiceV2",
                wire::Prepare {
                    attempt_key: self.attempt.clone(),
                    config: serde_json::from_value(
                        json!({"harness":"codex","sandbox":"workspace-write"}),
                    )?,
                    voice: None,
                },
                65_000,
            )
            .await?;
        if prepared.lease.host_device_id != self.device {
            return Err(anyhow!("Couldn't connect to the selected computer."));
        }
        *self.lease.borrow_mut() = Some(prepared.lease.clone());
        let lease = prepared.lease;
        let mut owner = host.subscribe(
            "OwnVoiceV2",
            json!({"targetDeviceId":self.device,"payload":lease}),
        )?;
        let _: Value = self
            .rpc(
                "UpdateVoiceContextV2",
                wire::ContextUpdate {
                    lease: lease.clone(),
                    context,
                },
                8_000,
            )
            .await?;
        // Ownership and heartbeats run during negotiation as well as the call.
        let operation = async {
            let offer = self.media.offer().await?;
            let negotiation_id = wire::AttemptKey::new();
            let answer: wire::Negotiated = self
                .rpc(
                    "NegotiateVoiceV2",
                    wire::Negotiate {
                        lease: lease.clone(),
                        negotiation_id: negotiation_id.clone(),
                        offer,
                    },
                    95_000,
                )
                .await?;
            if answer.negotiation_id != negotiation_id {
                return Err(anyhow!("Couldn't connect the call."));
            }
            self.media.answer(&answer.answer).await?;
            let snapshot: VoiceSnapshot = self
                .rpc(
                    "ConfirmVoiceMediaV2",
                    wire::Confirm {
                        lease: lease.clone(),
                        negotiation_id,
                        muted: self.media.muted.get(),
                    },
                    8_000,
                )
                .await?;
            if snapshot.session_id != lease.voice.session_id
                || snapshot.generation != lease.voice.generation
            {
                return Err(anyhow!("This call has ended."));
            }
            (event.borrow_mut())(VoiceEvent::Snapshot { snapshot });
            self.media.ready.set(true);
            self.media.audio.set_muted(false);
            self.media.set_muted(self.media.muted.get());
            loop {
                sleep(100).await;
                if !self.media.healthy() {
                    return Err(anyhow!("The call disconnected. Try again."));
                }
                let (microphone, speaker) = self.media.levels();
                (event.borrow_mut())(VoiceEvent::Levels {
                    generation: lease.voice.generation,
                    microphone,
                    speaker,
                });
            }
        };
        let watch = async {
            loop {
                let item = owner
                    .items
                    .next()
                    .await
                    .ok_or_else(|| anyhow!("The call disconnected. Try again."))??;
                let item: VoiceEvent = serde_json::from_value(item)?;
                if !current(&item, &lease) {
                    continue;
                }
                let closed = match &item {
                    VoiceEvent::Closed { reason, .. } => Some(reason.is_some()),
                    _ => None,
                };
                (event.borrow_mut())(item);
                if let Some(failed) = closed {
                    return if failed {
                        Err(anyhow!("The call disconnected. Try again."))
                    } else {
                        Ok(())
                    };
                }
            }
        };
        let heartbeat = async {
            loop {
                self.sequence.set(self.sequence.get() + 1);
                let sequence = self.sequence.get();
                let ack: wire::Ack = self
                    .rpc(
                        "ReportVoiceMediaV2",
                        wire::Report {
                            lease: lease.clone(),
                            sequence,
                            muted: self.media.muted.get(),
                            state: wire::MediaState::Ready,
                        },
                        10_000,
                    )
                    .await?;
                if ack.sequence != sequence {
                    return Err(anyhow!("The call disconnected. Try again."));
                }
                sleep(5_000).await;
            }
        };
        futures::pin_mut!(operation, watch, heartbeat);
        match select(operation, select(watch, heartbeat)).await {
            Either::Left((result, _)) => result,
            Either::Right((Either::Left((result, _)), _)) => result,
            Either::Right((Either::Right((result, _)), _)) => result,
        }
    }
}
impl Drop for Call {
    fn drop(&mut self) {
        self.stop();
        if let (Some(window), Some(listener)) =
            (web_sys::window(), self.pagehide.borrow_mut().take())
        {
            let _ = window
                .remove_event_listener_with_callback("pagehide", listener.as_ref().unchecked_ref());
        }
    }
}

pub fn current(event: &VoiceEvent, lease: &wire::Lease) -> bool {
    match event {
        VoiceEvent::Snapshot { snapshot } => {
            snapshot.session_id == lease.voice.session_id
                && snapshot.generation == lease.voice.generation
        }
        VoiceEvent::Partial { generation, .. }
        | VoiceEvent::Closed { generation, .. }
        | VoiceEvent::InvalidatePlayout { generation, .. }
        | VoiceEvent::Levels { generation, .. } => *generation == lease.voice.generation,
        VoiceEvent::Final { transcript } => transcript.session_id == lease.voice.session_id,
        VoiceEvent::Audio { .. } => false,
    }
}
