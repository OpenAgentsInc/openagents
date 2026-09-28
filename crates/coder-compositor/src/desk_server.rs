//! What the compositor answers the desk protocol's verbs with.
//!
//! The socket thread reads a request and hands it here as a [`Call`]; this
//! is the half that runs on the event loop and reads the layout.

use std::path::Path;

use coder_desk::protocol::{
    Button, Chord, Hands as HandsStatus, Motion, Point, Refusal, Screen, Selector, Size, Stroke,
    Window, refusal,
};
use coder_wm::WinId;

use crate::layout::Placed;
use crate::state::Coder;
use coder_desk::serve::{Desk, Drag, Held, Open, Scroll, Shape};

/// What an `open` asked for the window the command is about to make.
#[derive(Clone, Copy, Debug)]
pub struct Pending {
    /// The process the compositor started.
    pub pid: u32,
    /// The desk the window belongs on.
    pub desk: Option<u32>,
    /// The window opens without taking the focus.
    pub silent: bool,
}

impl Coder {
    /// The window one selector names.
    pub fn resolve(&self, selector: &Selector) -> Result<WinId, Refusal> {
        let found = match selector {
            Selector::Address(address) => address_id(address).filter(|id| self.tile(*id).is_some()),
            Selector::Class(class) => self
                .tiles
                .iter()
                .map(|tile| tile.id)
                .find(|id| &self.app_id(*id) == class),
            Selector::Title(title) => self
                .tiles
                .iter()
                .map(|tile| tile.id)
                .find(|id| &self.title(*id) == title),
        };
        found.ok_or_else(|| {
            Refusal::new(
                refusal::NO_SUCH_WINDOW,
                format!("no window on this desk answers to {selector}"),
            )
        })
    }

    /// The row a `list` answer prints for one tile of one desk.
    fn row(&self, desk: usize, tile: coder_wm::Tile) -> Window {
        let placed = self.placed(desk, tile);
        Window {
            handle: handle(tile.id),
            app_id: self.app_id(tile.id),
            title: self.title(tile.id),
            pid: self.tile(tile.id).and_then(|held| held.pid),
            screen: self
                .home(desk)
                .map(|head| head.name.clone())
                .unwrap_or_default(),
            desk: desk as u32,
            at: Point {
                x: placed.x as i64,
                y: placed.y as i64,
            },
            size: Size {
                width: placed.width as i64,
                height: placed.height as i64,
            },
            floating: tile.floating,
            pinned: self.manager.is_pinned(tile.id),
            fullscreen: self.fills_screen(desk, tile.id),
        }
    }

    /// Moves one window to one desk without moving the screen to it.
    fn move_to_desk(&mut self, id: WinId, desk: u32) -> Result<(), Refusal> {
        if !(1..=9).contains(&desk) {
            return Err(Refusal::new(
                refusal::MALFORMED,
                format!("a desk is 1 through 9, and the request named {desk}"),
            ));
        }
        let home = self.manager.workspace();
        self.manager.focus_id(id);
        self.manager.movetoworkspace(desk as usize - 1);
        self.manager.switch_workspace(home);
        self.after_layout();
        Ok(())
    }
}

impl Desk for Coder {
    fn windows(&mut self) -> Vec<Window> {
        self.manager
            .all_tiles()
            .into_iter()
            .map(|(desk, tile)| self.row(desk, tile))
            .collect()
    }

    fn screens(&mut self) -> Vec<Screen> {
        self.screens
            .heads()
            .iter()
            .map(|head| Screen {
                name: head.name.clone(),
                at: Point {
                    x: i64::from(head.at.0),
                    y: i64::from(head.at.1),
                },
                size: Size {
                    width: i64::from(head.mode.width),
                    height: i64::from(head.mode.height),
                },
                scale: head.scale,
                desk: head.desk as u32 + 1,
            })
            .collect()
    }

    fn focused(&mut self) -> Option<Window> {
        let id = self.manager.focus()?;
        let desk = self.desk_of(id)?;
        let tile = self
            .manager
            .all_tiles()
            .into_iter()
            .find(|(_, tile)| tile.id == id)
            .map(|(_, tile)| tile)?;
        Some(self.row(desk, tile))
    }

