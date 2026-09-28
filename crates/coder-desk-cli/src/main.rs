//! `coder-desk`: ask the desktop session on this computer what is on its
//! screens, and change one window.
//!
//! One subcommand per verb of the desk protocol, which
//! `crates/coder-desk/src/protocol.rs` defines. The client is `crates/coder-desk`, which reads what the session
//! announced, picks a backend, and speaks to whatever desk is in front of
//! it. Nothing here names a compositor.
//!
//! `list`, `screens`, `focused`, and `reading` print JSON on stdout, so a
//! script reads them with `jq`. The rest print nothing and say whether the
//! change went through with their exit status. The drive verbs run the
//! session from a shell, which is how an agent on `ssh` runs the owner's
//! checks and proves them with a screenshot: `key`, `type`, `click`,
//! `move`, and `shot` do what one hand movement does, and `press`,
//! `release`, `drag`, and `scroll` hold what the others only tap, so a
//! drag, a held modifier, and a wheel are checkable with nobody at the
//! machine.
//!
//! # Exit status
//!
//! | Status | What happened |
//! | --- | --- |
//! | 0 | The desk answered. |
//! | 2 | The command line was wrong. |
//! | 3 | There is no desktop session here. |
//! | 4 | A session was announced, and its desk did not answer. |
//! | 5 | The desk refused the request, and stderr says why. |
//! | 6 | The desk answered something this command cannot read. |

use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use coder_desk::{
    Absent, Blocking, Border, Button, Chord, DeskError, Modifiers, Motion, Open, Point, Selector,
    Shape, Size, Stroke,
};

/// The command line was wrong, which is the status `clap` itself exits
/// with.
const USAGE: u8 = 2;
/// Nothing announced a session, so this run is not inside one.
const NO_SESSION: u8 = 3;
/// A session was announced and its desk does not answer.
const UNREACHABLE: u8 = 4;
/// The desk refused the request.
const REFUSED: u8 = 5;
/// The desk answered something this command cannot read.
const UNREADABLE: u8 = 6;

/// What a run reads when nothing announced a session. It says which of the
/// two cases this is, because they call for different answers: this one
/// means the run is somewhere without a screen.
const NO_SESSION_COPY: &str = "error: there is no desktop session here, so there is no desk to \
                               ask. A run reached over SSH and a host with no compositor have \
                               none.";

