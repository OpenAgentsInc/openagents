//! Android's JNI exports. See the parent module for the contract.
use super::{
    BridgeError, MAX_CONFIG_BYTES, MAX_HANDLES, MAX_REQUEST_BYTES, MAX_VERSE_CONFIG_BYTES,
    MAX_VERSE_REQUEST_BYTES, create_app, editors, error, guarded, hold_studio_links, packet_text,
    respond, surface_config, take_studio_links, transcripts,
};
use crate::App;
use coder_mobile::VerseHandle;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JFloatArray, JLongArray, JObject, JString};
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

/// Blocks the calling thread until the app packet changes; see
/// `crate::wake::wait`. Call it from a thread of its own, never the app
/// worker; it needs no handle.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_waitChange<'local>(
    _unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    seen: i64,
    timeout_ms: i32,
) -> i64 {
    let seen = seen.cast_unsigned();
    let limit = std::time::Duration::from_millis(u64::try_from(timeout_ms).unwrap_or(0));
    std::panic::catch_unwind(|| crate::wake::wait(seen, limit))
        .unwrap_or(seen)
        .cast_signed()
}

/// Whether this build shows the preview features; see `crate::preview`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_preview<'local>(
    _unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> bool {
    crate::preview::ON
}

/// Whether the Coder tab shows; see `crate::wake::set_shown`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_coderShown<'local>(
    _unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    shown: bool,
) {
    crate::wake::set_shown(shown);
}

