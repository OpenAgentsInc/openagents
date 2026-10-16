//! JSON bridge from Video Lab's local HTTP service to the Rust editor.
use std::io::{self, Read};

fn main() {
    let result = (|| {
        let mut data = String::new();
        io::stdin()
            .take(16 * 1024 * 1024)
            .read_to_string(&mut data)
            .map_err(|e| e.to_string())?;
        let input =
            serde_json::from_str(&data).map_err(|e| format!("The edit could not be read: {e}"))?;
        verse_film::timeline::request(&input)
    })();
    let output = result.unwrap_or_else(|error| serde_json::json!({"error":error}));
    println!("{output}");
}
