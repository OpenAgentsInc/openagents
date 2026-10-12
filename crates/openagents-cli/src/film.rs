//! `openagents film`: the film engine's take (`verse-film`), inspected and
//! made. `docs/verse/film.md` is the spec.

use std::path::Path;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use serde_json::{Value, json};

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents film COMMAND [ARGS]
  inspect FILE              Read a take and print its header, counts, and notes.
  demo OUT [--pictures N] [--hold N]
                            Write a short demo take (a square sliding across a
                            1280×720 frame at 60 fps) to OUT.
A take is a scene recorded once as uploads and frames, which any renderer
draws on demand; see docs/verse/film.md.";

/// What the commands do, for the chat router's command tree.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("inspect", Effect::ReadOnly),
    Declared::computer("demo", Effect::LocalWrite),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("film", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("film", &message, USAGE),
    };
    match command.as_str() {
        "inspect" => {
            let [file] = args.positional() else {
                return output.usage("film inspect", "one FILE is required", USAGE);
            };
            match inspect(Path::new(file)) {
                Ok(summary) => {
                    output.emit(&summary, render);
                    0
                }
                Err(message) => output.fail("film inspect", &message),
            }
        }
        "demo" => {
            let [out] = args.positional() else {
                return output.usage("film demo", "one OUT file is required", USAGE);
            };
            let pictures = match args.number("pictures", 60u32) {
                Ok(n) if n > 0 => n,
                _ => return output.usage("film demo", "--pictures needs a count above 0", USAGE),
            };
            let hold = match args.number("hold", 1u32) {
                Ok(n) if n > 0 => n,
                _ => return output.usage("film demo", "--hold needs a count above 0", USAGE),
            };
            let bytes = verse_film::take::demo(pictures, hold);
            if let Err(error) = std::fs::write(out, &bytes) {
                return output.fail("film demo", &format!("can't write {out}: {error}"));
            }
            let mut summary = verse_film::read(&bytes)
                .expect("the demo take reads")
                .summary();
            summary["path"] = json!(out);
            summary["bytes"] = json!(bytes.len());
            output.emit(&summary, render);
            0
        }
        other => output.usage("film", &format!("unknown command `{other}`"), USAGE),
    }
}

fn inspect(file: &Path) -> Result<Value, String> {
    let bytes =
        std::fs::read(file).map_err(|error| format!("can't read {}: {error}", file.display()))?;
    let take = verse_film::read(&bytes).map_err(|error| error.to_string())?;
    let mut summary = take.summary();
    summary["path"] = json!(file.display().to_string());
    summary["bytes"] = json!(bytes.len());
    Ok(summary)
}

fn render(v: &Value) -> String {
    format!(
        "{} {}×{} @ {} fps: {} pictures, {} frames ({:.2} s), {} paths, {} images, {} records{}",
        v["path"].as_str().unwrap_or(""),
        v["width"],
        v["height"],
        v["fps"],
        v["pictures"],
        v["frames"],
        v["seconds"].as_f64().unwrap_or(0.0),
        v["uploads"]["paths"],
        v["uploads"]["images"],
        v["records"],
        match v["failed"] {
            Value::Null => ", cut short",
            Value::Bool(true) => ", scene failed",
            _ => "",
        }
    )
}
