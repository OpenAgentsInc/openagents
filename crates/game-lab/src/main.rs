//! `game-lab serve [--port 8793] [--dir DIR] [--repo CHECKOUT]`: the Game
//! Lab service on 127.0.0.1 (docs/game-lab/mvp.md).
//! `game-lab schema`: prints the `game.v1` JSON Schema.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use game_lab::lab::{BUILD_FILES, Build, Lab};
use serde_json::{Value, json};

const USAGE: &str =
    "usage: game-lab serve [--port PORT] [--dir DIR] [--repo CHECKOUT] | game-lab schema";

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("schema") => {
            println!(
                "{}",
                serde_json::to_string_pretty(&game_lab::doc::schema()).unwrap_or_default()
            );
            std::process::ExitCode::SUCCESS
        }
        Some("serve") => match serve(&args[1..]) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(why) => {
                eprintln!("game-lab: {why}");
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
    let mut port: u16 = std::env::var("GAME_LAB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8793);
    let mut dir = Lab::default_root();
    let mut repo = std::env::var_os("GAME_LAB_REPO").map_or_else(
        || PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")),
        PathBuf::from,
    );
    let mut words = args.iter();
    while let Some(word) = words.next() {
        let mut value = || words.next().cloned().ok_or(format!("{word} needs a value"));
        match word.as_str() {
            "--port" => port = value()?.parse().map_err(|_| "--port needs a number")?,
            "--dir" => dir = PathBuf::from(value()?),
            "--repo" => repo = PathBuf::from(value()?),
            _ => return Err(USAGE.into()),
        }
    }
    let mut lab = Lab::open(dir, repo)?;
    lab.base = format!("http://127.0.0.1:{port}");
    let lab = Arc::new(lab);
    lab.ensure_build();
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let app = Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .route("/api/tools", get(|| async { Json(game_lab::lab::tools()) }))
            .route("/api/docs", get(docs))
            .route(
                "/api/schema",
                get(|| async { Json(game_lab::doc::schema()) }),
            )
            .route("/api/tool/{name}", post(tool))
            .route("/api/card", get(card))
            .route("/api/note", post(note))
            .route("/build/{file}", get(build_file))
            .route("/play", get(play))
            .with_state(lab.clone());
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .map_err(|e| format!("can't listen on 127.0.0.1:{port} ({e})"))?;
        eprintln!(
            "game-lab: http://127.0.0.1:{port} (documents in {})",
            lab.root.display()
        );
        axum::serve(listener, app).await.map_err(|e| e.to_string())
    })
}

async fn docs(State(lab): State<Arc<Lab>>) -> Json<Value> {
    Json(lab.run("game_list", &json!({})))
}

async fn tool(
    State(lab): State<Arc<Lab>>,
    Path(name): Path<String>,
    body: Option<Json<Value>>,
) -> Json<Value> {
    let args = body.map_or(json!({}), |Json(v)| v);
    let args = if args.is_object() { args } else { json!({}) };
    eprintln!("game-lab: {name}");
    let result = tokio::task::spawn_blocking(move || lab.run(&name, &args))
        .await
        .unwrap_or_else(|e| json!({"error": e.to_string()}));
    Json(result)
}

async fn card(
    State(lab): State<Arc<Lab>>,
    Query(query): Query<Vec<(String, String)>>,
) -> Json<Value> {
    let get = |key: &str| query.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let game = get("doc").unwrap_or_else(|| game_lab::doc::ICE_CREAM_MAN.into());
    let version = get("v")
        .and_then(|v| v.parse().ok())
        .or_else(|| lab.latest(&game));
    let against = get("a").and_then(|v| v.parse().ok());
    let Some(version) = version else {
        return Json(json!({"error": "There's no such game."}));
    };
    lab.ensure_build();
    Json(
        lab.card(&game, version, against)
            .unwrap_or_else(|why| json!({"error": why})),
    )
}

async fn note(State(lab): State<Arc<Lab>>, Json(args): Json<Value>) -> Json<Value> {
    let game = args["doc"]
        .as_str()
        .unwrap_or(game_lab::doc::ICE_CREAM_MAN)
        .to_owned();
    Json(match lab.add_note(&game, &args) {
        Ok(note) => json!({"saved": true, "note": note}),
        Err(why) => json!({"saved": false, "error": why}),
    })
}

async fn build_file(State(lab): State<Arc<Lab>>, Path(file): Path<String>) -> Response {
    if !BUILD_FILES.contains(&file.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    if lab.build_state() != Build::Ready {
        lab.ensure_build();
        return (StatusCode::SERVICE_UNAVAILABLE, "The game is being built.").into_response();
    }
    let Some(dir) = lab.build_dir() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(bytes) = tokio::fs::read(dir.join(&file)).await else {
        lab.ensure_build();
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let kind = if file.ends_with(".wasm") {
        "application/wasm"
    } else {
        "text/javascript; charset=utf-8"
    };
    (
        [
            (header::CONTENT_TYPE, kind),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        bytes,
    )
        .into_response()
}

async fn play(State(lab): State<Arc<Lab>>, Query(query): Query<Vec<(String, String)>>) -> Response {
    let get = |key: &str| query.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let game = get("doc").unwrap_or_else(|| game_lab::doc::ICE_CREAM_MAN.into());
    let Some(version) = get("v")
        .and_then(|v| v.parse().ok())
        .or_else(|| lab.latest(&game))
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    lab.ensure_build();
    match lab.play_page(&game, version) {
        Ok(page) => axum::response::Html(page).into_response(),
        Err(why) => (StatusCode::NOT_FOUND, why).into_response(),
    }
}
