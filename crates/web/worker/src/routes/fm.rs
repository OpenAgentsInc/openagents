//! FM Bridge (Apple Foundation Models) visualization page

use worker::*;

/// View the FM Bridge visualization page: /fm
pub async fn view_fm(_env: Env) -> Result<Response> {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>FM Bridge - OpenAgents</title>
    <meta name="description" content="Apple Foundation Models inference visualization.">
    <link rel="stylesheet" href="/static/MyWebfontsKit.css">
    <style>
        html, body {
            margin: 0;
            padding: 0;
            background: #0a0a0a;
            color: #e0e0e0;
            font-family: 'Vera Mono', 'DejaVu Sans Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
            overflow: hidden;
            width: 100%;
            height: 100%;
        }
        #hud-container {
            width: 100vw;
            height: 100vh;
            overflow: hidden;
        }
        canvas {
            width: 100%;
            height: 100%;
            display: block;
        }
    </style>
</head>
<body>
    <div id="hud-container">
        <canvas id="canvas"></canvas>
    </div>
    <script type="module">
        window.FM_PAGE = true;

        // FM Bridge URL - default to localhost:11435
        window.FM_BRIDGE_URL = 'http://localhost:11435';

        const params = new URLSearchParams(window.location.search);
        const bridgeUrl = params.get("bridge");
        if (bridgeUrl) {
            window.FM_BRIDGE_URL = bridgeUrl;
        }

        const dataUrl = params.get("data");
        if (dataUrl) {
            window.FM_DATA_URL = dataUrl;
        }

        import init, { start_demo } from '/pkg/openagents_web_client.js';

        async function run() {
            await init();
            await start_demo('canvas');
        }

        run().catch(console.error);
    </script>
</body>
</html>"#;

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("Cross-Origin-Opener-Policy", "same-origin")?;
    headers.set("Cross-Origin-Embedder-Policy", "require-corp")?;
    headers.set("X-Frame-Options", "SAMEORIGIN")?;

    Ok(Response::ok(html)?.with_headers(headers))
}
