//! The Android host's JNI surface, beside the C ABI the iOS host uses. It
//! carries the same JSON requests and packets, so both hosts stay thin.
//!
//! Application handles live on one background worker thread; Verse handles
//! live on Android's main thread. Handle IDs are counters, never native
//! pointers, and a disposed ID is never reused. Every entry point bounds its
//! input and turns a Rust error or panic into a Java `RuntimeException`.
//! The JNI exports are in `exports`, built for Android only; the checks they
//! share are here, so host tests cover them.
use crate::{App, Config, Launch, Request};

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) mod editors;
#[cfg(target_os = "android")]
mod exports;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) mod transcripts;

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) const MAX_HANDLES: usize = 4;
pub(crate) const MAX_CONFIG_BYTES: usize = 16 * 1024;
pub(crate) const MAX_REQUEST_BYTES: usize = 128 * 1024;
/// The largest Verse request: a Gym connection code. Every other request
/// stays within Coder's 4 KiB bound, which `VerseHandle` enforces per request.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) const MAX_VERSE_REQUEST_BYTES: usize = 96 * 1024;
/// The largest Verse surface configuration, with a saved Gym connection code.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) const MAX_VERSE_CONFIG_BYTES: usize = 96 * 1024;
/// The largest saved Gym connection code.
const MAX_GYM_CODE_BYTES: usize = 65_536;
pub(crate) const MAX_PACKET_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct BridgeError(pub(crate) String);
impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for BridgeError {}
impl From<String> for BridgeError {
    fn from(message: String) -> Self {
        Self(message)
    }
}
pub(crate) fn error(message: &str) -> BridgeError {
    BridgeError(message.into())
}

thread_local! {
    /// Where the last panic on this thread happened, for the error it becomes.
    static LAST_PANIC: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Records each panic's location, so a caught panic names where it happened.
/// Android discards a process's standard error, where the default hook writes.
fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let place = info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_default();
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|m| (*m).to_owned())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            let mut text = format!("{message} ({place})");
            text.truncate(512);
            #[cfg(target_os = "android")]
            exports::log_error(&text);
            LAST_PANIC.with(|last| *last.borrow_mut() = Some(text));
            previous(info);
        }));
    });
}

/// Runs `body`, turning a panic into an error so it never unwinds into Java.
pub(crate) fn guarded<T>(body: impl FnOnce() -> Result<T, BridgeError>) -> Result<T, BridgeError> {
    install_panic_hook();
    LAST_PANIC.with(|last| *last.borrow_mut() = None);
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).unwrap_or_else(|_| {
        let place = LAST_PANIC.with(|last| last.borrow_mut().take());
        Err(BridgeError(match place {
            Some(place) => format!("The native library failed unexpectedly: {place}"),
            None => "The native library failed unexpectedly".into(),
        }))
    })
}

/// Checks a packet before it becomes a Java string.
pub(crate) fn packet_text(bytes: Vec<u8>) -> Result<String, BridgeError> {
    if bytes.is_empty() {
        return Err(error("The native request failed"));
    }
    if bytes.len() > MAX_PACKET_BYTES {
        return Err(error("Native state exceeds its size limit"));
    }
    String::from_utf8(bytes).map_err(|_| error("Native state is not valid UTF-8"))
}

/// Creates the app from `{"state_dir": ..., "secret_hex": ...}` and the
/// same launch options as the C ABI, such as `native_computers`.
pub(crate) fn create_app(config: &str) -> Result<App, BridgeError> {
    if config.is_empty() || config.len() > MAX_CONFIG_BYTES {
        return Err(error("Native input exceeds its size limit"));
    }
    let launch: Launch =
        serde_json::from_str(config).map_err(|_| error("Invalid native app configuration"))?;
    let config: Config =
        serde_json::from_str(config).map_err(|_| error("Invalid native app configuration"))?;
    App::open(config, launch).map_err(BridgeError::from)
}

/// Answers one request as the iOS bridge does: the app packet, or the
/// terminal packet for a terminal request.
pub(crate) fn respond(app: &mut App, request: &str) -> Result<Vec<u8>, BridgeError> {
    if request.is_empty() || request.len() > MAX_REQUEST_BYTES {
        return Err(error("Native input exceeds its size limit"));
    }
    let request: Request =
        serde_json::from_str(request).map_err(|_| error("Invalid native app request"))?;
    Ok(app.respond(request))
}

