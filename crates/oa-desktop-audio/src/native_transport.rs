//! Real process-opaque media backend. OS device handles, PCM, the authenticated
//! WebSocket, and playback queues stay in this process; callers receive only
//! bounded lifecycle events.

use crate::{VoiceIdentity, AUDIO_PROTOCOL_VERSION, MAX_AUDIO_PAYLOAD_BYTES};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{bounded, Receiver, RecvTimeoutError, Sender};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    io::ErrorKind,
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tungstenite::{
    client::IntoClientRequest, connect, stream::MaybeTlsStream, Error as WsError, Message,
    WebSocket,
};

pub const GOOGLE_AUDIO_CHUNK_LIMIT: usize = 15_360;
const CAPTURE_QUEUE_PACKETS: usize = 32;
const PLAYBACK_QUEUE_SAMPLES: usize = 48_000 * 2;

#[derive(Clone)]
pub struct NativeTransportConfig {
    pub gateway_url: String,
    pub application_grant: String,
}

impl std::fmt::Debug for NativeTransportConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeTransportConfig")
            .field("gateway_url", &"redacted")
            .field("application_grant", &"redacted")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeTransportEvent {
    Live,
    Ack {
        generation: u32,
        sequence: u64,
    },
    Sent {
        generation: u32,
        sequence: u64,
        payload_length: usize,
        sha256: String,
    },
    Backpressured,
    DeviceChanged,
    Offline,
    Revoked,
    Crashed,
}

pub struct NativeTransportHandle {
    stop: Arc<AtomicBool>,
    capture: Option<cpal::Stream>,
    playback: Option<cpal::Stream>,
    worker: Option<JoinHandle<()>>,
    capture_enabled: Arc<AtomicBool>,
}

impl NativeTransportHandle {
    pub fn set_capture_enabled(&self, enabled: bool) -> Result<(), String> {
        self.capture_enabled.store(enabled, Ordering::Release);
        let stream = self.capture.as_ref().ok_or("capture_closed")?;
        if enabled {
            stream.play().map_err(|_| "capture_state_failed".into())
        } else {
            stream.pause().map_err(|_| "capture_state_failed".into())
        }
    }
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Release);
        self.capture.take();
        self.playback.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for NativeTransportHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Default)]
pub struct MonoResampler {
    accumulator: u64,
}

