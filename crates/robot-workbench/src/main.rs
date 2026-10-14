//! The local Robot Lab service requires explicit private project storage.
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let command = args.next();
    if command.as_deref() == Some("publish") {
        return robot_workbench::record::publish_cli(args.collect()).await;
    }
    if command.as_deref() == Some("export") {
        return robot_workbench::record::export_cli(args.collect());
    }
    if command.as_deref() == Some("record") {
        return robot_workbench::record::cli(args.collect());
    }
    if command.as_deref() != Some("serve") {
        return Err("Use: robot-workbench serve --root FOLDER --project PROJECT --build FOLDER [--port PORT] [--catalog FOLDER]".into());
    }
    let mut root = None;
    let mut project = None;
    let mut build = None;
    let mut port = 8794_u16;
    let mut catalog = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../robot-description/assets/so101");
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("Choose a value for this option")?;
        match flag.as_str() {
            "--root" => root = Some(std::path::PathBuf::from(value)),
            "--project" => project = Some(value),
            "--build" => build = Some(std::path::PathBuf::from(value)),
            "--port" => port = value.parse()?,
            "--catalog" => catalog = value.into(),
            _ => return Err("Choose a supported Robot Lab option".into()),
        }
    }
    let store = robot_lab::Store::open(
        root.ok_or("Choose --root for the lab's private files")?,
        &project.ok_or("Choose --project for an existing project")?,
    )?;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let base = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let host = robot_workbench::host::Host::open(store, &catalog, &base)?;
    let server = std::sync::Arc::new(robot_workbench::server::Server::new(
        host,
        &build.ok_or("Choose --build for the player files")?,
    ));
    eprintln!("Robot Lab: {base}");
    axum::serve(listener, server.router()).await?;
    Ok(())
}
#[cfg(target_arch = "wasm32")]
fn main() {}