    fn open(&mut self, open: Open) -> Result<(), Refusal> {
        let Some(command) = open.command else {
            return Err(Refusal::new(
                refusal::UNSUPPORTED,
                "this compositor has no file viewer, so an open request needs a command",
            ));
        };
        if let Some(desk) = open.desk
            && !(1..=9).contains(&desk)
        {
            return Err(Refusal::new(
                refusal::MALFORMED,
                format!("a desk is 1 through 9, and the request named {desk}"),
            ));
        }
        self.start_xwayland();
        let pid = crate::exec::spawn(&command, &self.session)
            .map_err(|err| Refusal::new(refusal::UNSUPPORTED, err))?;
        self.pending.push(Pending {
            pid,
            desk: open.desk,
            silent: open.silent,
        });
        Ok(())
    }

    fn focus(&mut self, handle: &Selector) -> Result<(), Refusal> {
        let id = self.resolve(handle)?;
        self.manager.focus_id(id);
        // The window named comes over the floats of its layer whether or
        // not it had the focus already, which is what a launcher that
        // finds its window open asks for.
        self.manager.raise(id);
        self.after_layout();
        Ok(())
    }

    fn place(&mut self, handle: &Selector, desk: u32) -> Result<(), Refusal> {
        let id = self.resolve(handle)?;
        self.move_to_desk(id, desk)
    }

    fn raise(&mut self, handle: &Selector) -> Result<(), Refusal> {
        let id = self.resolve(handle)?;
        // The layout raises a float over the other floats of its layer,
        // and the stack is put back from the layout, so a raised float
        // never goes over a pinned one and a raised tile moves nothing. A
        // raise that moves nothing touches nothing, so the log stays quiet
        // for a script that raises its window on every pass.
        if !crate::stacking::raise_moves(&self.stack_windows(), id) {
            return Ok(());
        }
        self.manager.raise(id);
        self.restack();
        Ok(())
    }

    fn close(&mut self, handle: &Selector) -> Result<(), Refusal> {
        let id = self.resolve(handle)?;
        self.close_window(id);
        Ok(())
    }

    fn shape(&mut self, handle: &Selector, shape: Shape) -> Result<(), Refusal> {
        let id = self.resolve(handle)?;
        // The request changes the window it names and nothing else. The
        // desk you look at, the window that has the focus, and the order
        // of the floats stay as they are, and a request that changes
        // nothing arranges nothing: the strip under the camera shapes and
        // pins itself on every pass, and each pass used to take the
        // keyboard from the window you were typing into and raise the
        // strip over the circle.
        let home = self.manager.workspace();
        let focus = self.manager.focus();
        let before = (self.manager.all_tiles(), self.manager.is_pinned(id));
        if let Some(float) = shape.float {
            self.manager.set_floating(id, float);
        }
        if let Some(pin) = shape.pin {
            self.manager.set_pinned(id, pin);
        }
        // The aspect ratio, the border, and the shadow join the effects the
        // rule table put in force, the way a rule read does. The border is
        // drawn at one pixel or not at all, the corners are square, and
        // the ratio and the shadow are recorded: `crate::rules` says why.
        if let Some(tile) = self.tile_mut(id) {
            if let Some(keep) = shape.aspect {
                tile.effects.keep_aspect = keep;
            }
            if let Some(size) = shape.border.and_then(|border| border.size) {
                tile.effects.border = Some(size.clamp(0, i32::MAX as i64) as i32);
            }
            if let Some(shadow) = shape.shadow {
                tile.effects.shadow = Some(shadow);
            }
        }
        if shape.at.is_some() || shape.size.is_some() {
            let desk = self.desk_of(id).unwrap_or(home + 1);
            if let Some(tile) = self.tile_rect(id) {
                let placed = self.placed(desk, tile);
                if tile.floating {
                    // A float goes to the pixels asked for, through the
                    // inverse of the layout's placement, so `list` reads
                    // back the corner and the size the request named. The
                    // rectangle is set whole rather than moved by a delta:
                    // a delta is clamped against the size the window has
                    // when it is applied, which after a scale change is not
                    // the size asked for.
                    let target = Placed {
                        x: shape.at.map_or(placed.x, |at| at.x as i32),
                        y: shape.at.map_or(placed.y, |at| at.y as i32),
                        width: shape.size.map_or(placed.width, |size| size.width as i32),
                        height: shape.size.map_or(placed.height, |size| size.height as i32),
                    };
                    let rect = self.normalized(desk, target);
                    self.manager.place_float(id, rect);
                } else if let Some(size) = shape.size {
                    // A tiled window resizes along the drag the size asks
                    // for, and a move does nothing to it. The layout
                    // resizes the focused window, so the focus visits it
                    // and comes back below.
                    let screen = self.home_size(desk);
                    let dx = (size.width as i32 - placed.width) as f32 / screen.width.max(1) as f32;
                    let dy =
                        (size.height as i32 - placed.height) as f32 / screen.height.max(1) as f32;
                    self.manager.focus_id(id);
                    self.manager.resize_grab(dx, dy, false, false);
                }
            }
        }
        // Pinning brings a window to the desk you look at and focuses it
        // there, and the tiled resize above focused the window it resized:
        // the focus goes back to the window that had it.
        if let Some(focus) = focus {
            self.manager.focus_id(focus);
        }
        self.manager.switch_workspace(home);
        if (self.manager.all_tiles(), self.manager.is_pinned(id)) != before {
            self.after_layout();
        }
        Ok(())
    }

