//! The drive verbs: everything a person does at the machine.
//!
//! The compositor answers them by feeding its own input path, so a driven
//! press behaves as a press on the keyboard does: `key` runs the bind
//! filter in [`crate::input`] first, and a chord the table does not hold
//! reaches the focused window. `type` presses one key per character.
//! `click` and `move` drive the pointer at a point in the space every
//! screen shares, so the press lands on the window under the point in draw
//! order. `shot` writes a PNG of one screen from the pixels the screen's
//! next frame draws, and answers once the file is written.
//!
//! The primitives hold what the composites only tap. A pointer button goes
//! down with `press` and comes back up with `release`, a key or a modifier
//! does the same, and a `drag` is the three together: the modifiers down,
//! the button down at one point, the pointer stepped to another, and
//! everything let go. A button that goes down through
//! [`crate::input::button`] runs the bind table's mouse rows first, so a
//! driven press with Super held starts the drag [`crate::drag`] holds, the
//! same as a press from the mouse. A `scroll` sends a wheel's notches or a
//! trackpad's smooth axis.
//!
//! A motion takes the steps and the time the request names, and a drag
//! that names neither runs what a hand runs: a compositor that reads one
//! jump is not reading what a person makes, and a grab that samples the
//! motion can miss it. The steps run on the loop's own thread, so a motion
//! is held to [`coder_desk::protocol::MOST_MS`], well inside the two
//! seconds the socket answers in.
//!
//! A key is named by a keysym, and which keycode presses a keysym depends
//! on the keyboard layout the session loaded. This module compiles that
//! layout once, the layout `crate::keys` hands the seat, and reads back
//! which keycode and shift level reach each keysym. A layout that has no
//! key for what a request names is refused by name.
//!
//! The verbs exist so an agent on `ssh` can run the owner's checks in the
//! session and prove what it did with a screenshot.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use coder_desk::protocol::{
    Answer, Button, Chord, Modifier, Motion, Refusal, Reply, Stroke, Verb, refusal,
};
use coder_desk::serve::{Call, Deferred, Drag, Held, Scroll};
use smithay::backend::input::{Axis, AxisSource, KeyState};
use smithay::input::keyboard::{Keycode, Keysym, keysyms, xkb};
use smithay::input::pointer::AxisFrame;

use crate::input::Kept;
use crate::keys::Layout;
use crate::layout::Screen;
use crate::screencopy::Rows;
use crate::state::Coder;

/// The button codes the kernel gives a mouse, which are the codes a client
/// reads on `wl_pointer`.
const BTN_LEFT: u32 = 0x110;
/// The right button's code.
const BTN_RIGHT: u32 = 0x111;
/// The middle button's code.
const BTN_MIDDLE: u32 = 0x112;

/// The pixels one notch of a wheel scrolls a client that reads the smooth
/// axis rather than the notch, which is what `libinput` reports beside a
/// notch of its own.
const WHEEL_PIXELS: f64 = 15.0;

/// The v120 value one notch of a wheel carries, which is the unit
/// `wl_pointer.axis_value120` counts in.
const WHEEL_V120: i32 = 120;

/// The code a client reads for one pointer button.
fn code_of(button: Button) -> u32 {
    match button {
        Button::Left => BTN_LEFT,
        Button::Right => BTN_RIGHT,
        Button::Middle => BTN_MIDDLE,
    }
}

/// The keysym a modifier presses. A chord holds the left one of each pair,
/// which is what a keyboard sends when a person holds it down.
fn keysym_of(modifier: Modifier) -> Keysym {
    Keysym::from(match modifier {
        Modifier::Super => keysyms::KEY_Super_L,
        Modifier::Shift => keysyms::KEY_Shift_L,
        Modifier::Ctrl => keysyms::KEY_Control_L,
        Modifier::Alt => keysyms::KEY_Alt_L,
    })
}

/// The bytes one pixel of a screenshot takes: red, green, blue, and the
/// padding the renderer reads back.
const BYTES_PER_PIXEL: usize = 4;

/// What the compositor drives its own input with: the keys the session's
/// layout can press, and the screenshots waiting for a frame.
///
/// The layout is read when the first key arrives and kept, because the
/// seat reads it from the environment once, at start.
#[derive(Default)]
pub struct Driver {
    keys: Option<Keys>,
    shots: Vec<Shot>,
}

