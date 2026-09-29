//! `openagents-deck`: the OpenAgents deck in a native window, as text, or
//! as PNG files.
//!
//! With no option it opens the default deck in a window. `--text` prints
//! every slide's grid, `--check` lists the slides still waiting on facts,
//! and `--capture DIR` paints every slide to a PNG with the window's own
//! painter, so a screenshot in the repository is the frame the window
//! shows.

mod window;

use openagents_deck::paint::{FIELD, Frame, Painter, fitting_size};
use openagents_deck::{Canvas, Deck, overview, slide_grid};
use std::path::PathBuf;
use std::process::ExitCode;

/// What the command line asked for.
#[derive(Debug, Default)]
pub struct Options {
    pub deck: Option<String>,
    pub slide: usize,
    pub notes: bool,
    pub still: bool,
    pub fullscreen: bool,
    text: bool,
    check: bool,
    decks: bool,
    help: bool,
    capture: Option<PathBuf>,
    size: (usize, usize),
}

const USAGE: &str = "\
openagents-deck: the OpenAgents deck

Usage: openagents-deck [options]

  --deck NAME       the deck under crates/openagents-deck/decks/ (default: the first)
  --decks           list the decks
  --slide N         open on slide N (from 1)
  --notes           open with the presenter's notes showing
  --still           no arrival animation
  --fullscreen      open fullscreen
  --text            print every slide as the text of its grid
  --check           list the slides still waiting on facts; exits 1 while any do
  --capture DIR     paint every slide, and the overview, to PNG files in DIR
  --size WxH        the capture size in pixels (default 1920x1080)
  --help            this text

Keys: right, space, n, j: next · left, p, k: back · home, end · a number then
enter: jump · o: overview · t: notes · .: black · s: motion · f: fullscreen ·
cmd or ctrl with =, -, 0: zoom · escape: close what is open · q: quit";

fn parse(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        size: (1920, 1080),
        ..Options::default()
    };
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--deck" => options.deck = Some(value("--deck")?),
            "--decks" => options.decks = true,
            "--slide" => {
                let number: usize = value("--slide")?
                    .parse()
                    .map_err(|_| "--slide takes a number".to_string())?;
                options.slide = number.saturating_sub(1);
            }
            "--notes" => options.notes = true,
            "--still" => options.still = true,
            "--fullscreen" => options.fullscreen = true,
            "--text" => options.text = true,
            "--check" => options.check = true,
            "--capture" => options.capture = Some(PathBuf::from(value("--capture")?)),
            "--size" => {
                let size = value("--size")?;
                let (w, h) = size
                    .split_once('x')
                    .ok_or_else(|| "--size takes WIDTHxHEIGHT".to_string())?;
                options.size = (
                    w.parse().map_err(|_| "--size takes WIDTHxHEIGHT")?,
                    h.parse().map_err(|_| "--size takes WIDTHxHEIGHT")?,
                );
            }
            "--help" | "-h" => options.help = true,
            // macOS passes a process serial number to an app opened from
            // the Finder on some versions.
            other if other.starts_with("-psn_") => {}
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    Ok(options)
}

fn main() -> ExitCode {
    let options = match parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(complaint) => {
            eprintln!("{complaint}");
            return ExitCode::from(2);
        }
    };
    if options.help {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if options.decks {
        for name in Deck::names() {
            println!("{name}");
        }
        return ExitCode::SUCCESS;
    }
    let deck = match &options.deck {
        Some(name) => match Deck::named(name) {
            Some(deck) => deck,
            None => {
                eprintln!(
                    "no deck named {name}; the decks are: {}",
                    Deck::names().join(", ")
                );
                return ExitCode::from(2);
            }
        },
        None => Deck::load(),
    };
    if options.text {
        for index in 0..deck.len() {
            let slide = deck.slide(index).expect("the slide");
            println!("── {} / {} · {} ──", index + 1, deck.len(), slide.id);
            print!(
                "{}",
                slide_grid(&deck, index, Canvas::DEFAULT, 1.0).to_text()
            );
        }
        return ExitCode::SUCCESS;
    }
    if options.check {
        let waiting = deck.unfilled();
        for (index, slide) in &waiting {
            println!("{} {} waits on facts", index + 1, slide.id);
        }
        if waiting.is_empty() {
            println!("every slide of {} has its facts", deck.name);
            return ExitCode::SUCCESS;
        }
        return ExitCode::FAILURE;
    }
    if let Some(directory) = &options.capture {
        return match capture(&deck, directory, options.size) {
            Ok(count) => {
                println!("wrote {count} files to {}", directory.display());
                ExitCode::SUCCESS
            }
            Err(complaint) => {
                eprintln!("{complaint}");
                ExitCode::FAILURE
            }
        };
    }
    match window::run(deck, &options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(complaint) => {
            eprintln!("{complaint}");
            ExitCode::FAILURE
        }
    }
}

/// Paints every slide of `deck`, and the overview, to PNG files in
/// `directory` at `width` by `height` pixels. Returns how many it wrote.
fn capture(
    deck: &Deck,
    directory: &PathBuf,
    (width, height): (usize, usize),
) -> Result<usize, String> {
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    let canvas = Canvas::DEFAULT;
    let mut painter = Painter::new(fitting_size(
        width as f32,
        height as f32,
        canvas.cells,
        canvas.rows,
    ));
    let (w, h) = painter.extent(canvas.cells, canvas.rows);
    let (x, y) = ((width as f32 - w) / 2.0, (height as f32 - h) / 2.0);
    let mut write = |name: String, grid: &openagents_deck::Grid| -> Result<(), String> {
        let mut frame = Frame::new(width, height, FIELD);
        painter.paint(&mut frame, grid, x, y);
        let path = directory.join(name);
        std::fs::write(&path, frame.png()?)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    };
    for index in 0..deck.len() {
        let slide = deck.slide(index).expect("the slide");
        let grid = slide_grid(deck, index, canvas, 1.0);
        write(format!("{:02}-{}.png", index + 1, slide.id), &grid)?;
    }
    write("overview.png".to_string(), &overview(deck, 0, canvas))?;
    Ok(deck.len() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> impl Iterator<Item = String> + '_ {
        line.split_whitespace().map(str::to_string)
    }

    #[test]
    fn the_options_parse() {
        let options = parse(args(
            "--deck test-time-capabilities --slide 3 --notes --size 800x450",
        ))
        .expect("the options parse");
        assert_eq!(options.deck.as_deref(), Some("test-time-capabilities"));
        assert_eq!(options.slide, 2);
        assert!(options.notes);
        assert_eq!(options.size, (800, 450));
        assert!(parse(args("--bogus")).is_err());
        assert!(parse(args("--slide")).is_err());
    }

    #[test]
    fn a_capture_writes_every_slide_and_the_overview() {
        let directory =
            std::env::temp_dir().join(format!("openagents-deck-{}", std::process::id()));
        let deck = Deck::load();
        let count = capture(&deck, &directory, (320, 180)).expect("the capture");
        assert_eq!(count, deck.len() + 1);
        assert!(directory.join("overview.png").exists());
        let _ = std::fs::remove_dir_all(&directory);
    }
}
