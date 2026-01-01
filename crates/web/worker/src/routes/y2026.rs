//! 2026 page - Key themes and links

use worker::*;

/// View the 2026 page: /2026
pub async fn view_2026(_env: Env) -> Result<Response> {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>2026 - OpenAgents</title>
    <meta name="description" content="Key themes for 2026: Local AI, Swarm AI, Open vs Closed, Agents vs Models, Autopilots, Agent Networks">
    <link rel="stylesheet" href="/static/MyWebfontsKit.css">
    <style>
        html, body {
            margin: 0;
            padding: 0;
            background: #0a0a0a;
            color: #e0e0e0;
            font-family: 'Square721StdRoman', sans-serif;
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
        window.Y2026_PAGE = true;

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
