use data_runtime::configs::input_camera::load_default;

#[test]
fn env_overrides_parse_alt_hold() {
    unsafe {
        std::env::set_var("MOUSE_SENS_DEG", "0.2");
        std::env::set_var("INVERT_Y", "true");
        std::env::set_var("MIN_PITCH_DEG", "-70");
        std::env::set_var("MAX_PITCH_DEG", "70");
        std::env::set_var("ALT_HOLD", "true");
    }
    let cfg = load_default().expect("load");
    assert_eq!(cfg.sensitivity_deg_per_count, Some(0.2));
    assert_eq!(cfg.invert_y, Some(true));
    assert_eq!(cfg.min_pitch_deg, Some(-70.0));
    assert_eq!(cfg.max_pitch_deg, Some(70.0));
    assert_eq!(cfg.alt_hold, Some(true));
}