impl Driver {
    /// The keys the session's layout presses, compiled on the first use.
    fn keys(&mut self) -> Result<&Keys, Refusal> {
        if self.keys.is_none() {
            let layout = Layout::from_environment();
            self.keys =
                Some(Keys::read(&layout).map_err(|why| Refusal::new(refusal::UNSUPPORTED, why))?);
        }
        self.keys.as_ref().ok_or_else(|| {
            Refusal::new(
                refusal::UNSUPPORTED,
                "this session loaded no keyboard layout, so it presses no key",
            )
        })
    }

    /// Every screenshot waiting for one screen, taken off the list.
    pub fn take_shots(&mut self, screen: &str) -> Vec<Shot> {
        let (mine, rest): (Vec<Shot>, Vec<Shot>) = std::mem::take(&mut self.shots)
            .into_iter()
            .partition(|shot| shot.screen == screen);
        self.shots = rest;
        mine
    }

    /// Keeps one screenshot for its screen's next frame.
    pub fn push_shot(&mut self, shot: Shot) {
        self.shots.push(shot);
    }
}

/// One screenshot waiting for its screen's next frame.
pub struct Shot {
    /// The screen to write.
    screen: String,
    /// The file to write it to.
    path: PathBuf,
    /// The caller's answer, when the loop took the call. A `shot` that
    /// reached the desk through the dispatch rather than the loop is
    /// answered before the file is written, and carries none.
    answer: Option<Deferred>,
}

/// Takes a `shot` off the socket's calls, so the compositor answers it
/// after the frame that fills it, and hands every other call back for the
/// dispatch to answer.
///
/// The desk protocol answers one request once, and the pixels a screenshot
/// holds are drawn on the screen's next frame, so this verb leaves the
/// dispatch and keeps the caller's channel. Both backends call it before
/// they answer a call.
pub fn take_shot(state: &mut Coder, call: Call) -> Option<Call> {
    let Some((path, screen)) = asks_for_a_shot(&call.request.verb) else {
        return Some(call);
    };
    match queue(state, &path, screen.as_deref()) {
        Ok(mut shot) => {
            shot.answer = Some(call.deferred());
            state.drive.shots.push(shot);
        }
        Err(refusal) => call.answer(Answer::new(Reply::Refused(refusal))),
    }
    None
}

/// The file and the screen a `shot` names, or nothing for every other
/// verb.
fn asks_for_a_shot(verb: &Verb) -> Option<(String, Option<String>)> {
    match verb {
        Verb::Shot { path, screen } => Some((path.clone(), screen.clone())),
        _ => None,
    }
}

/// The screenshot one request asks for: the file to write, and the screen
/// to write, which is the screen the pointer is on when the request names
/// none.
pub fn queue(state: &Coder, path: &str, screen: Option<&str>) -> Result<Shot, Refusal> {
    if path.is_empty() {
        return Err(Refusal::new(
            refusal::MALFORMED,
            "a shot names the file to write",
        ));
    }
    let names: Vec<String> = state
        .screens
        .heads()
        .iter()
        .map(|head| head.name.clone())
        .collect();
    let screen = match screen {
        Some(named) => names
            .iter()
            .find(|name| name.as_str() == named)
            .cloned()
            .ok_or_else(|| {
                Refusal::new(
                    refusal::NO_SUCH_SCREEN,
                    format!(
                        "no screen is named {named}; the screens are {}",
                        listed(&names)
                    ),
                )
            })?,
        None => {
            let index = state
                .screens
                .head_at(state.pointer_at.x, state.pointer_at.y)
                .unwrap_or(state.screens.focused_index());
            state
                .screens
                .at(index)
                .map(|head| head.name.clone())
                .ok_or_else(|| {
                    Refusal::new(
                        refusal::NO_SUCH_SCREEN,
                        "no screen is connected, so there is nothing to write",
                    )
                })?
        }
    };
    Ok(Shot {
        screen,
        path: PathBuf::from(path),
        answer: None,
    })
}

/// Writes one screen's pixels to every screenshot waiting for it and
/// answers each caller.
///
/// `pixels` is what the backend read back, four bytes a pixel, and `rows`
/// says which way its rows run: the nested backend reads its window's
/// framebuffer from the bottom up, and the hardware backend draws a copy
/// of its own from the top down.
pub fn write_shots(shots: Vec<Shot>, pixels: &Result<Vec<u8>, String>, screen: Screen, rows: Rows) {
    for shot in shots {
        let written = match pixels {
            Ok(pixels) => write_png(&shot.path, pixels, screen, rows),
            Err(why) => Err(why.clone()),
        };
        let answer = match &written {
            Ok(()) => Answer::done(),
            Err(why) => {
                log::warn!("a shot of {} was not written: {why}", shot.screen);
                Answer::refused(
                    refusal::UNSUPPORTED,
                    format!(
                        "the screen was not written to {}: {why}",
                        shot.path.display()
                    ),
                )
            }
        };
        if let Some(deferred) = shot.answer {
            deferred.answer(answer);
        }
    }
}

