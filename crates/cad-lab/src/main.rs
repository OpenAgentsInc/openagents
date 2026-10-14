//! `cad-lab serve [--port 8794] [--dir DIR]`: the CAD Lab service on
//! 127.0.0.1 (docs/cad-lab/mvp.md).
//! `cad-lab schema`: prints the `part.v1` and `assembly.v1` JSON Schemas.
//! `cad-lab tool NAME [JSON]`: runs one tool here, without the service.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use cad_lab::lab::{FILES, Lab};
use serde_json::{Value, json};

const USAGE: &str =
    "usage: cad-lab serve [--port PORT] [--dir DIR] | cad-lab schema | cad-lab tool NAME [JSON]";

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("schema") => {
            println!(
                "{}",
                serde_json::to_string_pretty(&cad::doc::schema()).unwrap_or_default()
            );
            std::process::ExitCode::SUCCESS
        }
        Some("tool") if args.len() >= 2 => {
            let input = args.get(2).map_or("{}", String::as_str);
            let parsed: Value = match serde_json::from_str(input) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("cad-lab: the arguments aren't JSON ({e})");
                    return std::process::ExitCode::from(2);
                }
            };
            let lab = match Lab::open(Lab::default_root()) {
                Ok(lab) => lab,
                Err(why) => {
                    eprintln!("cad-lab: {why}");
                    return std::process::ExitCode::FAILURE;
                }
            };
            let result = lab.run(&args[1], &parsed);
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
            if result["error"].is_string() {
                std::process::ExitCode::FAILURE
            } else {
                std::process::ExitCode::SUCCESS
            }
        }
        Some("serve") => match serve(&args[1..]) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(why) => {
                eprintln!("cad-lab: {why}");
                std::process::ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("{USAGE}");
            std::process::ExitCode::from(2)
        }
    }
}

fn serve(args: &[String]) -> Result<(), String> {
    let mut port: u16 = std::env::var("CAD_LAB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8794);
    let mut dir = Lab::default_root();
    let mut words = args.iter();
    while let Some(word) = words.next() {
        let mut value = || words.next().cloned().ok_or(format!("{word} needs a value"));
        match word.as_str() {
            "--port" => port = value()?.parse().map_err(|_| "--port needs a number")?,
            "--dir" => dir = PathBuf::from(value()?),
            _ => return Err(USAGE.into()),
        }
    }
    let mut lab = Lab::open(dir)?;
    lab.base = format!("http://127.0.0.1:{port}");
    let lab = Arc::new(lab);
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let app = Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .route("/api/tools", get(|| async { Json(cad_lab::lab::tools()) }))
            .route("/api/docs", get(docs))
            .route("/api/schema", get(|| async { Json(cad::doc::schema()) }))
            .route("/api/tool/{name}", post(tool))
            .route("/api/card", get(card))
            .route("/api/file", get(file))
            .route("/api/out", get(out))
            .with_state(lab.clone());
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .map_err(|e| format!("can't listen on 127.0.0.1:{port} ({e})"))?;
        eprintln!(
            "cad-lab: http://127.0.0.1:{port} (documents in {})",
            lab.root.display()
        );
        axum::serve(listener, app).await.map_err(|e| e.to_string())
    })
}

async fn docs(State(lab): State<Arc<Lab>>) -> Json<Value> {
    Json(lab.run("cad_list", &json!({})))
}

async fn tool(
    State(lab): State<Arc<Lab>>,
    Path(name): Path<String>,
    body: Option<Json<Value>>,
) -> Json<Value> {
    let args = body.map_or(json!({}), |Json(v)| v);
    let args = if args.is_object() { args } else { json!({}) };
    eprintln!("cad-lab: {name}");
    let result = tokio::task::spawn_blocking(move || lab.run(&name, &args))
        .await
        .unwrap_or_else(|e| json!({"error": e.to_string()}));
    Json(result)
}

fn query(q: &[(String, String)], key: &str) -> Option<String> {
    q.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

fn safe(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 120
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.+".contains(c))
        && !name.starts_with('.')
}

async fn card(State(lab): State<Arc<Lab>>, Query(q): Query<Vec<(String, String)>>) -> Json<Value> {
    let Some(doc) = query(&q, "doc").filter(|d| safe(d)) else {
        return Json(json!({"error": "Name the document."}));
    };
    let Some(v) = query(&q, "v")
        .and_then(|v| v.parse().ok())
        .or_else(|| lab.latest(&doc))
    else {
        return Json(json!({"error": "There's no such document."}));
    };
    let against = query(&q, "a").and_then(|v| v.parse().ok());
    Json(
        lab.card(&doc, v, against)
            .unwrap_or_else(|why| json!({"error": why})),
    )
}

fn kind(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or_default() {
        "png" => "image/png",
        "glb" => "model/gltf-binary",
        "step" => "model/step",
        "3mf" => "model/3mf",
        "stl" => "model/stl",
        _ => "application/octet-stream",
    }
}

async fn send(path: PathBuf, name: &str) -> Response {
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, kind(name)),
                (header::CACHE_CONTROL, "max-age=3600"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// A version's viewer model or picture.
async fn file(State(lab): State<Arc<Lab>>, Query(q): Query<Vec<(String, String)>>) -> Response {
    let (Some(doc), Some(v), Some(name)) = (
        query(&q, "doc"),
        query(&q, "v").and_then(|v| v.parse::<u64>().ok()),
        query(&q, "name"),
    ) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !safe(&doc) || !FILES.contains(&name.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    send(lab.version_dir(&doc, v).join(&name), &name).await
}

/// A render or export of a version.
async fn out(State(lab): State<Arc<Lab>>, Query(q): Query<Vec<(String, String)>>) -> Response {
    let (Some(doc), Some(v), Some(name)) = (
        query(&q, "doc"),
        query(&q, "v").and_then(|v| v.parse::<u64>().ok()),
        query(&q, "name"),
    ) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !safe(&doc) || !safe(&name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    send(lab.version_dir(&doc, v).join("out").join(&name), &name).await
}
