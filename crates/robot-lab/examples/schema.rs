//! Regenerate the checked-in document schema with `cargo run -p robot-lab
//! --example schema -- crates/robot-lab/schemas/documents.v1.json`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Give the schema's output path")?;
    let schema = schemars::schema_for!(robot_lab::Document);
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&schema)?),
    )?;
    Ok(())
}
