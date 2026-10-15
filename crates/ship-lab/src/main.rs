//! `ship-lab serve [--port 8796] [--dir DIR]`: the Ship Lab service on
//! 127.0.0.1, which the web chat's `ship_*` tools call.
//! `ship-lab tool NAME [JSON]`: runs one tool here, without the service.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use ship_lab::Lab;

const USAGE: &str = "usage: ship-lab serve [--port PORT] [--dir DIR] | ship-lab tool NAME [JSON]";

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("serve") => serve(&args[1..]),
        Some("tool") if args.len() >= 2 => tool_here(&args[1], args.get(2).map(String::as_str)),
        _ => Err(USAGE.into()),
    };
    match outcome {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("ship-lab: {why}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn tool_here(name: &str, input: Option<&str>) -> Result<(), String> {
    let args: Value = serde_json::from_str(input.unwrap_or("{}"))
        .map_err(|e| format!("the arguments aren't JSON ({e})"))?;
    let lab = Lab::open(Lab::default_root())?;
    let result = lab.run(name, &args);
    println!(
        "{}",
        serde_json::to_string_pretty(&result).unwrap_or_default()
    );
    Ok(())
}

fn serve(args: &[String]) -> Result<(), String> {
    let mut port: u16 = std::env::var("SHIP_LAB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8796);
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
    let lab = Arc::new(Lab::open(dir)?);
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let app = Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .route("/api/tools", get(|| async { Json(ship_lab::tools()) }))
            .route("/api/docs", get(docs))
            .route("/api/tool/{name}", post(tool))
            .with_state(lab.clone());
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .map_err(|e| format!("can't listen on 127.0.0.1:{port} ({e})"))?;
        eprintln!(
            "ship-lab: http://127.0.0.1:{port} (ships in {})",
            lab.root.display()
        );
        axum::serve(listener, app).await.map_err(|e| e.to_string())
    })
}

async fn docs(State(lab): State<Arc<Lab>>) -> Json<Value> {
    Json(lab.run("ship_list", &json!({})))
}

async fn tool(
    State(lab): State<Arc<Lab>>,
    Path(name): Path<String>,
    body: Option<Json<Value>>,
) -> Json<Value> {
    let args = body.map_or(json!({}), |Json(v)| v);
    let args = if args.is_object() { args } else { json!({}) };
    eprintln!("ship-lab: {name}");
    let result = lab.run(&name, &args);
    if let Some(why) = result["error"].as_str() {
        eprintln!("ship-lab: {name} refused: {why}");
    }
    Json(result)
}
