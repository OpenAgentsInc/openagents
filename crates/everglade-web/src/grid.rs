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
    /// Profile names from the site (`/verse/names`), by key; `None` while
    /// asked or when the key has no profile (#12261).
    profiles: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, Option<String>>>>,
    /// When the site was last asked for names.
    asked: Option<Instant>,
    /// The owner tag the person's own key signed for this player's key,
    /// once the site has it (#12313).
    owner_tag: std::rc::Rc<std::cell::RefCell<Option<Vec<String>>>>,
    /// Whether the session carries it yet.
    owned: bool,
    /// Agents and pylons drawn on the plaza, from the site (#12313).
    places: std::rc::Rc<std::cell::RefCell<Vec<crate::presence_ui::Place>>>,
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
        // A signed-in person's profile name, from the page (/verse).
        let profile_name = window
            .document()
            .and_then(|d| d.get_element_by_id("everglade"))
            .and_then(|e| e.get_attribute("data-name"))
            .filter(|n| !n.trim().is_empty());
        session.set_display_name(
            options
                .name
                .clone()
                .or_else(|| get(NAME_KEY))
                .or(profile_name)
                .as_deref(),
        );
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
        // Signed in: ask the site for this key's owner tag, which the
        // person's phone or computer signs (#12313).
        let owner_tag: std::rc::Rc<std::cell::RefCell<Option<Vec<String>>>> = Default::default();
        if let Some(csrf) = window
            .document()
            .and_then(|d| d.get_element_by_id("everglade"))
            .and_then(|e| e.get_attribute("data-csrf"))
        {
            ask_owner(session.pubkey().to_owned(), csrf, owner_tag.clone());
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
            profiles: Default::default(),
            asked: None,
            owner_tag,
            owned: false,
            places: load_places(),
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
        if !self.owned
            && let Some(tag) = self.owner_tag.borrow().clone()
        {
            self.session
                .set_owner_tag(Some(nostr::domain::Tag::new(tag)));
            self.owned = true;
        }
        self.session.tick_world(now, runtime);
        self.ask_names(now);
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

    /// The tag over `pubkey`: its own frames' name, else its profile's.
    fn label(&self, pubkey: &str) -> String {
        let profiles = self.profiles.borrow();
        // A verified owner's profile name comes first (#12313).
        let owner = self
            .session
            .owner_of(pubkey)
            .and_then(|owner| profiles.get(owner).and_then(Option::as_deref));
        crate::presence_ui::label(
            owner.or(self.session.shown_name(pubkey)),
            profiles.get(pubkey).and_then(Option::as_deref),
            self.session.name_of(pubkey),
        )
    }

    /// Ask the site, at most every two seconds, for the profile names of
    /// players whose frames gave none.
    fn ask_names(&mut self, now: Instant) {
        if self
            .asked
            .is_some_and(|at| now.duration_since(at) < Duration::from_secs(2))
        {
            return;
        }
        let wanted: Vec<String> = {
            let profiles = self.profiles.borrow();
            self.session
                .crowd
                .shown(now)
                .into_iter()
                .filter(|shown| shown.role == "avatar")
                .flat_map(|shown| match self.session.owner_of(&shown.pubkey) {
                    // A verified owner's profile names the avatar; else the
                    // key's own profile, when its frames gave no name.
                    Some(owner) => vec![owner.to_owned()],
                    None if self.session.shown_name(&shown.pubkey).is_none() => {
                        vec![shown.pubkey]
                    }
                    None => Vec::new(),
                })
                .chain(
                    self.session
                        .owner_of(self.session.pubkey())
                        .map(str::to_owned),
                )
                .filter(|key| !profiles.contains_key(key))
                .take(crate::presence_ui::NAMES_MAX)
                .collect()
        };
        if wanted.is_empty() {
            return;
        }
        self.asked = Some(now);
        {
            let mut profiles = self.profiles.borrow_mut();
            for key in &wanted {
                profiles.insert(key.clone(), None);
            }
        }
        let url = crate::presence_ui::names_url(&wanted);
        let profiles = self.profiles.clone();
        wasm_bindgen_futures::spawn_local(async move {
            use wasm_bindgen::JsCast;
            let Some(window) = web_sys::window() else {
                return;
            };
            let Ok(answer) =
                wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(&url)).await
            else {
                return;
            };
            let Ok(response) = answer.dyn_into::<web_sys::Response>() else {
                return;
            };
            let Ok(text) = response.text() else {
                return;
            };
            let Ok(text) = wasm_bindgen_futures::JsFuture::from(text).await else {
                return;
            };
            let names = crate::presence_ui::read_names(&text.as_string().unwrap_or_default());
            let mut profiles = profiles.borrow_mut();
            for (key, name) in names {
                profiles.insert(key, Some(name));
            }
        });
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
        // Agents and pylons, where the site's directory puts them.
        for place in self.places.borrow().iter() {
            let at = Vec3::new(place.at[0], TAG_LIFT, place.at[1]);
            if at.distance(runtime.player.pos) > TAG_RANGE * 1.5 {
                continue;
            }
            let Some([x, y]) = verse::hud::project(view_proj, size, at) else {
                continue;
            };
            let width = atlas.measure(&place.label);
            ui.text(
                atlas,
                x - width / 2.0,
                y - atlas.line,
                &place.label,
                crate::theme::linear(coder_ui::coder_noir::CONTENT, 0.7),
            );
        }
        for (pubkey, head) in tags {
            let Some([x, y]) = verse::hud::project(view_proj, size, head) else {
                continue;
            };
            let name = self.label(&pubkey);
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
        let name = self.label(pubkey);
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

/// Offer this player's key to the site and wait for its owner tag, which
/// the person's phone or computer signs; at most an hour of asking.
fn ask_owner(
    pubkey: String,
    csrf: String,
    into: std::rc::Rc<std::cell::RefCell<Option<Vec<String>>>>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        use wasm_bindgen::JsCast;
        let Some(window) = web_sys::window() else {
            return;
        };
        let call = |method: &str, body: Option<String>| {
            let init = web_sys::RequestInit::new();
            init.set_method(method);
            init.set_credentials(web_sys::RequestCredentials::SameOrigin);
            let headers = web_sys::Headers::new().ok()?;
            headers.set("content-type", "application/json").ok()?;
            headers.set("x-openagents-csrf", &csrf).ok()?;
            init.set_headers(&headers);
            if let Some(body) = body {
                init.set_body(&JsValue::from_str(&body));
            }
            Some(window.fetch_with_str_and_init("/v1/verse/key", &init))
        };
        let read = |promise: js_sys::Promise| async move {
            let response = wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .ok()?
                .dyn_into::<web_sys::Response>()
                .ok()?;
            if !response.ok() {
                return None;
            }
            let text = wasm_bindgen_futures::JsFuture::from(response.text().ok()?)
                .await
                .ok()?
                .as_string()?;
            serde_json::from_str::<serde_json::Value>(&text).ok()
        };
        let tag_of = |answer: &Option<serde_json::Value>| {
            answer
                .as_ref()
                .and_then(|a| a["auth"].as_array())
                .map(|t| {
                    t.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                })
                .filter(|t| t.len() == 4)
        };
        let offer = serde_json::json!({ "pubkey": pubkey }).to_string();
        let Some(offered) = call("POST", Some(offer)) else {
            return;
        };
        let mut answer = read(offered).await;
        for _ in 0..180 {
            if let Some(tag) = tag_of(&answer) {
                web_sys::console::info_1(&JsValue::from_str(
                    "Grid: this avatar is owned by your key",
                ));
                *into.borrow_mut() = Some(tag);
                return;
            }
            gloo_timers::future::TimeoutFuture::new(20_000).await;
            let Some(again) = call("GET", None) else {
                return;
            };
            answer = read(again).await;
        }
    });
}

/// The agents and pylons the site's directory lists, read once.
fn load_places() -> std::rc::Rc<std::cell::RefCell<Vec<crate::presence_ui::Place>>> {
    let places: std::rc::Rc<std::cell::RefCell<Vec<crate::presence_ui::Place>>> =
        Default::default();
    let into = places.clone();
    wasm_bindgen_futures::spawn_local(async move {
        use wasm_bindgen::JsCast;
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(answer) =
            wasm_bindgen_futures::JsFuture::from(window.fetch_with_str("/verse/places")).await
        else {
            return;
        };
        let Ok(response) = answer.dyn_into::<web_sys::Response>() else {
            return;
        };
        if !response.ok() {
            return;
        }
        let Ok(text) = response.text() else {
            return;
        };
        let Ok(text) = wasm_bindgen_futures::JsFuture::from(text).await else {
            return;
        };
        let found = crate::presence_ui::read_places(&text.as_string().unwrap_or_default());
        web_sys::console::info_1(&JsValue::from_str(&format!(
            "Grid: {} agents and pylons on the plaza",
            found.len()
        )));
        *into.borrow_mut() = found;
    });
    places
}
