//! Base layout - pure black screen

/// Base HTML layout - pure black with WebSocket support
pub fn base_layout(content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenAgents</title>
    <script src="https://unpkg.com/htmx.org@2.0.4"></script>
    <script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js"></script>
    <style>
        @font-face {{
            font-family: 'Berkeley Mono';
            src: local('Berkeley Mono');
        }}

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
            font-family: 'Berkeley Mono', ui-monospace, monospace;
        }}

        body {{
            display: flex;
            align-items: center;
            justify-content: center;
        }}
    </style>
</head>
<body hx-ext="ws" ws-connect="/ws">
    {content}
</body>
</html>"#,
        content = content
    )
}