/// Writes one screen's pixels to a PNG file.
///
/// The renderer reads a pixel back as red, green, blue, and padding, and
/// the file carries an opaque alpha in the padding's place.
pub fn write_png(path: &Path, pixels: &[u8], screen: Screen, rows: Rows) -> Result<(), String> {
    let width = screen.width.max(0) as usize;
    let height = screen.height.max(0) as usize;
    let row = width * BYTES_PER_PIXEL;
    if width == 0 || height == 0 {
        return Err("the screen has no pixels".to_string());
    }
    if pixels.len() < row * height {
        return Err(format!(
            "the read-back holds {} bytes and the screen needs {}",
            pixels.len(),
            row * height
        ));
    }
    let mut image = Vec::with_capacity(row * height);
    for line in 0..height {
        let source = match rows {
            Rows::BottomUp => height - 1 - line,
            Rows::TopDown => line,
        };
        let start = source * row;
        for pixel in pixels[start..start + row].chunks_exact(BYTES_PER_PIXEL) {
            image.extend_from_slice(&[pixel[0], pixel[1], pixel[2], u8::MAX]);
        }
    }
    let file = std::fs::File::create(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|err| format!("the PNG header: {err}"))?;
    writer
        .write_image_data(&image)
        .map_err(|err| format!("the PNG rows: {err}"))
}

/// Presses one chord the way a press on the keyboard presses it.
pub fn key(state: &mut Coder, chord: &Chord) -> Result<(), Refusal> {
    let press = {
        let keys = state.drive.keys()?;
        keys.chord(chord)
            .map_err(|why| Refusal::new(refusal::MALFORMED, why))?
    };
    run(state, press)
}

/// Types text into the focused window, one key press per character.
pub fn type_text(state: &mut Coder, text: &str) -> Result<(), Refusal> {
    let presses = {
        let keys = state.drive.keys()?;
        text.chars()
            .map(|letter| keys.character(letter))
            .collect::<Result<Vec<Press>, String>>()
            .map_err(|why| Refusal::new(refusal::MALFORMED, why))?
    };
    for press in presses {
        run(state, press)?;
    }
    Ok(())
}

/// Moves the pointer to a point in the space every screen shares, which
/// moves the focus with it the way a mouse does.
///
/// The motion says how it travels: one step puts the pointer there, and
/// more step it across, so a grab reads motion rather than one jump.
pub fn move_pointer(state: &mut Coder, x: i64, y: i64, motion: Motion) -> Result<(), Refusal> {
    let to = on_a_screen(state, x, y)?;
    let from = (state.pointer_at.x, state.pointer_at.y);
    let pause = motion.pause();
    let steps = steps_between(from, to, motion.steps);
    let last = steps.len().saturating_sub(1);
    for (step, at) in steps.into_iter().enumerate() {
        put_pointer(state, at);
        if step < last && !pause.is_zero() {
            std::thread::sleep(pause);
        }
    }
    Ok(())
}

/// The points one motion puts the pointer at: `count` of them, evenly
/// spaced, the last one exactly where the request named. A motion of one
/// step is the jump a `move` makes when it names no count.
fn steps_between(from: (f64, f64), to: (f64, f64), count: u32) -> Vec<(f64, f64)> {
    let count = count.max(1);
    (1..=count)
        .map(|step| {
            let part = f64::from(step) / f64::from(count);
            (
                from.0 + (to.0 - from.0) * part,
                from.1 + (to.1 - from.1) * part,
            )
        })
        .collect()
}

/// Puts the pointer at one point and tells the client under it, which is
/// one step of a motion.
fn put_pointer(state: &mut Coder, at: (f64, f64)) {
    let time = now(state);
    state.pointer_at = at.into();
    crate::input::move_pointer(state, time);
    let pointer = state.pointer.clone();
    pointer.frame(state);
}

/// Presses and releases a pointer button at a point in the space every
/// screen shares, on the window under it in draw order.
pub fn click(state: &mut Coder, x: i64, y: i64, button: Button) -> Result<(), Refusal> {
    move_pointer(state, x, y, Motion::JUMP)?;
    button_held(state, button, Held::Down);
    button_held(state, button, Held::Up);
    Ok(())
}

