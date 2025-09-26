use anyhow::{anyhow, Result};
use codex_core::figma_tools::get_figma_token;
use regex_lite::Regex;
use serde_json::{json, Value};
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};

fn default_codex_home() -> PathBuf {
    if let Ok(val) = std::env::var("CODEX_HOME") { if !val.is_empty() { return PathBuf::from(val); } }
    dirs::home_dir().map(|mut h| { h.push(".codex"); h }).expect("home dir")
}

fn extract_file_key(url: &str) -> Option<String> {
    let re_file = Regex::new(r"^https://(?:www\.)?figma\.com/(?:file|proto|design)/([A-Za-z0-9]+)").ok()?;
    re_file.captures(url).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 {
        eprintln!("Usage: figma_dump_all <figma_url> <root_node_id> [outdir] [sleep_ms]" );
        std::process::exit(2);
    }
    let figma_url = args.remove(0);
    let root_id = args.remove(0);
    let file_key = extract_file_key(&figma_url).ok_or_else(|| anyhow!("invalid figma url"))?;
    let outdir = if !args.is_empty() { PathBuf::from(args.remove(0)) } else {
        let mut p = default_codex_home(); p.push("figma_dumps"); p.push(format!("{}_{}", file_key, root_id.replace(":","_"))); p
    };
    let sleep_ms: u64 = args.get(0).and_then(|s| s.parse().ok()).unwrap_or(200);

    let base = std::env::current_dir().ok();
    let token = get_figma_token(base.as_deref())?;
    fs::create_dir_all(&outdir)?;

    let client = reqwest::Client::builder().timeout(Duration::from_secs(15)).build()?;

    // helper: GET JSON with retries/backoff
    async fn get_json(client: &reqwest::Client, token: &str, url: &str, attempts: u32) -> Result<Value> {
        let mut try_no = 0u32;
        loop {
            let resp = client
                .get(url)
                .header("X-Figma-Token", token.to_string())
                .header("Accept", "application/json")
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    let v: Value = r.json().await?;
                    return Ok(v);
                }
                Ok(r) => {
                    if try_no + 1 >= attempts { return Err(anyhow!(format!("http {} {}", r.status(), url))); }
                }
                Err(e) => {
                    if try_no + 1 >= attempts { return Err(anyhow!(format!("req err {e}: {url}"))); }
                }
            }
            try_no += 1;
            let back = (200u64 * try_no as u64).min(2000);
            sleep(Duration::from_millis(back)).await;
        }
    }

    async fn download_bytes(client: &reqwest::Client, url: &str, attempts: u32) -> Result<bytes::Bytes> {
        let mut try_no = 0u32;
        loop {
            let resp = client.get(url).send().await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    return Ok(r.bytes().await?);
                }
                Ok(r) => {
                    if try_no + 1 >= attempts { return Err(anyhow!(format!("http {} {}", r.status(), url))); }
                }
                Err(e) => {
                    if try_no + 1 >= attempts { return Err(anyhow!(format!("req err {e}: {url}"))); }
                }
            }
            try_no += 1;
            let back = (250u64 * try_no as u64).min(2500);
            sleep(Duration::from_millis(back)).await;
        }
    }

    // Step 1: fetch root children (depth=1)
    let root_url = format!("https://api.figma.com/v1/files/{}/nodes?ids={}&depth=1", file_key, root_id.replace(":","%3A"));
    let body: Value = get_json(&client, &token, &root_url, 5).await?;
    let children = body["nodes"][&root_id]["document"]["children"].as_array().cloned().unwrap_or_default();
    let top_list: Vec<Value> = children.iter().map(|c| json!({
        "id": c.get("id").and_then(|v| v.as_str()).unwrap_or(""),
        "name": c.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        "type": c.get("type").and_then(|v| v.as_str()).unwrap_or(""),
        "parent_id": root_id,
        "depth": 1
    })).collect();
    fs::write(outdir.join("nodes_depth1.json"), serde_json::to_string_pretty(&top_list)?)?;

    // Index
    let index = json!({
        "file_key": file_key,
        "root_node_id": root_id,
        "count_depth1": top_list.len(),
        "generated_at": chrono::Utc::now().to_rfc3339(),
    });
    fs::write(outdir.join("index.json"), serde_json::to_string_pretty(&index)?)?;

    // Step 2: traverse breadth-first over all descendants, writing per-parent children files
    let perdir = outdir.join("nodes_by_parent");
    fs::create_dir_all(&perdir)?;
    let rawdir = outdir.join("nodes_raw");
    fs::create_dir_all(&rawdir)?;
    let mut queue: std::collections::VecDeque<(String,u32)> = std::collections::VecDeque::new();
    for node in &top_list {
        if let Some(pid) = node["id"].as_str() { queue.push_back((pid.to_string(), 2)); }
    }
    let mut processed = 0usize;
    let index_path = outdir.join("nodes_index.ndjson");
    let mut index_file = fs::OpenOptions::new().create(true).append(true).open(&index_path)?;
    // Seed index with depth1 entries
    for n in &top_list { writeln!(&mut index_file, "{}", serde_json::to_string(n)?)?; }
    while let Some((pid, depth)) = queue.pop_front() {
        let path = perdir.join(format!("{}.json", pid.replace(":","_")));
        if path.exists() { continue; }
        let url = format!("https://api.figma.com/v1/files/{}/nodes?ids={}&depth=1", file_key, pid.replace(":","%3A"));
        let b: Value = match get_json(&client, &token, &url, 4).await { Ok(v) => v, Err(e) => { eprintln!("warn: {} -> {}", pid, e); continue; } };
        let doc = b["nodes"][&pid]["document"].clone();
        // Save raw document for richer offline traversal
        fs::write(rawdir.join(format!("{}.json", pid.replace(":","_"))), serde_json::to_string_pretty(&doc)?)?;
        let kids = doc["children"].as_array().cloned().unwrap_or_default();
        let mut outkids: Vec<Value> = Vec::with_capacity(kids.len());
        for c in kids {
            let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let typ = c.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let rec = json!({ "id": id, "name": name, "type": typ, "parent_id": pid, "depth": depth });
            outkids.push(rec.clone());
            // Append to global index
            writeln!(&mut index_file, "{}", serde_json::to_string(&rec)?)?;
            // Queue next level
            if !id.is_empty() { queue.push_back((id, depth + 1)); }
        }
        fs::write(&path, serde_json::to_string_pretty(&outkids)?)?;
        processed += 1;
        if (processed % 25) == 0 { eprintln!("progress: processed {} parents", processed); }
        sleep(Duration::from_millis(sleep_ms)).await;
        // Soft cap to prevent runaway duration in a single invocation
        if processed >= 1500 { eprintln!("soft cap reached ({} parents). You can re-run to continue.", processed); break; }
    }

    // Step 3: export images for frames/components/instances
    let export_images = true; // always on for now
    if export_images {
        let allowed = ["FRAME", "COMPONENT", "INSTANCE"]; // omit CANVAS by default
        let mut ids: Vec<String> = Vec::new();
        // include depth1
        for n in &top_list {
            let ty = n["type"].as_str().unwrap_or("");
            if allowed.contains(&ty) {
                if let Some(id) = n["id"].as_str() { ids.push(id.to_string()); }
            }
        }
        // include from ndjson index (depth >=2)
        if let Ok(text) = fs::read_to_string(&index_path) {
            for line in text.lines() {
                if line.trim().is_empty() { continue; }
                if let Ok(v) = serde_json::from_str::<Value>(line) {
                    let ty = v["type"].as_str().unwrap_or("");
                    if allowed.contains(&ty) {
                        if let Some(id) = v["id"].as_str() { ids.push(id.to_string()); }
                    }
                }
            }
        }
        // de-dup
        ids.sort(); ids.dedup();
        let assets_dir = outdir.join("assets/images");
        fs::create_dir_all(&assets_dir)?;
        let mut manifest = fs::OpenOptions::new().create(true).append(true).open(outdir.join("assets_index.ndjson"))?;
        let batch_size = 5usize;
        for (batch_idx, chunk) in ids.chunks(batch_size).enumerate() {
            // Build image export URL
            let ids_param = chunk.iter().map(|s| s.replace(":","%3A")).collect::<Vec<_>>().join(",");
            let url = format!("https://api.figma.com/v1/images/{}?ids={}&format=png&scale=2", file_key, ids_param);
            let body: Value = match get_json(&client, &token, &url, 4).await { Ok(v)=>v, Err(e)=>{ eprintln!("warn: export batch {}: {}", batch_idx, e); continue; } };
            if let Some(map) = body.get("images").and_then(|m| m.as_object()) {
                for (id, urlv) in map.iter() {
                    let url = urlv.as_str().unwrap_or("");
                    if url.is_empty() { continue; }
                    let fname = format!("{}.png", id.replace(":","_"));
                    let path = assets_dir.join(&fname);
                    if path.exists() { continue; }
                    // Download PNG
                    match download_bytes(&client, url, 4).await {
                        Ok(bytes) => {
                            if let Err(e) = fs::write(&path, &bytes) {
                                eprintln!("warn: write asset {} failed: {}", path.display(), e);
                            } else {
                                let rec = json!({"id": id, "url": url, "path": path.display().to_string(), "format": "png", "scale": 2});
                                let _ = writeln!(&mut manifest, "{}", serde_json::to_string(&rec).unwrap_or("{}".into()));
                            }
                        }
                        Err(e) => eprintln!("warn: fetch asset {} err {}", url, e),
                    }
                    sleep(Duration::from_millis(sleep_ms)).await;
                }
            }
            // Gentle pause between image batches
            sleep(Duration::from_millis(sleep_ms * 2)).await;
            // Soft cap on batches per run
            if batch_idx >= 300 { eprintln!("soft cap on image batches reached ({}). Rerun to continue.", batch_idx); break; }
        }
    }

    println!("Dump complete: {}", outdir.display());
    Ok(())
}