impl MonoResampler {
    pub fn convert_f32(
        &mut self,
        interleaved: &[f32],
        channels: usize,
        input_rate: u32,
        output_rate: u32,
    ) -> Vec<i16> {
        if channels == 0 || input_rate == 0 || output_rate == 0 {
            return Vec::new();
        }
        let mut output = Vec::with_capacity(interleaved.len() / channels);
        for frame in interleaved.chunks_exact(channels) {
            let mono = frame.iter().copied().sum::<f32>() / channels as f32;
            self.accumulator += output_rate as u64;
            while self.accumulator >= input_rate as u64 {
                self.accumulator -= input_rate as u64;
                output.push((mono.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
            }
        }
        output
    }
}

fn send_capture(
    sender: &Sender<Vec<u8>>,
    samples: Vec<i16>,
    events: &Sender<NativeTransportEvent>,
) {
    for chunk in samples.chunks(GOOGLE_AUDIO_CHUNK_LIMIT / 2) {
        let mut bytes = Vec::with_capacity(chunk.len() * 2);
        for sample in chunk {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        if sender.try_send(bytes).is_err() {
            let _ = events.try_send(NativeTransportEvent::Backpressured);
        }
    }
}

fn build_input_stream(
    sender: Sender<Vec<u8>>,
    events: Sender<NativeTransportEvent>,
) -> Result<cpal::Stream, String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("microphone_unavailable")?;
    let supported = device
        .default_input_config()
        .map_err(|_| "microphone_config_unavailable")?;
    let config: cpal::StreamConfig = supported.clone().into();
    let channels = config.channels as usize;
    let rate = config.sample_rate.0;
    let resampler = Arc::new(Mutex::new(MonoResampler::default()));
    let on_error = {
        let events = events.clone();
        move |_| {
            let _ = events.try_send(NativeTransportEvent::DeviceChanged);
        }
    };
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            let resampler = resampler.clone();
            let events = events.clone();
            device.build_input_stream(
                &config,
                move |data: &[f32], _| {
                    send_capture(
                        &sender,
                        resampler
                            .lock()
                            .unwrap()
                            .convert_f32(data, channels, rate, 16_000),
                        &events,
                    )
                },
                on_error,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let resampler = resampler.clone();
            let events = events.clone();
            device.build_input_stream(
                &config,
                move |data: &[i16], _| {
                    let values: Vec<f32> =
                        data.iter().map(|v| *v as f32 / i16::MAX as f32).collect();
                    send_capture(
                        &sender,
                        resampler
                            .lock()
                            .unwrap()
                            .convert_f32(&values, channels, rate, 16_000),
                        &events,
                    )
                },
                on_error,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let resampler = resampler.clone();
            let events = events.clone();
            device.build_input_stream(
                &config,
                move |data: &[u16], _| {
                    let values: Vec<f32> = data
                        .iter()
                        .map(|v| (*v as f32 / u16::MAX as f32) * 2.0 - 1.0)
                        .collect();
                    send_capture(
                        &sender,
                        resampler
                            .lock()
                            .unwrap()
                            .convert_f32(&values, channels, rate, 16_000),
                        &events,
                    )
                },
                on_error,
                None,
            )
        }
        _ => return Err("microphone_format_unsupported".into()),
    }
    .map_err(|_| "microphone_open_failed")?;
    stream.play().map_err(|_| "microphone_start_failed")?;
    Ok(stream)
}

fn build_output_stream(
    queue: Arc<Mutex<VecDeque<f32>>>,
    events: Sender<NativeTransportEvent>,
) -> Result<(cpal::Stream, u32), String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("playback_unavailable")?;
    let supported = device
        .default_output_config()
        .map_err(|_| "playback_config_unavailable")?;
    let config: cpal::StreamConfig = supported.clone().into();
    let channels = config.channels as usize;
    let on_error = move |_| {
        let _ = events.try_send(NativeTransportEvent::DeviceChanged);
    };
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config,
            move |out: &mut [f32], _| fill_output(out, channels, &queue, |v| v),
            on_error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config,
            move |out: &mut [i16], _| {
                fill_output(out, channels, &queue, |v| (v * i16::MAX as f32) as i16)
            },
            on_error,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config,
            move |out: &mut [u16], _| {
                fill_output(out, channels, &queue, |v| {
                    ((v + 1.0) * 0.5 * u16::MAX as f32) as u16
                })
            },
            on_error,
            None,
        ),
        _ => return Err("playback_format_unsupported".into()),
    }
    .map_err(|_| "playback_open_failed")?;
    stream.play().map_err(|_| "playback_start_failed")?;
    let rate = config.sample_rate.0;
    Ok((stream, rate))
}

fn fill_output<T: Copy>(
    output: &mut [T],
    channels: usize,
    queue: &Arc<Mutex<VecDeque<f32>>>,
    convert: impl Fn(f32) -> T,
) {
    let mut queue = queue.lock().unwrap();
    for frame in output.chunks_mut(channels) {
        let sample = queue.pop_front().unwrap_or(0.0);
        for channel in frame {
            *channel = convert(sample);
        }
    }
}

fn set_nonblocking(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>) -> std::io::Result<()> {
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => stream.set_nonblocking(true),
        MaybeTlsStream::Rustls(stream) => stream.sock.set_nonblocking(true),
        _ => Err(std::io::Error::new(
            ErrorKind::Unsupported,
            "unsupported_tls_backend",
        )),
    }
}

