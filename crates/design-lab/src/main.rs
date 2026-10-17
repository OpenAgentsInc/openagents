//! `design-lab serve [--port 8798] [--captures DIR]`: the board on 127.0.0.1.
//! `design-lab export DIR [--captures DIR]`: the same board as static files
//! (`index.html`, `board.json`, `mocks.json`, `img/<run>/<file>`) for any
//! file host; mocks and notes are read-only there.
//! `design-lab spec MOCK_OR_SCREEN [--out DIR] [--captures DIR]`: the Markdown
//! spec of a mock (with its PNG written to `--out`, default
//! `<captures>/specs`) or of a screen and its notes, for a building agent.
//!
//! `serve` also edits mocks and notes: `POST /mocks/change` takes one checked
//! change, and only from this computer.
//! Capture runs come from `apps/openagents/scripts/design-lab-capture.sh`.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::extract::{ConnectInfo, Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use design_lab::{Board, mock};
use lab_kit::Version;
use serde_json::{Value, json};

const USAGE: &str = "usage: design-lab serve [--port PORT] [--captures DIR] | \
design-lab export DIR [--captures DIR] | design-lab spec MOCK_OR_SCREEN [--out DIR] [--captures DIR]";

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("serve") => serve(&args[1..]),
        Some("export") if args.len() >= 2 => export(&args[1], &args[2..]),
        Some("spec") if args.len() >= 2 => spec(&args[1], &args[2..]),
        _ => Err(USAGE.into()),
    };
    match outcome {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("design-lab: {why}");
            std::process::ExitCode::FAILURE
        }
    }
}

struct Options {
    port: u16,
    captures: PathBuf,
    out: Option<PathBuf>,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        port: std::env::var("DESIGN_LAB_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8798),
        captures: design_lab::default_captures(),
        out: None,
    };
    let mut words = args.iter();
    while let Some(word) = words.next() {
        let mut value = || words.next().cloned().ok_or(format!("{word} needs a value"));
        match word.as_str() {
            "--port" => options.port = value()?.parse().map_err(|_| "--port needs a number")?,
            "--captures" => options.captures = PathBuf::from(value()?),
            "--out" => options.out = Some(PathBuf::from(value()?)),
            _ => return Err(USAGE.into()),
        }
    }
    Ok(options)
}

fn scanned(captures: &Path) -> Result<Board, String> {
    let board = design_lab::scan(captures)?;
    eprintln!(
        "design-lab: {} screens, {} versions",
        board.screens.len(),
        board.versions()
    );
    Ok(board)
}

fn export(dir: &str, args: &[String]) -> Result<(), String> {
    let options = options(args)?;
    let board = scanned(&options.captures)?;
    let dir = PathBuf::from(dir);
    let img = dir.join("img");
    std::fs::create_dir_all(&img).map_err(|e| format!("can't make {} ({e})", img.display()))?;
    for version in board.screens.iter().flat_map(|s| &s.versions) {
        let files = [
            Some(&version.image),
            version.dark.as_ref(),
            version.elements.as_ref(),
            version.dark_elements.as_ref(),
        ];
        for file in files.into_iter().flatten() {
            let bytes = design_lab::image(&options.captures, &board, file)
                .ok_or(format!("can't read {file}"))?;
            let path = img.join(file);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(path, bytes).map_err(|e| e.to_string())?;
        }
    }
    let json = serde_json::to_vec(&board).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("board.json"), json).map_err(|e| e.to_string())?;
    let mocks = mock::load(&options.captures)?;
    let json = serde_json::to_vec(&mocks_body(&mocks, false)).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("mocks.json"), json).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("index.html"), design_lab::PAGE).map_err(|e| e.to_string())?;
    eprintln!("design-lab: wrote {}", dir.display());
    Ok(())
}

/// The spec of a mock or a screen on stdout; a mock's PNG goes to `--out`.
fn spec(name: &str, args: &[String]) -> Result<(), String> {
    let options = options(args)?;
    let board = design_lab::scan(&options.captures)?;
    let doc = mock::of(&mock::load(&options.captures)?.content).map_err(|r| r.message)?;
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| options.captures.join("specs"));
    let file_of = |id: &str| out.join(format!("{id}.png"));
    let made = make_spec(&options.captures, &board, &doc, name, &|id| {
        file_of(id).display().to_string()
    })?;
    if let Some((id, png)) = made.1 {
        std::fs::create_dir_all(&out).map_err(|e| format!("can't make {} ({e})", out.display()))?;
        std::fs::write(file_of(&id), png).map_err(|e| e.to_string())?;
        eprintln!("design-lab: wrote {}", file_of(&id).display());
    }
    print!("{}", made.0);
    Ok(())
}

/// The Markdown, and for a mock its id and PNG at 3 pixels a point.
fn make_spec(
    captures: &Path,
    board: &Board,
    doc: &mock::Mocks,
    name: &str,
    picture: &dyn Fn(&str) -> String,
) -> Result<(String, Option<(String, Vec<u8>)>), String> {
    let spec = mock::spec(
        board,
        doc,
        name,
        &|p| design_lab::elements(captures, board, p),
        picture,
    )?;
    let png = match &spec.mock {
        Some(id) => {
            let m = doc
                .mocks
                .iter()
                .find(|m| &m.id == id)
                .ok_or("no such mock")?;
            let png = mock::render(m, 3.0, &|p| design_lab::image(captures, board, p))?;
            Some((id.clone(), png))
        }
        None => None,
    };
    Ok((spec.markdown, png))
}

struct Lab {
    captures: PathBuf,
    board: Board,
    port: u16,
    mocks: Mutex<Version>,
}

