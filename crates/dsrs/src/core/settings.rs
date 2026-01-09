use std::sync::{Arc, LazyLock, RwLock};

use super::LM;
use crate::adapter::Adapter;
use crate::callbacks::{DspyCallback, NoopCallback};

/// Global settings for DSPy execution.
pub struct Settings {
    /// The language model to use.
    pub lm: Arc<LM>,
    /// The adapter for formatting prompts.
    pub adapter: Arc<dyn Adapter>,
    /// Optional callback for observing execution.
    pub callback: Arc<dyn DspyCallback>,
}

impl Settings {
    /// Create new settings with LM and adapter.
    pub fn new(lm: LM, adapter: impl Adapter + 'static) -> Self {
        Self {
            lm: Arc::new(lm),
            adapter: Arc::new(adapter),
            callback: Arc::new(NoopCallback),
        }
    }

    /// Create settings with a callback.
    pub fn with_callback(lm: LM, adapter: impl Adapter + 'static, callback: impl DspyCallback + 'static) -> Self {
        Self {
            lm: Arc::new(lm),
            adapter: Arc::new(adapter),
            callback: Arc::new(callback),
        }
    }
}

pub static GLOBAL_SETTINGS: LazyLock<RwLock<Option<Settings>>> =
    LazyLock::new(|| RwLock::new(None));

/// Get the configured LM.
pub fn get_lm() -> Arc<LM> {
    Arc::clone(&GLOBAL_SETTINGS.read().unwrap().as_ref().unwrap().lm)
}

/// Get the configured callback.
pub fn get_callback() -> Arc<dyn DspyCallback> {
    GLOBAL_SETTINGS
        .read()
        .unwrap()
        .as_ref()
        .map(|s| Arc::clone(&s.callback))
        .unwrap_or_else(|| Arc::new(NoopCallback))
}

/// Configure DSPy with LM and adapter.
pub fn configure(lm: LM, adapter: impl Adapter + 'static) {
    let settings = Settings::new(lm, adapter);
    *GLOBAL_SETTINGS.write().unwrap() = Some(settings);
}

/// Configure DSPy with LM, adapter, and callback.
pub fn configure_with_callback(
    lm: LM,
    adapter: impl Adapter + 'static,
    callback: impl DspyCallback + 'static,
) {
    let settings = Settings::with_callback(lm, adapter, callback);
    *GLOBAL_SETTINGS.write().unwrap() = Some(settings);
}

/// Set the callback for an already-configured system.
pub fn set_callback(callback: impl DspyCallback + 'static) {
    if let Some(settings) = GLOBAL_SETTINGS.write().unwrap().as_mut() {
        settings.callback = Arc::new(callback);
    }
}