fn media_frame(identity: &VoiceIdentity, sequence: u64, payload: &[u8]) -> Vec<u8> {
    let digest = format!("{:x}", Sha256::digest(payload));
    let header = serde_json::to_vec(&json!({ "schema": AUDIO_PROTOCOL_VERSION, "kind": "client_audio", "identity": { "ownerRef": identity.owner_ref, "deviceRef": identity.device_ref, "threadRef": identity.thread_ref, "sessionRef": identity.session_ref, "generation": identity.generation }, "sequence": sequence, "codec": "pcm_s16le", "sampleRateHz": 16_000, "channels": 1, "payloadLength": payload.len(), "sha256": digest })).unwrap();
    let mut frame = Vec::with_capacity(8 + header.len() + payload.len());
    frame.extend_from_slice(b"OAA1");
    frame.extend_from_slice(&(header.len() as u32).to_be_bytes());
    frame.extend_from_slice(&header);
    frame.extend_from_slice(payload);
    frame
}

fn enqueue_tts(
    frame: &[u8],
    queue: &Arc<Mutex<VecDeque<f32>>>,
    output_rate: u32,
    resampler: &mut MonoResampler,
) {
    if frame.len() < 8 || &frame[..4] != b"OAA1" {
        return;
    }
    let header_len = u32::from_be_bytes(frame[4..8].try_into().unwrap()) as usize;
    if header_len > 8_192 || frame.len() < 8 + header_len {
        return;
    }
    let Ok(header) = serde_json::from_slice::<Value>(&frame[8..8 + header_len]) else {
        return;
    };
    if header.get("kind").and_then(Value::as_str) != Some("server_tts") {
        return;
    }
    let payload = &frame[8 + header_len..];
    if payload.len() > MAX_AUDIO_PAYLOAD_BYTES as usize || !payload.len().is_multiple_of(2) {
        return;
    }
    let digest = format!("{:x}", Sha256::digest(payload));
    if header.get("sha256").and_then(Value::as_str) != Some(&digest) {
        return;
    }
    let input_rate = header
        .get("sampleRateHz")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    if !matches!(input_rate, 24_000 | 48_000) {
        return;
    }
    let input: Vec<f32> = payload
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / i16::MAX as f32)
        .collect();
    let samples = resampler.convert_f32(&input, 1, input_rate, output_rate);
    let mut queue = queue.lock().unwrap();
    for sample in samples {
        if queue.len() >= PLAYBACK_QUEUE_SAMPLES {
            break;
        }
        queue.push_back(sample as f32 / i16::MAX as f32);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkerExit {
    Network,
    Revoked,
    Stopped,
}

#[allow(clippy::too_many_arguments)]
fn worker_once(
    config: &NativeTransportConfig,
    identity: &VoiceIdentity,
    capture: &Receiver<Vec<u8>>,
    playback: &Arc<Mutex<VecDeque<f32>>>,
    playback_rate: u32,
    events: &Sender<NativeTransportEvent>,
    stop: &Arc<AtomicBool>,
    capture_enabled: &Arc<AtomicBool>,
    sequence: &mut u64,
    acknowledged: &mut Option<u64>,
    playback_resampler: &mut MonoResampler,
) -> WorkerExit {
    let mut request = match config.gateway_url.as_str().into_client_request() {
        Ok(value) => value,
        Err(_) => {
            let _ = events.send(NativeTransportEvent::Crashed);
            return WorkerExit::Revoked;
        }
    };
    let Ok(grant) = config.application_grant.parse() else {
        let _ = events.send(NativeTransportEvent::Revoked);
        return WorkerExit::Revoked;
    };
    request
        .headers_mut()
        .insert("x-openagents-audio-grant", grant);
    let (mut socket, _) = match connect(request) {
        Ok(value) => value,
        Err(_) => {
            let _ = events.send(NativeTransportEvent::Offline);
            return WorkerExit::Network;
        }
    };
    if set_nonblocking(&mut socket).is_err() {
        let _ = events.send(NativeTransportEvent::Crashed);
        return WorkerExit::Revoked;
    }
    let _ = events.send(NativeTransportEvent::Live);
    while !stop.load(Ordering::Acquire) {
        match capture.recv_timeout(Duration::from_millis(5)) {
            Ok(payload) => {
                if !capture_enabled.load(Ordering::Acquire) {
                    continue;
                }
                let outstanding =
                    sequence.saturating_sub(acknowledged.map_or(0, |value| value + 1));
                if outstanding >= CAPTURE_QUEUE_PACKETS as u64 {
                    let _ = events.try_send(NativeTransportEvent::Backpressured);
                    continue;
                }
                let digest = format!("{:x}", Sha256::digest(&payload));
                if socket
                    .send(Message::Binary(
                        media_frame(identity, *sequence, &payload).into(),
                    ))
                    .is_err()
                {
                    let _ = events.send(NativeTransportEvent::Offline);
                    return WorkerExit::Network;
                }
                let _ = events.try_send(NativeTransportEvent::Sent {
                    generation: identity.generation,
                    sequence: *sequence,
                    payload_length: payload.len(),
                    sha256: digest,
                });
                *sequence += 1;
            }
            Err(RecvTimeoutError::Disconnected) => return WorkerExit::Stopped,
            Err(RecvTimeoutError::Timeout) => {}
        }
        loop {
            match socket.read() {
                Ok(Message::Text(text)) => {
                    if let Ok(frame) = serde_json::from_str::<Value>(&text) {
                        if frame.get("_tag").and_then(Value::as_str) == Some("ack") {
                            if let Some(ack) = frame
                                .get("acknowledgedClientSequence")
                                .and_then(Value::as_u64)
                                .filter(|ack| *ack < *sequence)
                            {
                                *acknowledged =
                                    Some(acknowledged.map_or(ack, |current| current.max(ack)));
                                let _ = events.try_send(NativeTransportEvent::Ack {
                                    generation: identity.generation,
                                    sequence: ack,
                                });
                            }
                        }
                    }
                }
                Ok(Message::Binary(frame)) => {
                    enqueue_tts(&frame, playback, playback_rate, playback_resampler)
                }
                Ok(Message::Close(_)) => {
                    let _ = events.send(NativeTransportEvent::Revoked);
                    return WorkerExit::Revoked;
                }
                Ok(_) => {}
                Err(WsError::Io(error)) if error.kind() == ErrorKind::WouldBlock => break,
                Err(_) => {
                    let _ = events.send(NativeTransportEvent::Offline);
                    return WorkerExit::Network;
                }
            }
        }
    }
    let _ = socket.close(None);
    WorkerExit::Stopped
}

#[allow(clippy::too_many_arguments)]
fn worker(
    config: NativeTransportConfig,
    identity: VoiceIdentity,
    capture: Receiver<Vec<u8>>,
    playback: Arc<Mutex<VecDeque<f32>>>,
    playback_rate: u32,
    events: Sender<NativeTransportEvent>,
    stop: Arc<AtomicBool>,
    capture_enabled: Arc<AtomicBool>,
) {
    let mut sequence = 0u64;
    let mut acknowledged = None;
    let mut playback_resampler = MonoResampler::default();
    for attempt in 0..3u64 {
        match worker_once(
            &config,
            &identity,
            &capture,
            &playback,
            playback_rate,
            &events,
            &stop,
            &capture_enabled,
            &mut sequence,
            &mut acknowledged,
            &mut playback_resampler,
        ) {
            WorkerExit::Network if attempt < 2 && !stop.load(Ordering::Acquire) => {
                thread::sleep(Duration::from_millis(50 * (1 << attempt)))
            }
            _ => return,
        }
    }
}

pub fn start_native_transport(
    config: NativeTransportConfig,
    identity: VoiceIdentity,
) -> Result<(NativeTransportHandle, Receiver<NativeTransportEvent>), String> {
    if !config.gateway_url.starts_with("wss://")
        && !config.gateway_url.starts_with("ws://127.0.0.1:")
    {
        return Err("gateway_url_refused".into());
    }
    if config.application_grant.is_empty() || config.application_grant.len() > 4096 {
        return Err("gateway_grant_refused".into());
    }
    let (capture_tx, capture_rx) = bounded(CAPTURE_QUEUE_PACKETS);
    let (event_tx, event_rx) = bounded(64);
    let playback_queue = Arc::new(Mutex::new(VecDeque::with_capacity(PLAYBACK_QUEUE_SAMPLES)));
    let capture = build_input_stream(capture_tx, event_tx.clone())?;
    let (playback, playback_rate) = build_output_stream(playback_queue.clone(), event_tx.clone())?;
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let capture_enabled = Arc::new(AtomicBool::new(true));
    let worker_capture_enabled = capture_enabled.clone();
    let worker = thread::spawn(move || {
        worker(
            config,
            identity,
            capture_rx,
            playback_queue,
            playback_rate,
            event_tx,
            worker_stop,
            worker_capture_enabled,
        )
    });
    Ok((
        NativeTransportHandle {
            stop,
            capture: Some(capture),
            playback: Some(playback),
            worker: Some(worker),
            capture_enabled,
        },
        event_rx,
    ))
}

#[cfg(test)]
#[allow(clippy::result_large_err)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use tungstenite::{
        accept_hdr,
        handshake::server::{Request, Response},
    };
    #[test]
    fn stereo_48k_downmixes_to_mono_16k() {
        let input = [1.0f32, -1.0].repeat(480);
        let out = MonoResampler::default().convert_f32(&input, 2, 48_000, 16_000);
        assert_eq!(out.len(), 160);
        assert!(out.iter().all(|v| *v == 0));
    }
    #[test]
    fn media_frame_is_audio1_bounded_and_digest_bound() {
        let identity = VoiceIdentity {
            owner_ref: "o".into(),
            device_ref: "d".into(),
            thread_ref: "t".into(),
            session_ref: "s".into(),
            generation: 1,
        };
        let payload = [1u8, 2, 3, 4];
        let frame = media_frame(&identity, 9, &payload);
        assert_eq!(&frame[..4], b"OAA1");
        let n = u32::from_be_bytes(frame[4..8].try_into().unwrap()) as usize;
        let header: Value = serde_json::from_slice(&frame[8..8 + n]).unwrap();
        assert_eq!(header["sequence"], 9);
        assert_eq!(&frame[8 + n..], payload);
    }
    #[test]
    fn transport_refuses_non_tls_remote_and_unbounded_grant_before_device_access() {
        let identity = VoiceIdentity {
            owner_ref: "o".into(),
            device_ref: "d".into(),
            thread_ref: "t".into(),
            session_ref: "s".into(),
            generation: 1,
        };
        assert!(
            matches!(start_native_transport(NativeTransportConfig { gateway_url: "ws://example.com/v1/stream".into(), application_grant: "x".into() }, identity.clone()), Err(ref reason) if reason == "gateway_url_refused")
        );
        assert!(
            matches!(start_native_transport(NativeTransportConfig { gateway_url: "wss://audio.example/v1/stream".into(), application_grant: "x".repeat(4097) }, identity), Err(ref reason) if reason == "gateway_grant_refused")
        );
    }
    #[test]
    fn direct_authenticated_loopback_websocket_sends_audio1_and_receives_ack() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = accept_hdr(stream, |request: &Request, response: Response| {
                assert_eq!(
                    request.headers()["x-openagents-audio-grant"],
                    "short-lived-grant"
                );
                Ok(response)
            })
            .unwrap();
            let Message::Binary(frame) = socket.read().unwrap() else {
                panic!("binary frame required")
            };
            assert_eq!(&frame[..4], b"OAA1");
            let n = u32::from_be_bytes(frame[4..8].try_into().unwrap()) as usize;
            let header: Value = serde_json::from_slice(&frame[8..8 + n]).unwrap();
            assert_eq!(header["sequence"], 0);
            assert_eq!(&frame[8 + n..], &[3, 0, 4, 0]);
            socket
                .send(Message::Text(
                    json!({"_tag":"ack","acknowledgedClientSequence":0})
                        .to_string()
                        .into(),
                ))
                .unwrap();
            let _ = socket.close(None);
        });
        let identity = VoiceIdentity {
            owner_ref: "o".into(),
            device_ref: "d".into(),
            thread_ref: "t".into(),
            session_ref: "s".into(),
            generation: 9,
        };
        let (capture_tx, capture_rx) = bounded(2);
        let (event_tx, event_rx) = bounded(16);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let playback = Arc::new(Mutex::new(VecDeque::new()));
        let capture_enabled = Arc::new(AtomicBool::new(false));
        let worker_capture_enabled = capture_enabled.clone();
        let client = thread::spawn(move || {
            worker(
                NativeTransportConfig {
                    gateway_url: format!("ws://{address}/v1/stream"),
                    application_grant: "short-lived-grant".into(),
                },
                identity,
                capture_rx,
                playback,
                48_000,
                event_tx,
                worker_stop,
                worker_capture_enabled,
            )
        });
        assert_eq!(
            event_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            NativeTransportEvent::Live
        );
        capture_tx.send(vec![1, 0, 2, 0]).unwrap();
        thread::sleep(Duration::from_millis(20));
        capture_enabled.store(true, Ordering::Release);
        capture_tx.send(vec![3, 0, 4, 0]).unwrap();
        let mut saw_live = true;
        let mut saw_sent = false;
        let mut saw_ack = false;
        for _ in 0..8 {
            match event_rx.recv_timeout(Duration::from_secs(1)).unwrap() {
                NativeTransportEvent::Live => saw_live = true,
                NativeTransportEvent::Sent {
                    sequence: 0,
                    generation: 9,
                    ..
                } => saw_sent = true,
                NativeTransportEvent::Ack {
                    sequence: 0,
                    generation: 9,
                } => {
                    saw_ack = true;
                    break;
                }
                _ => {}
            }
        }
        stop.store(true, Ordering::Release);
        client.join().unwrap();
        server.join().unwrap();
        assert!(saw_live && saw_sent && saw_ack);
    }
    #[test]
    fn validated_tts_pcm_is_resampled_into_bounded_playback_queue() {
        let payload: Vec<u8> = [0i16, i16::MAX, i16::MIN, 1000]
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect();
        let digest = format!("{:x}", Sha256::digest(&payload));
        let header =
            serde_json::to_vec(&json!({"kind":"server_tts","sampleRateHz":24_000,"sha256":digest}))
                .unwrap();
        let mut frame = b"OAA1".to_vec();
        frame.extend_from_slice(&(header.len() as u32).to_be_bytes());
        frame.extend_from_slice(&header);
        frame.extend_from_slice(&payload);
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        enqueue_tts(&frame, &queue, 48_000, &mut MonoResampler::default());
        assert_eq!(queue.lock().unwrap().len(), 8);
        let mut corrupt = frame;
        *corrupt.last_mut().unwrap() ^= 1;
        let before = queue.lock().unwrap().len();
        enqueue_tts(&corrupt, &queue, 48_000, &mut MonoResampler::default());
        assert_eq!(queue.lock().unwrap().len(), before);
    }
    #[test]
    fn reconnect_is_bounded_to_three_generation_preserving_attempts() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let identity = VoiceIdentity {
            owner_ref: "o".into(),
            device_ref: "d".into(),
            thread_ref: "t".into(),
            session_ref: "s".into(),
            generation: 5,
        };
        let (_capture_tx, capture_rx) = bounded(1);
        let (event_tx, event_rx) = bounded(8);
        let stop = Arc::new(AtomicBool::new(false));
        let client = thread::spawn(move || {
            worker(
                NativeTransportConfig {
                    gateway_url: format!("ws://{address}/v1/stream"),
                    application_grant: "grant".into(),
                },
                identity,
                capture_rx,
                Arc::new(Mutex::new(VecDeque::new())),
                48_000,
                event_tx,
                stop,
                Arc::new(AtomicBool::new(true)),
            )
        });
        client.join().unwrap();
        assert_eq!(
            event_rx
                .try_iter()
                .filter(|event| *event == NativeTransportEvent::Offline)
                .count(),
            3
        );
    }
}
