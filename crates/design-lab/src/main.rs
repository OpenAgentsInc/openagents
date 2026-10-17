//! `design-lab serve [--port 8798] [--captures DIR]`: the board on 127.0.0.1.
//! `design-lab export DIR [--captures DIR]`: the same board as static files
//! (`index.html`, `board.json`, `img/<run>/<file>`) for any file host.
//!
//! Capture runs come from `apps/openagents/scripts/design-lab-capture.sh`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use design_lab::Board;

const USAGE: &str = "usage: design-lab serve [--port PORT] [--captures DIR] | design-lab export DIR [--captures DIR]";

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("serve") => serve(&args[1..]),
        Some("export") if args.len() >= 2 => export(&args[1], &args[2..]),
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
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        port: std::env::var("DESIGN_LAB_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8798),
        captures: design_lab::default_captures(),
    };
    let mut words = args.iter();
    while let Some(word) = words.next() {
        let mut value = || words.next().cloned().ok_or(format!("{word} needs a value"));
        match word.as_str() {
            "--port" => options.port = value()?.parse().map_err(|_| "--port needs a number")?,
            "--captures" => options.captures = PathBuf::from(value()?),
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
        let bytes = design_lab::image(&options.captures, &board, &version.image)
            .ok_or(format!("can't read {}", version.image))?;
        let path = img.join(&version.image);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, bytes).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec(&board).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("board.json"), json).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("index.html"), design_lab::PAGE).map_err(|e| e.to_string())?;
    eprintln!("design-lab: wrote {}", dir.display());
    Ok(())
}

struct Lab {
    captures: PathBuf,
    board: Board,
}

fn serve(args: &[String]) -> Result<(), String> {
    let options = options(args)?;
    let lab = Arc::new(Lab {
        board: scanned(&options.captures)?,
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
            .with_state(lab);
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .map_err(|e| format!("can't listen on 127.0.0.1:{port} ({e})"))?;
        eprintln!("design-lab: http://127.0.0.1:{port}");
        axum::serve(listener, app).await.map_err(|e| e.to_string())
    })
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
