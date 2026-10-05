fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let name = args
        .next()
        .ok_or("Usage: verse-content ritual|observatory DIRECTORY")?;
    let dir = args
        .next()
        .ok_or("Usage: verse-content ritual|observatory DIRECTORY")?;
    if args.next().is_some() {
        return Err("Unexpected compiler argument".into());
    }
    let path = std::path::Path::new(&dir);
    if path.exists() {
        return Err("Compiler output directory must not exist".into());
    }
    let name = name.to_str().ok_or("World recipe must be UTF-8")?;
    if !matches!(name, "ritual" | "observatory") {
        return Err("Unknown world recipe".into());
    }
    let (pack, scene, _) = verse_content::compiler::worlds::compile(name, path)?;
    let identity = verse_content::remote_content::identity(&pack, &scene, path)?;
    println!(
        "{}",
        serde_json::json!({"world": name, "models": pack.models.len(), "content": identity})
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