/// The surface's size in pixels and its density.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SurfaceConfig {
    /// Whether the host mounts the computer HUD, which the iOS and Android
    /// hosts send since the shared glyph grid (01700b2413). Accepted so the
    /// world still starts; the HUD itself is driven by the world's packet.
    #[serde(default)]
    #[allow(dead_code)]
    computer_hud: bool,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scale: f32,
    /// At creation only: the world identity's secret as 64 hexadecimal
    /// characters, a key kept only for avatar presence and never the device
    /// key, as in `openagents_verse_create`. Without it the world stays
    /// offline.
    #[serde(default)]
    pub(crate) world_secret_hex: Option<String>,
    /// The name shown over this player's head, from Account.
    #[serde(default)]
    pub(crate) display_name: Option<String>,
    /// At creation only: the Gym connection the host saved for the world
    /// key, a `gym-connect:` code. Rust validates it; an invalid code shows
    /// on the Gym board rather than refusing the world.
    #[serde(default)]
    pub(crate) gym_code: Option<String>,
    /// At creation only: the labeled synthetic Gym board, offline, for
    /// emulator checks. The host sends it only in debug builds.
    #[serde(default)]
    pub(crate) gym_preview: bool,
    /// At creation only: the app's cache directory, where the RESULTS board
    /// keeps verified copies of the published results between visits.
    #[serde(default)]
    pub(crate) results_cache_directory: Option<String>,
    /// At creation only: where the RESULTS board reads the published
    /// results, for emulator checks against a mirror.
    #[serde(default)]
    pub(crate) results_base: Option<String>,
    /// At creation only: levels over heads from the labeled tutorial
    /// fixture, offline. Debug builds only; a release build ignores it.
    #[serde(default)]
    pub(crate) xp_preview: bool,
    /// At creation only: **Compare notes** on the Gym's EVALS board, as the
    /// host saved it. Off until the player switches it on.
    #[serde(default)]
    pub(crate) gym_notes: bool,
}

impl SurfaceConfig {
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn presence(&self) -> Result<Option<coder_mobile::BarePresence>, BridgeError> {
        let Some(secret) = &self.world_secret_hex else {
            return Ok(None);
        };
        if secret.len() != 64 || !secret.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(error("Invalid Verse world identity"));
        }
        Ok(Some(coder_mobile::BarePresence {
            secret_hex: secret.clone(),
            relay: None,
            name: self.display_name.clone(),
        }))
    }
}

impl SurfaceConfig {
    /// The Gym and RESULTS board setup, as the iOS host's: this host shows
    /// both native panels.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn gym(&self) -> Result<coder_mobile::BareGym, BridgeError> {
        if self
            .gym_code
            .as_ref()
            .is_some_and(|code| code.len() > MAX_GYM_CODE_BYTES)
        {
            return Err(error("That Gym connection code is too long"));
        }
        Ok(coder_mobile::BareGym {
            code: self.gym_code.clone(),
            preview: self.gym_preview && cfg!(debug_assertions),
            panel: true,
            results_panel: true,
            results_base: self.results_base.clone().filter(|_| cfg!(debug_assertions)),
            results_cache_directory: self.results_cache_directory.clone(),
            xp_preview: self.xp_preview && cfg!(debug_assertions),
            // The host draws the EVALS panel natively (`VersePanels`).
            evals_panel: true,
            notes: self.gym_notes,
            check_relay: None,
            zone_cache_directory: crate::verse::grid_zone_cache(
                self.results_cache_directory.as_deref(),
            ),
            // The block list lives beside the zone packs.
            blocklist_directory: None,
            // A normal build's Grid is plain: no Gym (the release gate).
            without_gym: !crate::preview::ON,
        })
    }
}

pub(crate) fn surface_config(text: &str) -> Result<SurfaceConfig, BridgeError> {
    let config: SurfaceConfig =
        serde_json::from_str(text).map_err(|_| error("Invalid native Verse configuration"))?;
    if config.width == 0 || config.height == 0 || config.width > 4096 || config.height > 4096 {
        return Err(error("Native Verse surface dimensions exceed their bounds"));
    }
    if !config.scale.is_finite() || !(0.25..=8.0).contains(&config.scale) {
        return Err(error("Native Verse surface scale is out of bounds"));
    }
    Ok(config)
}

