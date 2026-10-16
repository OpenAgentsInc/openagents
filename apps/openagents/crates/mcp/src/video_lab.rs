//! OpenAgents Video Lab tools (`lab_*`) for every agent Coder runs.
//!
//! Video Lab is the OpenAgents service that keeps films, shots and scenes as
//! typed documents with versions, checks every edit, and renders previews
//! (`scripts/film/lab.py` in the openagents repository, `127.0.0.1:8792`).
//! Its tools are declared by the Lab itself (`GET /api/tools`) and run by it
//! (`POST /api/tool/{name}`), so this server offers exactly the tools the
//! running Lab has, with the Lab's own descriptions, and adds nothing of its
//! own except starting the Lab when it is installed but not running.
//!
//! Results keep the Lab's `card` (`"card": "video_lab"`): the desktop draws
//! it in the transcript with the poster, the render state and a button that
//! opens the Lab player in a Browser tab.
//!
//! The tools are offered only where a Lab is: `VIDEO_LAB_URL` is set, a Lab
//! answers on the default address, or the Lab script is on this computer
//! (`OPENAGENTS_VIDEO_LAB_SCRIPT`, else `~/work/openagents/scripts/film/lab.py`).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use serde_json::{Value, json};

use crate::tools::ToolDef;

const DEFAULT_URL: &str = "http://127.0.0.1:8792";
/// One tool call may render a preview or read a clip's speech.
const CALL_TIMEOUT: Duration = Duration::from_secs(180);

static TOOLS: OnceLock<Vec<ToolDef>> = OnceLock::new();

/// The Lab's address: `VIDEO_LAB_URL`, else the default loopback port.
pub fn url() -> String {
    std::env::var("VIDEO_LAB_URL")
        .ok()
        .map(|u| u.trim().trim_end_matches('/').to_owned())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_owned())
}

/// The Lab script on this computer, if there is one.
pub fn script() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    [
        std::env::var_os("OPENAGENTS_VIDEO_LAB_SCRIPT").map(PathBuf::from),
        home.map(|h| h.join("work/openagents/scripts/film/lab.py")),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.is_file())
}

fn client(timeout: Duration) -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(timeout)
        .no_proxy()
        .build()
        .ok()
}

async fn up(http: &reqwest::Client, base: &str) -> bool {
    http.get(format!("{base}/healthz"))
        .timeout(Duration::from_secs(1))
        .send()
        .await
        .is_ok_and(|r| r.status().is_success())
}

/// Make sure the Lab answers, starting it from its script when needed.
pub async fn ensure(http: &reqwest::Client, base: &str) -> Result<(), String> {
    if up(http, base).await {
        return Ok(());
    }
    let Some(script) = script() else {
        return Err(format!(
            "Video Lab isn't running at {base}, and its script isn't on this computer."
        ));
    };
    let log_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("work/film-out/lab");
    std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    let log = std::fs::File::create(log_dir.join("serve.log")).map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    std::process::Command::new("python3")
        .arg(&script)
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(err)
        .spawn()
        .map_err(|e| format!("Video Lab couldn't start ({e})"))?;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        if up(http, base).await {
            return Ok(());
        }
    }
    Err(format!(
        "Video Lab didn't start; see {}",
        log_dir.join("serve.log").display()
    ))
}

/// Read the running Lab's tool declarations once per server. A computer
/// without a Lab offers none.
pub async fn prime() {
    if TOOLS.get().is_some() {
        return;
    }
    let base = url();
    let configured = std::env::var("VIDEO_LAB_URL").is_ok_and(|u| !u.trim().is_empty());
    let tools = match client(Duration::from_secs(5)) {
        Some(http) if configured || script().is_some() || up(&http, &base).await => {
            match ensure(&http, &base).await {
                Ok(()) => fetch(&http, &base).await.unwrap_or_default(),
                Err(error) => {
                    tracing::warn!(%error, "video lab unavailable");
                    Vec::new()
                }
            }
        }
        _ => Vec::new(),
    };
    let _ = TOOLS.set(tools);
}

