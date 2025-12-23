//! Base layout - pure black screen with Tailwind

use std::sync::OnceLock;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use ui::{TAILWIND_CDN, TAILWIND_THEME};

fn vera_mono_font_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let regular = BASE64_STANDARD.encode(include_bytes!("../assets/fonts/VeraMono.ttf"));
        let italic = BASE64_STANDARD.encode(include_bytes!("../assets/fonts/VeraMono-Italic.ttf"));
        let bold = BASE64_STANDARD.encode(include_bytes!("../assets/fonts/VeraMono-Bold.ttf"));
        let bold_italic =
            BASE64_STANDARD.encode(include_bytes!("../assets/fonts/VeraMono-Bold-Italic.ttf"));

        format!(
            r#"
        @font-face {{
            font-family: 'Vera Mono';
            src: url("data:font/ttf;base64,{regular}") format("truetype");
            font-weight: 400;
            font-style: normal;
        }}

        @font-face {{
            font-family: 'Vera Mono';
            src: url("data:font/ttf;base64,{italic}") format("truetype");
            font-weight: 400;
            font-style: italic;
        }}

        @font-face {{
            font-family: 'Vera Mono';
            src: url("data:font/ttf;base64,{bold}") format("truetype");
            font-weight: 700;
            font-style: normal;
        }}

        @font-face {{
            font-family: 'Vera Mono';
            src: url("data:font/ttf;base64,{bold_italic}") format("truetype");
            font-weight: 700;
            font-style: italic;
        }}
"#,
            regular = regular,
            italic = italic,
            bold = bold,
            bold_italic = bold_italic
        )
    })
}

/// Base HTML layout with optional auth token for WebSocket connection
///
/// SECURITY: The auth token is included in the WebSocket connection URL
/// to authenticate WebSocket upgrades.
pub fn base_layout_with_token(content: &str, auth_token: Option<&str>) -> String {
    let ws_url = if let Some(token) = auth_token {
        format!("/ws?token={}", token)
    } else {
        "/ws".to_string()
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenAgents</title>
    <script src="https://unpkg.com/htmx.org@2.0.4"></script>
    <script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js"></script>
    <script>{tailwind_cdn}</script>
    <style type="text/tailwindcss">
        {tailwind_theme}

        {font_css}

        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}

        html, body {{
            height: 100%;
            width: 100%;
            background: #000;
            color: #fafafa;
            font-family: 'Vera Mono', ui-monospace, monospace;
        }}

        body {{
            display: flex;
            align-items: center;
            justify-content: center;
        }}
    </style>
</head>
<body hx-ext="ws" ws-connect="{ws_url}">
    {content}
</body>
</html>"#,
        tailwind_cdn = TAILWIND_CDN,
        tailwind_theme = TAILWIND_THEME,
        font_css = vera_mono_font_css(),
        ws_url = ws_url,
        content = content
    )
}
