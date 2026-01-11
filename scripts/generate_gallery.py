#!/usr/bin/env python3
"""
DEPRECATED: Demo gallery moved to ~/code/backroom
This script is no longer used. Demo and demos folders have been archived.
"""

import sys
print("ERROR: Demo gallery has been moved to ~/code/backroom")
print("This script is deprecated and should not be run.")
sys.exit(1)

import json
from pathlib import Path

def generate_gallery_html():
    """Generate complete gallery.html from index.json."""

    # Load demo index
    with open('demos/index.json', 'r') as f:
        data = json.load(f)

    # Pre-format the template with stats
    demo_cards_html = generate_demo_cards(data['demos'])

    html = """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenAgents Autopilot - Demo Gallery</title>
    <meta name="description" content="Watch OpenAgents Autopilot autonomously write code, fix bugs, and complete tasks. Real session replays demonstrating AI-powered software engineering.">
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: #0a0a0a;
            color: #e0e0e0;
            line-height: 1.6;
        }

        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
        }

        header {
            text-align: center;
            padding: 60px 20px;
            background: linear-gradient(135deg, #1a1a1a 0%, #2d2d2d 100%);
            border-bottom: 2px solid #00ff88;
        }

        h1 {
            font-size: 3em;
            margin-bottom: 20px;
            color: #00ff88;
        }

        .subtitle {
            font-size: 1.3em;
            color: #aaa;
            margin-bottom: 30px;
        }

        .stats {
            display: flex;
            justify-content: center;
            gap: 40px;
            flex-wrap: wrap;
            margin-top: 30px;
        }

        .stat {
            text-align: center;
        }

        .stat-value {
            font-size: 2.5em;
            font-weight: bold;
            color: #00ff88;
        }

        .stat-label {
            font-size: 0.9em;
            color: #888;
            text-transform: uppercase;
        }

        .filters {
            display: flex;
            gap: 15px;
            margin: 40px 0;
            flex-wrap: wrap;
        }

        .filter-btn {
            padding: 10px 20px;
            background: #2d2d2d;
            border: 1px solid #444;
            color: #e0e0e0;
            cursor: pointer;
            border-radius: 5px;
            transition: all 0.3s;
        }

        .filter-btn:hover,
        .filter-btn.active {
            background: #00ff88;
            color: #0a0a0a;
            border-color: #00ff88;
        }

        .demo-grid {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
            gap: 30px;
            margin: 40px 0;
        }

        .demo-card {
            background: #1a1a1a;
            border: 1px solid #333;
            border-radius: 8px;
            padding: 25px;
            transition: all 0.3s;
            cursor: pointer;
        }

        .demo-card:hover {
            border-color: #00ff88;
            transform: translateY(-5px);
            box-shadow: 0 10px 30px rgba(0, 255, 136, 0.2);
        }

        .demo-header {
            display: flex;
            justify-content: space-between;
            align-items: start;
            margin-bottom: 15px;
        }

        .demo-title {
            font-size: 1.3em;
            color: #00ff88;
            margin-bottom: 5px;
        }

        .score-badge {
            background: #00ff88;
            color: #0a0a0a;
            padding: 5px 12px;
            border-radius: 20px;
            font-weight: bold;
            font-size: 0.9em;
        }

        .difficulty-badge {
            display: inline-block;
            padding: 3px 10px;
            border-radius: 3px;
            font-size: 0.8em;
            text-transform: uppercase;
            margin-top: 5px;
        }

        .difficulty-beginner { background: #4CAF50; color: white; }
        .difficulty-intermediate { background: #FF9800; color: white; }
        .difficulty-advanced { background: #F44336; color: white; }
        .difficulty-expert { background: #9C27B0; color: white; }

        .demo-description {
            color: #aaa;
            margin: 15px 0;
            line-height: 1.5;
        }

        .demo-highlights {
            margin: 15px 0;
        }

        .demo-highlights h4 {
            font-size: 0.9em;
            color: #888;
            margin-bottom: 8px;
        }

        .demo-highlights ul {
            list-style: none;
        }

        .demo-highlights li {
            padding: 5px 0;
            color: #bbb;
            font-size: 0.9em;
        }

        .demo-highlights li:before {
            content: "✓ ";
            color: #00ff88;
            font-weight: bold;
            margin-right: 5px;
        }

        .demo-meta {
            display: flex;
            gap: 15px;
            margin: 15px 0;
            font-size: 0.85em;
            color: #888;
        }

        .demo-meta span {
            display: flex;
            align-items: center;
            gap: 5px;
        }

        .demo-actions {
            display: flex;
            gap: 10px;
            margin-top: 20px;
        }

        .btn {
            padding: 10px 20px;
            border: none;
            border-radius: 5px;
            cursor: pointer;
            font-size: 0.9em;
            transition: all 0.3s;
            text-decoration: none;
            display: inline-block;
        }

        .btn-primary {
            background: #00ff88;
            color: #0a0a0a;
            font-weight: bold;
        }

        .btn-primary:hover {
            background: #00cc6f;
            transform: scale(1.05);
        }

        .btn-secondary {
            background: transparent;
            color: #00ff88;
            border: 1px solid #00ff88;
        }

        .btn-secondary:hover {
            background: rgba(0, 255, 136, 0.1);
        }

        footer {
            text-align: center;
            padding: 60px 20px;
            margin-top: 80px;
            border-top: 1px solid #333;
            color: #666;
        }

        footer a {
            color: #00ff88;
            text-decoration: none;
        }

        footer a:hover {
            text-decoration: underline;
        }

        @media (max-width: 768px) {
            h1 { font-size: 2em; }
            .demo-grid { grid-template-columns: 1fr; }
            .stats { gap: 20px; }
        }
    </style>
</head>
<body>
    <header>
        <h1>🤖 Autopilot Demo Gallery</h1>
        <p class="subtitle">Watch AI autonomously write code, fix bugs, and ship features</p>

        <div class="stats">
            <div class="stat">
                <div class="stat-value">{total_demos}</div>
                <div class="stat-label">Demos</div>
            </div>
            <div class="stat">
                <div class="stat-value">{avg_score:.1f}</div>
                <div class="stat-label">Avg Quality</div>
            </div>
            <div class="stat">
                <div class="stat-value">{total_tokens:,}</div>
                <div class="stat-label">Total Tokens</div>
            </div>
            <div class="stat">
                <div class="stat-value">{total_tools}</div>
                <div class="stat-label">Tools Used</div>
            </div>
        </div>
    </header>

    <div class="container">
        <div class="filters">
            <button class="filter-btn active" data-filter="all">All Demos</button>
            <button class="filter-btn" data-filter="beginner">Beginner</button>
            <button class="filter-btn" data-filter="intermediate">Intermediate</button>
            <button class="filter-btn" data-filter="advanced">Advanced</button>
            <button class="filter-btn" data-filter="expert">Expert</button>
        </div>

        <div class="demo-grid">
{demo_cards}
        </div>
    </div>

    <footer>
        <p>Built with <a href="https://github.com/OpenAgentsInc/openagents">OpenAgents</a> •
        Powered by <a href="https://codex.ai">Codex</a> •
        <a href="https://github.com/OpenAgentsInc/openagents/issues">Report Issues</a></p>
        <p style="margin-top: 20px; font-size: 0.9em;">
            These are real, unedited autopilot sessions. No cherry-picking, no manual intervention.
        </p>
    </footer>

    <script>
        // Filter functionality
        document.querySelectorAll('.filter-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                // Update active state
                document.querySelectorAll('.filter-btn').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');

                // Filter demos
                const filter = btn.dataset.filter;
                document.querySelectorAll('.demo-card').forEach(card => {
                    if (filter === 'all' || card.dataset.difficulty === filter) {
                        card.style.display = 'block';
                    } else {
                        card.style.display = 'none';
                    }
                });
            });
        });

        // Demo card click handling
        document.querySelectorAll('.demo-card').forEach(card => {
            card.addEventListener('click', (e) => {
                // Don't trigger if clicking a button
                if (e.target.tagName === 'A' || e.target.tagName === 'BUTTON') return;

                // Download bundle
                const downloadBtn = card.querySelector('.btn-primary');
                if (downloadBtn) downloadBtn.click();
            });
        });
    </script>
</body>
</html>
"""

    # Replace placeholders
    html = html.replace('{total_demos}', str(data['stats']['total_demos']))
    html = html.replace('{avg_score:.1f}', f"{data['stats']['average_quality_score']:.1f}")
    html = html.replace('{total_tokens:,}', f"{data['stats']['total_tokens']:,}")
    html = html.replace('{total_tools}', str(len(data['stats']['tools_demonstrated'])))
    html = html.replace('{demo_cards}', demo_cards_html)

    return html