    fn scale(&mut self, screen: &str, scale: f64) -> Result<(), Refusal> {
        scale_screen(self, screen, scale)
    }

    fn notice(&mut self, text: &str) -> Result<(), Refusal> {
        self.raise_notice(text);
        Ok(())
    }

    fn key(&mut self, chord: &Chord) -> Result<(), Refusal> {
        crate::drive::key(self, chord)
    }

    fn type_text(&mut self, text: &str) -> Result<(), Refusal> {
        crate::drive::type_text(self, text)
    }

    fn click(&mut self, at: Point, button: Button) -> Result<(), Refusal> {
        crate::drive::click(self, at.x, at.y, button)
    }

    fn move_pointer(&mut self, at: Point, motion: Motion) -> Result<(), Refusal> {
        crate::drive::move_pointer(self, at.x, at.y, motion)
    }

    /// Holds a pointer button down where the pointer is, or lets it go.
    /// The press runs the bind table's mouse rows first, so a press with
    /// Super held starts a drag rather than reaching the client.
    fn button(&mut self, button: Button, held: Held) -> Result<(), Refusal> {
        crate::drive::button_held(self, button, held);
        Ok(())
    }

    fn stroke(&mut self, stroke: &Stroke, held: Held) -> Result<(), Refusal> {
        crate::drive::stroke_held(self, stroke, held)
    }

    fn drag(&mut self, drag: Drag) -> Result<(), Refusal> {
        crate::drive::drag(self, drag)
    }

    fn scroll(&mut self, scroll: Scroll) -> Result<(), Refusal> {
        crate::drive::scroll(self, scroll)
    }

    /// Keeps a screenshot for its screen's next frame, which is where the
    /// pixels come from. Both backends take a `shot` off the socket's
    /// calls before the dispatch reaches here, so the caller hears once
    /// the file is written; a caller that reached this method without a
    /// loop hears now, and the file is written on the next frame.
    fn shot(&mut self, path: &Path, screen: Option<&str>) -> Result<(), Refusal> {
        let shot = crate::drive::queue(self, &path.to_string_lossy(), screen)?;
        self.drive.push_shot(shot);
        Ok(())
    }

    /// Whether a tracked hand drives the desk, which Super+H and the host's
    /// grant both set.
    fn status(&mut self) -> HandsStatus {
        HandsStatus {
            on: self.hands.is_on(),
        }
    }

    fn reload(&mut self) -> Result<(), Refusal> {
        // The compositor reads nothing from the host while it runs. The
        // bind table and the window rules are compiled in, and the keyboard
        // layout is read from the environment at start, so a rebuild's
        // change to those reaches the next login.
        log::info!(
            "a reload has nothing to re-read here; the bind table, the window rules, and the \
             keyboard layout are read when the compositor starts, so a change to them reaches \
             the next login"
        );
        Ok(())
    }
}

/// What the compositor answers a `scale` request with, over anything that
/// holds screens and can set one's scale.
pub trait Scales {
    /// The names of the screens, left to right.
    fn screen_names(&self) -> Vec<String>;
    /// Sets one screen's scale and answers the scale it took.
    fn set_scale(&mut self, screen: &str, scale: f64) -> Result<f64, String>;
}