/// Ask the desktop session what is on its screens, and change one window.
#[derive(Parser, Debug)]
#[command(
    name = "coder-desk",
    version,
    about = "Ask the desktop session what is on its screens, and change one window",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Every window the session holds, as a JSON array
    List,
    /// The screens the session holds, as a JSON array
    Screens,
    /// The window that has the focus, as JSON, or null when none does
    Focused,
    /// The screens, the name of the one the focus is on, and the desks, as
    /// one JSON object
    Reading,
    /// Start a program in a new window, or show a file in a new pane
    Open(OpenArgs),
    /// Give one window the focus
    Focus {
        /// A handle, `class:<app-id>`, or `title:<title>`
        handle: String,
    },
    /// Move one window to a desk, without switching the screen to it
    Place {
        /// A handle, `class:<app-id>`, or `title:<title>`
        handle: String,
        /// The desk to move it to
        desk: u32,
    },
    /// Raise one window above the others on its desk
    Raise {
        /// A handle, `class:<app-id>`, or `title:<title>`
        handle: String,
    },
    /// Close one window
    Close {
        /// A handle, `class:<app-id>`, or `title:<title>`
        handle: String,
    },
    /// Change how the desk draws one window
    Shape(ShapeArgs),
    /// Set the scale one screen draws at
    Scale {
        /// The screen's name, such as `DP-2`
        screen: String,
        /// The scale to draw at, such as 1.0 or 2.0
        scale: f64,
    },
    /// Raise a notice the operator reads
    Notice {
        /// What the notice says
        text: String,
    },
    /// Ask the session to reload its configuration
    Reload,
    /// Press one chord, the way a press on the keyboard runs: a chord the
    /// bind table holds runs its action, and any other reaches the focused
    /// window
    Key {
        /// The chord in the bind table's spelling: modifiers and a key
        /// joined by `+`, such as `super+shift+d`, `ctrl+c`, `return`, or
        /// `escape`
        chord: String,
    },
    /// Type text into the focused window
    Type {
        /// The text, typed one key press per character; a newline presses
        /// Return
        text: String,
    },
    /// Press and release a pointer button at a point on the screens, on the
    /// window under it
    #[command(allow_negative_numbers = true)]
    Click {
        /// The point's x, in pixels across every screen
        x: i64,
        /// The point's y, in pixels across every screen
        y: i64,
        /// The button: `left`, `right`, or `middle`
        #[arg(default_value = "left", value_parser = button)]
        button: Button,
    },
    /// Move the pointer to a point on the screens, which moves the focus
    /// with it
    #[command(allow_negative_numbers = true)]
    Move {
        /// The point's x, in pixels across every screen
        x: i64,
        /// The point's y, in pixels across every screen
        y: i64,
        /// The steps the pointer takes to get there. Left out, it goes in
        /// one, and a grab reads a jump rather than motion
        #[arg(long, value_name = "COUNT")]
        steps: Option<u32>,
        /// The milliseconds the steps are spread over
        #[arg(long, value_name = "MS")]
        ms: Option<u64>,
    },
    /// Hold a pointer button or a key down, where the pointer is and on
    /// the window that has the focus
    Press(HoldArgs),
    /// Let a pointer button or a key go
    Release(HoldArgs),
    /// Press a button at one point, move to another, and let it go: the
    /// drag a person makes with the mouse
    #[command(allow_negative_numbers = true)]
    Drag(DragArgs),
    /// Scroll where the pointer is, a wheel's notches or a trackpad's
    /// smooth axis
    #[command(allow_negative_numbers = true)]
    Scroll {
        /// How far to scroll across, positive to the right
        dx: f64,
        /// How far to scroll down, positive down
        dy: f64,
        /// Send a wheel's notches rather than a trackpad's smooth axis
        #[arg(long)]
        discrete: bool,
        /// The steps the scroll is sent in
        #[arg(long, value_name = "COUNT")]
        steps: Option<u32>,
        /// The milliseconds the steps are spread over
        #[arg(long, value_name = "MS")]
        ms: Option<u64>,
    },
    /// Write a PNG of one screen, and answer once the file is written
    Shot {
        /// The file to write
        path: String,
        /// The screen to write, by name. Left out, the screen the pointer
        /// is on is written
        #[arg(long, value_name = "NAME")]
        screen: Option<String>,
    },
    /// Which backend answers here, and the socket it answers on
    /// Which backend answers here, the socket it answers on, and whether
    /// hands drive the desk
    Status,
}

/// What a `press` or a `release` names: a pointer button, or a key on the
/// keyboard.
///
/// `left`, `right`, and `middle` are the buttons, and every other word is
/// a key or a modifier, such as `super`, `shift`, `a`, or `return`. The
/// two vocabularies meet at `left` and `right`, which are also the arrow
/// keys, so `--key` reads the word as a key and `--button` reads it as a
/// button.
#[derive(Args, Debug)]
struct HoldArgs {
    /// The button, the key, or the modifier to hold
    what: String,
    /// Read the word as a key, so `left` is the arrow key
    #[arg(long, conflicts_with = "button")]
    key: bool,
    /// Read the word as a pointer button
    #[arg(long)]
    button: bool,
}

