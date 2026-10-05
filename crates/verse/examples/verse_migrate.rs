//! Plans, applies, or rolls back an offline chamber migration under the writer lock.
use std::{io::Read, path::Path};
use verse_world::{
    play::Game,
    service::{
        host::Config,
        persistence::{Store, migration::Review},
    },
};
fn bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|_| "Cannot open migration input")?;
    if !file
        .metadata()
        .map_err(|_| "Cannot inspect migration input")?
        .is_file()
    {
        return Err("Migration input must be a regular file".into());
    }
    let mut bytes = vec![];
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read migration input")?;
    if bytes.len() > limit {
        return Err("Migration input exceeds its byte budget".into());
    }
    Ok(bytes)
}
fn load(path: &Path) -> Result<Config, String> {
    Config::from_json(&bounded(path, 64 * 1024)?)
}
fn prepare(config: &Config) -> Result<(Game, [u8; 32]), String> {
    let scene = verse_engine::director::Scene::from_json(&bounded(&config.scene, 1024 * 1024)?)?;
    let pack = verse_engine::assets::Pack::read(&config.pack)?;
    verse::imported::remote_content::outfit_models(&pack, &config.outfits)?;
    verse::imported::remote_content::equipment_models(
        &pack,
        &scene,
        &config.outfits,
        &config.equipment,
    )?;
    let content = verse::imported::remote_content::identity(
        &pack,
        &scene,
        config.pack.parent().unwrap_or(Path::new(".")),
    )?;
    let content = config.bind_content(content)?;
    let mut game = config.prepare_game(scene)?;
    verse::imported::props::admit_collision(&pack, &mut game)?;
    Ok((game, content))
}
fn print(value: &impl serde::Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|_| "Cannot encode migration output")?
    );
    Ok(())
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let usage = "Usage: verse_migrate plan SOURCE.json TARGET.json | apply SOURCE.json TARGET.json REVIEW.json | rollback CURRENT.json MIGRATION_ID";
    let mode = args.first().and_then(|s| s.to_str()).ok_or(usage)?;
    match (mode, args.len()) {
        ("plan", 3) | ("apply", 4) => {
            let source = load(Path::new(&args[1]))?;
            let target = load(Path::new(&args[2]))?;
            let (source_game, source_content) = prepare(&source)?;
            let (target_game, target_content) = prepare(&target)?;
            let root = source
                .state_dir
                .as_ref()
                .ok_or("Migration requires a durable source state directory")?;
            let mut store = Store::open(root, source_content, source.instance)?;
            // Scene comparison must use the same authored input as normal startup.
            // Planning retains the store's pending recovery for the offline migration API.
            store.validate_migration_source(&source, &source_game)?;
            if mode == "plan" {
                print(&store.plan_migration(&source, &target, target_game, target_content)?)
            } else {
                let review: Review =
                    serde_json::from_slice(&bounded(Path::new(&args[3]), 256 * 1024)?)
                        .map_err(|_| "Invalid reviewed migration")?;
                print(&store.apply_migration(
                    &source,
                    &target,
                    target_game,
                    target_content,
                    &review,
                )?)
            }
        }
        ("rollback", 3) => {
            let current = load(Path::new(&args[1]))?;
            let (prepared, content) = prepare(&current)?;
            let root = current
                .state_dir
                .as_ref()
                .ok_or("Rollback requires a durable state directory")?;
            let mut store = Store::open(root, content, current.instance)?;
            store.validate_migration_source(&current, &prepared)?;
            let id = args[2].to_str().ok_or("Invalid migration identity")?;
            print(&store.rollback_migration(id)?)
        }
        _ => Err(usage.into()),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