impl Scales for Coder {
    fn screen_names(&self) -> Vec<String> {
        self.screens
            .heads()
            .iter()
            .map(|head| head.name.clone())
            .collect()
    }

    fn set_scale(&mut self, screen: &str, scale: f64) -> Result<f64, String> {
        self.set_output_scale(screen, scale)
    }
}

/// Sets one screen's scale, refusing a screen no monitor drives and a
/// scale out of range.
pub fn scale_screen(desk: &mut dyn Scales, screen: &str, scale: f64) -> Result<(), Refusal> {
    let names = desk.screen_names();
    if !names.iter().any(|name| name == screen) {
        let list = if names.is_empty() {
            "none".to_string()
        } else {
            names.join(", ")
        };
        return Err(Refusal::new(
            refusal::NO_SUCH_SCREEN,
            format!("no screen is named {screen}; the screens are {list}"),
        ));
    }
    desk.set_scale(screen, scale)
        .map(|_| ())
        .map_err(|err| Refusal::new(refusal::MALFORMED, err))
}

impl Coder {
    /// The layout crate's rectangle for one window, wherever it sits.
    fn tile_rect(&self, id: WinId) -> Option<coder_wm::Tile> {
        self.manager
            .all_tiles()
            .into_iter()
            .find(|(_, tile)| tile.id == id)
            .map(|(_, tile)| tile)
    }
}

/// The handle a `list` answer prints for one window, in the shape the
/// Hyprland session printed addresses in.
pub fn handle(id: WinId) -> String {
    format!("0x{:x}", id.0)
}

