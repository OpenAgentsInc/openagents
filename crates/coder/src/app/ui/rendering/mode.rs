fn coder_mode_display(mode: CoderMode) -> &'static str {
    match mode {
        CoderMode::BypassPermissions => "Bypass",
        CoderMode::Plan => "Plan",
        CoderMode::Autopilot => "Autopilot",
    }
}

fn coder_mode_color(mode: CoderMode, _palette: &UiPalette) -> Hsla {
    // Hue is normalized 0.0-1.0 (divide degrees by 360)
    match mode {
        CoderMode::BypassPermissions => Hsla::new(0.0 / 360.0, 0.7, 0.5, 1.0),   // Red
        CoderMode::Plan => Hsla::new(200.0 / 360.0, 0.8, 0.55, 1.0),             // Blue
        CoderMode::Autopilot => Hsla::new(120.0 / 360.0, 0.7, 0.5, 1.0),         // Green
    }
}