/// Holds a pointer button down where the pointer is, or lets it go.
///
/// The press goes through the same path a press from the mouse takes, so
/// the window under the pointer reads it in draw order and a press the
/// bind table binds to a drag takes the pointer instead of reaching the
/// client.
pub fn button_held(state: &mut Coder, button: Button, held: Held) {
    crate::input::button(state, code_of(button), held.down());
}

/// Holds one key or one modifier down, or lets it go.
///
/// A press runs the bind filter the way a press from the keyboard does, so
/// a chord finished by hand runs its action, and the modifier a caller
/// holds is the modifier a later press and a later drag read.
pub fn stroke_held(state: &mut Coder, stroke: &Stroke, held: Held) -> Result<(), Refusal> {
    let code = {
        let keys = state.drive.keys()?;
        keys.stroke_of(stroke)
            .map_err(|why| Refusal::new(refusal::MALFORMED, why))?
    };
    let kept = press_key(state, code, held);
    kept_by(state, kept)
}

/// Presses a button at one point, steps the pointer to another, and lets
/// the button go there, with the modifiers held across all of it.
pub fn drag(state: &mut Coder, drag: Drag) -> Result<(), Refusal> {
    // Both points are read before anything moves, so a drag that names a
    // point off every screen presses nothing.
    on_a_screen(state, drag.from.x, drag.from.y)?;
    on_a_screen(state, drag.to.x, drag.to.y)?;
    let modifiers: Vec<Keycode> = {
        let keys = state.drive.keys()?;
        drag.modifiers
            .held()
            .into_iter()
            .map(|modifier| keys.modifier(modifier))
            .collect::<Result<Vec<Keycode>, String>>()
            .map_err(|why| Refusal::new(refusal::MALFORMED, why))?
    };
    move_pointer(state, drag.from.x, drag.from.y, Motion::JUMP)?;
    for code in &modifiers {
        press_key(state, *code, Held::Down);
    }
    button_held(state, drag.button, Held::Down);
    let moved = move_pointer(state, drag.to.x, drag.to.y, drag.motion);
    button_held(state, drag.button, Held::Up);
    for code in modifiers.iter().rev() {
        press_key(state, *code, Held::Up);
    }
    moved
}

/// Scrolls where the pointer is: a wheel's notches, or the smooth axis a
/// trackpad reports.
///
/// A wheel sends the notch and the pixels it stands for, which is what
/// `libinput` reports for one, so a client that reads either reads the
/// same scroll. A trackpad's axis ends with the stop a finger lifting
/// sends.
pub fn scroll(state: &mut Coder, scroll: Scroll) -> Result<(), Refusal> {
    if !scroll.dx.is_finite() || !scroll.dy.is_finite() {
        return Err(Refusal::new(
            refusal::MALFORMED,
            "a scroll names how far it goes across and down",
        ));
    }
    let steps = scroll.motion.steps.max(1);
    let pause = scroll.motion.pause();
    let part = f64::from(steps);
    for step in 1..=steps {
        let frame = axis_frame(
            now(state),
            scroll.dx / part,
            scroll.dy / part,
            scroll.discrete,
        );
        send_axis(state, frame);
        if step < steps && !pause.is_zero() {
            std::thread::sleep(pause);
        }
    }
    if !scroll.discrete {
        // A finger lifts, and the client reads the axis it was following
        // as stopped.
        let mut frame = AxisFrame::new(now(state)).source(AxisSource::Finger);
        for (axis, amount) in [(Axis::Horizontal, scroll.dx), (Axis::Vertical, scroll.dy)] {
            if amount != 0.0 {
                frame = frame.stop(axis);
            }
        }
        send_axis(state, frame);
    }
    Ok(())
}

/// One scroll frame: a wheel's notch and the pixels it stands for, or the
/// pixels a trackpad reports on their own.
fn axis_frame(time: u32, dx: f64, dy: f64, discrete: bool) -> AxisFrame {
    let source = match discrete {
        true => AxisSource::Wheel,
        false => AxisSource::Finger,
    };
    let mut frame = AxisFrame::new(time).source(source);
    for (axis, amount) in [(Axis::Horizontal, dx), (Axis::Vertical, dy)] {
        if amount == 0.0 {
            continue;
        }
        match discrete {
            true => {
                frame = frame
                    .value(axis, amount * WHEEL_PIXELS)
                    .v120(axis, (amount * f64::from(WHEEL_V120)).round() as i32);
            }
            false => frame = frame.value(axis, amount),
        }
    }
    frame
}

