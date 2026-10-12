//! `openagents film`: the film engine's take (`verse-film`), inspected and
//! made. `docs/verse/film.md` is the spec.

use std::path::Path;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use serde_json::{Value, json};

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents film COMMAND [ARGS]
  inspect FILE              Read a take and print its header, counts, and notes.
  demo OUT [--pictures N] [--hold N] [--scene square|desktop] [--image FILE]
                            Write a short demo take (a square sliding across a
                            1280×720 frame at 60 fps, or a Coder Noir desktop
                            scene over FILE, a PNG or JPEG) to OUT.
  render FILE DIR [--picture N] [--background R,G,B]
                            Draw a take's pictures with wgpu into DIR as
                            frame-NNNN.png (every picture, or only picture N).
  export FILE OUT.mp4 [--background R,G,B] [--crf N]
                            Draw every frame of a take and encode it with
                            ffmpeg (H.264, yuv420p) at the take's frame rate.
A take is a scene recorded once as uploads and frames, which any renderer
draws on demand; see docs/verse/film.md.";

/// What the commands do, for the chat router's command tree.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("inspect", Effect::ReadOnly),
    Declared::computer("demo", Effect::LocalWrite),
    Declared::computer("render", Effect::LocalWrite),
    Declared::computer("export", Effect::LocalWrite),
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
            let bytes = match args.option("scene").unwrap_or("square") {
                "square" => verse_film::take::demo(pictures, hold),
                "desktop" => {
                    let image = args.option("image").map(Path::new);
                    match verse_film::layers::demo(image, pictures, hold) {
                        Ok(bytes) => bytes,
                        Err(message) => return output.fail("film demo", &message),
                    }
                }
                other => {
                    return output.usage("film demo", &format!("unknown scene `{other}`"), USAGE);
                }
            };
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
        "render" => {
            let [file, dir] = args.positional() else {
                return output.usage("film render", "FILE and DIR are required", USAGE);
            };
            let picture = match args.number("picture", u32::MAX) {
                Ok(n) => n,
                Err(_) => return output.usage("film render", "--picture needs a number", USAGE),
            };
            let background = match args.option("background") {
                None => [0.0, 0.0, 0.0, 1.0],
                Some(text) => match parse_color(text) {
                    Some(color) => color,
                    None => {
                        return output.usage(
                            "film render",
                            "--background needs R,G,B between 0 and 1",
                            USAGE,
                        );
                    }
                },
            };
            match render_take(Path::new(file), Path::new(dir), picture, background) {
                Ok(summary) => {
                    output.emit(&summary, |v| {
                        format!(
                            "{} pictures of {} drawn into {} ({}×{})",
                            v["drawn"],
                            v["path"].as_str().unwrap_or(""),
                            v["dir"].as_str().unwrap_or(""),
                            v["width"],
                            v["height"]
                        )
                    });
                    0
                }
                Err(message) => output.fail("film render", &message),
            }
        }
        "export" => {
            let [file, out] = args.positional() else {
                return output.usage("film export", "FILE and OUT.mp4 are required", USAGE);
            };
            let crf = match args.number("crf", 18u32) {
                Ok(n) if n <= 51 => n,
                _ => {
                    return output.usage("film export", "--crf needs a number from 0 to 51", USAGE);
                }
            };
            let background = match args.option("background") {
                None => [0.0, 0.0, 0.0, 1.0],
                Some(text) => match parse_color(text) {
                    Some(color) => color,
                    None => {
                        return output.usage(
                            "film export",
                            "--background needs R,G,B between 0 and 1",
                            USAGE,
                        );
                    }
                },
            };
            match export_take(Path::new(file), Path::new(out), background, crf) {
                Ok(summary) => {
                    output.emit(&summary, |v| {
                        format!(
                            "{} frames of {} encoded to {} ({}×{} @ {} fps)",
                            v["frames"],
                            v["path"].as_str().unwrap_or(""),
                            v["out"].as_str().unwrap_or(""),
                            v["width"],
                            v["height"],
                            v["fps"]
                        )
                    });
                    0
                }
                Err(message) => output.fail("film export", &message),
            }
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

fn parse_color(text: &str) -> Option<[f32; 4]> {
    let parts: Vec<f32> = text
        .split(',')
        .map(|part| part.trim().parse::<f32>().ok())
        .collect::<Option<_>>()?;
    match parts[..] {
        [r, g, b] if [r, g, b].iter().all(|c| (0.0..=1.0).contains(c)) => Some([r, g, b, 1.0]),
        _ => None,
    }
}

fn render_take(
    file: &Path,
    dir: &Path,
    picture: u32,
    background: [f32; 4],
) -> Result<Value, String> {
    let bytes =
        std::fs::read(file).map_err(|error| format!("can't read {}: {error}", file.display()))?;
    let take = verse_film::read(&bytes).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("can't create {}: {error}", dir.display()))?;
    let mut renderer = verse_film::render::Renderer::new(&take)?;
    let (width, height) = renderer.size();
    let mut drawn = 0u32;
    for (index, frame) in take.frames.iter().enumerate() {
        if picture != u32::MAX && index as u32 != picture {
            continue;
        }
        let pixels = renderer.draw(frame, background)?;
        let out = dir.join(format!("frame-{index:04}.png"));
        verse_film::render::write_png(&out, width, height, &pixels)?;
        drawn += 1;
    }
    if picture != u32::MAX && drawn == 0 {
        return Err(format!(
            "the take has {} pictures; there is no picture {picture}",
            take.frames.len()
        ));
    }
    Ok(json!({
        "path": file.display().to_string(),
        "dir": dir.display().to_string(),
        "width": width,
        "height": height,
        "drawn": drawn,
        "pictures": take.frames.len(),
    }))
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

fn export_take(file: &Path, out: &Path, background: [f32; 4], crf: u32) -> Result<Value, String> {
    let bytes =
        std::fs::read(file).map_err(|error| format!("can't read {}: {error}", file.display()))?;
    let take = verse_film::read(&bytes).map_err(|error| error.to_string())?;
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("can't create {}: {error}", parent.display()))?;
    }
    let exported = verse_film::export::export(&take, out, background, crf)?;
    let size = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    Ok(json!({
        "path": file.display().to_string(),
        "out": out.display().to_string(),
        "width": exported.width,
        "height": exported.height,
        "fps": take.fps,
        "frames": exported.frames,
        "bytes": size,
    }))
}