/// The window one printed handle names.
pub fn address_id(address: &str) -> Option<WinId> {
    let digits = address.strip_prefix("0x").unwrap_or(address);
    u64::from_str_radix(digits, 16).ok().map(WinId)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Screen as Size2;
    use crate::screens::Screens;
    use coder_desk::protocol::{Reply, Request, Verb};
    use coder_desk::serve::{Desk, Open, answer};

    /// Two screens and nothing else, which is what a `scale` request and a
    /// `screens` request read, with the `reload` requests it answered.
    #[derive(Default)]
    struct TwoScreens {
        screens: Screens,
        reloads: u64,
    }

    impl TwoScreens {
        fn new() -> TwoScreens {
            let mut screens = Screens::default();
            screens.add(
                "DP-2",
                Size2 {
                    width: 2560,
                    height: 1440,
                },
                1.0,
            );
            screens.add(
                "DP-3",
                Size2 {
                    width: 1920,
                    height: 1080,
                },
                1.0,
            );
            TwoScreens {
                screens,
                reloads: 0,
            }
        }
    }

    impl Scales for TwoScreens {
        fn screen_names(&self) -> Vec<String> {
            self.screens
                .heads()
                .iter()
                .map(|head| head.name.clone())
                .collect()
        }

        fn set_scale(&mut self, screen: &str, scale: f64) -> Result<f64, String> {
            self.screens.set_scale(screen, scale)
        }
    }

    impl Desk for TwoScreens {
        fn windows(&mut self) -> Vec<Window> {
            Vec::new()
        }

        fn screens(&mut self) -> Vec<Screen> {
            self.screens
                .heads()
                .iter()
                .map(|head| Screen {
                    name: head.name.clone(),
                    at: Point {
                        x: i64::from(head.at.0),
                        y: i64::from(head.at.1),
                    },
                    size: Size {
                        width: i64::from(head.mode.width),
                        height: i64::from(head.mode.height),
                    },
                    scale: head.scale,
                    desk: head.desk as u32 + 1,
                })
                .collect()
        }

        fn focused(&mut self) -> Option<Window> {
            None
        }

        fn open(&mut self, _open: Open) -> Result<(), Refusal> {
            Ok(())
        }

        fn focus(&mut self, _handle: &Selector) -> Result<(), Refusal> {
            Ok(())
        }

        fn place(&mut self, _handle: &Selector, _desk: u32) -> Result<(), Refusal> {
            Ok(())
        }

        fn raise(&mut self, _handle: &Selector) -> Result<(), Refusal> {
            Ok(())
        }

        fn close(&mut self, _handle: &Selector) -> Result<(), Refusal> {
            Ok(())
        }

        fn shape(&mut self, _handle: &Selector, _shape: Shape) -> Result<(), Refusal> {
            Ok(())
        }

        fn scale(&mut self, screen: &str, scale: f64) -> Result<(), Refusal> {
            scale_screen(self, screen, scale)
        }

        fn notice(&mut self, _text: &str) -> Result<(), Refusal> {
            Ok(())
        }

        fn reload(&mut self) -> Result<(), Refusal> {
            self.reloads += 1;
            Ok(())
        }
    }

    fn scale_of(desk: &mut TwoScreens, name: &str) -> Option<f64> {
        match answer(Request::new(Verb::Screens), desk).reply {
            Reply::Screens { screens } => screens
                .into_iter()
                .find(|screen| screen.name == name)
                .map(|screen| screen.scale),
            _ => None,
        }
    }

    #[test]
    fn presentation_mode_sets_a_scale_and_puts_it_back() {
        let mut desk = TwoScreens::new();
        let on = Request::new(Verb::Scale {
            screen: "DP-2".to_string(),
            scale: 1.25,
        });
        assert_eq!(answer(on, &mut desk).reply, Reply::Done);
        assert_eq!(scale_of(&mut desk, "DP-2"), Some(1.25));
        let off = Request::new(Verb::Scale {
            screen: "DP-2".to_string(),
            scale: 1.0,
        });
        assert_eq!(answer(off, &mut desk).reply, Reply::Done);
        assert_eq!(scale_of(&mut desk, "DP-2"), Some(1.0));
        assert_eq!(
            scale_of(&mut desk, "DP-3"),
            Some(1.0),
            "the other screen kept its scale"
        );
    }

    #[test]
    fn a_scaled_screen_moves_the_screen_beside_it_in_the_answer() {
        let mut desk = TwoScreens::new();
        let on = Request::new(Verb::Scale {
            screen: "DP-2".to_string(),
            scale: 1.25,
        });
        answer(on, &mut desk);
        match answer(Request::new(Verb::Screens), &mut desk).reply {
            Reply::Screens { screens } => {
                assert_eq!(screens[1].at.x, 2048);
                assert_eq!(screens[0].size.width, 2560, "the size is the mode's pixels");
            }
            other => panic!("the desk answered {other:?}"),
        }
    }

    #[test]
    fn a_scale_for_a_screen_no_monitor_drives_is_refused_by_name() {
        let mut desk = TwoScreens::new();
        let request = Request::new(Verb::Scale {
            screen: "HDMI-A-1".to_string(),
            scale: 1.25,
        });
        match answer(request, &mut desk).reply {
            Reply::Refused(refusal) => {
                assert_eq!(refusal.code, refusal::NO_SUCH_SCREEN);
                assert!(
                    refusal.message.contains("DP-2, DP-3"),
                    "{}",
                    refusal.message
                );
            }
            other => panic!("the desk answered {other:?}"),
        }
    }

    #[test]
    fn a_reload_answers_done_over_the_socket_and_the_loop_takes_it_once() {
        let path = std::env::temp_dir().join(format!(
            "coder-compositor-reload-{}.sock",
            std::process::id()
        ));
        let server = coder_desk::serve::bind_at(path.clone()).expect("the socket binds");
        let mut desk = TwoScreens::new();
        let asking = {
            let path = path.clone();
            std::thread::spawn(move || coder_desk::serve::ask(&path, Verb::Reload))
        };
        let call = server
            .calls
            .recv()
            .expect("the socket hands the loop the request");
        let reply = answer(call.request.clone(), &mut desk);
        call.answer(reply);
        let answered = asking
            .join()
            .expect("the caller returns")
            .expect("the desk answers");
        assert_eq!(answered.reply, Reply::Done);
        assert_eq!(desk.reloads, 1, "the desk answered one reload");
    }

    #[test]
    fn a_scale_out_of_range_is_refused_as_malformed() {
        let mut desk = TwoScreens::new();
        let request = Request::new(Verb::Scale {
            screen: "DP-2".to_string(),
            scale: 12.0,
        });
        match answer(request, &mut desk).reply {
            Reply::Refused(refusal) => assert_eq!(refusal.code, refusal::MALFORMED),
            other => panic!("the desk answered {other:?}"),
        }
        assert_eq!(scale_of(&mut desk, "DP-2"), Some(1.0));
    }
}