fn serve(args: &[String]) -> Result<(), String> {
    let options = options(args)?;
    let lab = Arc::new(Lab {
        board: scanned(&options.captures)?,
        mocks: Mutex::new(mock::load(&options.captures)?),
        port: options.port,
        captures: options.captures,
    });
    let port = options.port;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let app = Router::new()
            .route("/", get(|| async { Html(design_lab::PAGE) }))
            .route("/healthz", get(|| async { "ok" }))
            .route("/board.json", get(board))
            .route("/img/{run}/{file}", get(image))
            .route("/mocks.json", get(mocks))
            .route("/mocks/change", post(change))
            .route("/spec/{name}", get(spec_route))
            .with_state(lab);
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .map_err(|e| format!("can't listen on 127.0.0.1:{port} ({e})"))?;
        eprintln!("design-lab: http://127.0.0.1:{port}");
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| e.to_string())
    })
}

fn mocks_body(version: &Version, editable: bool) -> Value {
    json!({
        "version": version.number,
        "editable": editable,
        "doc": version.content,
    })
}

async fn mocks(State(lab): State<Arc<Lab>>) -> Response {
    let version = lab.mocks.lock().map(|v| v.clone());
    match version {
        Ok(v) => (
            [(header::CACHE_CONTROL, "no-store")],
            Json(mocks_body(&v, true)),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Only this computer may change the mocks: the connection is from loopback,
/// and the page that sends it is this server's own (no other site's page in
/// the same browser, and no name that only resolves here).
fn local(lab: &Lab, peer: &SocketAddr, headers: &HeaderMap) -> bool {
    let port = lab.port;
    let ours = |v: &str| {
        [
            format!("127.0.0.1:{port}"),
            format!("localhost:{port}"),
            format!("[::1]:{port}"),
        ]
        .iter()
        .any(|h| h.as_str() == v)
    };
    let host_ok = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(ours);
    let origin_ok = match headers.get(header::ORIGIN).map(|o| o.to_str()) {
        None => true,
        Some(Ok(o)) => o.strip_prefix("http://").is_some_and(ours),
        Some(Err(_)) => false,
    };
    peer.ip().is_loopback() && host_ok && origin_ok
}

fn refused(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

/// `{"base": 4, "change": {"intent": "item_add", ...}, "summary": "...",
/// "author": "..."}` → the new version.
async fn change(
    State(lab): State<Arc<Lab>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if !local(&lab, &peer, &headers) {
        return refused(
            StatusCode::FORBIDDEN,
            "local_only",
            "Mocks change only from this computer.",
        );
    }
    let lab2 = lab.clone();
    let done = tokio::task::spawn_blocking(move || {
        let lab = lab2;
        let mut current = lab
            .mocks
            .lock()
            .map_err(|_| lab_kit::Refusal::new("save_failed", "We couldn't save the mocks."))?;
        let summary = body["summary"].as_str().unwrap_or("Edited mocks");
        let author = body["author"].as_str().unwrap_or("board");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let next = mock::change(
            &lab.captures,
            &current,
            body["base"].as_u64(),
            &body["change"],
            author,
            summary,
            now,
        )?;
        *current = next.clone();
        Ok::<Version, lab_kit::Refusal>(next)
    })
    .await;
    match done {
        Ok(Ok(v)) => Json(mocks_body(&v, true)).into_response(),
        Ok(Err(r)) => {
            let status = if r.code == "stale_base" {
                StatusCode::CONFLICT
            } else if r.code == "save_failed" {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            };
            refused(status, &r.code, &r.message)
        }
        Err(_) => refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            "save_failed",
            "We couldn't save the mocks.",
        ),
    }
}

/// `/spec/m3` is Markdown; `/spec/m3.png` is the mock's picture.
async fn spec_route(State(lab): State<Arc<Lab>>, UrlPath(name): UrlPath<String>) -> Response {
    let Ok(doc) = lab.mocks.lock().map(|v| v.content.clone()) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let made = tokio::task::spawn_blocking(move || {
        let doc = mock::of(&doc).map_err(|r| r.message)?;
        let (id, png) = match name.strip_suffix(".png") {
            Some(id) => (id.to_string(), true),
            None => (name.clone(), false),
        };
        let made = make_spec(&lab.captures, &lab.board, &doc, &id, &|m| {
            format!("/spec/{m}.png")
        })?;
        Ok::<_, String>((made, png))
    })
    .await;
    match made {
        Ok(Ok(((_, Some((_, png))), true))) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            png,
        )
            .into_response(),
        Ok(Ok(((markdown, _), false))) => (
            [
                (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            markdown,
        )
            .into_response(),
        Ok(Ok(_)) => StatusCode::NOT_FOUND.into_response(),
        Ok(Err(why)) => (StatusCode::NOT_FOUND, why).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn board(State(lab): State<Arc<Lab>>) -> Json<Board> {
    Json(lab.board.clone())
}

async fn image(
    State(lab): State<Arc<Lab>>,
    UrlPath((run, file)): UrlPath<(String, String)>,
) -> Response {
    let picture = format!("{run}/{file}");
    let found =
        tokio::task::spawn_blocking(move || design_lab::image(&lab.captures, &lab.board, &picture))
            .await
            .ok()
            .flatten();
    match found {
        Some(bytes) => {
            let kind = if bytes.starts_with(b"\x89PNG") {
                "image/png"
            } else if bytes.starts_with(b"{") {
                "application/json"
            } else {
                "image/jpeg"
            };
            (
                [
                    (header::CONTENT_TYPE, kind),
                    // A run never changes its pictures.
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                bytes,
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