/// Publish the JVM and the application context to iroh's DNS resolver,
/// which reads Android's DNS configuration through JNI. Call it once, with
/// the application context, before the app is created; a later call does
/// nothing.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_installContext<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    context: JObject<'local>,
) {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            if context.is_null() {
                return Err(error("The Android context is missing"));
            }
            let vm = env.get_java_vm()?;
            let context = env.new_global_ref(&context)?;
            INSTALLED.call_once(|| {
                let vm = vm.get_raw().cast::<std::ffi::c_void>();
                let object = context.as_obj().as_raw().cast::<std::ffi::c_void>();
                // SAFETY: the VM lives as long as the process, and the
                // global reference is leaked here, so both pointers stay
                // valid until the process exits, as iroh requires.
                unsafe { openagents_connect::iroh::dns::install_android_jni_context(vm, object) };
                std::mem::forget(context);
            });
            Ok(())
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
    handle: Box<VerseHandle>,
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
                    let presence = config.presence()?;
                    let world = presence.is_some();
                    let mut handle = unsafe {
                        VerseHandle::create_bare_with_gym(
                            window.0.as_ptr().cast(),
                            config.width,
                            config.height,
                            config.scale,
                            false,
                            presence,
                            config.gym()?,
                        )
                    }?;
                    // The owner's private characters in Everglade.
                    crate::verse_private::mount(&mut handle, world);
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

/// On the app's worker: takes the app's link to the paired computer whose
/// host key is `host` for Everglade's studio, and answers a token for
/// `verseStudioConnect`. A missing link is not an error here; the connect
/// call answers its reason.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_studioLinks<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    host: JString<'local>,
) -> i64 {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(false)?;
            let host = input(env, &host, 256)?;
            guarded(|| {
                APPS.with(|apps| {
                    let apps = apps
                        .try_borrow()
                        .map_err(|_| error("An app call is already in progress"))?;
                    let app = apps
                        .get(&handle)
                        .ok_or_else(|| error("App handle is stale or belongs to another thread"))?;
                    Ok(hold_studio_links(app, &host))
                })
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// On the main thread: connects the Verse handle's Everglade studio through
/// the link `studioLinks` took under `token`, as
/// `openagents_verse_studio_connect` does. Answers
/// `{"connected":true,"rights":[...]}` or `{"connected":false,"error":"..."}`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_verseStudioConnect<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    token: i64,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(true)?;
            let bytes = guarded(|| {
                VERSES.with(|verses| {
                    let mut verses = verses
                        .try_borrow_mut()
                        .map_err(|_| error("A Verse call is already in progress"))?;
                    let verse = verses.get_mut(&handle).ok_or_else(|| {
                        error("Verse handle is stale or belongs to another thread")
                    })?;
                    let result =
                        crate::studio::connect_through(&mut verse.handle, take_studio_links(token));
                    Ok(crate::studio::reply(result))
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

// Transcript layout (`super::transcripts`), for `TranscriptNative` in the
// host. A transcript is updated on its own worker; frames are read on the UI
// thread.

fn long_array<'local>(
    env: &mut Env<'local>,
    values: &[i64],
) -> Result<JLongArray<'local>, BridgeError> {
    let array = JLongArray::new(env, values.len())?;
    array.set_region(env, 0, values)?;
    Ok(array)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_create<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> i64 {
    unowned
        .with_env(|_| -> Result<_, BridgeError> { guarded(transcripts::create) })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_update<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    request: JString<'local>,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            let request = input(env, &request, transcripts::MAX_UPDATE_BYTES)?;
            let bytes = guarded(|| transcripts::update(handle, &request))?;
            output(env, bytes)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_destroy<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
) {
    unowned
        .with_env(|_| -> Result<_, BridgeError> {
            guarded(|| {
                transcripts::destroy(handle);
                Ok(())
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_frameRelease<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    frame: i64,
) {
    unowned
        .with_env(|_| -> Result<_, BridgeError> {
            guarded(|| {
                transcripts::release(frame);
                Ok(())
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_frameHeight<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    frame: i64,
) -> f32 {
    unowned
        .with_env(|_| -> Result<_, BridgeError> { guarded(|| transcripts::height(frame)) })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_frameAll<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    frame: i64,
) -> JLongArray<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            let values = guarded(|| transcripts::all(frame))?;
            long_array(env, &values)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_frameRows<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    frame: i64,
    y0: f32,
    y1: f32,
) -> JLongArray<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            let values = guarded(|| transcripts::rows(frame, y0, y1))?;
            long_array(env, &values)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_frameKeys<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    frame: i64,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            let keys = guarded(|| transcripts::keys(frame))?;
            Ok(JString::from_str(env, keys)?)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_frameDisplay<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    frame: i64,
    index: i32,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            let bytes = guarded(|| transcripts::display(frame, index))?;
            output(env, bytes)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_fontSpec<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    size: f32,
    weight: i32,
    italic: bool,
    mono: bool,
) -> JFloatArray<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            let spec = transcripts::font_spec(size, weight, italic, mono);
            let array = JFloatArray::new(env, spec.len())?;
            array.set_region(env, 0, &spec)?;
            Ok(array)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_fontData<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    face: i32,
) -> JByteArray<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            let data = transcripts::font(face)?;
            Ok(env.byte_array_from_slice(data)?)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_publish<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    name: JString<'local>,
    node: JString<'local>,
) {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            let name = input(env, &name, 96)?;
            let node = input(env, &node, transcripts::MAX_UPDATE_BYTES)?;
            guarded(|| transcripts::publish(&name, &node))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_EditorNative_create<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> i64 {
    unowned
        .with_env(|_| -> Result<_, BridgeError> { guarded(editors::create) })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_EditorNative_call<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    request: JString<'local>,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            let request = input(env, &request, editors::MAX_REQUEST_CHARS)?;
            let bytes = guarded(|| editors::call(handle, &request))?;
            output(env, bytes)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_EditorNative_destroy<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
) {
    unowned
        .with_env(|_| -> Result<_, BridgeError> {
            guarded(|| {
                editors::destroy(handle);
                Ok(())
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Paint-only syntax spans for one code block (`editors::highlight`). Call
/// it from a worker thread; the first call compiles the grammars.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_TranscriptNative_highlight<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    language: JString<'local>,
    text: JString<'local>,
    light: bool,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            let language = input(env, &language, 32)?;
            let text = input(env, &text, rust_native::syntax::MAX_BYTES)?;
            let bytes = guarded(|| Ok(editors::highlight(&language, &text, light)))?;
            output(env, bytes)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Attach an image the photo picker read to the open chat's draft
/// (`App::attach_image`); answers with the app packet.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_attachImage<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    name: JString<'local>,
    bytes: JByteArray<'local>,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(false)?;
            let name = input(env, &name, 1024)?;
            let length = bytes.len(env)?;
            if length == 0 || length > openagents_chat_app::attachments::MAX_IMAGE_BYTES {
                return Err(error("The image exceeds its size limit"));
            }
            let data = env.convert_byte_array(&bytes)?;
            let packet = guarded(|| {
                APPS.with(|apps| {
                    let mut apps = apps
                        .try_borrow_mut()
                        .map_err(|_| error("An app call is already in progress"))?;
                    let app = apps
                        .get_mut(&handle)
                        .ok_or_else(|| error("App handle is stale or belongs to another thread"))?;
                    Ok(app.attach_image(&name, data))
                })
            })?;
            output(env, packet)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The encoded bytes of the chat's image surface `resource`, or an empty
/// array.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_openagents_app_OpenAgentsNative_image<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: i64,
    resource: JString<'local>,
) -> JByteArray<'local> {
    unowned
        .with_env(|env| -> Result<_, BridgeError> {
            main_thread(false)?;
            let resource = input(env, &resource, 96)?;
            let data = guarded(|| {
                APPS.with(|apps| {
                    let apps = apps
                        .try_borrow()
                        .map_err(|_| error("An app call is already in progress"))?;
                    let app = apps
                        .get(&handle)
                        .ok_or_else(|| error("App handle is stale or belongs to another thread"))?;
                    Ok(app.image(&resource))
                })
            })?;
            Ok(env.byte_array_from_slice(&data)?)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