/// What a `drag` asks for.
#[derive(Args, Debug)]
struct DragArgs {
    /// Where the button goes down: its x, in pixels across every screen
    x1: i64,
    /// Where the button goes down: its y
    y1: i64,
    /// Where the button comes up: its x
    x2: i64,
    /// Where the button comes up: its y
    y2: i64,
    /// The button: `left`, `right`, or `middle`
    #[arg(default_value = "left", value_parser = button)]
    button: Button,
    /// The modifiers held across the drag, joined by `+`, such as `super`
    /// or `super+shift`
    #[arg(long, value_name = "MODS", default_value = "", value_parser = modifiers)]
    modifier: Modifiers,
    /// The steps the pointer takes between the two points
    #[arg(long, value_name = "COUNT")]
    steps: Option<u32>,
    /// The milliseconds the steps are spread over
    #[arg(long, value_name = "MS")]
    ms: Option<u64>,
}

/// What an `open` asks for. It names exactly one of a command and a path.
#[derive(Args, Debug)]
struct OpenArgs {
    /// The program to start, as one line for the shell the session runs it
    /// with
    command: Option<String>,
    /// A file to show in a new pane, instead of starting a program
    #[arg(long, value_name = "FILE")]
    path: Option<String>,
    /// The desk to open it on. Left out, the desk showing now takes it
    #[arg(long, value_name = "NUMBER")]
    desk: Option<u32>,
    /// Open it without giving it the focus
    #[arg(long)]
    silent: bool,
}

/// How the desk draws one window. A flag left out keeps what the window
/// has.
#[derive(Args, Debug)]
struct ShapeArgs {
    /// A handle, `class:<app-id>`, or `title:<title>`
    handle: String,
    /// Float the window over the layout
    #[arg(long, conflicts_with = "tile")]
    float: bool,
    /// Put the window back in the layout
    #[arg(long)]
    tile: bool,
    /// Show the window on every desk. On a session whose pin toggles, a
    /// second `--pin` takes it back off
    #[arg(long)]
    pin: bool,
    /// Move the window's top left corner to this point
    #[arg(long, value_name = "X,Y", value_parser = point)]
    at: Option<Point>,
    /// Size the window to these pixels
    #[arg(long, value_name = "WIDTHxHEIGHT", value_parser = size)]
    size: Option<Size>,
    /// Keep the window's aspect ratio when the layout resizes it
    #[arg(long, conflicts_with = "no_aspect")]
    aspect: bool,
    /// Let the layout change the window's aspect ratio
    #[arg(long)]
    no_aspect: bool,
    /// The border's thickness in pixels
    #[arg(long, value_name = "PIXELS")]
    border: Option<i64>,
    /// The border's corner rounding in pixels
    #[arg(long, value_name = "PIXELS")]
    rounding: Option<i64>,
    /// Draw the window's drop shadow
    #[arg(long, conflicts_with = "no_shadow")]
    shadow: bool,
    /// Draw no drop shadow
    #[arg(long)]
    no_shadow: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(answered) => {
            if !answered.is_empty() {
                println!("{answered}");
            }
            ExitCode::SUCCESS
        }
        Err(failure) => {
            eprintln!("{}", failure.say);
            ExitCode::from(failure.code)
        }
    }
}

