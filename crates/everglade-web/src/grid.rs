//! The browser Grid's presence: the NIP-MV session the phone's bare Grid
//! runs ([`verse::session::Session::start_presence`] in
//! [`verse::session::BARE_WORLD`]), over the browser's WebSocket, with the
//! phone's cadence and crowd rules, name tags over heads, and a player card
//! that blocks or mutes.
//!
//! The page keeps the player's key, display name, and block and mute lists
//! in its local storage, so a reload is the same player with the same
//! lists. Query parameters choose the relay (`relay=`, the public relay by
//! default), the display name (`name=`), or no presence at all (`offline`).
//! Once a second the page logs the frame times and the players in view to
//! the console as `Grid frames {...}`.
use std::time::Duration;

use glam::{Mat4, Vec3};
use verse::crowd::Figure;
use verse::grid_frame::{FrameTimes, Timing};
use verse::identity::{self, Identity};
use verse::runtime::WorldRuntime;
use verse::session::{self, PublishIntervals, Session, Status};
use verse::ui::{Atlas, UiBatch};
use wasm_bindgen::JsValue;
use web_sys::{Storage, Window};
use web_time::Instant;

pub use crate::presence_ui::Options;
use crate::presence_ui::{Button, CARD_PAD, card_buttons, card_hit, card_rect};

/// The player's secret key, 64 hex characters.
const SECRET_KEY: &str = "openagents.grid.secret";
/// The display name the player chose.
const NAME_KEY: &str = "openagents.grid.name";
/// The block and mute lists ([`verse::blocklist::Blocklist::to_json`]).
const LISTS_KEY: &str = "openagents.grid.blocked";
/// The profile name the browser's identity carries.
const PROFILE: &str = "browser";
/// How long a returning player waits for their last pose before a fresh
/// spawn, as on the phone.
const SPAWN_WAIT: Duration = Duration::from_millis(1500);
/// Remote players are drawn this far behind the moving cadence, as on the
/// phone, so they walk continuously between sparse poses.
const PRESENCE_MARGIN: Duration = Duration::from_millis(300);
/// Players farther than this, in meters, have no tag.
const TAG_RANGE: f32 = 60.0;
/// A tag's height above the feet, in meters.
const TAG_LIFT: f32 = 2.2;
/// How far from a tag's center a click still picks its player, in CSS
/// pixels.
const TAG_REACH: f32 = 28.0;

/// The browser Grid's session and what it draws over the world.
pub struct Presence {
    session: Session,
    storage: Option<Storage>,
    spawn_pending: bool,
    timing: Timing,
    /// Whose card is open.
    card: Option<String>,
}

impl Presence {
    /// Joins the Grid through `options.relay`, or the public relay, as the
    /// player whose key the page's storage holds (made on the first visit).
    /// `None` when `options.offline` asks for no presence.
    ///
    /// # Errors
    ///
    /// Returns a message when the stored key or the session cannot be used.
    pub fn open(window: &Window, options: &Options) -> Result<Option<Self>, String> {
        if options.offline {
            return Ok(None);
        }
        let storage = window.local_storage().ok().flatten();
        let get = |key: &str| {
            storage
                .as_ref()
                .and_then(|s| s.get_item(key).ok().flatten())
        };
        let set = |key: &str, value: &str| {
            if let Some(storage) = &storage {
                let _ = storage.set_item(key, value);
            }
        };
        let identity = match get(SECRET_KEY).map(|hex| Identity::from_secret_hex(PROFILE, &hex)) {
            Some(Ok(identity)) => identity,
            _ => {
                let identity = Identity::from_secret(PROFILE, identity::random_secret())?;
                set(SECRET_KEY, &identity.secret_hex());
                identity
            }
        };
        let relay = options.relay.as_deref().unwrap_or(session::PUBLIC_RELAY);
        let mut session = Session::start_presence(identity, relay, session::BARE_WORLD)?;
        if let Some(name) = &options.name {
            set(NAME_KEY, name);
        }
        session.set_display_name(options.name.clone().or_else(|| get(NAME_KEY)).as_deref());
        let intervals = PublishIntervals::mobile();
        session.set_publish_intervals(intervals)?;
        session.crowd.set_delay(intervals.moving + PRESENCE_MARGIN);
        // Only players there now: nobody who left stays behind as a figure.
        session.crowd.set_live_only(true);
        if let Some(Ok(lists)) =
            get(LISTS_KEY).map(|text| verse::blocklist::Blocklist::from_json(&text))
        {
            session.set_blocklist(lists);
        }
        session.begin_spawn(SPAWN_WAIT);
        web_sys::console::info_1(&JsValue::from_str(&format!(
            "Grid: joining {} on {relay} as {}",
            session::BARE_WORLD,
            session.pubkey()
        )));
        Ok(Some(Self {
            session,
            storage,
            spawn_pending: true,
            timing: Timing::start(Instant::now()),
            card: None,
        }))
    }

