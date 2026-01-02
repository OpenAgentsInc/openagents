//! The Agent Network - Episode 200 transcript

use worker::*;
use pulldown_cmark::{Parser, html};

/// View the agent network blog post: /the-agent-network
pub async fn view_the_agent_network(_env: Env) -> Result<Response> {
    // Include markdown at compile time
    let markdown_content = include_str!("../../../../../docs/transcripts/200-the-agent-network.md");

    // Parse markdown to HTML
    let parser = Parser::new(markdown_content);
    let mut body_html = String::new();
    html::push_html(&mut body_html, parser);

    // Escape backticks in markdown for JavaScript string literal
    let markdown_escaped = markdown_content.replace('`', "\\`");

    let html = format!(r##"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>The Agent Network - OpenAgents</title>
    <meta name="description" content="Episode 200: The Agent Network - Reed's Law, group-forming networks, and the future of agent coordination">
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
        h1 {{
            font-size: 24px;
            margin-bottom: 8px;
            font-weight: normal;
        }}
        h2 {{
            font-size: 18px;
            margin-top: 32px;
            margin-bottom: 16px;
            color: #fff;
        }}
        .header-row {{
            display: flex;
            justify-content: space-between;
            align-items: flex-start;
            margin-bottom: 40px;
            gap: 24px;
        }}
        .episode-label {{
            color: #888;
            font-size: 12px;
            margin: 0 0 4px 0;
            letter-spacing: 0.05em;
        }}
        h1 {{
            margin: 0 0 8px 0;
        }}
        .episode-date {{
            color: #666;
            font-size: 12px;
            margin: 0;
        }}
        .copy-btn {{
            background: #f80;
            color: #000;
            border: none;
            padding: 6px 12px;
            font-size: 12px;
            font-family: 'Vera Mono', 'DejaVu Sans Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
            cursor: pointer;
            border-radius: 2px;
            font-weight: bold;
        }}
        .copy-btn:hover {{
            background: #ff9922;
        }}
        .copy-btn:active {{
            background: #dd7700;
        }}
        p {{
            margin: 16px 0;
        }}
        strong {{
            color: #fff;
        }}
        a {{
            color: #f80;
            text-decoration: none;
        }}
        a:hover {{
            text-decoration: underline;
        }}
        blockquote {{
            border-left: 2px solid #333;
            margin: 16px 0;
            padding-left: 16px;
            color: #aaa;
        }}
        .tweet-embed {{
            width: 100%;
            margin: 24px 0 32px;
        }}
        .twitter-tweet,
        iframe.twitter-tweet {{
            max-width: 100% !important;
            width: 100% !important;
        }}
        iframe.twitter-tweet {{
            display: block;
        }}
        code {{
            background: #111;
            padding: 2px 6px;
            font-size: 13px;
        }}
        pre {{
            background: #111;
            padding: 12px;
            overflow-x: auto;
            margin: 16px 0;
        }}
        ul, ol {{
            margin: 16px 0;
            padding-left: 24px;
        }}
        li {{
            margin: 8px 0;
        }}
        hr {{
            border: none;
            border-top: 1px solid #222;
            margin: 32px 0;
        }}
        em {{
            color: #ccc;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header-row">
            <div>
                <p class="episode-label">EPISODE 200</p>
                <h1>The Agent Network</h1>
                <p class="episode-date">January 1, 2026</p>
            </div>
            <button class="copy-btn" id="copyMarkdownBtn">Copy as markdown</button>
        </div>

        <p>In this episode, we explore 2026 predictions including local AI, swarm AI, and the fundamental shift from individual agents to agent networks. We introduce Reed's Law of group-forming networks—a crucial economic principle showing why networks of autonomous agents can create exponentially greater value than previous network models.</p>

        <div class="tweet-embed">
            <blockquote class="twitter-tweet" data-media-max-width="720"><p lang="en" dir="ltr">Episode 200: The Agent Network<br><br>We predict six major themes for 2026: local &amp; swarm AI, open &gt; closed, agents &gt; models, autopilots, and agent networks.<br><br>We introduce Reed&#39;s Law of group-forming networks, a concept from network economics crucial for understanding agent networks.… <a href="https://t.co/dIatR1rLCU">https://t.co/dIatR1rLCU</a> <a href="https://t.co/gZIXIy8xUQ">pic.twitter.com/gZIXIy8xUQ</a></p>&mdash; OpenAgents (@OpenAgentsInc) <a href="https://twitter.com/OpenAgentsInc/status/2006956979298685216?ref_src=twsrc%5Etfw">January 2, 2026</a></blockquote>
        </div>
        <script async src="https://platform.twitter.com/widgets.js" charset="utf-8"></script>
        <script>
        (function() {{
            function resizeTweetEmbeds() {{
                var wrappers = document.querySelectorAll(".tweet-embed");
                wrappers.forEach(function(wrapper) {{
                    var iframe = wrapper.querySelector("iframe.twitter-tweet");
                    if (!iframe) {{
                        return;
                    }}
                    var baseWidth = parseInt(iframe.getAttribute("width"), 10) || iframe.offsetWidth;
                    var baseHeight = parseInt(iframe.getAttribute("height"), 10) || iframe.offsetHeight;
                    var targetWidth = wrapper.clientWidth;
                    if (!baseWidth || !targetWidth) {{
                        return;
                    }}
                    var scale = targetWidth / baseWidth;
                    if (scale <= 1) {{
                        iframe.style.transform = "none";
                        iframe.style.width = "100%";
                        iframe.style.maxWidth = "100%";
                        wrapper.style.height = "";
                        return;
                    }}
                    iframe.style.transformOrigin = "top left";
                    iframe.style.transform = "scale(" + scale + ")";
                    iframe.style.width = baseWidth + "px";
                    iframe.style.maxWidth = baseWidth + "px";
                    if (baseHeight) {{
                        wrapper.style.height = (baseHeight * scale) + "px";
                    }}
                }});
            }}

            function bindWidgetEvents() {{
                if (window.twttr && window.twttr.events && window.twttr.events.bind) {{
                    window.twttr.events.bind("rendered", resizeTweetEmbeds);
                    return true;
                }}
                return false;
            }}

            window.addEventListener("load", function() {{
                resizeTweetEmbeds();
                bindWidgetEvents();
                window.addEventListener("resize", resizeTweetEmbeds);

                var attempts = 0;
                var timer = setInterval(function() {{
                    attempts += 1;
                    resizeTweetEmbeds();
                    if (bindWidgetEvents() || attempts > 20) {{
                        clearInterval(timer);
                    }}
                }}, 250);
            }});
        }})();
        </script>

        {}

        <hr>
        <p style="color: #666; text-align: center; margin-top: 40px;"><a href="/">OpenAgents</a></p>
    </div>
    <script>
        const rawMarkdown = `{}`;
        const markdownWithMetadata = `---
Title: The Agent Network
URL: https://openagents.com/the-agent-network
Episode: 200
Date: January 1, 2026
Source: OpenAgents
---

` + rawMarkdown;

        document.getElementById('copyMarkdownBtn').addEventListener('click', function() {{
            navigator.clipboard.writeText(markdownWithMetadata).then(() => {{
                const btn = this;
                const originalText = btn.textContent;
                btn.textContent = 'Copied!';
                setTimeout(() => {{
                    btn.textContent = originalText;
                }}, 2000);
            }}).catch(err => {{
                console.error('Failed to copy:', err);
                alert('Failed to copy markdown');
            }});
        }});
    </script>
</body>
</html>"##, body_html, markdown_escaped);

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("X-Frame-Options", "SAMEORIGIN")?;

    Ok(Response::ok(html)?.with_headers(headers))
}
