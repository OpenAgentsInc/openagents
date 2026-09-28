//! The Hyprland backend: the desk protocol translated to the line protocol
//! a Hyprland session answers.
//!
//! The translation is one `match`, so a test per verb reads the exact bytes
//! that go out. Everything Hyprland's vocabulary holds stops here: the verb
//! spellings, the JSON field names, the selector grammar, and the regular
//! expression a `class:` selector becomes.

use serde_json::Value;

use crate::{
    Backend, Button, Chord, Codec, DeskError, DeskRow, Point, Reading, Refusal, Reply, Screen,
    Selector, Shape, Size, Verb, Window, refusal,
};

/// How long a notice stays on the screen, in milliseconds. The protocol's
/// `notice` carries no duration, so the backend picks one long enough to
/// read while the operator is doing something else.
const NOTICE_MS: u64 = 6_000;

/// The backend for a Hyprland session.
///
/// A session presses no key and reads no screen of its own, so the drive
/// verbs start a program in the session: `wtype` types, `grim` writes a
/// screenshot, and `wlrctl` clicks. The backend reads whether each is on
/// this run's `PATH` before it sends the request, because the session
/// answers `ok` to an `exec` whether or not the program exists.
#[derive(Clone, Copy)]
pub struct Hyprland {
    /// Whether a program the session runs for a drive verb is on `PATH`.
    on_path: fn(&str) -> bool,
}

impl Hyprland {
    /// The backend, reading `PATH` for the programs the drive verbs run.
    pub fn new() -> Hyprland {
        Hyprland { on_path }
    }

    /// The backend with its own answer to whether a program is on `PATH`,
    /// which is what a test that sends every drive verb passes.
    pub fn with_tools(on_path: fn(&str) -> bool) -> Hyprland {
        Hyprland { on_path }
    }

    /// The refusal a drive verb makes when the program it runs is missing.
    fn needs(&self, program: &str, does: &str) -> Result<(), DeskError> {
        if (self.on_path)(program) {
            return Ok(());
        }
        Err(DeskError::unsupported(format!(
            "this session {does} with `{program}`, which is not on this run's PATH"
        )))
    }
}

impl Default for Hyprland {
    fn default() -> Hyprland {
        Hyprland::new()
    }
}

impl std::fmt::Debug for Hyprland {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Hyprland")
    }
}

/// Whether a program is on this run's `PATH`.
fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