/// Sends one scroll frame to the client under the pointer.
fn send_axis(state: &mut Coder, frame: AxisFrame) {
    let pointer = state.pointer.clone();
    pointer.axis(state, frame);
    pointer.frame(state);
}

/// Presses or releases one key on the seat's keyboard and answers what the
/// compositor kept, which is what a chord the bind table holds leaves.
fn press_key(state: &mut Coder, code: Keycode, held: Held) -> Option<Kept> {
    let time = now(state);
    let key_state = match held {
        Held::Down => KeyState::Pressed,
        Held::Up => KeyState::Released,
    };
    crate::input::press(state, code, key_state, time)
}

/// Runs one press: the modifiers go down, the key goes down and up, and
/// the modifiers come back up. A press the bind table holds runs its
/// action once the keys are back up, so the window a launcher opens takes
/// the keyboard with no modifier held.
fn run(state: &mut Coder, press: Press) -> Result<(), Refusal> {
    let time = now(state);
    for code in &press.mods {
        crate::input::press(state, *code, KeyState::Pressed, time);
    }
    let kept = crate::input::press(state, press.key, KeyState::Pressed, time);
    crate::input::press(state, press.key, KeyState::Released, time);
    for code in press.mods.iter().rev() {
        crate::input::press(state, *code, KeyState::Released, time);
    }
    kept_by(state, kept)
}

/// What a press the compositor kept asks of it: a chord runs its action,
/// and a virtual terminal switch is refused.
fn kept_by(state: &mut Coder, kept: Option<Kept>) -> Result<(), Refusal> {
    match kept {
        Some(Kept::Chord(action)) => {
            log::info!("the chord runs {action:?}");
            state.run(action);
            Ok(())
        }
        // Ctrl+Alt with a function key leaves this compositor for another
        // session on another terminal, which is a request the person at
        // the keyboard makes and a caller on a socket does not.
        Some(Kept::Vt(vt)) => Err(Refusal::new(
            refusal::UNSUPPORTED,
            format!("the desk socket does not switch to virtual terminal {vt}"),
        )),
        None => Ok(()),
    }
}

/// The point one request names, refused when no screen holds it.
fn on_a_screen(state: &Coder, x: i64, y: i64) -> Result<(f64, f64), Refusal> {
    let (x, y) = (x as f64, y as f64);
    if state.screens.head_at(x, y).is_some() {
        return Ok((x, y));
    }
    let named: Vec<String> = state
        .screens
        .heads()
        .iter()
        .map(|head| {
            let size = head.logical();
            format!(
                "{} at {},{} sized {}x{}",
                head.name, head.at.0, head.at.1, size.width, size.height
            )
        })
        .collect();
    Err(Refusal::new(
        refusal::MALFORMED,
        format!(
            "no screen holds the point {x},{y}; the screens are {}",
            listed(&named)
        ),
    ))
}

/// The milliseconds a press or a press of a button carries, counted from
/// the start the way every other event this compositor sends is.
fn now(state: &Coder) -> u32 {
    state.started.elapsed().as_millis() as u32
}

/// A list of names for a refusal to read.
fn listed(names: &[String]) -> String {
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    }
}

/// What one chord or one character presses: the modifiers to hold, and the
/// key to press while they are held.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Press {
    mods: Vec<Keycode>,
    key: Keycode,
}

/// The key that presses one keysym: its keycode, and whether Shift is held
/// to reach it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tap {
    code: Keycode,
    shift: bool,
}

/// Which key presses each keysym on the session's keyboard layout.
pub struct Keys {
    by_sym: HashMap<u32, Tap>,
}

impl Keys {
    /// Reads one layout and answers which key presses each keysym it
    /// carries, at the two shift levels a chord and a character reach: the
    /// key alone, and the key with Shift held.
    fn read(layout: &Layout) -> Result<Keys, String> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let options = if layout.options.is_empty() {
            None
        } else {
            Some(layout.options.clone())
        };
        let keymap = xkb::Keymap::new_from_names(
            &context,
            layout.rules.as_str(),
            layout.model.as_str(),
            layout.layout.as_str(),
            layout.variant.as_str(),
            options,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or_else(|| format!("the keyboard layout {} did not compile", layout.named()))?;
        let mut by_sym = HashMap::new();
        for raw in keymap.min_keycode().raw()..=keymap.max_keycode().raw() {
            let code = Keycode::new(raw);
            for level in 0..2u32 {
                for sym in keymap.key_get_syms_by_level(code, 0, level) {
                    by_sym.entry(sym.raw()).or_insert(Tap {
                        code,
                        shift: level == 1,
                    });
                }
            }
        }
        if by_sym.is_empty() {
            return Err(format!(
                "the keyboard layout {} carries no key",
                layout.named()
            ));
        }
        Ok(Keys { by_sym })
    }

