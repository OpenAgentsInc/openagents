//! Install instructions page for agents

use worker::*;

/// View the install page: /install
pub async fn view_install(_env: Env) -> Result<Response> {
    let instructions = r#"# Install Pylon

Pylon connects your machine to the OpenAgents compute network as a provider.

## Requirements

**Apple Silicon Mac required** - Pylon's compute provider functionality
currently only works fully on Apple Silicon Macs (M1/M2/M3/M4). This is
required for local AI inference using Metal GPU acceleration.

- macOS on Apple Silicon (M1, M2, M3, M4)
- Rust toolchain (install from https://rustup.rs if needed)

## Install

```bash
# Clone the repository
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents

# Build Pylon
cargo build --release -p pylon

# Run Pylon (connects to the network)
./target/release/pylon
```

## Configuration

Pylon connects to the OpenAgents relay network automatically.
No additional configuration required for basic usage.

## Verify

After starting Pylon, you should see:
- Connection to relay established
- Ready to receive compute jobs

## Troubleshooting

If you encounter issues:
1. Ensure Rust is up to date: `rustup update`
2. Check your firewall allows outbound WebSocket connections
3. View logs with: `RUST_LOG=debug ./target/release/pylon`

## Next Steps

Once Pylon is running, your machine is connected to the OpenAgents
compute network and can receive AI inference jobs.

Run `./target/release/pylon help` to see available commands.
"#;

    // Escape backticks for JavaScript
    let instructions_escaped = instructions.replace('`', "\\`");

    let html = format!(r##"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Install Pylon - OpenAgents</title>
    <meta name="description" content="Install Pylon to join the OpenAgents compute network">

    <meta property="og:type" content="website">
    <meta property="og:title" content="Install Pylon">
    <meta property="og:description" content="Install Pylon to join the OpenAgents compute network">
    <meta property="og:url" content="https://openagents.com/install">
    <meta property="og:site_name" content="OpenAgents">

    <meta name="twitter:card" content="summary">
    <meta name="twitter:site" content="@OpenAgentsInc">
    <meta name="twitter:title" content="Install Pylon">
    <meta name="twitter:description" content="Install Pylon to join the OpenAgents compute network">
    <style>
        html, body {{
            margin: 0;
            padding: 0;
            background: #000;
            color: #fff;
            font-family: 'Vera Mono', 'DejaVu Sans Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
            font-size: 14px;
            line-height: 1.6;
        }}
        .container {{
            max-width: 720px;
            margin: 0 auto;
            padding: 40px 20px;
        }}
        .header-row {{
            margin-bottom: 16px;
        }}
        .page-label {{
            color: #f80;
            font-size: 12px;
            margin: 0 0 4px 0;
            letter-spacing: 0.05em;
        }}
        h1 {{
            font-size: 24px;
            margin: 0 0 8px 0;
            font-weight: normal;
        }}
        .subtitle {{
            color: #666;
            font-size: 12px;
            margin: 0;
        }}
        .copy-btn {{
            background: #f80;
            color: #000;
            border: none;
            padding: 6px 12px;
            font-size: 11px;
            font-family: 'Vera Mono', 'DejaVu Sans Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
            cursor: pointer;
            font-weight: bold;
        }}
        .copy-btn:hover {{
            background: #ff9922;
        }}
        .copy-btn:active {{
            background: #dd7700;
        }}
        .copy-btn .full-text {{
            display: none;
        }}
        .copy-btn .short-text {{
            display: inline;
        }}
        @media (min-width: 480px) {{
            .copy-btn .full-text {{
                display: inline;
            }}
            .copy-btn .short-text {{
                display: none;
            }}
        }}
        .instructions-frame {{
            background: #0a0a0a;
            border: 1px solid #222;
            padding: 0;
            margin: 0 0 24px 0;
        }}
        .frame-header {{
            background: #111;
            border-bottom: 1px solid #222;
            padding: 8px 12px;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }}
        .frame-label {{
            color: #666;
            font-size: 11px;
            text-transform: uppercase;
            letter-spacing: 0.1em;
        }}
        .instructions-content {{
            padding: 20px;
            white-space: pre-wrap;
            font-size: 13px;
            max-height: 500px;
            overflow-y: auto;
        }}
        .instructions-content::-webkit-scrollbar {{
            width: 8px;
        }}
        .instructions-content::-webkit-scrollbar-track {{
            background: #111;
        }}
        .instructions-content::-webkit-scrollbar-thumb {{
            background: #333;
        }}
        .instructions-content::-webkit-scrollbar-thumb:hover {{
            background: #444;
        }}
        .note {{
            color: #666;
            font-size: 12px;
            margin-top: 16px;
        }}
        hr {{
            border: none;
            border-top: 1px solid #222;
            margin: 32px 0;
        }}
        a {{
            color: #f80;
            text-decoration: none;
        }}
        a:hover {{
            text-decoration: underline;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header-row">
            <p class="page-label">AGENT INSTRUCTIONS</p>
            <h1>Install Pylon</h1>
            <p class="subtitle">Copy these instructions to your agent</p>
        </div>

        <div class="instructions-frame">
            <div class="frame-header">
                <span class="frame-label">Instructions</span>
                <button class="copy-btn" id="copyBtn"><span class="short-text">Copy</span><span class="full-text">Copy instructions</span></button>
            </div>
            <div class="instructions-content" id="instructionsContent">{instructions}</div>
        </div>

        <p class="note">These instructions are designed for AI agents to follow. Copy and paste to your agent to install Pylon on your machine.</p>

        <hr>
        <p style="color: #666; text-align: center; margin-top: 40px;"><a href="/">OpenAgents</a></p>
    </div>
    <script>
        const rawInstructions = `{instructions_escaped}`;

        document.getElementById('copyBtn').addEventListener('click', function() {{
            navigator.clipboard.writeText(rawInstructions).then(() => {{
                const btn = this;
                const shortSpan = btn.querySelector('.short-text');
                const fullSpan = btn.querySelector('.full-text');
                const origShort = shortSpan.textContent;
                const origFull = fullSpan.textContent;
                shortSpan.textContent = 'Copied!';
                fullSpan.textContent = 'Copied!';
                setTimeout(() => {{
                    shortSpan.textContent = origShort;
                    fullSpan.textContent = origFull;
                }}, 2000);
            }}).catch(err => {{
                console.error('Failed to copy:', err);
                alert('Failed to copy instructions');
            }});
        }});
    </script>
</body>
</html>"##,
        instructions = instructions,
        instructions_escaped = instructions_escaped,
    );

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("X-Frame-Options", "SAMEORIGIN")?;

    Ok(Response::ok(html)?.with_headers(headers))
}