async fn fetch(http: &reqwest::Client, base: &str) -> Option<Vec<ToolDef>> {
    let body: Value = http
        .get(format!("{base}/api/tools"))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    Some(declarations(&body))
}

/// The Lab's `/api/tools` answer as MCP tool definitions. Names and
/// descriptions are kept for the life of the process (a few dozen strings).
pub fn declarations(body: &Value) -> Vec<ToolDef> {
    let list = body
        .get("tools")
        .and_then(Value::as_array)
        .or_else(|| body.as_array());
    list.into_iter()
        .flatten()
        .filter_map(|tool| {
            let tool = tool.get("function").unwrap_or(tool);
            let name = tool.get("name")?.as_str()?;
            if !is_lab_name(name) {
                return None;
            }
            let description = tool
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let schema = tool
                .get("parameters")
                .or_else(|| tool.get("input_schema"))
                .cloned()
                .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
            Some(ToolDef {
                name: Box::leak(name.to_owned().into_boxed_str()),
                description: Box::leak(
                    format!("Video Lab: {description} Results carry the Lab player link; Coder shows the film in a card with that link.")
                        .into_boxed_str(),
                ),
                input_schema: schema,
            })
        })
        .collect()
}

fn is_lab_name(name: &str) -> bool {
    name.starts_with("lab_")
        && name.len() > 4
        && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
}

/// The Lab tools this server offers.
pub fn tools() -> &'static [ToolDef] {
    TOOLS.get().map(Vec::as_slice).unwrap_or(&[])
}

pub fn has(name: &str) -> bool {
    tools().iter().any(|tool| tool.name == name)
}

/// Run one Lab tool. A refusal is a result (the model reads its reason and
/// fixes the edit), not an error.
pub async fn call(name: &str, arguments: Value) -> Result<Value, String> {
    let base = url();
    let http = client(CALL_TIMEOUT).ok_or("Video Lab client couldn't start")?;
    ensure(&http, &base).await?;
    let mut result: Value = http
        .post(format!("{base}/api/tool/{name}"))
        .json(&arguments)
        .send()
        .await
        .map_err(|e| format!("Video Lab didn't answer ({})", e.without_url()))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if let Some(error) = result.get("error").and_then(Value::as_str)
        && result.get("refused").is_none()
    {
        return Err(error.to_owned());
    }
    absolute(&mut result, &base);
    Ok(result)
}

/// Make the card's media addresses absolute, so the desktop can fetch them
/// without knowing where the Lab runs.
fn absolute(result: &mut Value, base: &str) {
    let Some(card) = result.get_mut("card").and_then(Value::as_object_mut) else {
        return;
    };
    card.entry("lab").or_insert_with(|| json!(base));
    for key in ["video", "poster", "image"] {
        if let Some(path) = card.get(key).and_then(Value::as_str)
            && path.starts_with('/')
        {
            let full = format!("{base}{path}");
            card.insert(key.into(), json!(full));
        }
    }
    if let Some(compare) = card.get_mut("compare").and_then(Value::as_object_mut) {
        for side in ["a", "b"] {
            if let Some(video) = compare
                .get_mut(side)
                .and_then(|s| s.get_mut("video"))
                .filter(|v| v.as_str().is_some_and(|p| p.starts_with('/')))
            {
                *video = json!(format!("{base}{}", video.as_str().unwrap_or_default()));
            }
        }
    }
    if let Some(strip) = card.get_mut("strip").and_then(Value::as_array_mut) {
        for item in strip.iter_mut() {
            if let Some(path) = item.as_str()
                && path.starts_with('/')
            {
                *item = json!(format!("{base}{path}"));
            }
        }
    }
}

/// The card Coder shows for a Lab result: a native visual (Luau script and
/// its picture) with the document, version, render state, the picture the
/// Lab made (a frame, a strip, a side-by-side compare, or the poster) and
/// links that open the render and the Lab player in Coder's browser.
pub struct CardVisual {
    pub title: String,
    pub script: String,
    /// The picture as a file for the visual's assets, when there is one.
    pub picture: Option<(String, PathBuf)>,
}