/// The links to paired computers the app's worker took for Everglade's
/// studio, by token, until the main thread connects a Verse handle with one
/// (`crate::studio`). An app handle lives on the worker and a Verse handle
/// on the main thread, so no one call can reach both. At most
/// [`MAX_HANDLES`] wait; a newer one drops the oldest.
static STUDIO_LINKS: std::sync::Mutex<
    std::collections::BTreeMap<i64, Result<crate::studio::Links, String>>,
> = std::sync::Mutex::new(std::collections::BTreeMap::new());
static NEXT_STUDIO_TOKEN: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);

/// On the app's worker: takes `app`'s link to the paired computer `host`,
/// or the reason it has none, and answers a token for
/// [`take_studio_links`].
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn hold_studio_links(app: &App, host: &str) -> i64 {
    let links = crate::studio::links(app, host);
    let token = NEXT_STUDIO_TOKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut held = STUDIO_LINKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    while held.len() >= MAX_HANDLES {
        held.pop_first();
    }
    held.insert(token, links);
    token
}

/// On the main thread: the link [`hold_studio_links`] took under `token`,
/// once.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn take_studio_links(token: i64) -> Result<crate::studio::Links, String> {
    STUDIO_LINKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&token)
        .unwrap_or_else(|| Err("Try connecting the studio again".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(dir: &tempfile::TempDir) -> String {
        serde_json::json!({
            "state_dir": dir.path(),
            "secret_hex": "11".repeat(32),
        })
        .to_string()
    }

    #[test]
    fn the_app_answers_the_same_json_as_the_c_abi() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut app = create_app(&config(&dir)).expect("app");
        let text = packet_text(respond(&mut app, r#"{"op":"snapshot"}"#).expect("snapshot"))
            .expect("packet");
        let packet: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(packet["schema"], "openagents.mobile.v1");
        assert_eq!(packet["device"].as_str().map(str::len), Some(64));
        assert!(packet["computers"].is_object());
        // A terminal request answers with the terminal packet.
        let text =
            packet_text(respond(&mut app, r#"{"op":"terminal_poll","known":null}"#).expect("poll"))
                .expect("packet");
        let packet: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(packet["open"], false);
        // The C ABI's packet for the same request is byte-for-byte the same shape.
        let request = br#"{"op":"snapshot"}"#;
        let raw = Box::into_raw(Box::new(create_app(&config(&dir)).expect("second app")));
        let buffer = unsafe { crate::openagents_mobile_call(raw, request.as_ptr(), request.len()) };
        let c_packet: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) })
                .expect("c packet");
        unsafe {
            crate::openagents_mobile_buffer_free(buffer);
            crate::openagents_mobile_destroy(raw);
        }
        assert_eq!(c_packet["schema"], "openagents.mobile.v1");
    }

    #[test]
    fn launch_options_reach_the_app_as_through_the_c_abi() {
        let dir = tempfile::tempdir().expect("temp dir");
        let native = serde_json::json!({"state_dir": dir.path(), "secret_hex": "11".repeat(32),
            "native_computers": true})
        .to_string();
        let mut app = create_app(&native).expect("app");
        let packet: serde_json::Value = serde_json::from_str(
            &packet_text(respond(&mut app, r#"{"op":"snapshot"}"#).expect("snapshot"))
                .expect("packet"),
        )
        .expect("json");
        // The host draws the Computers list, so Rust sends it as rows.
        assert!(packet["computers_home"].is_object(), "{packet}");
        let mut shared = create_app(&config(&dir)).expect("app");
        let packet: serde_json::Value = serde_json::from_str(
            &packet_text(respond(&mut shared, r#"{"op":"snapshot"}"#).expect("snapshot"))
                .expect("packet"),
        )
        .expect("json");
        assert!(packet["computers_home"].is_null());
    }

    #[test]
    fn a_held_studio_link_is_taken_once() {
        let dir = tempfile::tempdir().expect("temp dir");
        let app = create_app(&config(&dir)).expect("app");
        let token = hold_studio_links(&app, "  ");
        assert_eq!(
            take_studio_links(token).err().as_deref(),
            Some("Choose a computer for the studio")
        );
        assert_eq!(
            take_studio_links(token).err().as_deref(),
            Some("Try connecting the studio again")
        );
    }

    #[test]
    fn malformed_and_oversized_input_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(create_app("").is_err());
        assert!(create_app(&"x".repeat(MAX_CONFIG_BYTES + 1)).is_err());
        let bad_key = serde_json::json!({"state_dir": dir.path(), "secret_hex": "zz"}).to_string();
        assert!(create_app(&bad_key).is_err());
        let unknown = serde_json::json!({"state_dir": dir.path(), "secret_hex": "11".repeat(32),
            "relay": "wss://x"})
        .to_string();
        assert!(
            create_app(&unknown).is_ok(),
            "the C ABI's config ignores unknown fields too"
        );
        let mut app = create_app(&config(&dir)).expect("app");
        assert_eq!(
            respond(&mut app, r#"{"op":"launch_missiles"}"#)
                .unwrap_err()
                .0,
            "Invalid native app request"
        );
        assert!(respond(&mut app, "").is_err());
        let long = format!(
            r#"{{"op":"coder_input","token":"t","value":"{}"}}"#,
            "a".repeat(MAX_REQUEST_BYTES)
        );
        assert_eq!(
            respond(&mut app, &long).unwrap_err().0,
            "Native input exceeds its size limit"
        );
        assert!(packet_text(vec![]).is_err());
        assert!(packet_text(vec![0xff, 0xfe]).is_err());
        assert!(packet_text(vec![b' '; MAX_PACKET_BYTES + 1]).is_err());
    }

    #[test]
    fn verse_surface_configuration_is_bounded() {
        let ok = surface_config(r#"{"width":1080,"height":2400,"scale":2.625}"#).expect("config");
        assert_eq!((ok.width, ok.height), (1080, 2400));
        for bad in [
            r#"{"width":0,"height":10,"scale":1}"#,
            r#"{"width":5000,"height":10,"scale":1}"#,
            r#"{"width":10,"height":10,"scale":0}"#,
            r#"{"width":10,"height":10,"scale":1,"relay":"wss://x"}"#,
            "not json",
        ] {
            assert!(surface_config(bad).is_err(), "{bad}");
        }
        let world = |key: String| {
            surface_config(&format!(
                r#"{{"width":10,"height":10,"scale":1,"world_secret_hex":"{key}"}}"#
            ))
            .expect("config")
            .presence()
        };
        assert!(world("ab".repeat(32)).expect("presence").is_some());
        for key in ["zz".repeat(32), "11".repeat(31)] {
            assert_eq!(
                world(key).err().map(|e| e.0).as_deref(),
                Some("Invalid Verse world identity")
            );
        }
        assert!(ok.presence().expect("offline").is_none());
        // The Android host shows both Gym panels, as iOS does.
        let gym = ok.gym().expect("gym");
        assert!(gym.panel && gym.results_panel && gym.code.is_none() && !gym.preview);
        let saved = surface_config(
            r#"{"width":10,"height":10,"scale":1,"gym_code":"gym-connect:x","gym_preview":true,
                "results_cache_directory":"/cache"}"#,
        )
        .expect("config")
        .gym()
        .expect("gym");
        assert_eq!(saved.code.as_deref(), Some("gym-connect:x"));
        assert!(saved.preview);
        assert_eq!(saved.results_cache_directory.as_deref(), Some("/cache"));
        let long = format!(
            r#"{{"width":10,"height":10,"scale":1,"gym_code":"{}"}}"#,
            "x".repeat(MAX_GYM_CODE_BYTES + 1)
        );
        assert_eq!(
            surface_config(&long)
                .expect("config")
                .gym()
                .err()
                .map(|e| e.0)
                .as_deref(),
            Some("That Gym connection code is too long")
        );
    }

    #[test]
    fn a_panic_becomes_an_error() {
        let result: Result<(), _> = guarded(|| panic!("boom"));
        let message = result.unwrap_err().0;
        assert!(
            message.starts_with("The native library failed unexpectedly: boom (")
                && message.contains("android.rs"),
            "{message}"
        );
    }
}
