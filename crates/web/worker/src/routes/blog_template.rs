//! Shared blog post template
//!
//! All blog posts use this template for consistent styling.

use pulldown_cmark::{Parser, html};

/// Blog post metadata for rendering the page
pub struct BlogMetadata {
    pub title: &'static str,
    pub slug: &'static str,
    pub date: &'static str,
    pub description: &'static str,
}

/// Render a blog post page
///
/// Takes blog metadata and markdown content.
/// Returns the complete HTML page as a string.
pub fn render_blog_page(meta: &BlogMetadata, markdown_content: &str) -> String {
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
    <meta name="description" content="{description}">

    <!-- Open Graph -->
    <meta property="og:type" content="article">
    <meta property="og:title" content="{title}">
    <meta property="og:description" content="{description}">
    <meta property="og:url" content="https://openagents.com/blog/{slug}">
    <meta property="og:site_name" content="OpenAgents">

    <!-- Twitter Card -->
    <meta name="twitter:card" content="summary_large_image">
    <meta name="twitter:site" content="@OpenAgentsInc">
    <meta name="twitter:title" content="{title}">
    <meta name="twitter:description" content="{description}">
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
        h3 {{
            font-size: 16px;
            margin-top: 24px;
            margin-bottom: 12px;
            color: #ddd;
        }}
        .header-row {{
            display: flex;
            justify-content: space-between;
            align-items: flex-start;
            margin-bottom: 40px;
            gap: 24px;
        }}
        .post-label {{
            color: #f80;
            font-size: 12px;
            margin: 0 0 4px 0;
            letter-spacing: 0.05em;
        }}
        h1 {{
            margin: 0 0 8px 0;
        }}
        .post-date {{
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
        pre code {{
            background: none;
            padding: 0;
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
        table {{
            border-collapse: collapse;
            margin: 16px 0;
            width: 100%;
        }}
        th, td {{
            border: 1px solid #333;
            padding: 8px 12px;
            text-align: left;
        }}
        th {{
            background: #111;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header-row">
            <div>
                <p class="post-label">BLOG POST</p>
                <h1>{title}</h1>
                <p class="post-date">{date}</p>
            </div>
            <button class="copy-btn" id="copyMarkdownBtn"><span class="short-text">Copy</span><span class="full-text">Copy as markdown</span></button>
        </div>

        {body_html}

        <hr>
        <p style="color: #666; text-align: center; margin-top: 40px;"><a href="/">OpenAgents</a></p>
    </div>
    <script>
        const rawMarkdown = `{markdown_escaped}`;
        const markdownWithMetadata = `---
Title: {title}
URL: https://openagents.com/blog/{slug}
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
        slug = meta.slug,
        date = meta.date,
        description = meta.description,
        body_html = body_html,
        markdown_escaped = markdown_escaped,
    )
}