/// One command, and what it prints.
fn run(command: Command) -> Result<String, Failure> {
    let desk = Blocking::here().map_err(absent)?;
    match command {
        Command::Status => {
            let mut lines = format!(
                "backend: {}\nsocket: {}",
                desk.desk().backend(),
                desk.desk().socket().display()
            );
            // A desk that answers `status` says whether hands drive it; one
            // that refuses the verb, such as a Hyprland session, says nothing
            // more.
            if let Ok(hands) = desk.status() {
                lines.push_str(if hands.on {
                    "\nhands: on"
                } else {
                    "\nhands: off"
                });
            }
            Ok(lines)
        }
        Command::List => written(serde_json::to_string(&desk.list().map_err(asked)?)),
        Command::Screens => written(serde_json::to_string(&desk.screens().map_err(asked)?)),
        Command::Focused => written(serde_json::to_string(&desk.focused().map_err(asked)?)),
        Command::Reading => {
            let reading = desk.reading().map_err(asked)?;
            let desks: Vec<serde_json::Value> = reading
                .desks
                .iter()
                .map(|desk| {
                    serde_json::json!({
                        "id": desk.id,
                        "screen": desk.screen,
                        "windows": desk.windows,
                    })
                })
                .collect();
            written(serde_json::to_string(&serde_json::json!({
                "screens": serde_json::to_value(&reading.screens).unwrap_or_default(),
                "focused": reading.focused,
                "desks": desks,
            })))
        }
        Command::Open(open) => changed(desk.open(opening(open)?)),
        Command::Focus { handle } => changed(desk.focus(&Selector::parse(&handle))),
        Command::Place { handle, desk: on } => changed(desk.place(&Selector::parse(&handle), on)),
        Command::Raise { handle } => changed(desk.raise(&Selector::parse(&handle))),
        Command::Close { handle } => changed(desk.close(&Selector::parse(&handle))),
        Command::Shape(shape) => {
            changed(desk.shape(&Selector::parse(&shape.handle), shaped(&shape)))
        }
        Command::Scale { screen, scale } => changed(desk.scale(&screen, scale)),
        Command::Notice { text } => changed(desk.notice(&text)),
        Command::Reload => changed(desk.reload()),
        Command::Key { chord } => {
            let chord = Chord::parse(&chord).map_err(usage)?;
            changed(desk.key(&chord))
        }
        Command::Type { text } => changed(desk.type_text(&text)),
        Command::Click { x, y, button } => changed(desk.click(Point { x, y }, button)),
        Command::Move { x, y, steps, ms } => {
            let motion = motion(steps, ms, Motion::JUMP)?;
            changed(desk.move_pointer(Point { x, y }, motion))
        }
        Command::Press(hold) => changed(held(&desk, &hold, true)?),
        Command::Release(hold) => changed(held(&desk, &hold, false)?),
        Command::Drag(drag) => {
            let motion = motion(drag.steps, drag.ms, Motion::HAND)?;
            changed(desk.drag(
                Point {
                    x: drag.x1,
                    y: drag.y1,
                },
                Point {
                    x: drag.x2,
                    y: drag.y2,
                },
                drag.button,
                drag.modifier,
                motion,
            ))
        }
        Command::Scroll {
            dx,
            dy,
            discrete,
            steps,
            ms,
        } => {
            let motion = motion(steps, ms, Motion::JUMP)?;
            changed(desk.scroll(dx, dy, discrete, motion))
        }
        Command::Shot { path, screen } => changed(desk.shot(&path, screen.as_deref())),
    }
}

/// One `press` or one `release`, on the pointer or on the keyboard.
///
/// A word that names a button is a button, unless `--key` says to read it
/// as a key; `--button` says to read it as a button whatever it names.
fn held(desk: &Blocking, hold: &HoldArgs, down: bool) -> Result<Result<(), DeskError>, Failure> {
    let as_button = match (hold.key, hold.button) {
        (true, _) => None,
        (_, true) => Some(Button::parse(&hold.what).map_err(usage)?),
        _ => Button::parse(&hold.what).ok(),
    };
    Ok(match as_button {
        Some(button) => desk.button(button, down),
        None => desk.stroke(&Stroke::parse(&hold.what).map_err(usage)?, down),
    })
}

/// The motion one pair of flags asks for.
fn motion(steps: Option<u32>, ms: Option<u64>, unnamed: Motion) -> Result<Motion, Failure> {
    Motion::read(steps, ms, unnamed).map_err(usage)
}

/// The failure a command line this command cannot read makes.
fn usage(why: String) -> Failure {
    Failure {
        say: format!("error: {why}"),
        code: USAGE,
    }
}

/// Why a command did not answer, and the status it exits with.
struct Failure {
    /// What the operator reads on stderr.
    say: String,
    /// The status the command exits with.
    code: u8,
}

