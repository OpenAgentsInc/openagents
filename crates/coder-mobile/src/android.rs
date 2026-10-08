//! Android owns widgets and surface callbacks; Rust owns both application states.
//! Reader handles stay on one background worker. Verse handles stay on Android's
//! main thread. IDs are never native pointers and cannot be reused after disposal.
use crate::verse_app::{Config as VerseConfig, Scene};
use crate::verse_ffi::VerseHandle;
use crate::{App, Config, Request};
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString};
use jni::{Env, EnvUnowned, jni_sig, jni_str};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicI64, Ordering};

static NEXT_HANDLE: AtomicI64 = AtomicI64::new(1);
const MAX_HANDLES: usize = 4;

thread_local! {
    static READERS: RefCell<BTreeMap<i64, App>> = const { RefCell::new(BTreeMap::new()) };
    static VERSES: RefCell<BTreeMap<i64, AndroidVerse>> = const { RefCell::new(BTreeMap::new()) };
}

#[derive(Debug)]
struct BridgeError(String);
impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for BridgeError {}
impl From<jni::errors::Error> for BridgeError {
    fn from(_: jni::errors::Error) -> Self {
        Self("Android native call failed".into())
    }
}
impl From<String> for BridgeError {
    fn from(message: String) -> Self {
        Self(message)
    }
}
fn error(message: &str) -> BridgeError {
    BridgeError(message.into())
}

fn main_thread(required: bool) -> Result<(), BridgeError> {
    // Android creates the UI thread as the process's initial thread.
    let is_main = unsafe { libc::gettid() == libc::getpid() };
    if required != is_main {
        return Err(error(if required {
            "Verse must run on Android's main thread"
        } else {
            "The reader must run on a serial background worker"
        }));
    }
    Ok(())
}

fn next_handle() -> Result<i64, BridgeError> {
    NEXT_HANDLE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| error("Native handle IDs are exhausted"))
}

fn input(env: &mut Env<'_>, value: &JString<'_>, limit: usize) -> Result<String, BridgeError> {
    // Check UTF-16 length before allocating the UTF-8 representation, then
    // enforce the same byte limit used by the iOS bridge.
    let length = env
        .call_method(value, jni_str!("length"), jni_sig!("()I"), &[])?
        .i()?;
    if length <= 0 || length as usize > limit {
        return Err(error("Native input exceeds its size limit"));
    }
    let text = value.try_to_string(env)?;
    if text.len() > limit {
        return Err(error("Native input exceeds its size limit"));
    }
    Ok(text)
}

struct NativeWindow(NonNull<ndk_sys::ANativeWindow>);
impl NativeWindow {
    fn acquire(env: &Env<'_>, surface: &JObject<'_>) -> Result<Self, BridgeError> {
        if surface.is_null() {
            return Err(error("Android surface is null"));
        }
        // ANativeWindow_fromSurface acquires one reference. No JVM local
        // reference escapes this call; the acquired window owns its lifetime.
        let window = unsafe {
            ndk_sys::ANativeWindow_fromSurface(env.get_raw().cast(), surface.as_raw().cast())
        };
        NonNull::new(window)
            .map(Self)
            .ok_or_else(|| error("Cannot acquire the Android surface"))
    }
}
impl Drop for NativeWindow {
    fn drop(&mut self) {
        // SAFETY: this object owns exactly one acquired native-window reference.
        unsafe { ndk_sys::ANativeWindow_release(self.0.as_ptr()) };
    }
}

struct AndroidVerse {
    // Rust drops fields in declaration order: suspend and drop the renderer
    // before releasing the acquired native window it renders into.
    handle: VerseHandle,
    window: Option<NativeWindow>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceConfig {
    width: u32,
    height: u32,
    scale: f32,
}

impl AndroidVerse {
    fn attach(
        &mut self,
        env: &Env<'_>,
        surface: &JObject<'_>,
        config: SurfaceConfig,
    ) -> Result<(), BridgeError> {
        if self.window.is_some() || self.handle.renderer.is_some() {
            return Err(error("The native Verse surface is already attached"));
        }
        let window = NativeWindow::acquire(env, surface)?;
        // SAFETY: this acquired window is moved into the same owner after the
        // renderer, and is released only after the renderer has been dropped.
        unsafe {
            self.handle.attach_android(
                window.0.as_ptr().cast(),
                config.width,
                config.height,
                config.scale,
            )
        }
        .map_err(|message| error(&message))?;
        self.window = Some(window);
        Ok(())
    }

