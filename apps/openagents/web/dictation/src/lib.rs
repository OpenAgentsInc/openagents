//! Dictation for the GPUI web chat (#12292).
//!
//! The chat asks for it with `oa-dictation-start` (the chat's computer) and
//! `oa-dictation-stop`; the page's script loads this module and calls
//! [`web::start`] with an `AudioContext` it made during the click, so
//! Safari lets it run. The module records the microphone, brings it to
//! 16 kHz mono ([`Downsampler`]), and sends it in chunks to that computer
//! (`DictationAppend`), which transcribes it on Stop (`DictationFinish`).
//! Each step reaches the page as an `oa-dictation` event whose detail is
//! JSON: `{state: "listening" | "transcribing" | "done" | "failed", text,
//! message}`. The audio goes to the user's own computer and nowhere else.

/// The rate the computer's model takes.
pub const RATE: u32 = 16_000;
/// Samples a chunk carries: four seconds, 128 KB before base64.
pub const CHUNK: usize = RATE as usize * 4;
/// The longest recording, as on the desktop.
pub const MAX_SAMPLES: usize = RATE as usize * 60;

/// Brings the microphone's rate down to [`RATE`], averaging the samples
/// each output sample covers, across the buffers the browser hands over.
#[derive(Debug)]
pub struct Downsampler {
    step: f64,
    /// How far into the next output sample the input has come.
    filled: f64,
    sum: f64,
    count: u32,
}

impl Downsampler {
    pub fn new(from: f32) -> Self {
        Self {
            step: f64::from(RATE) / f64::from(from.max(RATE as f32)),
            filled: 0.0,
            sum: 0.0,
            count: 0,
        }
    }

    /// Adds `input`, pushing each finished sample onto `out`.
    pub fn push(&mut self, input: &[f32], out: &mut Vec<i16>) {
        for &sample in input {
            self.sum += f64::from(sample);
            self.count += 1;
            self.filled += self.step;
            if self.filled >= 1.0 {
                self.filled -= 1.0;
                let mean = self.sum / f64::from(self.count);
                out.push((mean.clamp(-1.0, 1.0) * 32_767.0).round() as i16);
                self.sum = 0.0;
                self.count = 0;
            }
        }
    }
}

/// A chunk's `pcm` param: little-endian samples, base64.
pub fn pcm(samples: &[i16]) -> String {
    use base64::Engine as _;
    let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(target_family = "wasm")]
pub mod web {
    use std::cell::RefCell;
    use std::rc::Rc;

    use futures::StreamExt as _;
    use futures::channel::mpsc;
    use openagents_web_client::web::Client;
    use serde_json::json;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;

    use super::{CHUNK, Downsampler, MAX_SAMPLES, pcm};

    enum Step {
        Chunk(Vec<i16>),
        Finish,
        Cancel,
    }

    /// The rate converter, the samples not sent yet, and how many there
    /// have been in all.
    type Captured = Rc<RefCell<(Downsampler, Vec<i16>, usize)>>;

    struct Recording {
        steps: mpsc::UnboundedSender<Step>,
        context: web_sys::AudioContext,
        stream: Option<web_sys::MediaStream>,
        captured: Option<Captured>,
        _process: Option<Closure<dyn FnMut(web_sys::AudioProcessingEvent)>>,
        _processor: Option<web_sys::ScriptProcessorNode>,
    }

    thread_local! {
        static NOW: RefCell<Option<Recording>> = const { RefCell::new(None) };
    }

