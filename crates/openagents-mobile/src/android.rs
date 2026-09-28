//! The Android host's JNI surface, beside the C ABI the iOS host uses. It
//! carries the same JSON requests and packets, so both hosts stay thin.
//!
//! Application handles live on one background worker thread; Verse handles
//! live on Android's main thread. Handle IDs are counters, never native
//! pointers, and a disposed ID is never reused. Every entry point bounds its
//! input and turns a Rust error or panic into a Java `RuntimeException`.
//! The JNI exports are in `exports`, built for Android only; the checks they
//! share are here, so host tests cover them.
use crate::{App, Config, Request};

#[cfg(target_os = "android")]
mod exports;

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) const MAX_HANDLES: usize = 4;
pub(crate) const MAX_CONFIG_BYTES: usize = 16 * 1024;
pub(crate) const MAX_REQUEST_BYTES: usize = 128 * 1024;
/// A Verse request carries no panel feeds, so it stays small.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) const MAX_VERSE_REQUEST_BYTES: usize = 4096;
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

/// Creates the app from `{"state_dir": ..., "secret_hex": ...}`.
pub(crate) fn create_app(config: &str) -> Result<App, BridgeError> {
    if config.is_empty() || config.len() > MAX_CONFIG_BYTES {
        return Err(error("Native input exceeds its size limit"));
    }
    let config: Config =
        serde_json::from_str(config).map_err(|_| error("Invalid native app configuration"))?;
    App::new(config).map_err(BridgeError::from)
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
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scale: f32,
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
