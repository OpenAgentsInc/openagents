/// Diff viewer page with syntax highlighting
pub fn diff_viewer_page(
    identifier: &str,
    item_id: &str,
    item_type: &str,
    diff_content: &str,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Diff - " (item_id) " - GitAfter" }
                style {
                    (include_str!("../styles.css"))
                    "
                    .diff-viewer {
                        font-family: 'Courier New', monospace;
                        font-size: 0.9rem;
                        background: #1a1a1a;
                        padding: 1rem;
                        overflow-x: auto;
                    }
                    .diff-file {
                        margin-bottom: 2rem;
                        border: 1px solid #333;
                    }
                    .diff-file-header {
                        background: #2a2a2a;
                        padding: 0.5rem 1rem;
                        font-weight: bold;
                        color: #fff;
                        border-bottom: 1px solid #333;
                    }
                    .diff-line {
                        display: flex;
                        padding: 0.2rem 0;
                    }
                    .diff-line-number {
                        width: 4rem;
                        text-align: right;
                        padding-right: 1rem;
                        color: #666;
                        user-select: none;
                    }
                    .diff-line-content {
                        flex: 1;
                        white-space: pre;
                    }
                    .diff-added {
                        background: #1a3d1a;
                        color: #66ff66;
                    }
                    .diff-removed {
                        background: #3d1a1a;
                        color: #ff6666;
                    }
                    .diff-context {
                        color: #ccc;
                    }
                    .diff-hunk-header {
                        background: #2a3a4a;
                        color: #88ccff;
                        padding: 0.3rem 1rem;
                        font-weight: bold;
                    }
                    "
                }
            }
            body {
                header {
                    h1 { "GitAfter" }
                    nav {
                        a href="/" { "Home" }
                        " | "
                        a href={"/repo/" (identifier)} { "Repository" }
                        " | "
                        @if item_type == "pr" {
                            a href={"/repo/" (identifier) "/pulls/" (item_id)} { "Pull Request" }
                        } @else {
                            a href={"/repo/" (identifier) "/patches/" (item_id)} { "Patch" }
                        }
                        " | "
                        span { "Diff" }
                    }
                }

                main {
                    h2 { "Diff View" }

                    div.diff-viewer {
                        (render_diff_lines(diff_content))
                    }

                    div.actions style="margin-top: 2rem;" {
                        @if item_type == "pr" {
                            a.button href={"/repo/" (identifier) "/pulls/" (item_id)} { "← Back to PR" }
                        } @else {
                            a.button href={"/repo/" (identifier) "/patches/" (item_id)} { "← Back to Patch" }
                        }
                    }
                }

                footer {
                    p { "GitAfter - Nostr-native GitHub alternative" }
                }
            }
        }
    }
}