    fn send(detail: serde_json::Value) {
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&JsValue::from_str(&detail.to_string()));
        if let Some(window) = web_sys::window()
            && let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("oa-dictation", &init)
        {
            let _ = window.dispatch_event(&event);
        }
    }

    fn failed(message: &str) {
        send(json!({ "state": "failed", "message": message }));
    }

    fn release() {
        if let Some(recording) = NOW.with(|now| now.borrow_mut().take()) {
            if let Some(stream) = &recording.stream {
                for track in stream.get_tracks().iter() {
                    if let Ok(track) = track.dyn_into::<web_sys::MediaStreamTrack>() {
                        track.stop();
                    }
                }
            }
            let _ = recording.context.close();
        }
    }

    /// Records for the chat's computer `device`, with the page's `context`.
    #[wasm_bindgen]
    pub fn start(device: String, context: web_sys::AudioContext) {
        console_error_panic_hook::set_once();
        release();
        let (steps, mut queue) = mpsc::unbounded();
        NOW.with(|now| {
            *now.borrow_mut() = Some(Recording {
                steps,
                context: context.clone(),
                stream: None,
                captured: None,
                _process: None,
                _processor: None,
            })
        });
        let name = format!("{:x}", js_sys::Math::random().to_bits());
        // Sends the chunks in order, then asks for the text.
        wasm_bindgen_futures::spawn_local({
            let device = device.clone();
            async move {
                let client = Client::from_page();
                let computer =
                    match openagents_web_client::computer::connect(&client, &device, || {}).await {
                        Ok(computer) => computer,
                        Err(error) => {
                            release();
                            return failed(&error.to_string());
                        }
                    };
                let ready = computer
                    .call("DictationStatus", json!({}))
                    .await
                    .is_ok_and(|status| status["ready"] == true);
                if !ready {
                    release();
                    return failed(
                        "Dictation isn't set up on this computer. Download its model in the OpenAgents app's Voice settings.",
                    );
                }
                let mut seq = 0u32;
                while let Some(step) = queue.next().await {
                    let result = match step {
                        Step::Chunk(samples) => {
                            let params =
                                json!({"recording": name, "seq": seq, "pcm": pcm(&samples)});
                            seq += 1;
                            computer.call("DictationAppend", params).await.map(|_| ())
                        }
                        Step::Cancel => {
                            let _ = computer
                                .call("DictationCancel", json!({"recording": name}))
                                .await;
                            return;
                        }
                        Step::Finish if seq == 0 => {
                            return send(json!({ "state": "done", "text": "" }));
                        }
                        Step::Finish => {
                            send(json!({ "state": "transcribing" }));
                            match computer
                                .call("DictationFinish", json!({"recording": name}))
                                .await
                            {
                                Ok(answer) => send(json!({
                                    "state": "done",
                                    "text": answer["text"].as_str().unwrap_or_default(),
                                })),
                                Err(error) => failed(&error.to_string()),
                            }
                            return;
                        }
                    };
                    if let Err(error) = result {
                        release();
                        let _ = computer
                            .call("DictationCancel", json!({"recording": name}))
                            .await;
                        return failed(&error.to_string());
                    }
                }
            }
        });
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(message) = listen(context).await {
                release();
                failed(&message);
            }
        });
    }

    async fn listen(context: web_sys::AudioContext) -> Result<(), String> {
        let denied = "Allow the microphone for this site to dictate.";
        let devices = web_sys::window()
            .and_then(|w| w.navigator().media_devices().ok())
            .ok_or_else(|| "This browser can't record.".to_string())?;
        let constraints = web_sys::MediaStreamConstraints::new();
        constraints.set_audio(&JsValue::TRUE);
        let promise = devices
            .get_user_media_with_constraints(&constraints)
            .map_err(|_| denied.to_string())?;
        let stream: web_sys::MediaStream = JsFuture::from(promise)
            .await
            .map_err(|_| denied.to_string())?
            .unchecked_into();
        if NOW.with(|now| now.borrow().is_none()) {
            // Stopped before the microphone opened.
            for track in stream.get_tracks().iter() {
                if let Ok(track) = track.dyn_into::<web_sys::MediaStreamTrack>() {
                    track.stop();
                }
            }
            return Ok(());
        }
        let _ = context.resume();
        let source = context
            .create_media_stream_source(&stream)
            .map_err(|_| "The microphone didn't start.".to_string())?;
        let processor = context
            .create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(4096, 1, 1)
            .map_err(|_| "The microphone didn't start.".to_string())?;
        let captured: Captured = Rc::new(RefCell::new((
            Downsampler::new(context.sample_rate()),
            Vec::<i16>::new(),
            0usize,
        )));
        let down = captured.clone();
        let process = Closure::<dyn FnMut(web_sys::AudioProcessingEvent)>::new(
            move |event: web_sys::AudioProcessingEvent| {
                let Ok(buffer) = event.input_buffer() else {
                    return;
                };
                let Ok(input) = buffer.get_channel_data(0) else {
                    return;
                };
                let mut held = down.borrow_mut();
                let (sampler, pending, total) = &mut *held;
                let before = pending.len();
                sampler.push(&input, pending);
                *total += pending.len() - before;
                if *total > MAX_SAMPLES {
                    pending.truncate(pending.len().saturating_sub(*total - MAX_SAMPLES));
                }
                let full = *total >= MAX_SAMPLES;
                if pending.len() >= CHUNK || full {
                    let chunk = std::mem::take(pending);
                    NOW.with(|now| {
                        if let Some(r) = now.borrow().as_ref() {
                            let _ = r.steps.unbounded_send(Step::Chunk(chunk));
                        }
                    });
                }
                if full {
                    // Not from inside this callback, which Stop drops.
                    wasm_bindgen_futures::spawn_local(async { stop() });
                }
            },
        );
        processor.set_onaudioprocess(Some(process.as_ref().unchecked_ref()));
        source
            .connect_with_audio_node(&processor)
            .and_then(|_| processor.connect_with_audio_node(&context.destination()))
            .map_err(|_| "The microphone didn't start.".to_string())?;
        NOW.with(|now| {
            if let Some(r) = now.borrow_mut().as_mut() {
                r.stream = Some(stream);
                r.captured = Some(captured);
                r._process = Some(process);
                r._processor = Some(processor);
            }
        });
        send(serde_json::json!({ "state": "listening" }));
        Ok(())
    }

    /// Stops recording and asks the computer for the text.
    #[wasm_bindgen]
    pub fn stop() {
        NOW.with(|now| {
            if let Some(r) = now.borrow().as_ref() {
                // The last part of a chunk, then the text.
                if let Some(captured) = &r.captured {
                    let rest = std::mem::take(&mut captured.borrow_mut().1);
                    if !rest.is_empty() {
                        let _ = r.steps.unbounded_send(Step::Chunk(rest));
                    }
                }
                let _ = r.steps.unbounded_send(Step::Finish);
            }
        });
        release();
    }

    /// Stops recording and drops what was recorded.
    #[wasm_bindgen]
    pub fn cancel() {
        NOW.with(|now| {
            if let Some(r) = now.borrow().as_ref() {
                let _ = r.steps.unbounded_send(Step::Cancel);
            }
        });
        release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_microphone_comes_down_to_the_models_rate() {
        let mut down = Downsampler::new(48_000.0);
        let mut out = Vec::new();
        // One second at 48 kHz, in the browser's 4096-sample buffers.
        let second = vec![0.5f32; 48_000];
        for buffer in second.chunks(4096) {
            down.push(buffer, &mut out);
        }
        assert!((15_999..=16_000).contains(&out.len()), "{}", out.len());
        assert!(out.iter().all(|&s| s == 16_384));
        // A rate at or below the model's passes through.
        let mut same = Downsampler::new(16_000.0);
        let mut out = Vec::new();
        same.push(&[1.0, -1.0, 2.0], &mut out);
        assert_eq!(out, vec![32_767, -32_767, 32_767]);
        assert_eq!(pcm(&[1, -1]), "AQD//w==");
    }
}
