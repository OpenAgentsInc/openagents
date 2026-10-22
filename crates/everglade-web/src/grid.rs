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
/// Players farther than this from the camera, in meters, have no tag.
const TAG_RANGE: f32 = 60.0;
/// The overview's tag range: the whole plaza (#12406).
const OVERVIEW_TAG_RANGE: f32 = 160.0;
/// The most tags drawn at once, nearest the camera first.
const MAX_TAGS: usize = 24;
/// Where a returning player's last pose still counts as free: two radii
/// and a little from everyone shown.
const RESUME_SPACING: f32 = verse_world::social::controller::RADIUS * 2.0 + 0.3;
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
    /// Whether this player is in the world yet. `/home` opens watching:
    /// it shows the others and publishes nothing until the visitor joins
    /// ([`Self::join`], #12406).
    joined: bool,
    /// `/home`, whose header holds the top left: no connection line there.
    home: bool,
    /// The Grid Foreman's key, drawn by `crate::foreman` rather than as a
    /// player, with no player tag (#12445).
    foreman: Option<String>,
}

impl Presence {
    /// Joins the Grid through `options.relay`, or the public relay, as the
    /// player whose key the page's storage holds (made on the first visit,
    /// in this browser, with no network call; the secret never leaves the
    /// page's storage). `None` when `options.offline` asks for no presence.
    /// With `watch` (`/home`), the session only listens until
    /// [`Self::join`], and the site's agents and pylons are left to the
    /// page's own layer.
    ///
    /// # Errors
    ///
    /// Returns a message when the stored key or the session cannot be used.
    pub fn open(window: &Window, options: &Options, watch: bool) -> Result<Option<Self>, String> {
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
        session.set_display_name(
            options
                .name
                .clone()
                .or_else(|| get(NAME_KEY))
                .or_else(|| page_data(window, "data-name"))
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
        let mut presence = Self {
            session,
            storage,
            spawn_pending: false,
            timing: Timing::start(Instant::now()),
            card: None,
            profiles: Default::default(),
            asked: None,
            owner_tag: Default::default(),
            owned: false,
            places: if watch {
                Default::default()
            } else {
                load_places()
            },
            joined: false,
            home: watch,
            foreman: None,
        };
        if watch {
            web_sys::console::info_1(&JsValue::from_str(&format!(
                "Grid: watching {} on {relay}",
                session::BARE_WORLD
            )));
        } else {
            presence.join(window);
        }
        Ok(Some(presence))
    }

    /// Puts this player in the world: a spawn (their last pose, when it
    /// is still free, else a free spot in the spawn area), their profile
    /// name, and, signed in, the owner tag their own key signs. Only the
    /// first call does anything.
    pub fn join(&mut self, window: &Window) {
        if self.joined {
            return;
        }
        self.joined = true;
        // `/home` learns who is signed in after it opens.
        if self.session.display_name().is_none() {
            self.session
                .set_display_name(page_data(window, "data-name").as_deref());
        }
        // Signed in: ask the site for this key's owner tag, which the
        // person's phone or computer signs (#12313).
        if let Some(csrf) = page_data(window, "data-csrf") {
            ask_owner(
                self.session.pubkey().to_owned(),
                csrf,
                self.owner_tag.clone(),
            );
        }
        self.session.begin_spawn(SPAWN_WAIT);
        self.spawn_pending = true;
        web_sys::console::info_1(&JsValue::from_str(&format!(
            "Grid: joining {} on {} as {}",
            session::BARE_WORLD,
            self.session.relay(),
            self.session.pubkey()
        )));
    }

    /// Seals `suggestion` from this player's key to the suggestion box
    /// (#12460): the box's wrap and the player's own copy, each with fresh
    /// randomness.
    ///
    /// # Errors
    ///
    /// The stored key can't be read, or the suggestion can't be sealed.
    pub fn seal_suggestion(
        &self,
        suggestion: &playtest::suggestion::Suggestion,
    ) -> Result<playtest::suggestion::Sealed, String> {
        use std::str::FromStr;
        let hex = self
            .storage
            .as_ref()
            .and_then(|s| s.get_item(SECRET_KEY).ok().flatten())
            .ok_or("This browser's Grid key isn't stored.")?;
        let secret = secp256k1::SecretKey::from_str(&hex)
            .map_err(|_| "This browser's Grid key can't be read.")?;
        let inbox = playtest::triage_key(None).ok_or("The suggestion box has no key.")?;
        let random = || {
            let hide = |n: [u8; 2]| u64::from(u16::from_le_bytes(n)) * 2;
            playtest::report::Randomness {
                wrapper: identity::random_secret(),
                seal_nonce: identity::random_bytes(),
                wrap_nonce: identity::random_bytes(),
                seal_earlier: hide(identity::random_bytes()),
                wrap_earlier: hide(identity::random_bytes()),
            }
        };
        playtest::suggestion::wrap(suggestion, &secret, &inbox, &random(), &random())
    }

    /// Publishes `event` on this player's relay connection.
    pub fn publish(&self, event: nostr::domain::Event) {
        self.session.publish(event);
    }

    /// The relay's answer to a published event, once it gave one.
    #[must_use]
    pub fn answer(&self, id: &str) -> Option<bool> {
        self.session.answer(id)
    }

    /// Whether this player is in the world ([`Self::join`]).
    pub fn joined(&self) -> bool {
        self.joined
    }

    /// Opens a live subscription of the page's own on this session's
    /// relay connection ([`Session::watch`]).
    pub fn watch(&mut self, id: &str, filters: Vec<serde_json::Value>) {
        self.session.watch(id, filters);
    }

    /// What [`Self::watch`] subscriptions received since the last call.
    pub fn take_watched(&mut self) -> Vec<nostr::domain::Event> {
        self.session.take_watched()
    }

    /// The players as this page sees them, for `verse_players`.
    pub fn players(&self, runtime: &WorldRuntime) -> serde_json::Value {
        let now = Instant::now();
        let me = self.session.pubkey();
        let others: Vec<serde_json::Value> = self
            .session
            .crowd
            .shown(now)
            .into_iter()
            .filter(|shown| shown.role == "avatar" && shown.pubkey != me)
            .map(|shown| {
                let tag = self.tag(&shown.pubkey);
                serde_json::json!({
                    "pubkey": shown.pubkey,
                    "tag": [tag.0, tag.1],
                    "pos": shown.pos.to_array(),
                })
            })
            .collect();
        let tag = self.tag(me);
        serde_json::json!({
            "joined": self.joined && !self.spawn_pending,
            "me": {
                "pubkey": me,
                "tag": [tag.0, tag.1],
                "pos": runtime.player.pos.to_array(),
            },
            "others": others,
        })
    }

    /// Whether this player's spawn is still being chosen.
    pub fn placing(&self) -> bool {
        self.spawn_pending
    }

    /// Other players' feet, as shown now.
    fn others(&self, now: Instant) -> Vec<Vec3> {
        self.session
            .crowd
            .shown(now)
            .into_iter()
            .filter(|shown| shown.role == "avatar" && shown.pubkey != self.session.pubkey())
            .map(|shown| shown.pos)
            .collect()
    }

    /// Exchanges this frame with the relay and returns the players to draw.
    /// Until the relay answers with this player's last pose (or the wait
    /// ends) the session only listens.
    /// Names the Grid Foreman's key, which this presence then leaves for
    /// `crate::foreman` to draw.
    pub fn set_foreman(&mut self, pubkey: Option<&str>) {
        if self.foreman.as_deref() != pubkey {
            self.foreman = pubkey.map(str::to_owned);
        }
    }

    /// The status line `pubkey`'s avatar state last gave.
    #[must_use]
    pub fn status_of(&self, pubkey: &str) -> Option<String> {
        self.session.status_of(pubkey).map(str::to_owned)
    }

    pub fn tick(&mut self, runtime: &mut WorldRuntime, dt: f32) -> Vec<Figure> {
        let now = Instant::now();
        if !self.joined {
            self.session.listen(now);
            return self.session.crowd.figures(now, dt);
        }
        if self.spawn_pending {
            let Some(spawn) = self
                .session
                .poll_spawn(&runtime.world.blockers, runtime.zone_half())
            else {
                return self.session.crowd.figures(now, dt);
            };
            // A returning player stands where they left, when nobody
            // stands there now; anyone else gets a free spot in the spawn
            // area, which their key picks, away from everyone shown
            // (#12406).
            let others = self.others(now);
            let free = |pos: Vec3| others.iter().all(|o| o.distance(pos) >= RESUME_SPACING);
            // Nobody appears within reach of something that opens on
            // approach, so their first keys move them (#12479).
            let game_lab = verse::zones::game_place::POSITION;
            let triggers = crate::spawn_guard::triggers([game_lab.x, game_lab.z]);
            let (pos, yaw) = if spawn.resumed
                && free(spawn.pos)
                && !crate::spawn_guard::too_close([spawn.pos.x, spawn.pos.z], &triggers)
            {
                (spawn.pos, spawn.yaw)
            } else {
                let mut blockers = runtime.world.blockers.clone();
                blockers.extend(crate::spawn_guard::keep_out(&triggers));
                let arrival = verse_world::social::arrival::pick_spawn(
                    self.session.pubkey(),
                    verse_world::social::arrival::SpawnArea::GRID,
                    &others,
                    &blockers,
                );
                (arrival.pos, arrival.yaw)
            };
            // A capture's `?at=` place stands (`web::run_grid`).
            let placed = web_sys::window()
                .and_then(|w| crate::web::query_value(&w, "at"))
                .is_some();
            if !placed {
                let _ = runtime.set_spawn(pos, yaw);
            }
            web_sys::console::info_1(&JsValue::from_str(&format!(
                "Grid: spawned at {:.2},{:.2} ({} others shown{})",
                pos.x,
                pos.z,
                others.len(),
                if spawn.resumed { ", returning" } else { "" }
            )));
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

    /// The tag over `pubkey`: its npub prefix, and its name when it has
    /// one: a verified owner's profile name (#12313), else its own
    /// frames' name, else its key's profile name.
    fn tag(&self, pubkey: &str) -> (String, Option<String>) {
        let profiles = self.profiles.borrow();
        let owner = self
            .session
            .owner_of(pubkey)
            .and_then(|owner| profiles.get(owner).and_then(Option::as_deref));
        crate::presence_ui::tag(
            owner.or(self.session.shown_name(pubkey)),
            profiles.get(pubkey).and_then(Option::as_deref),
            pubkey,
        )
    }

    /// The card's title: the name, else the npub prefix.
    fn label(&self, pubkey: &str) -> String {
        let (prefix, name) = self.tag(pubkey);
        name.unwrap_or(prefix)
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

    /// The remote players whose tags show, with where their heads are:
    /// those within range of the camera at `eye`, nearest first, at most
    /// [`MAX_TAGS`].
    fn tagged(&self, eye: Vec3) -> Vec<(String, Vec3)> {
        let range = if self.joined {
            TAG_RANGE
        } else {
            OVERVIEW_TAG_RANGE
        };
        let mut tagged: Vec<(f32, String, Vec3)> = self
            .session
            .crowd
            .shown(Instant::now())
            .into_iter()
            .filter(|shown| {
                shown.role == "avatar"
                    && shown.pubkey != self.session.pubkey()
                    && self.foreman.as_deref() != Some(shown.pubkey.as_str())
            })
            .map(|shown| {
                let head = shown.pos + Vec3::Y * TAG_LIFT;
                (head.distance(eye), shown.pubkey, head)
            })
            .filter(|(distance, _, _)| *distance <= range)
            .collect();
        tagged.sort_by(|a, b| a.0.total_cmp(&b.0));
        tagged
            .into_iter()
            .take(MAX_TAGS)
            .map(|(_, pubkey, head)| (pubkey, head))
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
        eye: Vec3,
    ) {
        let mut tags = self.tagged(eye);
        if self.joined && !runtime.first_person() {
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
        // The name sits over the head and the npub prefix over the name.
        for (pubkey, head) in tags {
            let Some([x, y]) = verse::hud::project(view_proj, size, head) else {
                continue;
            };
            let (prefix, name) = self.tag(&pubkey);
            let mut top = y - atlas.line;
            if let Some(name) = &name {
                let width = atlas.measure(name);
                ui.text(
                    atlas,
                    x - width / 2.0,
                    top,
                    name,
                    crate::theme::linear(coder_ui::coder_noir::CONTENT, 1.0),
                );
                top -= atlas.line;
            }
            let width = atlas.measure(&prefix);
            ui.text(
                atlas,
                x - width / 2.0,
                top,
                &prefix,
                crate::theme::linear(coder_ui::coder_noir::CONTENT_SECONDARY, 1.0),
            );
        }
        if !self.home {
            let line = self.status_line();
            ui.text(
                atlas,
                CARD_PAD,
                CARD_PAD,
                &line,
                crate::theme::linear(coder_ui::coder_noir::CONTENT_SECONDARY, 1.0),
            );
        }
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
                let here = self.session.crowd.live_len(now) + usize::from(self.joined);
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
        view_proj: Mat4,
        eye: Vec3,
    ) -> Option<String> {
        self.tagged(eye)
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
        view_proj: Mat4,
        eye: Vec3,
    ) -> bool {
        if let Some(pubkey) = self.card.take() {
            if let Some(button) = card_hit(size, at) {
                if let Err(error) = self.act(&pubkey, button) {
                    web_sys::console::warn_1(&JsValue::from_str(&error));
                }
            }
            return true;
        }
        if let Some(pubkey) = self.player_at(at, size, line, view_proj, eye) {
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

/// A `data-` attribute the page puts on `#everglade` (`/verse`) or on the
/// canvas (`/home`, once it knows who is signed in); `None` when blank.
fn page_data(window: &Window, name: &str) -> Option<String> {
    let document = window.document()?;
    ["everglade", "everglade-canvas"]
        .into_iter()
        .filter_map(|id| document.get_element_by_id(id))
        .find_map(|element| element.get_attribute(name))
        .filter(|value| !value.trim().is_empty())
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
