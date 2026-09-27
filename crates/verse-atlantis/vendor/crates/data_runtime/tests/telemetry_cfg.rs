use data_runtime::configs::telemetry::load_default;

#[test]
fn env_overrides_parse() {
    unsafe {
        std::env::set_var("LOG_LEVEL", "debug");
        std::env::set_var("JSON_LOGS", "false");
        std::env::set_var("TRACE_SAMPLE", "0.25");
    }
    let cfg = load_default().expect("load");
    assert_eq!(cfg.log_level.as_deref(), Some("debug"));
    assert_eq!(cfg.json_logs, Some(false));
    assert_eq!(cfg.trace_sample, Some(0.25));
}