    /// What one chord presses, or why this layout cannot press it.
    fn chord(&self, chord: &Chord) -> Result<Press, String> {
        let tap = self.key_of(named(&chord.key)?)?;
        let mut mods = Vec::new();
        for (held, sym, word) in [
            (chord.super_key, keysyms::KEY_Super_L, "super"),
            (chord.shift || tap.shift, keysyms::KEY_Shift_L, "shift"),
            (chord.ctrl, keysyms::KEY_Control_L, "ctrl"),
            (chord.alt, keysyms::KEY_Alt_L, "alt"),
        ] {
            if held {
                mods.push(
                    self.key_of(Keysym::from(sym))
                        .map(|found| found.code)
                        .map_err(|_| {
                            format!("the keyboard layout this session loaded has no {word} key")
                        })?,
                );
            }
        }
        Ok(Press {
            mods,
            key: tap.code,
        })
    }

    /// What one character presses. A newline presses Return and a tab
    /// presses Tab, which is what a key on the keyboard sends for them.
    fn character(&self, letter: char) -> Result<Press, String> {
        let sym = match letter {
            '\n' | '\r' => Keysym::from(keysyms::KEY_Return),
            '\t' => Keysym::from(keysyms::KEY_Tab),
            other => xkb::utf32_to_keysym(other as u32),
        };
        let tap = self.key_of(sym).map_err(|_| {
            format!("the keyboard layout this session loaded has no key for `{letter}`")
        })?;
        let mods = if tap.shift {
            vec![
                self.key_of(Keysym::from(keysyms::KEY_Shift_L))
                    .map(|found| found.code)
                    .map_err(|_| {
                        "the keyboard layout this session loaded has no shift key".to_string()
                    })?,
            ]
        } else {
            Vec::new()
        };
        Ok(Press {
            mods,
            key: tap.code,
        })
    }

    /// The key one modifier presses, or why this layout has none.
    fn modifier(&self, modifier: Modifier) -> Result<Keycode, String> {
        self.key_of(keysym_of(modifier))
            .map(|found| found.code)
            .map_err(|_| {
                format!(
                    "the keyboard layout this session loaded has no {} key",
                    modifier.word()
                )
            })
    }

    /// The key one press or one release names: a modifier, or any other
    /// key by its keysym name. A key whose symbol needs Shift is reached
    /// by holding `shift` first, so nothing is held that the caller did
    /// not ask for.
    fn stroke_of(&self, stroke: &Stroke) -> Result<Keycode, String> {
        match stroke {
            Stroke::Modifier(modifier) => self.modifier(*modifier),
            Stroke::Key(key) => self.key_of(named(key)?).map(|found| found.code),
        }
    }

    /// The key that presses one keysym.
    fn key_of(&self, sym: Keysym) -> Result<Tap, String> {
        self.by_sym.get(&sym.raw()).copied().ok_or_else(|| {
            format!(
                "the keyboard layout this session loaded has no key for `{}`",
                xkb::keysym_get_name(sym)
            )
        })
    }
}

