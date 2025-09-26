use anyhow::{anyhow, Result};
use regex_lite::Regex;
use reqwest::header::{AUTHORIZATION, ACCEPT, USER_AGENT};
use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use dirs::home_dir;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct FindNodesArgs {
    pub figma_url: String,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub mode: Option<String>, // contains | name_regex | node_id
    #[serde(default)]
    pub node_types: Option<Vec<String>>, // FRAME, COMPONENT, INSTANCE, SECTION
    #[serde(default)]
    pub limit: Option<u32>, // default 10, max 25
}

pub async fn handle_find_nodes(args: FindNodesArgs, base_dir: Option<&Path>) -> Result<Value> {
    let file_key = extract_file_key(&args.figma_url)
        .ok_or_else(|| anyhow!("Invalid Figma URL format"))?;

    let token = get_figma_token(base_dir)?;

    let timeout = std::time::Duration::from_millis(10_000);
    let client = reqwest::Client::builder().timeout(timeout).build()?;

    let url = format!("https://api.figma.com/v1/files/{file_key}");
    let resp = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(ACCEPT, "application/json")
        .header(USER_AGENT, "codex-figma-tool/1.0")
        .send()
        .await?;

    match resp.status().as_u16() {
        200 => {
            let body: Value = resp.json().await?;
            let document = body.get("document").cloned().unwrap_or(json!({}));
            let query = args.query.unwrap_or_default();
            let mode = args.mode.unwrap_or_else(|| "contains".to_string());
            let types = args
                .node_types
                .unwrap_or_else(|| vec!["FRAME".into(), "COMPONENT".into(), "INSTANCE".into()]);
            let limit = args.limit.unwrap_or(10).min(25) as usize;

            let mut results: Vec<Value> = Vec::new();
            traverse_collect(
                &document,
                &query,
                &mode,
                &types,
                &mut results,
                limit,
            );

            let out = json!({
                "nodes": results,
                "returned": results.len(),
            });
            Ok(out)
        }
        403 => Err(anyhow!("Access denied to Figma file")),
        404 => Err(anyhow!("Figma file not found")),
        s => Err(anyhow!(format!("Figma API error: {s}"))),
    }
}

#[derive(Deserialize)]
pub struct ExportImagesArgs {
    pub figma_url: String,
    pub node_ids: Vec<String>,
    pub format: String, // png | jpg | svg
    #[serde(default)]
    pub scale: Option<u32>,
}

pub async fn handle_export_images(args: ExportImagesArgs, base_dir: Option<&Path>) -> Result<Value> {
    let file_key = extract_file_key(&args.figma_url)
        .ok_or_else(|| anyhow!("Invalid Figma URL format"))?;
    if args.node_ids.is_empty() {
        return Err(anyhow!("node_ids must be non-empty"));
    }
    let fmt = args.format.to_lowercase();
    if !matches!(fmt.as_str(), "png" | "jpg" | "svg") {
        return Err(anyhow!("Invalid format: {fmt}"));
    }
    let token = get_figma_token(base_dir)?;

    let ids_param = args.node_ids.join(",");
    let scale = args.scale.unwrap_or(1);

    let url = format!(
        "https://api.figma.com/v1/images/{file_key}?ids={ids}&format={fmt}&scale={scale}",
        file_key = file_key,
        ids = ids_param,
        fmt = fmt,
        scale = scale
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(15_000))
        .build()?;
    let resp = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(ACCEPT, "application/json")
        .header(USER_AGENT, "codex-figma-tool/1.0")
        .send()
        .await?;

    match resp.status().as_u16() {
        200 => {
            let body: Value = resp.json().await?;
            Ok(body)
        }
        403 => Err(anyhow!("Access denied to Figma file")),
        404 => Err(anyhow!("Figma file not found")),
        s => Err(anyhow!(format!("Figma API error: {s}"))),
    }
}

#[derive(Deserialize)]
pub struct GetNodesArgs {
    pub figma_url: String,
    pub node_ids: Vec<String>,
}

