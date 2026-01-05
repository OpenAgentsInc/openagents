//! FRLM (Fracking Apple Silicon) page - power comparison visualization

use worker::*;

/// View the FRLM page: /frack
/// Companion slide for "Fracking Apple Silicon" video showing power capacity comparison
pub async fn view_frlm(_env: Env) -> Result<Response> {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Fracking Apple Silicon - Power at Scale - OpenAgents</title>
    <meta name="description" content="Power at Scale: comparing AI data centers, OpenAI Stargate, and the Apple Silicon Mac swarm for distributed AI inference.">
    <meta property="og:title" content="Fracking Apple Silicon - Power at Scale">
    <meta property="og:description" content="110M Apple Silicon Macs = 5.5 GW of distributed AI compute. Stargate = 1.2 GW. The fleet already exists.">
    <meta property="og:type" content="website">
    <meta property="og:url" content="https://openagents.com/frack">
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
        window.FRLM_PAGE = true;

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