/// Build the card's visual. The picture is downloaded from the Lab into a
/// temporary file; a card without one still shows its text and links.
pub async fn card_visual(card: &Value) -> Option<CardVisual> {
    let card = card.as_object()?;
    if card.get("card").and_then(Value::as_str) != Some("video_lab") {
        return None;
    }
    let text = |key: &str| card.get(key).and_then(Value::as_str).unwrap_or_default();
    let title = if text("title").is_empty() {
        text("doc")
    } else {
        text("title")
    }
    .to_owned();
    let picture_url = ["image", "poster"]
        .into_iter()
        .find_map(|key| {
            card.get(key)
                .and_then(Value::as_str)
                .filter(|u| u.starts_with("http"))
        })
        .map(str::to_owned);
    let picture = match picture_url {
        Some(url) => download(&url).await,
        None => None,
    };
    let aspect = picture
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|bytes| dimensions(&bytes))
        .map(|(w, h)| w as f64 / h.max(1) as f64)
        .unwrap_or(16.0 / 9.0);
    let picture = picture.map(|path| {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg")
            .to_owned();
        (format!("picture.{ext}"), path)
    });
    let script = card_script(
        card,
        &title,
        picture.as_ref().map(|(name, _)| name.as_str()),
        aspect,
    );
    Some(CardVisual {
        title,
        script,
        picture,
    })
}

async fn download(url: &str) -> Option<PathBuf> {
    let http = client(Duration::from_secs(20))?;
    let response = http.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.len() > 7 * 1024 * 1024 {
        return None;
    }
    let ext = match dimensions_kind(&bytes)? {
        Kind::Png => "png",
        Kind::Jpeg => "jpg",
    };
    let dir = std::env::temp_dir().join("coder-video-lab");
    std::fs::create_dir_all(&dir).ok()?;
    let name = format!(
        "{:016x}.{ext}",
        fnv(url.as_bytes()) ^ fnv(&bytes[..bytes.len().min(4096)])
    );
    let path = dir.join(name);
    std::fs::write(&path, &bytes).ok()?;
    Some(path)
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

enum Kind {
    Png,
    Jpeg,
}

fn dimensions_kind(bytes: &[u8]) -> Option<Kind> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Kind::Png)
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        Some(Kind::Jpeg)
    } else {
        None
    }
}

/// Width and height from a PNG or JPEG header.
fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    match dimensions_kind(bytes)? {
        Kind::Png => {
            let w = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
            let h = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
            Some((w, h))
        }
        Kind::Jpeg => {
            let mut i = 2;
            while i + 9 < bytes.len() {
                if bytes[i] != 0xff {
                    i += 1;
                    continue;
                }
                let marker = bytes[i + 1];
                let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
                if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
                    let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
                    let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
                    return Some((w, h));
                }
                i += 2 + len;
            }
            None
        }
    }
}

/// A Luau string literal.
fn lua(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\{}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0);
    format!("{}:{:04.1}", (s / 60.0).floor() as u64, s % 60.0)
}

