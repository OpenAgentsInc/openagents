/// Render diff lines with proper formatting
fn render_diff_lines(diff_content: &str) -> Markup {
    let lines: Vec<&str> = diff_content.lines().collect();
    let mut html_output = String::new();
    let mut in_file = false;
    let mut old_line_num = 0;
    let mut new_line_num = 0;

    for line in lines {
        if line.starts_with("diff --git") {
            if in_file {
                html_output.push_str("</div>");
            }
            if let Some(filename) = extract_filename(line) {
                html_output.push_str(&format!(
                    r#"<div class="diff-file"><div class="diff-file-header">File: {}</div>"#,
                    filename
                ));
                in_file = true;
            }
        } else if line.starts_with("@@") {
            if let Some((old_start, new_start)) = parse_hunk_header(line) {
                old_line_num = old_start;
                new_line_num = new_start;
            }
            html_output.push_str(&format!(
                r#"<div class="diff-hunk-header">{}</div>"#,
                html_escape(line)
            ));
        } else if line.starts_with("+") && !line.starts_with("+++") {
            html_output.push_str(&format!(
                r#"<div class="diff-line diff-added"><span class="diff-line-number"></span><span class="diff-line-number">{}</span><span class="diff-line-content">{}</span></div>"#,
                new_line_num,
                html_escape(line)
            ));
            new_line_num += 1;
        } else if line.starts_with("-") && !line.starts_with("---") {
            html_output.push_str(&format!(
                r#"<div class="diff-line diff-removed"><span class="diff-line-number">{}</span><span class="diff-line-number"></span><span class="diff-line-content">{}</span></div>"#,
                old_line_num,
                html_escape(line)
            ));
            old_line_num += 1;
        } else if !line.starts_with("\\")
            && !line.starts_with("index ")
            && !line.starts_with("---")
            && !line.starts_with("+++")
            && !line.is_empty()
        {
            html_output.push_str(&format!(
                r#"<div class="diff-line diff-context"><span class="diff-line-number">{}</span><span class="diff-line-number">{}</span><span class="diff-line-content">{}</span></div>"#,
                old_line_num,
                new_line_num,
                html_escape(line)
            ));
            old_line_num += 1;
            new_line_num += 1;
        }
    }

    if in_file {
        html_output.push_str("</div>");
    }

    maud::PreEscaped(html_output)
}

/// HTML escape special characters
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Extract filename from git diff header
fn extract_filename(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 4 {
        let filename = parts[2].trim_start_matches("a/");
        Some(filename.to_string())
    } else {
        None
    }
}

/// Parse hunk header to extract line numbers
fn parse_hunk_header(line: &str) -> Option<(i32, i32)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 {
        let old_part = parts[1].trim_start_matches('-');
        let new_part = parts[2].trim_start_matches('+');

        let old_start = old_part.split(',').next()?.parse::<i32>().ok()?;
        let new_start = new_part.split(',').next()?.parse::<i32>().ok()?;

        Some((old_start, new_start))
    } else {
        None
    }
}

