//! Android's JNI exports. See the parent module for the contract.
use super::{
    BridgeError, MAX_CONFIG_BYTES, MAX_HANDLES, MAX_REQUEST_BYTES, MAX_VERSE_CONFIG_BYTES,
    MAX_VERSE_REQUEST_BYTES, create_app, error, guarded, packet_text, respond, surface_config,
};
use crate::App;
use coder_mobile::VerseHandle;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString};
use jni::{Env, EnvUnowned, jni_sig, jni_str};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicI64, Ordering};

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(
        priority: i32,
        tag: *const std::ffi::c_char,
        text: *const std::ffi::c_char,
    ) -> i32;
}

/// Writes one line to Android's log at error priority, under `OpenAgents`.
pub(super) fn log_error(text: &str) {
    const ERROR: i32 = 6;
    let Ok(text) = std::ffi::CString::new(text.replace('\0', " ")) else {
        return;
    };
    // SAFETY: both strings are valid NUL-terminated C strings for this call.
    unsafe { __android_log_write(ERROR, c"OpenAgents".as_ptr(), text.as_ptr()) };
}

static NEXT_HANDLE: AtomicI64 = AtomicI64::new(1);

thread_local! {
    static APPS: RefCell<BTreeMap<i64, App>> = const { RefCell::new(BTreeMap::new()) };
    static VERSES: RefCell<BTreeMap<i64, AndroidVerse>> = const { RefCell::new(BTreeMap::new()) };
}

impl From<jni::errors::Error> for BridgeError {
    fn from(_: jni::errors::Error) -> Self {
        Self("Android native call failed".into())
    }
}