def generate_demo_cards(demos):
    """Generate HTML for demo cards."""
    cards = []

    for demo in demos:
        highlights = '\n'.join(f'<li>{h}</li>' for h in demo['highlights'])
        tools = ', '.join(demo['tools_used'][:5])
        if len(demo['tools_used']) > 5:
            tools += f" +{len(demo['tools_used']) - 5} more"

        card = f"""
            <div class="demo-card" data-difficulty="{demo['difficulty']}">
                <div class="demo-header">
                    <div>
                        <h3 class="demo-title">{demo['title']}</h3>
                        <span class="difficulty-badge difficulty-{demo['difficulty']}">{demo['difficulty']}</span>
                    </div>
                    <div class="score-badge">{demo['quality_score']}/100</div>
                </div>

                <p class="demo-description">{demo['description']}</p>

                <div class="demo-highlights">
                    <h4>Highlights:</h4>
                    <ul>
                        {highlights}
                    </ul>
                </div>

                <div class="demo-meta">
                    <span>📅 {demo['date']}</span>
                    <span>💬 {demo['duration_tokens']:,} tokens</span>
                    <span>📦 {demo['size_kb']}KB</span>
                </div>

                <div class="demo-meta">
                    <span>🛠️ {tools}</span>
                </div>

                <div class="demo-actions">
                    <a href="demos/{demo['bundle']}" class="btn btn-primary" download>Download Bundle</a>
                    <a href="viewer.html?demo={demo['id']}" class="btn btn-secondary">View Replay</a>
                </div>
            </div>"""

        cards.append(card)

    return '\n'.join(cards)

if __name__ == '__main__':
    html = generate_gallery_html()
    print(html)