/// The keysym one key name names, read without regard to case, so `return`
/// and `Return` are the Return key.
fn named(key: &str) -> Result<Keysym, String> {
    let sym = xkb::keysym_from_name(key, xkb::KEYSYM_CASE_INSENSITIVE);
    if sym.raw() == keysyms::KEY_NoSymbol {
        return Err(format!(
            "`{key}` names no key; a key is a letter, a digit, or a name such as `return`, \
             `escape`, `space`, `tab`, `backspace`, `left`, or `f1`"
        ));
    }
    Ok(sym)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binds::{self, Action, Mods};

    /// The keys a US layout presses, which is what a host that names no
    /// layout loads.
    fn keys() -> Keys {
        Keys::read(&Layout::read(|name| match name {
            "XKB_DEFAULT_LAYOUT" => Some("us".to_string()),
            _ => None,
        }))
        .expect("the layout compiles")
    }

    /// What the bind table answers for the chord one spelling names, read
    /// through the keysym the drive verb presses.
    fn action_of(spelled: &str) -> Option<Action> {
        let chord = Chord::parse(spelled).expect("a chord");
        let key = crate::keys::key_of_sym(named(&chord.key).expect("a key"))?;
        binds::action(
            &binds::table(None),
            binds::Chord {
                mods: Mods {
                    logo: chord.super_key,
                    shift: chord.shift,
                    ctrl: chord.ctrl,
                    alt: chord.alt,
                },
                key,
            },
        )
    }

    #[test]
    fn the_chord_a_launcher_is_bound_to_runs_its_row() {
        assert_eq!(action_of("super+t"), Some(Action::OpenShell));
        assert_eq!(action_of("super+return"), Some(Action::OpenCoder));
        assert_eq!(action_of("super+b"), Some(Action::Exec("coder-browser")));
        assert_eq!(action_of("super+1"), Some(Action::Desk(1)));
    }

    #[test]
    fn a_chord_the_table_does_not_hold_reaches_the_window() {
        assert_eq!(action_of("ctrl+c"), None);
        assert_eq!(action_of("return"), None);
        assert_eq!(action_of("escape"), None);
    }

    #[test]
    fn a_chord_presses_its_key_with_its_modifiers_held() {
        let keys = keys();
        let shell = keys
            .chord(&Chord::parse("super+t").expect("a chord"))
            .expect("a press");
        assert_eq!(shell.mods.len(), 1, "super alone is held");
        let deck = keys
            .chord(&Chord::parse("super+shift+d").expect("a chord"))
            .expect("a press");
        assert_eq!(deck.mods.len(), 2, "super and shift are held");
        let plain = keys
            .chord(&Chord::parse("return").expect("a chord"))
            .expect("a press");
        assert!(plain.mods.is_empty(), "a key alone holds no modifier");
        assert_ne!(plain.key, shell.key, "Return is not T");
    }

    #[test]
    fn a_chord_naming_a_key_the_layout_has_not_got_is_refused_by_name() {
        let keys = keys();
        let refused = keys
            .chord(&Chord {
                key: "zilch".to_string(),
                ..Chord::default()
            })
            .expect_err("no such key");
        assert!(refused.contains("zilch"), "{refused}");
    }

    #[test]
    fn a_capital_letter_types_with_shift_held_and_a_lowercase_one_without() {
        let keys = keys();
        let lower = keys.character('a').expect("a press");
        let upper = keys.character('A').expect("a press");
        assert!(lower.mods.is_empty());
        assert_eq!(upper.mods.len(), 1, "shift reaches the capital");
        assert_eq!(lower.key, upper.key, "one key carries both cases");
    }

    #[test]
    fn a_newline_types_return_and_a_tab_types_tab() {
        let keys = keys();
        let return_key = keys
            .chord(&Chord::parse("return").expect("a chord"))
            .expect("a press");
        assert_eq!(keys.character('\n').expect("a press").key, return_key.key);
        assert_eq!(keys.character('\r').expect("a press").key, return_key.key);
        let tab_key = keys
            .chord(&Chord::parse("tab").expect("a chord"))
            .expect("a press");
        assert_eq!(keys.character('\t').expect("a press").key, tab_key.key);
    }

    #[test]
    fn every_character_a_shell_line_holds_presses_a_key() {
        let keys = keys();
        for letter in "echo hi > /tmp/a.txt && ls -l 'x' \"y\" #1 (2) [3] {4}".chars() {
            assert!(
                keys.character(letter).is_ok(),
                "the layout types `{letter}`"
            );
        }
    }

    #[test]
    fn a_motion_steps_evenly_and_lands_where_the_request_named() {
        let jump = steps_between((0.0, 0.0), (100.0, 50.0), 1);
        assert_eq!(jump, vec![(100.0, 50.0)], "one step is the jump");
        let stepped = steps_between((0.0, 0.0), (100.0, 50.0), 4);
        assert_eq!(
            stepped,
            vec![(25.0, 12.5), (50.0, 25.0), (75.0, 37.5), (100.0, 50.0)]
        );
        assert_eq!(
            steps_between((10.0, 10.0), (10.0, 10.0), 8).last(),
            Some(&(10.0, 10.0)),
            "a motion that goes nowhere still ends where it started"
        );
        // A count of zero is one step rather than none, so a request that
        // reached here without the contract's reading still moves.
        assert_eq!(steps_between((0.0, 0.0), (3.0, 3.0), 0).len(), 1);
    }

    #[test]
    fn a_wheel_sends_its_notch_and_the_pixels_it_stands_for() {
        let notch = axis_frame(7, 0.0, -2.0, true);
        assert_eq!(notch.source, Some(AxisSource::Wheel));
        assert_eq!(notch.axis, (0.0, -2.0 * WHEEL_PIXELS));
        assert_eq!(notch.v120, Some((0, -2 * WHEEL_V120)));
        assert_eq!(notch.stop, (false, false));
        assert_eq!(notch.time, 7);
    }

    #[test]
    fn a_trackpad_sends_the_smooth_axis_and_no_notch() {
        let smooth = axis_frame(9, 12.0, 0.0, false);
        assert_eq!(smooth.source, Some(AxisSource::Finger));
        assert_eq!(smooth.axis, (12.0, 0.0));
        assert_eq!(smooth.v120, None, "a trackpad clicks through no notch");
    }

    #[test]
    fn the_button_codes_are_the_ones_a_client_reads() {
        assert_eq!(code_of(Button::Left), BTN_LEFT);
        assert_eq!(code_of(Button::Right), BTN_RIGHT);
        assert_eq!(code_of(Button::Middle), BTN_MIDDLE);
    }

    #[test]
    fn a_press_names_a_modifier_or_a_key_and_the_layout_presses_it() {
        let keys = keys();
        let shift = keys
            .stroke_of(&Stroke::Modifier(Modifier::Shift))
            .expect("the layout has a shift key");
        let letter = keys
            .stroke_of(&Stroke::Key("a".to_string()))
            .expect("the layout has an A key");
        assert_ne!(shift, letter, "a modifier is not the key beside it");
        assert_eq!(
            keys.stroke_of(&Stroke::Key("return".to_string()))
                .expect("the layout has a Return key"),
            keys.chord(&Chord::parse("return").expect("a chord"))
                .expect("a press")
                .key,
            "a held key is the key a chord presses"
        );
        let refused = keys
            .stroke_of(&Stroke::Key("zilch".to_string()))
            .expect_err("no such key");
        assert!(refused.contains("zilch"), "{refused}");
    }

    #[test]
    fn each_modifier_presses_a_key_of_its_own() {
        let keys = keys();
        let mut pressed = Vec::new();
        for modifier in Modifier::ALL {
            let code = keys
                .modifier(modifier)
                .unwrap_or_else(|why| panic!("the layout holds {modifier}: {why}"));
            assert!(
                !pressed.contains(&code),
                "{modifier} presses a key of its own"
            );
            pressed.push(code);
        }
    }

    #[test]
    fn a_point_off_every_screen_is_refused_with_the_screens_named() {
        let named = vec!["DP-2 at 0,0 sized 2560x1440".to_string()];
        let refusal = Refusal::new(
            refusal::MALFORMED,
            format!(
                "no screen holds the point 9000,9000; the screens are {}",
                listed(&named)
            ),
        );
        assert!(refusal.message.contains("DP-2"), "{}", refusal.message);
        assert_eq!(listed(&[]), "none");
    }

    #[test]
    fn a_shot_writes_a_png_the_rows_of_which_run_top_down() {
        let screen = Screen {
            width: 2,
            height: 2,
        };
        // Two rows: the first red, the second blue, read from the bottom
        // up the way the nested backend reads its window.
        let pixels = vec![
            0, 0, 255, 0, 0, 0, 255, 0, // the bottom row, which is blue
            255, 0, 0, 0, 255, 0, 0, 0, // the top row, which is red
        ];
        let path = std::env::temp_dir().join(format!("coder-shot-{}.png", std::process::id()));
        write_png(&path, &pixels, screen, Rows::BottomUp).expect("the file is written");
        let written = std::fs::read(&path).expect("the file reads back");
        assert_eq!(&written[..8], b"\x89PNG\r\n\x1a\n", "the file is a PNG");
        let decoder = png::Decoder::new(std::io::Cursor::new(written));
        let mut reader = decoder.read_info().expect("the PNG reads");
        let size = reader
            .output_buffer_size()
            .expect("the image fits in memory");
        let mut image = vec![0; size];
        let info = reader.next_frame(&mut image).expect("the rows read");
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(
            &image[..4],
            &[255, 0, 0, 255],
            "the top row is red, and its alpha is opaque"
        );
        assert_eq!(&image[8..12], &[0, 0, 255, 255], "the bottom row is blue");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_shot_of_a_screen_with_no_pixels_is_refused() {
        let path = std::env::temp_dir().join("coder-shot-empty.png");
        let empty = Screen {
            width: 0,
            height: 0,
        };
        assert!(write_png(&path, &[], empty, Rows::TopDown).is_err());
        let short = Screen {
            width: 4,
            height: 4,
        };
        assert!(write_png(&path, &[0; 8], short, Rows::TopDown).is_err());
    }
}