/// The visual's Luau script.
fn card_script(
    card: &serde_json::Map<String, Value>,
    title: &str,
    picture: Option<&str>,
    aspect: f64,
) -> String {
    let text = |key: &str| card.get(key).and_then(Value::as_str).unwrap_or_default();
    let version = card.get("version").and_then(Value::as_u64);
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "ui.text {{ {}, size = \"lg\", weight = 600 }}",
        lua(title)
    ));
    let mut meta = String::new();
    if let Some(v) = version {
        meta.push_str(&format!("v{v}"));
    }
    if !text("summary").is_empty() {
        if !meta.is_empty() {
            meta.push_str(" · ");
        }
        meta.push_str(text("summary"));
    }
    if !meta.is_empty() {
        lines.push(format!(
            "ui.text {{ {}, size = \"sm\", color = \"muted\" }}",
            lua(&meta)
        ));
    }
    if let Some(compare) = card.get("compare").and_then(Value::as_object) {
        let side = |s: &str| {
            let v = &compare[s];
            format!(
                "v{} {}",
                v["version"].as_u64().unwrap_or_default(),
                v["summary"].as_str().unwrap_or_default()
            )
        };
        let at = compare
            .get("at")
            .and_then(Value::as_f64)
            .unwrap_or_default();
        lines.push(format!(
            "ui.text {{ {}, size = \"sm\" }}",
            lua(&format!(
                "Compare at {}: {} | {}",
                clock(at),
                side("a"),
                side("b")
            ))
        ));
    }
    if let Some(picture) = picture {
        // The reply column is about 680 px wide; a wide strip stays short.
        let height = (680.0 / aspect.max(0.1)).clamp(90.0, 380.0).round() as u32;
        lines.push(format!(
            "ui.shader {{ height = {height}, blend = \"alpha\", textures = {{ {} }}, inputs = {{ aspect = {aspect:.4} }}, code = [[\n\
void mainImage(out vec4 fragColor, in vec2 fragCoord) {{\n\
    vec2 r = iResolution.xy;\n\
    vec2 size = r.x / r.y > aspect ? vec2(r.y * aspect, r.y) : vec2(r.x, r.x / aspect);\n\
    vec2 origin = (r - size) * 0.5;\n\
    vec2 uv = (fragCoord - origin) / size;\n\
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {{ fragColor = vec4(0.0); return; }}\n\
    fragColor = vec4(texture(iTexture0, uv).rgb, 1.0);\n\
}}\n\
]] }}",
            lua(picture)
        ));
    }
    let job = card.get("job").filter(|j| !j.is_null());
    let video = text("video");
    let state = match job.and_then(|j| j["state"].as_str()) {
        _ if !video.is_empty() => format!("{} ready", capital(text("quality"))),
        Some("failed") => format!(
            "The {} render failed: {}",
            text("quality"),
            job.and_then(|j| j["error"].as_str())
                .unwrap_or("see the Lab log")
        ),
        Some(state) => {
            let estimate = job.and_then(|j| j["estimate"].as_f64()).unwrap_or_default();
            let state = if state == "queued" {
                "Waiting to render"
            } else {
                "Rendering"
            };
            if estimate > 0.0 {
                format!(
                    "{state} the {} (about {} min). The player shows it when it's ready.",
                    text("quality"),
                    (estimate / 60.0).ceil() as u64
                )
            } else {
                format!(
                    "{state} the {}. The player shows it when it's ready.",
                    text("quality")
                )
            }
        }
        None => String::new(),
    };
    if !state.is_empty() {
        lines.push(format!(
            "ui.text {{ {}, size = \"sm\", color = \"muted\" }}",
            lua(&state)
        ));
    }
    if let Some(private) = card
        .get("private")
        .and_then(Value::as_array)
        .filter(|p| !p.is_empty())
    {
        let reasons: Vec<&str> = private.iter().filter_map(Value::as_str).collect();
        lines.push(format!(
            "ui.text {{ {}, size = \"xs\", color = \"muted\" }}",
            lua(&format!(
                "Stays on this Mac, not for training: {}",
                reasons.join("; ")
            ))
        ));
    }
    let mut links = Vec::new();
    if !video.is_empty() {
        links.push(format!(
            "ui.link {{ label = \"Play the {}\", url = {} }}",
            text("quality"),
            lua(video)
        ));
    }
    if let Some(compare) = card.get("compare").and_then(Value::as_object) {
        for side in ["a", "b"] {
            if let (Some(url), Some(v)) = (
                compare[side]["video"].as_str(),
                compare[side]["version"].as_u64(),
            ) {
                links.push(format!(
                    "ui.link {{ label = \"Play v{v}\", url = {} }}",
                    lua(url)
                ));
            }
        }
    }
    if !text("player").is_empty() {
        links.push(format!(
            "ui.link {{ label = \"Open in the Lab player and timeline\", url = {} }}",
            lua(text("player"))
        ));
    }
    if !links.is_empty() {
        lines.push(format!(
            "ui.row {{ gap = 16, wrap = true, {} }}",
            links.join(", ")
        ));
    }
    // Coder Desktop reads this line and draws the card natively (live render
    // state, inline video); other clients run the script below it.
    let mut marker = serde_json::Value::Object(card.clone());
    if let Some(object) = marker.as_object_mut() {
        object.remove("versions");
    }
    format!(
        "-- coder:video_lab {}\nreturn function()\n  return ui.column {{ gap = 8,\n    {}\n  }}\nend\n",
        serde_json::to_string(&marker).unwrap_or_default(),
        lines.join(",\n    ")
    )
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "Render".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_keep_the_labs_own_tools_only() {
        let body = json!({"tools": [
            {"name": "lab_open", "description": "Open a document.", "parameters": {"type": "object", "properties": {"document": {"type": "string"}}, "required": ["document"]}},
            {"name": "game_open", "description": "Not ours."},
            {"name": "lab_Bad", "description": "Bad name."}
        ]});
        let tools = declarations(&body);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "lab_open");
        assert_eq!(tools[0].input_schema["required"][0], "document");
        assert!(
            tools[0]
                .description
                .starts_with("Video Lab: Open a document.")
        );
    }

    #[test]
    fn card_media_become_absolute() {
        let mut result = json!({"card": {"card": "video_lab", "video": "/media/a.mp4", "poster": "/media/a.jpg", "image": null, "strip": ["/media/s1.jpg"]}});
        absolute(&mut result, "http://127.0.0.1:8792");
        assert_eq!(result["card"]["video"], "http://127.0.0.1:8792/media/a.mp4");
        assert_eq!(
            result["card"]["poster"],
            "http://127.0.0.1:8792/media/a.jpg"
        );
        assert_eq!(
            result["card"]["strip"][0],
            "http://127.0.0.1:8792/media/s1.jpg"
        );
        assert_eq!(result["card"]["lab"], "http://127.0.0.1:8792");
        assert!(result["card"]["image"].is_null());
    }

    #[test]
    fn the_card_script_passes_the_visual_check() {
        let card = json!({"card": "video_lab", "doc": "episode-290", "title": "Episode 290 \"take\"\n2",
            "version": 3, "summary": "Delegation plays through", "quality": "preview",
            "video": "http://127.0.0.1:8792/media/a.mp4", "private": ["uses Berkeley Mono"],
            "player": "http://127.0.0.1:8792/player?doc=episode-290&v=3",
            "compare": {"at": 86.0, "a": {"version": 2, "summary": "Take 2", "video": "http://127.0.0.1:8792/media/b.mp4"},
                        "b": {"version": 3, "summary": "Plays through", "video": null}}});
        let plain = card_script(card.as_object().unwrap(), "Episode 290", None, 1.7778);
        zeron_visual::check(&plain, 1).unwrap_or_else(|e| panic!("{e}\n{plain}"));
        let with = card_script(
            card.as_object().unwrap(),
            "Episode 290",
            Some("picture.png"),
            1.7778,
        );
        let files = std::collections::HashMap::from([(
            "picture.png".to_owned(),
            std::sync::Arc::<[u8]>::from(tiny_png()),
        )]);
        zeron_visual::check_with_assets(&with, 1, zeron_visual::Assets::new(files))
            .unwrap_or_else(|e| panic!("{e}\n{with}"));
        let rendering = json!({"card": "video_lab", "doc": "d", "version": 1, "quality": "preview",
            "job": {"state": "running", "estimate": 166.0}, "player": "http://x/player"});
        let s = card_script(rendering.as_object().unwrap(), "d", None, 1.0);
        assert!(s.contains("Rendering the preview (about 3 min)"), "{s}");
        let marker = s
            .lines()
            .next()
            .unwrap()
            .strip_prefix("-- coder:video_lab ")
            .unwrap();
        let card: Value = serde_json::from_str(marker).unwrap();
        assert_eq!(card["card"], "video_lab");
        assert!(card.get("versions").is_none());
    }

    fn tiny_png() -> Vec<u8> {
        let mut png = Vec::new();
        image::RgbImage::from_raw(2, 1, vec![255, 0, 0, 0, 0, 255])
            .unwrap()
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        png
    }

    #[test]
    fn picture_sizes_come_from_their_headers() {
        let png = tiny_png();
        assert_eq!(dimensions(&png), Some((2, 1)));
    }
}