pub async fn handle_get_nodes(args: GetNodesArgs, base_dir: Option<&Path>) -> Result<Value> {
    let file_key = extract_file_key(&args.figma_url)
        .ok_or_else(|| anyhow!("Invalid Figma URL format"))?;
    if args.node_ids.is_empty() {
        return Err(anyhow!("node_ids must be non-empty"));
    }
    let token = get_figma_token(base_dir)?;

    let ids_param = args.node_ids.join(",");
    let url = format!(
        "https://api.figma.com/v1/nodes/{file_key}?ids={ids}",
        file_key = file_key,
        ids = ids_param
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(10_000))
        .build()?;
    let resp = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(ACCEPT, "application/json")
        .header(USER_AGENT, "codex-figma-tool/1.0")
        .send()
        .await?;
    match resp.status().as_u16() {
        200 => Ok(resp.json().await?),
        403 => Err(anyhow!("Access denied to Figma file")),
        404 => Err(anyhow!("Figma file not found")),
        s => Err(anyhow!(format!("Figma API error: {s}"))),
    }
}

#[derive(Deserialize)]
pub struct ExtractTokensArgs {
    pub figma_url: String,
}

pub async fn handle_extract_tokens(args: ExtractTokensArgs, base_dir: Option<&Path>) -> Result<Value> {
    let file_key = extract_file_key(&args.figma_url)
        .ok_or_else(|| anyhow!("Invalid Figma URL format"))?;
    let token = get_figma_token(base_dir)?;

    let url = format!("https://api.figma.com/v1/files/{file_key}/styles");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(10_000))
        .build()?;
    let resp = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(ACCEPT, "application/json")
        .header(USER_AGENT, "codex-figma-tool/1.0")
        .send()
        .await?;
    match resp.status().as_u16() {
        200 => {
            let body: Value = resp.json().await?;
            // Normalize into a compact token list: name + style_type
            let styles = body
                .get("meta")
                .and_then(|m| m.get("styles"))
                .and_then(|s| s.as_array())
                .cloned()
                .unwrap_or_default();
            let tokens: Vec<Value> = styles
                .into_iter()
                .map(|s| json!({
                    "name": s.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "style_type": s.get("style_type").and_then(|v| v.as_str()).unwrap_or(""),
                    "key": s.get("key").and_then(|v| v.as_str()).unwrap_or(""),
                }))
                .collect();
            Ok(json!({ "tokens": tokens, "count": tokens.len() }))
        }
        403 => Err(anyhow!("Access denied to Figma file")),
        404 => Err(anyhow!("Figma file not found")),
        s => Err(anyhow!(format!("Figma API error: {s}"))),
    }
}

fn traverse_collect(
    node: &Value,
    query: &str,
    mode: &str,
    allowed_types: &[String],
    out: &mut Vec<Value>,
    cap: usize,
) {
    if out.len() >= cap {
        return;
    }

    let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");

    if allowed_types.iter().any(|t| t == node_type) && matches_query(name, id, query, mode) {
        out.push(json!({
            "id": id,
            "name": name,
            "type": node_type,
        }));
    }

    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            if out.len() >= cap { break; }
            traverse_collect(child, query, mode, allowed_types, out, cap);
        }
    }
}

fn matches_query(name: &str, id: &str, query: &str, mode: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    match mode {
        "node_id" => id == query,
        "name_regex" => Regex::new(query).map(|re| re.is_match(name)).unwrap_or(false),
        _ => name.to_lowercase().contains(&query.to_lowercase()),
    }
}

fn extract_file_key(url: &str) -> Option<String> {
    // Support /file/<key>, /proto/<key>, and /design/<key> URL variants
    let re_file = Regex::new(r"^https://(?:www\.)?figma\.com/file/([A-Za-z0-9]+)").ok()?;
    let re_proto = Regex::new(r"^https://(?:www\.)?figma\.com/proto/([A-Za-z0-9]+)").ok()?;
    let re_design = Regex::new(r"^https://(?:www\.)?figma\.com/design/([A-Za-z0-9]+)").ok()?;
    if let Some(caps) = re_file.captures(url) {
        return Some(caps.get(1)?.as_str().to_string());
    }
    if let Some(caps) = re_proto.captures(url) {
        return Some(caps.get(1)?.as_str().to_string());
    }
    if let Some(caps) = re_design.captures(url) {
        return Some(caps.get(1)?.as_str().to_string());
    }
    None
}

#[derive(Deserialize)]
pub struct ListSubnodesArgs {
    pub figma_url: String,
    pub node_id: String,
    #[serde(default)]
    pub max_depth: Option<u32>,
    #[serde(default)]
    pub max_total: Option<usize>,
}

