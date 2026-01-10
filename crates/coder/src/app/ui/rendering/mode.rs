fn coder_mode_display(mode: CoderMode) -> &'static str {
    match mode {
        CoderMode::BypassPermissions => "Bypass",
        CoderMode::Plan => "Plan",
        CoderMode::Autopilot => "Autopilot",
    }
}

fn coder_mode_color(mode: CoderMode, _palette: &UiPalette) -> Hsla {
    match mode {
        CoderMode::BypassPermissions => Hsla::new(120.0, 0.6, 0.5, 1.0), // Green
        CoderMode::Plan => Hsla::new(200.0, 0.8, 0.5, 1.0), // Blue
        CoderMode::Autopilot => Hsla::new(280.0, 0.6, 0.5, 1.0), // Purple
    }
}