impl Backend for Hyprland {
    fn name(&self) -> &'static str {
        "hyprland"
    }

    fn codec(&self) -> Codec {
        Codec::Stream
    }

    fn requests(&self, verb: &Verb) -> Result<Vec<String>, DeskError> {
        match verb {
            Verb::List => Ok(vec!["j/clients".to_string()]),
            Verb::Screens => Ok(vec!["j/monitors".to_string()]),
            Verb::Focused => Ok(vec!["j/activewindow".to_string()]),
            Verb::Open {
                command,
                path,
                desk,
                silent,
            } => open(command.as_deref(), path.as_deref(), *desk, *silent),
            Verb::Focus { handle } => {
                Ok(vec![format!("/dispatch focuswindow {}", selector(handle))])
            }
            Verb::Place { handle, desk } => Ok(vec![format!(
                "/dispatch movetoworkspacesilent {desk},{}",
                selector(handle)
            )]),
            Verb::Raise { handle } => Ok(vec![format!(
                "/dispatch alterzorder top,{}",
                selector(handle)
            )]),
            Verb::Close { handle } => {
                Ok(vec![format!("/dispatch closewindow {}", selector(handle))])
            }
            Verb::Shape {
                handle,
                float,
                pin,
                at,
                size,
                aspect,
                border,
                shadow,
            } => shape(
                handle,
                Shape {
                    float: *float,
                    pin: *pin,
                    at: *at,
                    size: *size,
                    aspect: *aspect,
                    border: *border,
                    shadow: *shadow,
                },
            ),
            Verb::Scale { screen, scale } => Ok(vec![format!(
                "/keyword monitor {screen},preferred,auto,{scale}"
            )]),
            Verb::Notice { text } => {
                Ok(vec![format!("/notify -1 {NOTICE_MS} 0 {}", one_line(text))])
            }
            Verb::Reload => Ok(vec!["/reload".to_string()]),
            Verb::Key { chord } => key(&Chord::parse(chord).map_err(DeskError::malformed)?),
            Verb::Type { text } => {
                self.needs("wtype", "types")?;
                Ok(vec![format!("/dispatch exec wtype -- {}", quoted(text))])
            }
            Verb::Click { x, y, button } => {
                self.needs("wlrctl", "clicks")?;
                Ok(vec![
                    format!("/dispatch movecursor {x} {y}"),
                    format!(
                        "/dispatch exec wlrctl pointer click {}",
                        button_word(*button)
                    ),
                ])
            }
            Verb::Move { x, y, steps, ms } => {
                // `movecursor` puts the cursor somewhere in one dispatch,
                // and the session answers no request for where the cursor
                // is, so there is nothing to step from.
                if steps.is_some_and(|steps| steps > 1) || ms.is_some_and(|ms| ms > 0) {
                    return Err(holds_nothing(
                        "this session moves the cursor in one dispatch and answers no request \
                         for where the cursor is, so it cannot step a move or spread one over \
                         time",
                    ));
                }
                Ok(vec![format!("/dispatch movecursor {x} {y}")])
            }
            Verb::ButtonPress { .. } | Verb::ButtonRelease { .. } => Err(holds_nothing(
                "this session presses a button with `wlrctl pointer click`, which presses and \
                 releases in one run and holds nothing, so it does not press and release a \
                 button on their own",
            )),
            Verb::Drag { .. } => Err(holds_nothing(
                "this session holds no button across a motion: `wlrctl pointer click` presses \
                 and releases in one run",
            )),
            Verb::KeyPress { .. } | Verb::KeyRelease { .. } => Err(holds_nothing(
                "this session types with `wtype`, which releases every key it held when it \
                 exits, so it holds no key or modifier across a later request",
            )),
            Verb::Scroll {
                dx,
                dy,
                discrete,
                steps,
                ms,
            } => {
                self.needs("wlrctl", "scrolls")?;
                if *discrete {
                    return Err(holds_nothing(
                        "this session scrolls with `wlrctl pointer scroll`, which sends a \
                         trackpad's smooth axis and no wheel notch",
                    ));
                }
                if steps.is_some_and(|steps| steps > 1) || ms.is_some_and(|ms| ms > 0) {
                    return Err(holds_nothing(
                        "this session scrolls in one run of `wlrctl pointer scroll`, so it \
                         does not spread a scroll over time",
                    ));
                }
                // `wlrctl pointer scroll` takes the vertical amount first.
                Ok(vec![format!(
                    "/dispatch exec wlrctl pointer scroll {dy} {dx}"
                )])
            }
            Verb::Shot { path, screen } => {
                self.needs("grim", "writes a screenshot")?;
                let output = match screen {
                    Some(screen) => format!("-o {} ", quoted(screen)),
                    None => String::new(),
                };
                Ok(vec![format!(
                    "/dispatch exec grim {output}{}",
                    quoted(path)
                )])
            }
            Verb::Status => Err(DeskError::Refused(Refusal::new(
                refusal::UNSUPPORTED,
                "a Hyprland session has no hand tracking to report",
            ))),
            Verb::Unknown => Err(DeskError::Refused(Refusal::new(
                refusal::UNKNOWN_TYPE,
                "this client does not know the verb it was asked to send",
            ))),
        }
    }

    fn reply(&self, verb: &Verb, answers: &[String]) -> Result<Reply, DeskError> {
        let first = answers.first().map(String::as_str).unwrap_or_default();
        match verb {
            Verb::List => Ok(Reply::Windows {
                windows: windows(&read(first, "the windows")?),
            }),
            Verb::Screens => Ok(Reply::Screens {
                screens: screens(&read(first, "the screens")?),
            }),
            Verb::Focused => Ok(Reply::Focused {
                window: window(&read(first, "the focused window")?),
            }),
            _ => Ok(dispatched(answers)),
        }
    }

    fn reading_requests(&self) -> Vec<String> {
        vec!["j/monitors".to_string(), "j/workspaces".to_string()]
    }

    fn reading(&self, answers: &[String]) -> Result<Reading, DeskError> {
        let monitors = read(
            answers.first().map(String::as_str).unwrap_or_default(),
            "the screens",
        )?;
        let workspaces = read(
            answers.get(1).map(String::as_str).unwrap_or_default(),
            "the desks",
        )?;
        let focused = monitors
            .as_array()
            .and_then(|monitors| {
                monitors
                    .iter()
                    .find(|monitor| monitor.get("focused").and_then(Value::as_bool) == Some(true))
            })
            .map(|monitor| text(monitor, "name").to_string())
            .filter(|name| !name.is_empty());
        let mut desks: Vec<DeskRow> = workspaces
            .as_array()
            .map(|desks| {
                desks
                    .iter()
                    .map(|desk| DeskRow {
                        id: number(desk, "id"),
                        screen: text(desk, "monitor").to_string(),
                        windows: number(desk, "windows"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        desks.sort_by_key(|desk| desk.id);
        Ok(Reading {
            screens: screens(&monitors),
            focused,
            desks,
        })
    }
}

/// The exec request an `open` makes. A session starts a program; it shows
/// no file, so a caller that wants a viewer names the command that draws
/// one.
fn open(
    command: Option<&str>,
    path: Option<&str>,
    desk: Option<u32>,
    silent: bool,
) -> Result<Vec<String>, DeskError> {
    match (command, path) {
        (Some(command), None) => {
            let prefix = match (desk, silent) {
                (Some(desk), true) => format!("[workspace {desk} silent] "),
                (Some(desk), false) => format!("[workspace {desk}] "),
                (None, _) => String::new(),
            };
            Ok(vec![format!("/dispatch exec {prefix}{command}")])
        }
        (None, Some(_)) => Err(DeskError::unsupported(
            "this session shows no file of its own, so an `open` names the command that draws \
             one",
        )),
        _ => Err(DeskError::malformed(
            "an `open` names exactly one of a command and a path",
        )),
    }
}

/// The dispatches a `shape` makes, in the order a session applies them:
/// whether the window tiles, where it sits and how big it is, how it is
/// drawn, and whether it shows on every desk.
fn shape(handle: &Selector, shape: Shape) -> Result<Vec<String>, DeskError> {
    let Shape {
        float,
        pin,
        at,
        size,
        aspect,
        border,
        shadow,
    } = shape;
    let held = selector(handle);
    let mut out = Vec::new();
    match float {
        Some(true) => out.push(format!("/dispatch setfloating {held}")),
        Some(false) => out.push(format!("/dispatch settiled {held}")),
        None => {}
    }
    if let Some(size) = size {
        out.push(format!(
            "/dispatch resizewindowpixel exact {} {},{held}",
            size.width, size.height
        ));
    }
    if let Some(at) = at {
        out.push(format!(
            "/dispatch movewindowpixel exact {} {},{held}",
            at.x, at.y
        ));
    }
    if let Some(aspect) = aspect {
        out.push(format!(
            "/dispatch setprop {held} keep_aspect_ratio {}",
            flag(aspect)
        ));
    }
    if let Some(border) = border {
        if let Some(size) = border.size {
            out.push(format!("/dispatch setprop {held} border_size {size}"));
        }
        if let Some(rounding) = border.rounding {
            out.push(format!("/dispatch setprop {held} rounding {rounding}"));
        }
    }
    if let Some(shadow) = shadow {
        out.push(format!(
            "/dispatch setprop {held} no_shadow {}",
            flag(!shadow)
        ));
    }
    match pin {
        // A session's `pin` toggles, so it sets a window pinned and cannot
        // clear one by name.
        Some(true) => out.push(format!("/dispatch pin {held}")),
        Some(false) => {
            return Err(DeskError::unsupported(
                "this session's `pin` toggles, so it cannot unpin a window by name",
            ));
        }
        None => {}
    }
    Ok(out)
}

/// The request a `key` makes.
///
/// A session runs a bind when a key on the keyboard presses its chord and
/// has no request that presses one, so a chord that is a launcher row of
/// the shared bind table starts the launcher's command, and any other
/// chord goes to the focused window with `sendshortcut`, which delivers
/// the keys and runs no bind. A layout chord such as `super+f` reaches the
/// window on a session, where the compositor answers it itself.
fn key(chord: &Chord) -> Result<Vec<String>, DeskError> {
    let mods = coder_binds::Mods {
        super_key: chord.super_key,
        shift: chord.shift,
        ctrl: chord.ctrl,
        alt: chord.alt,
    };
    if let Some(key) = bind_key(&chord.key)
        && let Some(coder_binds::Action::Exec { command, .. }) =
            coder_binds::find(coder_binds::Surface::Compositor, mods, key)
    {
        return Ok(vec![format!("/dispatch exec {command}")]);
    }
    let held: Vec<&str> = [
        (chord.super_key, "SUPER"),
        (chord.shift, "SHIFT"),
        (chord.ctrl, "CTRL"),
        (chord.alt, "ALT"),
    ]
    .into_iter()
    .filter_map(|(held, word)| held.then_some(word))
    .collect();
    Ok(vec![format!(
        "/dispatch sendshortcut {}, {}",
        held.join(" "),
        chord.key
    )])
}

/// The key of the shared bind table one chord key names, or nothing for a
/// key the table has no row for.
fn bind_key(key: &str) -> Option<coder_binds::Key> {
    let mut letters = key.chars();
    Some(match (letters.next(), letters.next()) {
        (Some(letter), None) if letter.is_ascii_lowercase() => coder_binds::Key::Char(letter),
        (Some(digit), None) if ('1'..='9').contains(&digit) => {
            coder_binds::Key::Digit(digit as u8 - b'0')
        }
        _ => match key {
            "return" | "enter" => coder_binds::Key::Return,
            "space" => coder_binds::Key::Space,
            "tab" => coder_binds::Key::Tab,
            "left" => coder_binds::Key::Arrow(coder_binds::Dir::Left),
            "right" => coder_binds::Key::Arrow(coder_binds::Dir::Right),
            "up" => coder_binds::Key::Arrow(coder_binds::Dir::Up),
            "down" => coder_binds::Key::Arrow(coder_binds::Dir::Down),
            _ => return None,
        },
    })
}

/// The refusal a verb that needs a held button or a held key makes here.
/// The programs this session drives with press and release in one run.
fn holds_nothing(why: &str) -> DeskError {
    DeskError::Refused(Refusal::new(refusal::UNSUPPORTED, why))
}

/// The word `wlrctl pointer click` takes for a button.
fn button_word(button: Button) -> &'static str {
    button.word()
}

/// One argument for the shell the session runs an `exec` with, in single
/// quotes, with every quote inside it closed, escaped, and reopened.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// A property a session reads as `0` or `1`.
fn flag(on: bool) -> u8 {
    u8::from(on)
}

/// The string a request carries for one selector.
///
/// A session matches a `class:` selector as a regular expression, so an
/// app-id becomes one that matches it and nothing else, with every
/// character the syntax holds escaped. This is the one place in the
/// repository that builds one.
fn selector(handle: &Selector) -> String {
    match handle {
        Selector::Address(address) => format!("address:{address}"),
        Selector::Class(class) => format!("class:^({})$", escaped(class)),
        Selector::Title(title) => format!("title:{}", one_line(title)),
    }
}

/// One app-id as a regular expression that matches it literally.
fn escaped(class: &str) -> String {
    const SYNTAX: &str = r#".^$|()[]{}*+?\"#;
    let mut out = String::with_capacity(class.len());
    for character in class.chars() {
        if SYNTAX.contains(character) {
            out.push('\\');
        }
        out.push(character);
    }
    out
}

/// One line of a request: a session reads a request to its newline, so a
/// newline in a title or a notice would make it two.
fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

/// What a session answered a change verb with. It says `ok` or says why
/// not, and a window that is not there is the usual why not.
fn dispatched(answers: &[String]) -> Reply {
    for answer in answers {
        let answer = answer.trim();
        if answer != "ok" {
            return Reply::Refused(Refusal::new(refusal::UNSUPPORTED, answer));
        }
    }
    Reply::Done
}

/// One JSON answer, read.
fn read(answer: &str, what: &str) -> Result<Value, DeskError> {
    serde_json::from_str(answer).map_err(|error| {
        DeskError::Unreadable(format!(
            "the session answered {what} with something this client cannot read: {error}"
        ))
    })
}

/// Every window a session holds. A window it has not mapped is not on a
/// screen, so it is not a window this answers.
fn windows(clients: &Value) -> Vec<Window> {
    clients
        .as_array()
        .map(|windows| {
            windows
                .iter()
                .filter(|window| {
                    window
                        .get("mapped")
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
                })
                .filter_map(window)
                .collect()
        })
        .unwrap_or_default()
}

/// One window, or none when the session named none. A session answers `{}`
/// for the focused window when nothing has the focus.
fn window(value: &Value) -> Option<Window> {
    if !value.is_object() || value.as_object().map(|held| held.is_empty()) == Some(true) {
        return None;
    }
    let (x, y) = pair(value, "at");
    let (width, height) = pair(value, "size");
    Some(Window {
        handle: text(value, "address").to_string(),
        app_id: text(value, "class").to_string(),
        title: text(value, "title").to_string(),
        pid: value.get("pid").and_then(Value::as_i64),
        screen: text(value, "monitor").to_string(),
        desk: desk_of(value),
        at: Point { x, y },
        size: Size { width, height },
        floating: flagged(value, "floating"),
        pinned: flagged(value, "pinned"),
        fullscreen: number(value, "fullscreen") != 0,
    })
}

/// Every screen a session holds.
fn screens(monitors: &Value) -> Vec<Screen> {
    monitors
        .as_array()
        .map(|monitors| {
            monitors
                .iter()
                .map(|monitor| Screen {
                    name: text(monitor, "name").to_string(),
                    at: Point {
                        x: number(monitor, "x"),
                        y: number(monitor, "y"),
                    },
                    size: Size {
                        width: number(monitor, "width"),
                        height: number(monitor, "height"),
                    },
                    scale: monitor.get("scale").and_then(Value::as_f64).unwrap_or(1.0),
                    desk: monitor
                        .get("activeWorkspace")
                        .map(|desk| clamped(number(desk, "id")))
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The desk a window sits on.
fn desk_of(value: &Value) -> u32 {
    value
        .get("workspace")
        .map(|desk| clamped(number(desk, "id")))
        .unwrap_or_default()
}

/// One desk number as the contract holds it. A session numbers its own
/// scratch desks below zero, and the contract counts from one.
fn clamped(id: i64) -> u32 {
    u32::try_from(id).unwrap_or_default()
}

/// One flag of a JSON object, or false.
fn flagged(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// One number of a JSON object, or zero.
fn number(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or_default()
}

/// One pair of numbers, such as a position or a size.
fn pair(value: &Value, key: &str) -> (i64, i64) {
    let list = value.get(key).and_then(Value::as_array);
    let at = |index: usize| {
        list.and_then(|list| list.get(index))
            .and_then(Value::as_i64)
            .unwrap_or_default()
    };
    (at(0), at(1))
}

/// One string of a JSON object, or the empty string.
fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

#[cfg(test)]
#[path = "hyprland_tests.rs"]
mod tests;