pub async fn handle_list_subnodes(args: ListSubnodesArgs, base_dir: Option<&Path>) -> Result<Value> {
    let file_key = extract_file_key(&args.figma_url)
        .ok_or_else(|| anyhow!("Invalid Figma URL format"))?;
    let token = get_figma_token(base_dir)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(15_000))
        .build()?;

    let max_depth = args.max_depth.unwrap_or(10);
    let max_total = args.max_total.unwrap_or(10_000);

    // BFS over node tree using /files/{key}/nodes?ids=...&depth=1 to avoid huge payloads.
    use std::collections::{VecDeque, HashSet};
    let mut queue: VecDeque<(String, u32, Option<String>)> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut out: Vec<Value> = Vec::new();

    queue.push_back((args.node_id.clone(), 0, None));
    visited.insert(args.node_id.clone());

    while let Some((current, depth, parent)) = queue.pop_front() {
        if out.len() >= max_total { break; }
        // Batch just the current id (could batch multiple siblings in future)
        let ids_param = current.replace(":", "%3A");
        let url = format!(
            "https://api.figma.com/v1/files/{file_key}/nodes?ids={ids}&depth=1",
            file_key=file_key,
            ids=ids_param
        );
        let resp = client
            .get(&url)
            .header("X-Figma-Token", token.clone())
            .header(ACCEPT, "application/json")
            .header(USER_AGENT, "codex-figma-tool/1.0")
            .send()
            .await?;
        let status = resp.status().as_u16();
        if status != 200 {
            return Err(anyhow!(format!("Figma API error listing nodes: {status}")));
        }
        let body: Value = resp.json().await?;
        if let Some(node) = body.get("nodes").and_then(|m| m.get(&current)).and_then(|n| n.get("document")) {
            let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let typ = node.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
            out.push(json!({"id": id, "name": name, "type": typ, "parent_id": parent, "depth": depth }));

            if depth < max_depth {
                if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
                    for child in children {
                        if out.len() >= max_total { break; }
                        let cid = child.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if !cid.is_empty() && visited.insert(cid.clone()) {
                            queue.push_back((cid, depth + 1, Some(id.clone())));
                        }
                    }
                }
            }
        }
    }

    Ok(json!({
        "file_key": file_key,
        "root_node_id": args.node_id,
        "count": out.len(),
        "nodes": out,
    }))
}

pub fn get_figma_token(base_dir: Option<&Path>) -> Result<String> {
    // 1) Environment variable
    if let Ok(val) = std::env::var("FIGMA_API_TOKEN") {
        if !val.trim().is_empty() { return Ok(val); }
    }

    // 2) Project-local files (relative to base_dir if provided, else current_dir)
    let base: PathBuf = match base_dir {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    let candidates = vec![
        base.join(".secrets/FIGMA_API_TOKEN"),
        base.join(".secrets/figma_api_token"),
        base.join(".figma_token"),
    ];
    for p in candidates {
        if let Ok(token) = read_trimmed(&p) { if !token.is_empty() { return Ok(token); } }
    }

    // 3) .env.local (parse simple KEY=VALUE lines)
    let env_local = base.join(".env.local");
    if let Ok(contents) = fs::read_to_string(&env_local) {
        for line in contents.lines() {
            let line = line.trim();
            if line.starts_with("FIGMA_API_TOKEN=") {
                let mut v = line["FIGMA_API_TOKEN=".len()..].trim().to_string();
                // Strip optional surrounding quotes
                if (v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')) {
                    v = v[1..v.len()-1].to_string();
                }
                if !v.is_empty() { return Ok(v); }
            }
        }
    }

    // 4) Home directory (~/.codex/figma_api_token)
    if let Some(mut h) = home_dir() {
        h.push(".codex/figma_api_token");
        if let Ok(token) = read_trimmed(&h) { if !token.is_empty() { return Ok(token); } }
    }

    Err(anyhow!("No Figma API token configured (set FIGMA_API_TOKEN or create .secrets/FIGMA_API_TOKEN, .figma_token, or .env.local)"))
}

fn read_trimmed(p: &Path) -> Result<String> {
    let mut s = String::new();
    let mut f = fs::File::open(p)?;
    f.read_to_string(&mut s)?;
    Ok(s.trim().to_string())
}