fn main_thread(required: bool) -> Result<(), BridgeError> {
    // Android creates the UI thread as the process's initial thread.
    let is_main = unsafe { libc::gettid() == libc::getpid() };
    if required != is_main {
        return Err(error(if required {
            "Verse must run on Android's main thread"
        } else {
            "The app must run on a serial background worker"
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
    if value.is_null() {
        return Err(error("Native input is missing"));
    }
    // Check the UTF-16 length before allocating the UTF-8 copy, then apply
    // the iOS bridge's byte limit to the copy.
    let length = env
        .call_method(value, jni_str!("length"), jni_sig!("()I"), &[])?
        .i()?;
    if length <= 0 || length as usize > limit {
        return Err(error("Native input exceeds its size limit"));
    }
    let text = value.try_to_string(env)?;
    if text.is_empty() || text.len() > limit {
        return Err(error("Native input exceeds its size limit"));
    }
    Ok(text)
}

fn output<'local>(env: &mut Env<'local>, bytes: Vec<u8>) -> Result<JString<'local>, BridgeError> {
    Ok(JString::from_str(env, packet_text(bytes)?)?)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_create<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    config: JString<'local>,
) -> i64 {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(false)?;
            let config = input(env, &config, MAX_CONFIG_BYTES)?;
            guarded(|| {
                APPS.with(|apps| {
                    let mut apps = apps
                        .try_borrow_mut()
                        .map_err(|_| error("An app call is already in progress"))?;
                    if apps.len() >= MAX_HANDLES {
                        return Err(error("Too many native app handles"));
                    }
                    let app = create_app(&config)?;
                    let id = next_handle()?;
                    apps.insert(id, app);
                    Ok(id)
                })
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_call<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    request: JString<'local>,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(false)?;
            let request = input(env, &request, MAX_REQUEST_BYTES)?;
            let bytes = guarded(|| {
                APPS.with(|apps| {
                    let mut apps = apps
                        .try_borrow_mut()
                        .map_err(|_| error("An app call is already in progress"))?;
                    let app = apps
                        .get_mut(&handle)
                        .ok_or_else(|| error("App handle is stale or belongs to another thread"))?;
                    respond(app, &request)
                })
            })?;
            output(env, bytes)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_destroy<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
) {
    unowned
        .with_env(|_| -> Result<_, BridgeError> {
            main_thread(false)?;
            guarded(|| {
                APPS.with(|apps| {
                    let app = apps
                        .try_borrow_mut()
                        .map_err(|_| error("An app call is already in progress"))?
                        .remove(&handle)
                        .ok_or_else(|| error("App handle is stale or belongs to another thread"))?;
                    drop(app);
                    Ok(())
                })
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// One acquired `ANativeWindow` reference.
struct NativeWindow(NonNull<ndk_sys::ANativeWindow>);
impl NativeWindow {
    fn acquire(env: &Env<'_>, surface: &JObject<'_>) -> Result<Self, BridgeError> {
        if surface.is_null() {
            return Err(error("Android surface is null"));
        }
        // ANativeWindow_fromSurface acquires one reference, which this value
        // owns; no JVM local reference escapes the call.
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
        // SAFETY: this value owns exactly one acquired window reference.
        unsafe { ndk_sys::ANativeWindow_release(self.0.as_ptr()) };
    }
}

/// Verse's bare world and the window it draws in. Fields drop in order: the
/// renderer goes before the window it renders into.
struct AndroidVerse {
    handle: VerseHandle,
    window: Option<NativeWindow>,
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_verseCreate<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    surface: JObject<'local>,
    config: JString<'local>,
) -> i64 {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(true)?;
            let config = surface_config(&input(env, &config, MAX_VERSE_CONFIG_BYTES)?)?;
            let window = NativeWindow::acquire(env, &surface)?;
            guarded(|| {
                VERSES.with(|verses| {
                    let mut verses = verses
                        .try_borrow_mut()
                        .map_err(|_| error("A Verse call is already in progress"))?;
                    if verses.len() >= MAX_HANDLES {
                        return Err(error("Too many native Verse handles"));
                    }
                    // SAFETY: `window` is acquired on this main thread and is
                    // stored after the handle, so it outlives the renderer.
                    let handle = unsafe {
                        VerseHandle::create_bare_with_gym(
                            window.0.as_ptr().cast(),
                            config.width,
                            config.height,
                            config.scale,
                            false,
                            config.presence()?,
                            config.gym()?,
                        )
                    }?;
                    let id = next_handle()?;
                    verses.insert(
                        id,
                        AndroidVerse {
                            handle,
                            window: Some(window),
                        },
                    );
                    Ok(id)
                })
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_verseAttach<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    surface: JObject<'local>,
    config: JString<'local>,
) {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(true)?;
            let config = surface_config(&input(env, &config, MAX_VERSE_CONFIG_BYTES)?)?;
            let window = NativeWindow::acquire(env, &surface)?;
            guarded(|| {
                VERSES.with(|verses| {
                    let mut verses = verses
                        .try_borrow_mut()
                        .map_err(|_| error("A Verse call is already in progress"))?;
                    let verse = verses.get_mut(&handle).ok_or_else(|| {
                        error("Verse handle is stale or belongs to another thread")
                    })?;
                    if verse.window.is_some() {
                        return Err(error("The native Verse surface is already attached"));
                    }
                    // SAFETY: the window is stored beside the renderer and
                    // released only after `detach_android` or drop.
                    unsafe {
                        verse.handle.attach_android(
                            window.0.as_ptr().cast(),
                            config.width,
                            config.height,
                            config.scale,
                        )
                    }?;
                    verse.window = Some(window);
                    Ok(())
                })
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_verseDetach<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
) {
    unowned
        .with_env(|_| -> Result<_, BridgeError> {
            main_thread(true)?;
            guarded(|| {
                VERSES.with(|verses| {
                    let mut verses = verses
                        .try_borrow_mut()
                        .map_err(|_| error("A Verse call is already in progress"))?;
                    let verse = verses.get_mut(&handle).ok_or_else(|| {
                        error("Verse handle is stale or belongs to another thread")
                    })?;
                    // Drop the renderer before releasing its window.
                    let result = verse.handle.detach_android();
                    verse.window = None;
                    result.map_err(BridgeError::from)
                })
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_verseCall<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    request: JString<'local>,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(true)?;
            let request = input(env, &request, MAX_VERSE_REQUEST_BYTES)?;
            let bytes = guarded(|| {
                VERSES.with(|verses| {
                    let mut verses = verses
                        .try_borrow_mut()
                        .map_err(|_| error("A Verse call is already in progress"))?;
                    let verse = verses.get_mut(&handle).ok_or_else(|| {
                        error("Verse handle is stale or belongs to another thread")
                    })?;
                    verse
                        .handle
                        .call_bytes(request.as_bytes())
                        .map_err(BridgeError::from)
                })
            })?;
            output(env, bytes)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_verseDestroy<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
) {
    unowned
        .with_env(|_| -> Result<_, BridgeError> {
            main_thread(true)?;
            guarded(|| {
                VERSES.with(|verses| {
                    let verse = verses
                        .try_borrow_mut()
                        .map_err(|_| error("A Verse call is already in progress"))?
                        .remove(&handle)
                        .ok_or_else(|| {
                            error("Verse handle is stale or belongs to another thread")
                        })?;
                    drop(verse);
                    Ok(())
                })
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