    /// Exchanges this frame with the relay and returns the players to draw.
    /// Until the relay answers with this player's last pose (or the wait
    /// ends) the session only listens.
    pub fn tick(&mut self, runtime: &mut WorldRuntime, dt: f32) -> Vec<Figure> {
        let now = Instant::now();
        if self.spawn_pending {
            let Some(spawn) = self
                .session
                .poll_spawn(&runtime.world.blockers, runtime.zone_half())
            else {
                return Vec::new();
            };
            // A new player starts where the Grid puts everyone; only a
            // signed retained pose moves them.
            if spawn.resumed {
                let _ = runtime.set_spawn(spawn.pos, spawn.yaw);
            }
            self.spawn_pending = false;
        }
        self.session.tick_world(now, runtime);
        // Other players' avatars are solid, as on the phone.
        runtime.set_avatars(
            self.session
                .crowd
                .shown(now)
                .into_iter()
                .filter(|shown| shown.role == "avatar")
                .map(|shown| shown.pos)
                .collect::<Vec<_>>(),
        );
        self.session.crowd.figures(now, dt)
    }

    /// Records one frame; once a second logs the frame times, the players
    /// in view, and the connection to the console.
    pub fn frame_times(&mut self, dt: f32, instances: usize, render_ms: f64, players: usize) {
        let now = Instant::now();
        let Some(times) = self.timing.frame(now, dt, instances, render_ms) else {
            return;
        };
        log_frames(&times, players, &self.session);
    }

    /// The remote players whose tags show, with where their heads are.
    fn tagged(&self, runtime: &WorldRuntime) -> Vec<(String, Vec3)> {
        self.session
            .crowd
            .shown(Instant::now())
            .into_iter()
            .filter(|shown| {
                shown.role == "avatar"
                    && shown.pubkey != self.session.pubkey()
                    && shown.pos.distance(runtime.player.pos) <= TAG_RANGE
            })
            .map(|shown| (shown.pubkey, shown.pos + Vec3::Y * TAG_LIFT))
            .collect()
    }

    /// Name tags over every player in range, this player's included unless
    /// the camera is inside its head, the connection line, and the open
    /// card, laid out in CSS pixels on a `size` canvas.
    pub fn draw(
        &self,
        ui: &mut UiBatch,
        atlas: &Atlas,
        size: [f32; 2],
        runtime: &WorldRuntime,
        view_proj: Mat4,
    ) {
        let mut tags = self.tagged(runtime);
        if !runtime.first_person() {
            tags.push((
                self.session.pubkey().to_owned(),
                runtime.player.pos + Vec3::Y * TAG_LIFT,
            ));
        }
        for (pubkey, head) in tags {
            let Some([x, y]) = verse::hud::project(view_proj, size, head) else {
                continue;
            };
            let name = self.session.name_of(&pubkey);
            let width = atlas.measure(&name);
            ui.text(
                atlas,
                x - width / 2.0,
                y - atlas.line,
                &name,
                crate::theme::linear(coder_ui::coder_noir::CONTENT, 1.0),
            );
        }
        let line = self.status_line();
        ui.text(
            atlas,
            CARD_PAD,
            CARD_PAD,
            &line,
            crate::theme::linear(coder_ui::coder_noir::CONTENT_SECONDARY, 1.0),
        );
        self.draw_card(ui, atlas, size);
    }

    /// The connection, in one line: connecting, offline, full, or how
    /// many players are here.
    fn status_line(&self) -> String {
        let now = Instant::now();
        // The relay's population cap refuses this player's frames; the
        // session slows down and tries again until a player leaves.
        let full = self.session.throttled(now)
            && self
                .session
                .last_refusal()
                .is_some_and(|refusal| refusal.contains("world is full"));
        match self.session.status {
            _ if full => "THE GRID IS FULL; WAITING FOR ROOM".into(),
            Status::Connecting => "CONNECTING".into(),
            Status::Offline => "OFFLINE; RETRYING".into(),
            Status::Online => {
                let here = self.session.crowd.live_len(now) + 1;
                format!("ONLINE · {here} HERE")
            }
        }
    }

