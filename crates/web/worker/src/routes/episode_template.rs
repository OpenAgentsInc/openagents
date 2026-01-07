//! Shared episode transcript template
//!
//! All episode transcript pages use this template for consistent styling
//! and Twitter embed handling.

use pulldown_cmark::{Parser, html};

/// Episode metadata for rendering the page
pub struct EpisodeMetadata {
    pub number: u32,
    pub title: &'static str,
    pub slug: &'static str,
    pub date: &'static str,
    pub description: &'static str,
    pub tweet_embed: &'static str,
}

/// Render an episode transcript page
///
/// Takes episode metadata, markdown content, and the raw tweet blockquote HTML.
/// Returns the complete HTML page as a string.
pub fn render_episode_page(meta: &EpisodeMetadata, markdown_content: &str) -> String {
    // Parse markdown to HTML
    let parser = Parser::new(markdown_content);
    let mut body_html = String::new();
    html::push_html(&mut body_html, parser);

    // Escape backticks in markdown for JavaScript string literal
    let markdown_escaped = markdown_content.replace('`', "\\`");

    format!(r##"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{title} - OpenAgents</title>
    <meta name="description" content="Episode {number}: {title} - {description}">

    <!-- Open Graph -->
    <meta property="og:type" content="article">
    <meta property="og:title" content="{title} - Episode {number}">
    <meta property="og:description" content="{description}">
    <meta property="og:url" content="https://openagents.com/{slug}">
    <meta property="og:image" content="https://openagents.com/og/{slug}.png">
    <meta property="og:site_name" content="OpenAgents">

    <!-- Twitter Card -->
    <meta name="twitter:card" content="summary_large_image">
    <meta name="twitter:site" content="@OpenAgentsInc">
    <meta name="twitter:title" content="{title} - Episode {number}">
    <meta name="twitter:description" content="{description}">
    <meta name="twitter:image" content="https://openagents.com/og/{slug}.png">
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
            white-space: nowrap;
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
                <p class="episode-label">EPISODE {number}</p>
                <h1>{title}</h1>
                <p class="episode-date">{date}</p>
            </div>
            <button class="copy-btn" id="copyMarkdownBtn"><span class="short-text">Copy</span><span class="full-text">Copy as markdown</span></button>
        </div>

        <div class="tweet-embed">
            {tweet_embed}
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

        {body_html}

        <hr>
        <p style="color: #666; text-align: center; margin-top: 40px;"><a href="/">OpenAgents</a></p>
    </div>
    <script>
        const rawMarkdown = `{markdown_escaped}`;
        const markdownWithMetadata = `---
Title: {title}
URL: https://openagents.com/{slug}
Episode: {number}
Date: {date}
Source: OpenAgents
---

` + rawMarkdown;

        document.getElementById('copyMarkdownBtn').addEventListener('click', function() {{
            navigator.clipboard.writeText(markdownWithMetadata).then(() => {{
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
                alert('Failed to copy markdown');
            }});
        }});
    </script>
</body>
</html>"##,
        title = meta.title,
        number = meta.number,
        slug = meta.slug,
        date = meta.date,
        description = meta.description,
        tweet_embed = meta.tweet_embed,
        body_html = body_html,
        markdown_escaped = markdown_escaped,
    )
}
