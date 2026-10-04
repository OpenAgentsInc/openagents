//! Regenerate spell evidence without loading the renderer or character assets.
fn main() -> Result<(), String> {
    let output = std::env::args()
        .nth(1)
        .ok_or("Expected an output directory")?;
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    for scenario in verse_world::playground::scenarios()
        .into_iter()
        .filter(|s| {
            verse_world::spells::CATALOG
                .iter()
                .any(|spell| spell.key == s.key)
        })
    {
        let key = scenario.key;
        let mut run = verse_world::playground::Run::new(scenario)?;
        while !run.done() {
            run.advance()?;
        }
        if run.replay_identical != Some(true) {
            return Err(format!("{key}: replay diverged"));
        }
        let evidence = run.evidence()?;
        let path = std::path::Path::new(&output).join(format!("spell-{key}.json"));
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