    /// The remote player whose tag is nearest `at` (CSS pixels), within
    /// [`TAG_REACH`].
    fn player_at(
        &self,
        at: [f32; 2],
        size: [f32; 2],
        line: f32,
        runtime: &WorldRuntime,
        view_proj: Mat4,
    ) -> Option<String> {
        self.tagged(runtime)
            .into_iter()
            .filter_map(|(pubkey, head)| {
                let [x, y] = verse::hud::project(view_proj, size, head)?;
                let distance = (at[0] - x).hypot(at[1] - (y - line / 2.0));
                (distance <= TAG_REACH).then_some((distance, pubkey))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, pubkey)| pubkey)
    }

    /// A press at `at` (CSS pixels): with a card open, a button acts and
    /// anywhere else closes it; otherwise a press on a name tag opens that
    /// player's card. Returns whether the press was used.
    pub fn press(
        &mut self,
        at: [f32; 2],
        size: [f32; 2],
        line: f32,
        runtime: &WorldRuntime,
        view_proj: Mat4,
    ) -> bool {
        if let Some(pubkey) = self.card.take() {
            if let Some(button) = card_hit(size, at) {
                if let Err(error) = self.act(&pubkey, button) {
                    web_sys::console::warn_1(&JsValue::from_str(&error));
                }
            }
            return true;
        }
        if let Some(pubkey) = self.player_at(at, size, line, runtime, view_proj) {
            self.card = Some(pubkey);
            return true;
        }
        false
    }

    fn act(&mut self, pubkey: &str, button: Button) -> Result<(), String> {
        let changed = match button {
            Button::Block => self.session.block(pubkey)?,
            Button::Mute if self.session.blocklist().is_muted(pubkey) => {
                self.session.unmute_player(pubkey)
            }
            Button::Mute => self.session.mute_player(pubkey)?,
            Button::Close => false,
        };
        if changed && let Some(storage) = &self.storage {
            let _ = storage.set_item(LISTS_KEY, &self.session.blocklist().to_json());
        }
        Ok(())
    }

    fn draw_card(&self, ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2]) {
        let Some(pubkey) = &self.card else {
            return;
        };
        let [x, y, w, h] = card_rect(size);
        ui.rect(
            atlas,
            x,
            y,
            w,
            h,
            crate::theme::linear(coder_ui::coder_noir::SURFACE, 0.92),
        );
        ui.frame(
            atlas,
            x,
            y,
            w,
            h,
            1.0,
            crate::theme::linear(coder_ui::coder_noir::STROKE, 1.0),
        );
        let name = self.session.name_of(pubkey);
        ui.text(
            atlas,
            x + CARD_PAD,
            y + CARD_PAD,
            &name,
            crate::theme::linear(coder_ui::coder_noir::CONTENT, 1.0),
        );
        let muted = self.session.blocklist().is_muted(pubkey);
        for (button, [bx, by, bw, bh]) in card_buttons(size) {
            ui.frame(
                atlas,
                bx,
                by,
                bw,
                bh,
                1.0,
                crate::theme::linear(coder_ui::coder_noir::STROKE, 1.0),
            );
            let label = match button {
                Button::Block => "BLOCK",
                Button::Mute if muted => "UNMUTE",
                Button::Mute => "MUTE",
                Button::Close => "CLOSE",
            };
            let width = atlas.measure(label);
            ui.text(
                atlas,
                bx + (bw - width) / 2.0,
                by + (bh - atlas.line) / 2.0,
                label,
                crate::theme::linear(coder_ui::coder_noir::CONTENT, 1.0),
            );
        }
    }
}

fn log_frames(times: &FrameTimes, players: usize, session: &Session) {
    let line = serde_json::json!({
        "at_s": times.at_s,
        "frames": times.frames,
        "instances": times.instances,
        "p50_ms": times.p50_ms,
        "p95_ms": times.p95_ms,
        "max_ms": times.max_ms,
        "render_ms": times.render_ms,
        "players_drawn": players,
        "live": session.crowd.live_len(Instant::now()),
        "status": format!("{:?}", session.status),
        "frames_published": session.frames_published(),
        "refusals": session.refusals(),
        "last_refusal": session.last_refusal(),
    });
    web_sys::console::info_1(&JsValue::from_str(&format!("Grid frames {line}")));
}