    fn detach(&mut self) -> Result<(), BridgeError> {
        let result = self.handle.detach_renderer();
        self.window = None;
        result.map_err(BridgeError::from)
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_coder_CoderNative_createReader<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    config: JString<'local>,
) -> i64 {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(false)?;
            let config = input(env, &config, 16 * 1024)?;
            let config: Config = serde_json::from_str(&config)
                .map_err(|_| error("Invalid native reader configuration"))?;
            READERS.with(|readers| {
                let mut readers = readers
                    .try_borrow_mut()
                    .map_err(|_| error("Reader call is already in progress"))?;
                if readers.len() >= MAX_HANDLES {
                    return Err(error("Too many native reader handles"));
                }
                let app = App::new(config)?;
                let id = next_handle()?;
                readers.insert(id, app);
                Ok(id)
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_coder_CoderNative_readerCall<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    request: JString<'local>,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(false)?;
            let request = input(env, &request, 128 * 1024)?;
            let request: Request = serde_json::from_str(&request)
                .map_err(|_| error("Invalid native reader request"))?;
            let text = READERS.with(|readers| {
                let mut readers = readers
                    .try_borrow_mut()
                    .map_err(|_| error("Reader call is already in progress"))?;
                let app = readers
                    .get_mut(&handle)
                    .ok_or_else(|| error("Reader handle is stale or belongs to another thread"))?;
                let text = serde_json::to_string(&app.respond(request))
                    .map_err(|_| error("Cannot encode native reader state"))?;
                if text.len() > 1024 * 1024 {
                    return Err(error("Native reader state exceeds its size limit"));
                }
                Ok(text)
            })?;
            Ok(JString::from_str(env, text)?)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_coder_CoderNative_destroyReader<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
) {
    unowned
        .with_env(|_| -> Result<_, BridgeError> {
            main_thread(false)?;
            READERS.with(|readers| {
                let app = readers
                    .try_borrow_mut()
                    .map_err(|_| error("Reader call is already in progress"))?
                    .remove(&handle)
                    .ok_or_else(|| error("Reader handle is stale or belongs to another thread"))?;
                drop(app);
                Ok(())
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_coder_CoderNative_verseBlueprint<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            let text = serde_json::to_string(&crate::verse_app::blueprint())
                .map_err(|_| error("Cannot encode the native Verse view"))?;
            Ok(JString::from_str(env, text)?)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_coder_CoderNative_createVerse<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    surface: JObject<'local>,
    config: JString<'local>,
) -> i64 {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(true)?;
            let config = input(env, &config, 96 * 1024)?;
            let config: VerseConfig = serde_json::from_str(&config)
                .map_err(|_| error("Invalid native Verse configuration"))?;
            VERSES.with(|verses| {
                let mut verses = verses
                    .try_borrow_mut()
                    .map_err(|_| error("Verse call is already in progress"))?;
                if verses.len() >= MAX_HANDLES {
                    return Err(error("Too many native Verse handles"));
                }
                let surface_config = SurfaceConfig {
                    width: config.width,
                    height: config.height,
                    scale: config.scale,
                };
                let scene =
                    crate::verse_ffi::create_scene(move || Scene::new(config).map(Box::new))?;
                let mut verse = AndroidVerse {
                    handle: VerseHandle {
                        scene,
                        renderer: None,
                        rendered_zone_revision: u64::MAX,
                        rendered_chamber_revision: 0,
                        layer: std::ptr::null_mut(),
                    },
                    window: None,
                };
                verse.attach(env, &surface, surface_config)?;
                let id = next_handle()?;
                verses.insert(id, verse);
                Ok(id)
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_coder_CoderNative_attachVerse<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    surface: JObject<'local>,
    config: JString<'local>,
) {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(true)?;
            let config = input(env, &config, 4096)?;
            let config: SurfaceConfig = serde_json::from_str(&config)
                .map_err(|_| error("Invalid native Verse surface configuration"))?;
            VERSES.with(|verses| {
                let mut verses = verses
                    .try_borrow_mut()
                    .map_err(|_| error("Verse call is already in progress"))?;
                let verse = verses
                    .get_mut(&handle)
                    .ok_or_else(|| error("Verse handle is stale or belongs to another thread"))?;
                verse.attach(env, &surface, config)
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_coder_CoderNative_detachVerse<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
) {
    unowned
        .with_env(|_| -> Result<_, BridgeError> {
            main_thread(true)?;
            VERSES.with(|verses| {
                let mut verses = verses
                    .try_borrow_mut()
                    .map_err(|_| error("Verse call is already in progress"))?;
                let verse = verses
                    .get_mut(&handle)
                    .ok_or_else(|| error("Verse handle is stale or belongs to another thread"))?;
                verse.detach()
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_coder_CoderNative_verseCall<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    request: JString<'local>,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(true)?;
            let request = input(env, &request, 96 * 1024)?;
            let bytes = VERSES.with(|verses| {
                let mut verses = verses
                    .try_borrow_mut()
                    .map_err(|_| error("Verse call is already in progress"))?;
                let verse = verses
                    .get_mut(&handle)
                    .ok_or_else(|| error("Verse handle is stale or belongs to another thread"))?;
                verse
                    .handle
                    .call_bytes(request.as_bytes())
                    .map_err(BridgeError::from)
            })?;
            let text = String::from_utf8(bytes)
                .map_err(|_| error("Native Verse state is not valid UTF-8"))?;
            Ok(JString::from_str(env, text)?)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_coder_CoderNative_destroyVerse<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
) {
    unowned
        .with_env(|_| -> Result<_, BridgeError> {
            main_thread(true)?;
            VERSES.with(|verses| {
                let verse = verses
                    .try_borrow_mut()
                    .map_err(|_| error("Verse call is already in progress"))?
                    .remove(&handle)
                    .ok_or_else(|| error("Verse handle is stale or belongs to another thread"))?;
                drop(verse);
                Ok(())
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
