//! Base layout with navigation

/// Base HTML layout with navigation tabs
pub fn base_layout(title: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title} - OpenAgents</title>
    <script src="https://unpkg.com/htmx.org@2.0.4"></script>
    <style>
        :root {{
            --bg-primary: #0a0a0a;
            --bg-secondary: #141414;
            --bg-tertiary: #1e1e1e;
            --text-primary: #e5e5e5;
            --text-secondary: #a0a0a0;
            --accent: #3b82f6;
            --accent-hover: #2563eb;
            --border: #2e2e2e;
        }}

        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            min-height: 100vh;
        }}

        nav {{
            background: var(--bg-secondary);
            border-bottom: 1px solid var(--border);
            padding: 0 1rem;
            display: flex;
            gap: 0;
        }}

        nav a {{
            color: var(--text-secondary);
            text-decoration: none;
            padding: 1rem 1.5rem;
            display: block;
            border-bottom: 2px solid transparent;
            transition: all 0.2s;
        }}

        nav a:hover {{
            color: var(--text-primary);
            background: var(--bg-tertiary);
        }}

        nav a.active {{
            color: var(--accent);
            border-bottom-color: var(--accent);
        }}

        main {{
            padding: 2rem;
            max-width: 1400px;
            margin: 0 auto;
        }}

        h1 {{
            font-size: 2rem;
            margin-bottom: 1rem;
        }}

        .dashboard {{
            text-align: center;
            padding: 4rem 2rem;
        }}

        .quick-links {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 1.5rem;
            margin-top: 3rem;
        }}

        .card {{
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            padding: 2rem;
            text-decoration: none;
            color: inherit;
            transition: all 0.2s;
        }}

        .card:hover {{
            background: var(--bg-tertiary);
            border-color: var(--accent);
        }}

        .card h3 {{
            color: var(--accent);
            margin-bottom: 0.5rem;
        }}

        .card p {{
            color: var(--text-secondary);
            font-size: 0.9rem;
        }}

        footer {{
            position: fixed;
            bottom: 0;
            left: 0;
            right: 0;
            background: var(--bg-secondary);
            border-top: 1px solid var(--border);
            padding: 0.75rem 1rem;
            font-size: 0.85rem;
            color: var(--text-secondary);
            display: flex;
            justify-content: space-between;
        }}
    </style>
</head>
<body>
    <nav>
        <a href="/" class="active">Dashboard</a>
        <a href="/wallet">Wallet</a>
        <a href="/marketplace">Marketplace</a>
        <a href="/autopilot">Autopilot</a>
        <a href="/git">AgentGit</a>
        <a href="/daemon">Daemon</a>
    </nav>

    <main>
        {content}
    </main>

    <footer>
        <span>OpenAgents Desktop</span>
        <span id="status">Connecting...</span>
    </footer>

    <script>
        // WebSocket connection for real-time updates
        const ws = new WebSocket(`ws://${{location.host}}/ws`);
        ws.onopen = () => document.getElementById('status').textContent = 'Connected';
        ws.onclose = () => document.getElementById('status').textContent = 'Disconnected';
        ws.onmessage = (e) => {{
            // Handle HTMX out-of-band updates
            if (e.data.includes('hx-swap-oob')) {{
                document.body.insertAdjacentHTML('beforeend', e.data);
            }}
        }};
    </script>
</body>
</html>"#,
        title = title,
        content = content
    )
}
