use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BleepCategory {
    Background,
    Transition,
    Interaction,
    Notification,
    Voice,
}

#[derive(Clone, Debug, Default)]
pub struct BleepGeneralProps {
    pub preload: Option<bool>,
    pub async_load: Option<bool>,
    pub volume: Option<f32>,
    pub muted: Option<bool>,
    pub category: Option<BleepCategory>,
    pub fetch_headers: Option<BTreeMap<String, String>>,
    pub max_playback_delay: Option<Duration>,
    pub mute_on_window_blur: Option<bool>,
    pub disabled: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct BleepProps {
    pub sources: Vec<BleepSource>,
    pub looped: bool,
    pub category: Option<BleepCategory>,
    pub general: BleepGeneralProps,
}

#[derive(Clone, Debug, Default)]
pub struct BleepUpdate {
    pub volume: Option<f32>,
    pub muted: Option<bool>,
    pub disabled: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct BleepMasterProps {
    pub volume: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct BleepsManagerProps {
    pub master: Option<BleepMasterProps>,
    pub common: Option<BleepGeneralProps>,
    pub categories: BTreeMap<BleepCategory, BleepGeneralProps>,
    pub bleeps: BTreeMap<String, BleepProps>,
}

#[derive(Clone, Debug, Default)]
pub struct BleepsManagerUpdate {
    pub master: Option<BleepMasterProps>,
    pub common: Option<BleepGeneralProps>,
    pub categories: Option<BTreeMap<BleepCategory, BleepGeneralProps>>,
    pub bleeps: Option<BTreeMap<String, BleepUpdate>>,
}

#[derive(Clone, Debug)]
pub enum BleepSource {
    Path {
        path: std::path::PathBuf,
        mime: Option<String>,
    },
    Url {
        url: String,
        mime: Option<String>,
    },
    Bytes {
        data: Vec<u8>,
        mime: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedBleepSettings {
    preload: bool,
    async_load: bool,
    volume: f32,
    muted: bool,
    fetch_headers: Option<BTreeMap<String, String>>,
    max_playback_delay: Duration,
    mute_on_window_blur: bool,
    disabled: bool,
}

impl Default for ResolvedBleepSettings {
    fn default() -> Self {
        Self {
            preload: true,
            async_load: false,
            volume: 1.0,
            muted: false,
            fetch_headers: None,
            max_playback_delay: Duration::from_millis(250),
            mute_on_window_blur: false,
            disabled: false,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedBleepProps {
    sources: Vec<BleepSource>,
    looped: bool,
    settings: ResolvedBleepSettings,
    master_volume: f32,
}

trait BleepBackend: Sized {
    fn new(props: ResolvedBleepProps) -> Option<Self>;
    fn duration(&self) -> f32;
    fn volume(&self) -> f32;
    fn muted(&self) -> bool;
    fn is_loaded(&self) -> bool;
    fn is_playing(&self) -> bool;
    fn play(&self, caller: Option<&str>);
    fn stop(&self, caller: Option<&str>);
    fn load(&self);
    fn unload(&self);
    fn update(&self, settings: ResolvedBleepSettings, master_volume: f32);
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
    use std::io::Cursor;
    use std::sync::{Mutex, OnceLock};

    #[derive(Clone)]
    pub struct DesktopBleep {
        inner: Arc<Mutex<DesktopBleepInner>>,
    }

    struct DesktopBleepInner {
        props: ResolvedBleepProps,
        load_state: LoadState,
        data: Option<BleepData>,
        sink: Option<Sink>,
        callers: HashSet<String>,
        last_play_request: Option<Instant>,
        externally_muted: bool,
    }

    struct BleepData {
        bytes: Vec<u8>,
        duration: f32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum LoadState {
        Unloaded,
        Loading,
        Loaded,
        Error,
    }

    struct AudioEngine {
        _stream: OutputStream,
        handle: OutputStreamHandle,
    }

    // SAFETY: OutputStream is only held to keep the audio device alive. The handle
    // is used for playback control and is thread-safe in practice.
    unsafe impl Send for AudioEngine {}
    unsafe impl Sync for AudioEngine {}

    static AUDIO_ENGINE: OnceLock<Option<AudioEngine>> = OnceLock::new();

    fn audio_engine() -> Option<&'static AudioEngine> {
        AUDIO_ENGINE
            .get_or_init(|| {
                OutputStream::try_default()
                    .ok()
                    .map(|(stream, handle)| AudioEngine {
                        _stream: stream,
                        handle,
                    })
            })
            .as_ref()
    }

    impl DesktopBleep {
        fn with_lock<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&mut DesktopBleepInner) -> R,
        {
            let mut inner = self.inner.lock().expect("bleep lock");
            f(&mut inner)
        }

        fn effective_volume(inner: &DesktopBleepInner) -> f32 {
            if inner.props.settings.disabled || inner.props.settings.muted || inner.externally_muted
            {
                0.0
            } else {
                (inner.props.settings.volume * inner.props.master_volume).clamp(0.0, 1.0)
            }
        }

        fn start_playback(inner: &mut DesktopBleepInner) {
            if inner.props.settings.disabled || inner.props.settings.muted || inner.externally_muted
            {
                return;
            }

            let Some(engine) = audio_engine() else {
                return;
            };
            let Some(data) = inner.data.as_ref() else {
                return;
            };

            if inner.props.looped {
                if let Some(sink) = inner.sink.as_ref() {
                    if !sink.empty() {
                        return;
                    }
                }
            } else if let Some(sink) = inner.sink.take() {
                sink.stop();
            }

            let cursor = Cursor::new(data.bytes.clone());
            let decoder = match Decoder::new(cursor) {
                Ok(decoder) => decoder,
                Err(_) => return,
            };
            let source: Box<dyn Source<Item = i16> + Send> = if inner.props.looped {
                Box::new(decoder.repeat_infinite())
            } else {
                Box::new(decoder)
            };

            let sink = match Sink::try_new(&engine.handle) {
                Ok(sink) => sink,
                Err(_) => return,
            };
            sink.set_volume(Self::effective_volume(inner));
            sink.append(source);
            sink.play();
            inner.sink = Some(sink);
        }

        fn load_data(
            sources: &[BleepSource],
            fetch_headers: Option<&BTreeMap<String, String>>,
        ) -> Result<BleepData, ()> {
            for source in sources {
                let bytes = match source {
                    BleepSource::Bytes { data, .. } => data.clone(),
                    BleepSource::Path { path, .. } => std::fs::read(path).map_err(|_| ())?,
                    BleepSource::Url { url, .. } => {
                        let client = reqwest::blocking::Client::new();
                        let mut request = client.get(url);
                        if let Some(headers) = fetch_headers {
                            let mut header_map = HeaderMap::new();
                            for (key, value) in headers {
                                let Ok(name) = HeaderName::from_bytes(key.as_bytes()) else {
                                    continue;
                                };
                                let Ok(value) = HeaderValue::from_str(value) else {
                                    continue;
                                };
                                header_map.insert(name, value);
                            }
                            request = request.headers(header_map);
                        }
                        let response = request.send().map_err(|_| ())?;
                        response.bytes().map_err(|_| ())?.to_vec()
                    }
                };

                let cursor = Cursor::new(bytes.clone());
                let decoder = Decoder::new(cursor).map_err(|_| ())?;
                let duration = decoder
                    .total_duration()
                    .map(|d| d.as_secs_f32())
                    .unwrap_or(0.0);

                return Ok(BleepData { bytes, duration });
            }
            Err(())
        }
    }

    impl BleepBackend for DesktopBleep {
        fn new(props: ResolvedBleepProps) -> Option<Self> {
            if audio_engine().is_none() {
                return None;
            }
            let bleep = Self {
                inner: Arc::new(Mutex::new(DesktopBleepInner {
                    props,
                    load_state: LoadState::Unloaded,
                    data: None,
                    sink: None,
                    callers: HashSet::new(),
                    last_play_request: None,
                    externally_muted: false,
                })),
            };

            let preload = bleep.with_lock(|inner| inner.props.settings.preload);
            if preload {
                bleep.load();
            }
            Some(bleep)
        }

        fn duration(&self) -> f32 {
            self.with_lock(|inner| inner.data.as_ref().map(|d| d.duration).unwrap_or(0.0))
        }

        fn volume(&self) -> f32 {
            self.with_lock(|inner| inner.props.settings.volume)
        }

        fn muted(&self) -> bool {
            self.with_lock(|inner| inner.props.settings.muted)
        }

        fn is_loaded(&self) -> bool {
            self.with_lock(|inner| inner.load_state == LoadState::Loaded)
        }

        fn is_playing(&self) -> bool {
            self.with_lock(|inner| inner.sink.as_ref().map(|s| !s.empty()).unwrap_or(false))
        }

        fn play(&self, caller: Option<&str>) {
            let mut should_start = false;
            let mut trigger_load = false;
            let (async_load, max_delay) = self.with_lock(|inner| {
                if let Some(caller) = caller {
                    inner.callers.insert(caller.to_string());
                }
                inner.last_play_request = Some(Instant::now());
                if inner.load_state == LoadState::Loaded {
                    should_start = true;
                } else if inner.load_state == LoadState::Unloaded {
                    trigger_load = true;
                }
                (
                    inner.props.settings.async_load,
                    inner.props.settings.max_playback_delay,
                )
            });

            if should_start {
                self.with_lock(|inner| Self::start_playback(inner));
                return;
            }

            if trigger_load && async_load {
                self.with_lock(|inner| inner.load_state = LoadState::Loading);
                let inner = self.inner.clone();
                std::thread::spawn(move || {
                    let (sources, last_play, fetch_headers) = {
                        let inner = inner.lock().expect("bleep lock");
                        (
                            inner.props.sources.clone(),
                            inner.last_play_request,
                            inner.props.settings.fetch_headers.clone(),
                        )
                    };

                    let data = Self::load_data(&sources, fetch_headers.as_ref());
                    let mut inner = inner.lock().expect("bleep lock");
                    match data {
                        Ok(data) => {
                            inner.data = Some(data);
                            inner.load_state = LoadState::Loaded;
                            if let Some(last_play) = last_play {
                                if last_play.elapsed() <= max_delay {
                                    Self::start_playback(&mut inner);
                                }
                            }
                        }
                        Err(_) => {
                            inner.load_state = LoadState::Error;
                        }
                    }
                });
            } else if trigger_load {
                self.load();
                self.with_lock(|inner| Self::start_playback(inner));
            }
        }

        fn stop(&self, caller: Option<&str>) {
            self.with_lock(|inner| {
                if let Some(caller) = caller {
                    inner.callers.remove(caller);
                } else {
                    inner.callers.clear();
                }

                if inner.props.looped && !inner.callers.is_empty() {
                    return;
                }

                if let Some(sink) = inner.sink.take() {
                    sink.stop();
                }
            });
        }

        fn load(&self) {
            let async_load = self.with_lock(|inner| {
                if inner.load_state != LoadState::Unloaded {
                    return false;
                }
                inner.load_state = LoadState::Loading;
                inner.props.settings.async_load
            });

            if async_load {
                let inner = self.inner.clone();
                std::thread::spawn(move || {
                    let (sources, fetch_headers) = {
                        let inner = inner.lock().expect("bleep lock");
                        (
                            inner.props.sources.clone(),
                            inner.props.settings.fetch_headers.clone(),
                        )
                    };
                    let data = Self::load_data(&sources, fetch_headers.as_ref());
                    let mut inner = inner.lock().expect("bleep lock");
                    match data {
                        Ok(data) => {
                            inner.data = Some(data);
                            inner.load_state = LoadState::Loaded;
                        }
                        Err(_) => {
                            inner.load_state = LoadState::Error;
                        }
                    }
                });
            } else {
                let (sources, fetch_headers) = self.with_lock(|inner| {
                    (
                        inner.props.sources.clone(),
                        inner.props.settings.fetch_headers.clone(),
                    )
                });
                let data = Self::load_data(&sources, fetch_headers.as_ref());
                self.with_lock(|inner| match data {
                    Ok(data) => {
                        inner.data = Some(data);
                        inner.load_state = LoadState::Loaded;
                    }
                    Err(_) => {
                        inner.load_state = LoadState::Error;
                    }
                });
            }
        }

        fn unload(&self) {
            self.with_lock(|inner| {
                if let Some(sink) = inner.sink.take() {
                    sink.stop();
                }
                inner.data = None;
                inner.load_state = LoadState::Unloaded;
            });
        }

        fn update(&self, settings: ResolvedBleepSettings, master_volume: f32) {
            self.with_lock(|inner| {
                inner.props.settings = settings;
                inner.props.master_volume = master_volume;
                if inner.props.settings.disabled {
                    if let Some(sink) = inner.sink.take() {
                        sink.stop();
                    }
                }
                if let Some(sink) = inner.sink.as_ref() {
                    sink.set_volume(Self::effective_volume(inner));
                }
            });
        }
    }

    pub type BackendBleep = DesktopBleep;
}

#[cfg(all(target_arch = "wasm32", feature = "web"))]
mod backend {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen_futures::JsFuture;
    use wasm_bindgen_futures::spawn_local;
    use web_sys::{
        AudioBuffer, AudioBufferSourceNode, AudioContext, GainNode, Headers, Request, RequestInit,
    };

    #[derive(Clone)]
    pub struct WebBleep {
        inner: Rc<RefCell<WebBleepInner>>,
    }

    struct WebBleepInner {
        props: ResolvedBleepProps,
        context: AudioContext,
        gain: GainNode,
        buffer: Option<AudioBuffer>,
        source: Option<AudioBufferSourceNode>,
        on_end_closure: Option<Closure<dyn FnMut()>>,
        load_state: LoadState,
        callers: HashSet<String>,
        last_play_request: Option<f64>,
        externally_muted: bool,
        focus_closures: Option<(Closure<dyn FnMut()>, Closure<dyn FnMut()>)>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum LoadState {
        Unloaded,
        Loading,
        Loaded,
        Error,
    }

    impl WebBleep {
        fn with_lock<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&mut WebBleepInner) -> R,
        {
            let mut inner = self.inner.borrow_mut();
            f(&mut inner)
        }

        fn effective_volume(inner: &WebBleepInner) -> f32 {
            if inner.props.settings.disabled || inner.props.settings.muted || inner.externally_muted
            {
                0.0
            } else {
                (inner.props.settings.volume * inner.props.master_volume).clamp(0.0, 1.0)
            }
        }

        fn setup_focus_listeners(inner: &Rc<RefCell<WebBleepInner>>) {
            if !inner.borrow().props.settings.mute_on_window_blur
                || inner.borrow().focus_closures.is_some()
            {
                return;
            }

            let window = match web_sys::window() {
                Some(window) => window,
                None => return,
            };

            let inner_focus = inner.clone();
            let on_focus = Closure::wrap(Box::new(move || {
                let mut inner = inner_focus.borrow_mut();
                inner.externally_muted = false;
                let _ = inner.context.resume();
                let _ = inner
                    .gain
                    .gain()
                    .set_value(WebBleep::effective_volume(&inner));
            }) as Box<dyn FnMut()>);

            let inner_blur = inner.clone();
            let on_blur = Closure::wrap(Box::new(move || {
                let mut inner = inner_blur.borrow_mut();
                inner.externally_muted = true;
                let _ = inner.context.suspend();
                let _ = inner
                    .gain
                    .gain()
                    .set_value(WebBleep::effective_volume(&inner));
            }) as Box<dyn FnMut()>);

            let _ =
                window.add_event_listener_with_callback("focus", on_focus.as_ref().unchecked_ref());
            let _ =
                window.add_event_listener_with_callback("blur", on_blur.as_ref().unchecked_ref());

            inner.borrow_mut().focus_closures = Some((on_focus, on_blur));
        }

        async fn fetch_audio_buffer(
            source: &BleepSource,
            context: &AudioContext,
            fetch_headers: Option<&BTreeMap<String, String>>,
        ) -> Option<AudioBuffer> {
            match source {
                BleepSource::Bytes { data, .. } => {
                    let array = js_sys::Uint8Array::from(data.as_slice());
                    let buffer = array.buffer();
                    JsFuture::from(context.decode_audio_data(&buffer))
                        .await
                        .ok()?
                        .dyn_into()
                        .ok()
                }
                BleepSource::Url { url, .. } => {
                    let window = web_sys::window()?;
                    let response_value = if let Some(headers) = fetch_headers {
                        let init = RequestInit::new();
                        init.set_method("GET");
                        let header_map = Headers::new().ok()?;
                        for (key, value) in headers {
                            let _ = header_map.append(key, value);
                        }
                        let headers_js = JsValue::from(header_map);
                        init.set_headers(&headers_js);
                        let request = Request::new_with_str_and_init(url, &init).ok()?;
                        JsFuture::from(window.fetch_with_request(&request))
                            .await
                            .ok()?
                    } else {
                        JsFuture::from(window.fetch_with_str(url)).await.ok()?
                    };
                    let response: web_sys::Response = response_value.dyn_into().ok()?;
                    let buffer = JsFuture::from(response.array_buffer().ok()?).await.ok()?;
                    JsFuture::from(context.decode_audio_data(&buffer))
                        .await
                        .ok()?
                        .dyn_into()
                        .ok()
                }
                BleepSource::Path { .. } => None,
            }
        }

        fn select_source(sources: &[BleepSource]) -> Option<BleepSource> {
            let audio = web_sys::HtmlAudioElement::new().ok()?;
            for source in sources {
                if let Some(mime) = source_mime(source) {
                    let support = audio.can_play_type(mime.as_str());
                    if support == "probably" || support == "maybe" {
                        return Some(source.clone());
                    }
                } else {
                    return Some(source.clone());
                }
            }
            None
        }

        fn start_playback_if_ready(inner: &Rc<RefCell<WebBleepInner>>) {
            let (buffer, looped, max_delay, last_play) = {
                let inner_ref = inner.borrow();
                if inner_ref.load_state != LoadState::Loaded {
                    return;
                }
                if inner_ref.props.looped && inner_ref.source.is_some() {
                    return;
                }
                (
                    inner_ref.buffer.clone(),
                    inner_ref.props.looped,
                    inner_ref.props.settings.max_playback_delay,
                    inner_ref.last_play_request,
                )
            };

            if let Some(last_play) = last_play {
                let now = js_sys::Date::now();
                if (now - last_play) > max_delay.as_millis() as f64 {
                    return;
                }
            }

            let Some(buffer) = buffer else {
                return;
            };

            let mut inner_ref = inner.borrow_mut();
            let _ = inner_ref.context.resume();
            let source = match inner_ref.context.create_buffer_source() {
                Ok(source) => source,
                Err(_) => return,
            };
            source.set_buffer(Some(&buffer));
            source.set_loop(looped);
            let _ = inner_ref
                .gain
                .gain()
                .set_value(WebBleep::effective_volume(&inner_ref));
            if source.connect_with_audio_node(&inner_ref.gain).is_err() {
                return;
            }

            let inner_ref_rc = inner.clone();
            let on_end = Closure::wrap(Box::new(move || {
                inner_ref_rc.borrow_mut().source = None;
            }) as Box<dyn FnMut()>);
            source.set_onended(Some(on_end.as_ref().unchecked_ref()));
            inner_ref.on_end_closure = Some(on_end);

            let _ = source.start();
            inner_ref.source = Some(source);
        }
    }

    fn source_mime(source: &BleepSource) -> Option<String> {
        match source {
            BleepSource::Path { mime, .. } => mime.clone(),
            BleepSource::Url { mime, .. } => mime.clone(),
            BleepSource::Bytes { mime, .. } => mime.clone(),
        }
    }

    impl BleepBackend for WebBleep {
        fn new(props: ResolvedBleepProps) -> Option<Self> {
            let context = AudioContext::new().ok()?;
            let gain = context.create_gain().ok()?;
            gain.connect_with_audio_node(&context.destination()).ok()?;

            let inner = Rc::new(RefCell::new(WebBleepInner {
                props,
                context,
                gain,
                buffer: None,
                source: None,
                on_end_closure: None,
                load_state: LoadState::Unloaded,
                callers: HashSet::new(),
                last_play_request: None,
                externally_muted: false,
                focus_closures: None,
            }));

            inner
                .borrow_mut()
                .props
                .sources
                .retain(|source| !matches!(source, BleepSource::Path { .. }));
            WebBleep::setup_focus_listeners(&inner);
            {
                let inner_ref = inner.borrow();
                let _ = inner_ref
                    .gain
                    .gain()
                    .set_value(WebBleep::effective_volume(&inner_ref));
            }

            let bleep = Self { inner };

            if bleep.with_lock(|inner| inner.props.settings.preload) {
                bleep.load();
            }

            Some(bleep)
        }

        fn duration(&self) -> f32 {
            self.with_lock(|inner| {
                inner
                    .buffer
                    .as_ref()
                    .map(|b| b.duration() as f32)
                    .unwrap_or(0.0)
            })
        }

        fn volume(&self) -> f32 {
            self.with_lock(|inner| inner.props.settings.volume)
        }

        fn muted(&self) -> bool {
            self.with_lock(|inner| inner.props.settings.muted)
        }

        fn is_loaded(&self) -> bool {
            self.with_lock(|inner| inner.load_state == LoadState::Loaded)
        }

        fn is_playing(&self) -> bool {
            self.with_lock(|inner| inner.source.is_some())
        }

        fn play(&self, caller: Option<&str>) {
            let should_load = self.with_lock(|inner| {
                if let Some(caller) = caller {
                    inner.callers.insert(caller.to_string());
                }
                inner.last_play_request = Some(js_sys::Date::now());
                inner.load_state == LoadState::Unloaded
            });

            if should_load {
                self.load();
            }
            WebBleep::start_playback_if_ready(&self.inner);
        }

        fn stop(&self, caller: Option<&str>) {
            self.with_lock(|inner| {
                if let Some(caller) = caller {
                    inner.callers.remove(caller);
                } else {
                    inner.callers.clear();
                }

                if inner.props.looped && !inner.callers.is_empty() {
                    return;
                }

                if let Some(source) = inner.source.take() {
                    let _ = source.stop();
                }
            });
        }

        fn load(&self) {
            let should_load = self.with_lock(|inner| {
                if inner.load_state != LoadState::Unloaded {
                    return false;
                }
                inner.load_state = LoadState::Loading;
                true
            });

            if !should_load {
                return;
            }

            let inner = self.inner.clone();
            spawn_local(async move {
                let sources = { inner.borrow().props.sources.clone() };

                let source = WebBleep::select_source(&sources);
                let Some(source) = source else {
                    inner.borrow_mut().load_state = LoadState::Error;
                    return;
                };

                let (context, fetch_headers) = {
                    let inner_ref = inner.borrow();
                    (
                        inner_ref.context.clone(),
                        inner_ref.props.settings.fetch_headers.clone(),
                    )
                };
                let buffer =
                    WebBleep::fetch_audio_buffer(&source, &context, fetch_headers.as_ref()).await;
                {
                    let mut inner_mut = inner.borrow_mut();
                    match buffer {
                        Some(buffer) => {
                            inner_mut.buffer = Some(buffer);
                            inner_mut.load_state = LoadState::Loaded;
                        }
                        None => {
                            inner_mut.load_state = LoadState::Error;
                        }
                    }
                }
                WebBleep::start_playback_if_ready(&inner);
            });
        }

        fn unload(&self) {
            self.with_lock(|inner| {
                if let Some(source) = inner.source.take() {
                    let _ = source.stop();
                }
                inner.buffer = None;
                inner.on_end_closure = None;
                if let Some((focus, blur)) = inner.focus_closures.take() {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "focus",
                            focus.as_ref().unchecked_ref(),
                        );
                        let _ = window.remove_event_listener_with_callback(
                            "blur",
                            blur.as_ref().unchecked_ref(),
                        );
                    }
                }
                inner.load_state = LoadState::Unloaded;
            });
        }

        fn update(&self, settings: ResolvedBleepSettings, master_volume: f32) {
            self.with_lock(|inner| {
                inner.props.settings = settings;
                inner.props.master_volume = master_volume;
                if inner.props.settings.disabled {
                    if let Some(source) = inner.source.take() {
                        let _ = source.stop();
                    }
                }
                let _ = inner
                    .gain
                    .gain()
                    .set_value(WebBleep::effective_volume(inner));
            });
        }
    }

    pub type BackendBleep = WebBleep;
}

#[cfg(all(target_arch = "wasm32", not(feature = "web")))]
compile_error!("bleeps requires the `web` feature on wasm32 targets.");

#[cfg(not(target_arch = "wasm32"))]
use backend::BackendBleep;
#[cfg(all(target_arch = "wasm32", feature = "web"))]
use backend::BackendBleep;

#[derive(Clone)]
pub struct Bleep {
    backend: BackendBleep,
}

impl Bleep {
    pub(crate) fn new(props: ResolvedBleepProps) -> Option<Self> {
        BackendBleep::new(props).map(|backend| Self { backend })
    }

    pub fn duration(&self) -> f32 {
        self.backend.duration()
    }

    pub fn volume(&self) -> f32 {
        self.backend.volume()
    }

    pub fn muted(&self) -> bool {
        self.backend.muted()
    }

    pub fn is_loaded(&self) -> bool {
        self.backend.is_loaded()
    }

    pub fn is_playing(&self) -> bool {
        self.backend.is_playing()
    }

    pub fn play(&self, caller: Option<&str>) {
        self.backend.play(caller);
    }

    pub fn stop(&self, caller: Option<&str>) {
        self.backend.stop(caller);
    }

    pub fn load(&self) {
        self.backend.load();
    }

    pub fn unload(&self) {
        self.backend.unload();
    }

    pub(crate) fn update(&self, settings: ResolvedBleepSettings, master_volume: f32) {
        self.backend.update(settings, master_volume);
    }
}

pub struct BleepsManager {
    master_volume: f32,
    common: BleepGeneralProps,
    categories: BTreeMap<BleepCategory, BleepGeneralProps>,
    definitions: BTreeMap<String, BleepProps>,
    bleeps: BTreeMap<String, Option<Bleep>>,
}

impl BleepsManager {
    pub fn new(props: BleepsManagerProps) -> Self {
        let master_volume = props.master.and_then(|m| m.volume).unwrap_or(1.0);
        let common = props.common.unwrap_or_default();
        let categories = props.categories;
        let definitions = props.bleeps;

        let mut bleeps = BTreeMap::new();
        for (name, def) in definitions.iter() {
            let resolved = resolve_bleep(def, &common, &categories, master_volume);
            let instance = if resolved.settings.disabled {
                None
            } else {
                Bleep::new(resolved)
            };
            bleeps.insert(name.clone(), instance);
        }

        Self {
            master_volume,
            common,
            categories,
            definitions,
            bleeps,
        }
    }

    pub fn bleeps(&self) -> &BTreeMap<String, Option<Bleep>> {
        &self.bleeps
    }

    pub fn unload(&mut self) {
        for bleep in self.bleeps.values() {
            if let Some(bleep) = bleep {
                bleep.unload();
            }
        }
    }

    pub fn update(&mut self, update: BleepsManagerUpdate) {
        if let Some(master) = update.master {
            if let Some(volume) = master.volume {
                self.master_volume = volume.clamp(0.0, 1.0);
            }
        }

        if let Some(common) = update.common {
            merge_general_props(&mut self.common, common);
        }

        if let Some(categories) = update.categories {
            for (category, props) in categories {
                let entry = self.categories.entry(category).or_default();
                merge_general_props(entry, props);
            }
        }

        if let Some(bleeps_update) = update.bleeps {
            for (name, update) in bleeps_update {
                if let Some(def) = self.definitions.get_mut(&name) {
                    apply_bleep_update(def, update);
                }
            }
        }

        for (name, def) in self.definitions.clone() {
            let resolved = resolve_bleep(&def, &self.common, &self.categories, self.master_volume);
            let entry = self.bleeps.entry(name.clone()).or_insert(None);
            if resolved.settings.disabled {
                *entry = None;
                continue;
            }

            if let Some(bleep) = entry {
                bleep.update(resolved.settings.clone(), resolved.master_volume);
            } else {
                *entry = Bleep::new(resolved);
            }
        }
    }
}

fn merge_general_props(target: &mut BleepGeneralProps, update: BleepGeneralProps) {
    if update.preload.is_some() {
        target.preload = update.preload;
    }
    if update.async_load.is_some() {
        target.async_load = update.async_load;
    }
    if update.volume.is_some() {
        target.volume = update.volume;
    }
    if update.muted.is_some() {
        target.muted = update.muted;
    }
    if update.category.is_some() {
        target.category = update.category;
    }
    if update.fetch_headers.is_some() {
        target.fetch_headers = update.fetch_headers;
    }
    if update.max_playback_delay.is_some() {
        target.max_playback_delay = update.max_playback_delay;
    }
    if update.mute_on_window_blur.is_some() {
        target.mute_on_window_blur = update.mute_on_window_blur;
    }
    if update.disabled.is_some() {
        target.disabled = update.disabled;
    }
}

fn apply_bleep_update(def: &mut BleepProps, update: BleepUpdate) {
    def.general.volume = update.volume.or(def.general.volume);
    def.general.muted = update.muted.or(def.general.muted);
    def.general.disabled = update.disabled.or(def.general.disabled);
}

fn resolve_bleep(
    def: &BleepProps,
    common: &BleepGeneralProps,
    categories: &BTreeMap<BleepCategory, BleepGeneralProps>,
    master_volume: f32,
) -> ResolvedBleepProps {
    let mut resolved = ResolvedBleepSettings::default();

    apply_general(&mut resolved, common);
    let category = def.category.or(def.general.category).or(common.category);
    if let Some(category) = category {
        if let Some(category_props) = categories.get(&category) {
            apply_general(&mut resolved, category_props);
        }
    }
    apply_general(&mut resolved, &def.general);

    ResolvedBleepProps {
        sources: def.sources.clone(),
        looped: def.looped,
        settings: resolved,
        master_volume,
    }
}

fn apply_general(settings: &mut ResolvedBleepSettings, props: &BleepGeneralProps) {
    if let Some(preload) = props.preload {
        settings.preload = preload;
    }
    if let Some(async_load) = props.async_load {
        settings.async_load = async_load;
    }
    if let Some(volume) = props.volume {
        settings.volume = volume;
    }
    if let Some(muted) = props.muted {
        settings.muted = muted;
    }
    if let Some(fetch_headers) = &props.fetch_headers {
        settings.fetch_headers = Some(fetch_headers.clone());
    }
    if let Some(delay) = props.max_playback_delay {
        settings.max_playback_delay = delay;
    }
    if let Some(mute_on_window_blur) = props.mute_on_window_blur {
        settings.mute_on_window_blur = mute_on_window_blur;
    }
    if let Some(disabled) = props.disabled {
        settings.disabled = disabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bleeps_manager_settings_merge() {
        let mut categories = BTreeMap::new();
        categories.insert(
            BleepCategory::Interaction,
            BleepGeneralProps {
                volume: Some(0.2),
                ..BleepGeneralProps::default()
            },
        );

        let mut bleeps = BTreeMap::new();
        bleeps.insert(
            "click".to_string(),
            BleepProps {
                sources: vec![BleepSource::Bytes {
                    data: vec![0; 4],
                    mime: Some("audio/wav".to_string()),
                }],
                looped: false,
                category: Some(BleepCategory::Interaction),
                general: BleepGeneralProps {
                    volume: Some(0.4),
                    ..BleepGeneralProps::default()
                },
            },
        );

        let manager = BleepsManager::new(BleepsManagerProps {
            master: Some(BleepMasterProps { volume: Some(0.5) }),
            common: Some(BleepGeneralProps {
                volume: Some(0.1),
                ..BleepGeneralProps::default()
            }),
            categories,
            bleeps,
        });

        let resolved = resolve_bleep(
            manager.definitions.get("click").unwrap(),
            &manager.common,
            &manager.categories,
            manager.master_volume,
        );
        assert!((resolved.settings.volume - 0.4).abs() < 0.001);
    }
}
