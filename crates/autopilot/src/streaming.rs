use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::mpsc;

pub enum StreamToken {
    Chunk(String),
    Done,
    Error(String),
}

pub struct HarmonySegment {
    pub channel: String,
    pub content: String,
}

pub fn query_issue_summary(cwd: &Path) -> Option<String> {
    let db_path = cwd.join(".openagents/autopilot.db");
    if !db_path.exists() {
        return None;
    }

    let output = std::process::Command::new("sqlite3")
        .arg(&db_path)
        .arg("SELECT status, COUNT(*) FROM issues GROUP BY status; SELECT '---'; SELECT number, substr(title,1,50), status, priority FROM issues WHERE status != 'done' ORDER BY CASE priority WHEN 'urgent' THEN 1 WHEN 'high' THEN 2 WHEN 'medium' THEN 3 ELSE 4 END LIMIT 10;")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout);

    let mut done = 0;
    let mut in_progress = 0;
    let mut open = 0;
    let mut active_issues = Vec::new();
    let mut in_active = false;

    for line in raw.lines() {
        if line == "---" {
            in_active = true;
            continue;
        }
        if !in_active {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 2 {
                let count: i32 = parts[1].parse().unwrap_or(0);
                match parts[0] {
                    "done" => done = count,
                    "in_progress" => in_progress = count,
                    "open" => open = count,
                    _ => {}
                }
            }
        } else {
            active_issues.push(line.to_string());
        }
    }

    let total = done + in_progress + open;
    if total == 0 {
        return None;
    }

    let mut summary = format!(
        "Issue Status: {} total, {} done ({}%), {} in-progress, {} open\n\nActive issues:\n",
        total,
        done,
        (done * 100) / total,
        in_progress,
        open
    );

    for issue in active_issues {
        summary.push_str(&format!("- {}\n", issue));
    }

    summary.push_str("\nProvide brief analysis: health, top priority, risks.");
    Some(summary)
}

pub fn stream_gpt_oss_analysis(summary: &str, tx: mpsc::Sender<StreamToken>) {
    let request_body = serde_json::json!({
        "model": "gpt-oss-120b-mxfp4.gguf",
        "messages": [
            {"role": "system", "content": "You are a concise project analyst. Give brief insights in 3-4 sentences."},
            {"role": "user", "content": summary}
        ],
        "max_tokens": 300,
        "temperature": 0.3,
        "stream": true
    });

    let body = request_body.to_string();
    let request = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: localhost:8000\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );

    let stream = match TcpStream::connect("localhost:8000") {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(StreamToken::Error(e.to_string()));
            return;
        }
    };

    let mut stream_clone = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(StreamToken::Error(e.to_string()));
            return;
        }
    };

    if let Err(e) = stream_clone.write_all(request.as_bytes()) {
        let _ = tx.send(StreamToken::Error(e.to_string()));
        return;
    }

    let reader = BufReader::new(stream);
    let mut headers_done = false;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if !headers_done {
            if line.is_empty() {
                headers_done = true;
            }
            continue;
        }

        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                let _ = tx.send(StreamToken::Done);
                return;
            }

            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(content) = parsed["choices"][0]["delta"]["content"].as_str() {
                    let _ = tx.send(StreamToken::Chunk(content.to_string()));
                }
            }
        }
    }

    let _ = tx.send(StreamToken::Done);
}

pub fn parse_harmony_stream(text: &str) -> Vec<HarmonySegment> {
    let mut segments = Vec::new();
    let mut current_channel = String::new();
    let mut current_content = String::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if let Some(channel_start) = remaining.find("<|channel|>") {
            if !current_content.is_empty() && !current_channel.is_empty() {
                segments.push(HarmonySegment {
                    channel: current_channel.clone(),
                    content: current_content.trim().to_string(),
                });
                current_content.clear();
            }

            let after_channel = &remaining[channel_start + 11..];

            if let Some(msg_start) = after_channel.find("<|message|>") {
                current_channel = after_channel[..msg_start].to_string();
                remaining = &after_channel[msg_start + 11..];
            } else {
                let end = after_channel.find("<|").unwrap_or(after_channel.len());
                current_channel = after_channel[..end].to_string();
                remaining = &after_channel[end..];
            }
        } else if let Some(end_pos) = remaining.find("<|end|>") {
            current_content.push_str(&remaining[..end_pos]);
            if !current_content.is_empty() {
                segments.push(HarmonySegment {
                    channel: current_channel.clone(),
                    content: current_content.trim().to_string(),
                });
                current_content.clear();
            }
            remaining = &remaining[end_pos + 7..];
        } else if let Some(tag_start) = remaining.find("<|") {
            current_content.push_str(&remaining[..tag_start]);
            if let Some(tag_end) = remaining[tag_start..].find("|>") {
                remaining = &remaining[tag_start + tag_end + 2..];
            } else {
                break;
            }
        } else {
            current_content.push_str(remaining);
            break;
        }
    }

    if !current_content.is_empty() {
        segments.push(HarmonySegment {
            channel: if current_channel.is_empty() {
                "final".to_string()
            } else {
                current_channel
            },
            content: current_content.trim().to_string(),
        });
    }

    segments
}

pub fn extract_final_content(harmony_buffer: &str) -> String {
    let segments = parse_harmony_stream(harmony_buffer);

    for segment in segments.iter().rev() {
        if segment.channel == "final" || segment.channel == "response" {
            return segment.content.clone();
        }
    }

    for segment in segments.iter().rev() {
        if segment.channel != "analysis"
            && segment.channel != "commentary"
            && !segment.content.is_empty()
        {
            return segment.content.clone();
        }
    }

    harmony_buffer.to_string()
}
