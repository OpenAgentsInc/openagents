//! Check a consumer's message with the Rust contract and write it back.
use prost::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.len() != 3 {
        return Err("Usage: roundtrip INPUT.pb OUTPUT.pb".into());
    }
    let scene =
        engine_proto::decode(&std::fs::read(&arguments[1])?).map_err(std::io::Error::other)?;
    std::fs::write(&arguments[2], scene.encode_to_vec())?;
    println!(
        "{}: {} landmarks, {} colliders",
        scene.scene_id,
        scene.landmarks.len(),
        scene.colliders.len()
    );
    Ok(())
}
