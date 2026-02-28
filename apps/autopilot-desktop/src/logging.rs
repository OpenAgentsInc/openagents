use std::sync::Once;

static LOG_INIT: Once = Once::new();

pub fn init() {
    LOG_INIT.call_once(|| {
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,wgpu=warn,winit=warn"));

        let _ = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_level(true)
            .with_target(true)
            .without_time()
            .try_init();
    });
}