/// The failure a run with no desk makes.
fn absent(absent: Absent) -> Failure {
    match absent {
        Absent::NoSession => Failure {
            say: NO_SESSION_COPY.to_string(),
            code: NO_SESSION,
        },
        ended => Failure {
            say: ended.say(),
            code: UNREACHABLE,
        },
    }
}

/// The failure one refused or unanswered request makes.
fn asked(error: DeskError) -> Failure {
    let code = match error {
        DeskError::Unreachable(_) => UNREACHABLE,
        DeskError::Refused(_) => REFUSED,
        DeskError::Asked(_) | DeskError::Unreadable(_) => UNREADABLE,
    };
    Failure {
        say: error.say(),
        code,
    }
}

/// A change verb, which prints nothing when it goes through.
fn changed(answered: Result<(), DeskError>) -> Result<String, Failure> {
    answered.map(|()| String::new()).map_err(asked)
}

/// One JSON document for stdout.
fn written(answered: Result<String, serde_json::Error>) -> Result<String, Failure> {
    answered.map_err(|error| Failure {
        say: format!("error: this command could not write what the desk answered: {error}"),
        code: UNREADABLE,
    })
}

/// What an `open` asks the desk for.
fn opening(open: OpenArgs) -> Result<Open, Failure> {
    let asking = match (open.command, open.path) {
        (Some(command), None) => Open::command(command),
        (None, Some(path)) => Open::path(path),
        _ => {
            return Err(Failure {
                say: "error: an `open` names exactly one of a command and `--path`.".to_string(),
                code: USAGE,
            });
        }
    };
    let asking = match open.desk {
        Some(desk) => asking.on_desk(desk),
        None => asking,
    };
    Ok(match open.silent {
        true => asking.silently(),
        false => asking,
    })
}

/// The shape one set of flags asks for. A flag left out leaves its field
/// unset, which keeps what the window has.
fn shaped(flags: &ShapeArgs) -> Shape {
    Shape {
        float: pair(flags.float, flags.tile),
        // A session's `pin` toggles, so the protocol refuses `false` and a
        // caller that wants a window unpinned sends `--pin` again.
        pin: flags.pin.then_some(true),
        at: flags.at,
        size: flags.size,
        aspect: pair(flags.aspect, flags.no_aspect),
        border: border(flags),
        shadow: pair(flags.shadow, flags.no_shadow),
    }
}

/// The border one pair of flags asks for, or none when neither is set.
fn border(flags: &ShapeArgs) -> Option<Border> {
    match (flags.border, flags.rounding) {
        (None, None) => None,
        (size, rounding) => Some(Border { size, rounding }),
    }
}

/// What two opposed flags say, or nothing when neither is set.
fn pair(on: bool, off: bool) -> Option<bool> {
    match (on, off) {
        (true, _) => Some(true),
        (_, true) => Some(false),
        _ => None,
    }
}

/// One point, written `<x>,<y>`.
fn point(named: &str) -> Result<Point, String> {
    let (x, y) = named
        .split_once(',')
        .ok_or_else(|| format!("a point is written `<x>,<y>`, and this is `{named}`"))?;
    Ok(Point {
        x: whole(x, "an x")?,
        y: whole(y, "a y")?,
    })
}

/// One size, written `<width>x<height>`.
fn size(named: &str) -> Result<Size, String> {
    let (width, height) = named
        .split_once('x')
        .ok_or_else(|| format!("a size is written `<width>x<height>`, and this is `{named}`"))?;
    Ok(Size {
        width: whole(width, "a width")?,
        height: whole(height, "a height")?,
    })
}

/// One pointer button, by its word.
fn button(named: &str) -> Result<Button, String> {
    Button::parse(named)
}

/// The modifiers one spelling names, such as `super+shift`.
fn modifiers(named: &str) -> Result<Modifiers, String> {
    Modifiers::parse(named)
}

/// One whole number of a pair.
fn whole(named: &str, what: &str) -> Result<i64, String> {
    named
        .trim()
        .parse()
        .map_err(|_| format!("{what} is a whole number, and this is `{named}`"))
}
