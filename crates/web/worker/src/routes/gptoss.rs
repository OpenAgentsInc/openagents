//! GPT-OSS model loading + inference visualization page

use worker::*;

/// View the GPT-OSS visualization page: /gptoss
pub async fn view_gptoss(_env: Env) -> Result<Response> {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>GPT-OSS Pipeline Viz - OpenAgents</title>
    <meta name="description" content="GPT-OSS model loading + inference telemetry visualization.">
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
        window.GPTOSS_PAGE = true;

        const params = new URLSearchParams(window.location.search);
        const dataUrl = params.get("data");
        if (dataUrl) {
            window.GPTOSS_DATA_URL = dataUrl;
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
