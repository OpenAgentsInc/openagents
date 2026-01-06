//! RLM (Recursive Language Model) visualization page - interactive execution movie

use worker::*;

/// View the RLM page: /rlm
/// Interactive visualization of Recursive Language Model execution showing
/// structure discovery, chunking, extraction, and synthesis phases.
pub async fn view_rlm(_env: Env) -> Result<Response> {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>RLM Execution Visualizer - OpenAgents</title>
    <meta name="description" content="Interactive visualization of Recursive Language Model execution - watch AI decompose documents and synthesize answers in real-time.">
    <meta property="og:title" content="RLM Execution Visualizer - OpenAgents">
    <meta property="og:description" content="Watch Recursive Language Models process documents: structure discovery, semantic chunking, parallel extraction, and synthesis.">
    <meta property="og:type" content="website">
    <meta property="og:url" content="https://openagents.com/rlm">
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
        window.RLM_PAGE = true;

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
