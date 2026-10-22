//! The Grid's build billboard (#12442) in the depth-tested world pass.
//!
//! The board's frame is a static placement; its letters, strip cells, and
//! flashes are small models built here at start from
//! [`deploy_board::layout`] and placed on its face each frame, so the board
//! adds no download. The page's Grid session subscribes to NIP-DEPLOY
//! statuses and livehost pointers ([`deploy_board::filters`]) on its
//! existing relay connection, and every event is checked
//! ([`deploy_board::Ingest`]) before it moves the board.
//!
//! `?deploys=test` shows only test events, for checks that publish their
//! own (`openagents deploy event --test`).

use coder_ui::theme::Intensity;
use deploy_board::layout::{self, Art, Cell, Ink, Piece};
use deploy_board::state::Flash;
use deploy_board::{Ingest, font};
use glam::{Mat4, Quat, Vec3};
use verse::{imported::flat, palette};
use verse_engine::{
    assets::{Model, Pack, Placement, Surface, Topology, Vertex},
    inventory::{Asset, Binding, fingerprint},
    presentation::Instance,
};

/// The frame's model.
const FRAME: &str = "board/frame";
/// The shared computer's lit screen.
const SCREEN: &str = "board/screen";
/// The subscription's ID on the session's relay connection.
const SUBSCRIPTION: &str = "deploy-board";
/// The most instances a frame may hold in all (the engine refuses more
/// than 1024), so the board yields to players and the Bay.
const FRAME_INSTANCES: usize = 1000;

fn letter_key(c: u8, ink: Ink) -> String {
    format!("board/l-{}-{}", ink.name(), c)
}

fn word_key(index: usize) -> String {
    format!("board/w-{index}")
}

fn cell_key(cell: Cell) -> String {
    format!("board/cell-{}", cell.name())
}

fn flash_key(flash: Flash) -> &'static str {
    match flash {
        Flash::Live => "board/flash-live",
        Flash::Failed => "board/flash-failed",
    }
}

/// The ink's color: a step on the Grid's neutral ladder, or the product
/// palette's success and danger colors for live and failed.
fn color(ink: Ink) -> [f32; 3] {
    let step = |i: usize| palette::neutral(palette::amber(Intensity::ALL[i]));
    match ink {
        Ink::Faint => step(0),
        Ink::Quiet => step(1),
        Ink::Present => step(2),
        Ink::Bright => step(3),
        Ink::Live => palette::linear(coder_ui::coder_noir::SUCCESS),
        Ink::Fail => palette::linear(coder_ui::coder_noir::DANGER),
    }
}

/// The dark faces' tint: the terminal's near-black.
const DARK: [f32; 3] = [0.002; 3];

/// A flat surface that shares each corner among the lines and faces that
/// meet there: the Grid's start digests every vertex (#12423).
struct Shared {
    surface: Surface,
}

impl Shared {
    fn new(white: usize, tint: [f32; 3], topology: Topology) -> Self {
        Self {
            surface: flat::surface(white, tint, topology),
        }
    }

    fn at(&mut self, p: [f64; 3]) -> u32 {
        let position = [p[0] as f32, p[1] as f32, p[2] as f32];
        let vertices = &mut self.surface.vertices;
        if let Some(found) = vertices.iter().position(|v| v.position == position) {
            return found as u32;
        }
        vertices.push(Vertex {
            position,
            normal: [0., 0., 1.],
            uv: [0.5, 0.5],
            joints: [0; 4],
            weights: [1., 0., 0., 0.],
        });
        (vertices.len() - 1) as u32
    }

    fn line(&mut self, a: [f64; 3], b: [f64; 3]) {
        let (a, b) = (self.at(a), self.at(b));
        self.surface.indices.extend([a, b]);
    }

    fn quad(&mut self, corners: [[f64; 3]; 4]) {
        let [a, b, c, d] = corners.map(|p| self.at(p));
        self.surface.indices.extend([a, b, c, a, c, d]);
    }

    fn done(self, out: &mut Vec<Surface>) {
        if !self.surface.indices.is_empty() {
            out.push(self.surface);
        }
    }
}

/// A model of `art`, its points moved by `shift`.
fn model(source: &str, art: &Art, white: usize, height: f32, shift: [f64; 3]) -> Model {
    let moved = |p: [f64; 3]| [p[0] + shift[0], p[1] + shift[1], p[2] + shift[2]];
    let mut surfaces = Vec::new();
    let mut faces = Shared::new(white, DARK, Topology::Triangles);
    for corners in &art.faces {
        faces.quad(corners.map(moved));
    }
    faces.done(&mut surfaces);
    for ink in Ink::ALL {
        let mut lines = Shared::new(white, color(ink), Topology::Lines);
        for (a, b, _) in art.lines.iter().filter(|l| l.2 == ink) {
            lines.line(moved(*a), moved(*b));
        }
        lines.done(&mut surfaces);
        let mut lit = Shared::new(white, color(ink), Topology::Triangles);
        for (corners, _) in art.lit.iter().filter(|q| q.1 == ink) {
            lit.quad(corners.map(moved));
        }
        lit.done(&mut surfaces);
    }
    flat::model(source, surfaces, height)
}

/// Adds the board's models to the Grid's `pack`, whose white texel is slot
/// `white` and listed as `texture`, and stands its frame.
pub fn add(pack: &mut Pack, white: usize, texture: &str) -> Result<(), String> {
    let mut models: Vec<(String, Model)> = Vec::new();
    // The frame centered on its site, so the placement turns it in place.
    let center = [-layout::WIDTH / 2.0, 0.0, 0.0];
    let height = (layout::LIFT + layout::HEIGHT) as f32;
    models.push((
        FRAME.into(),
        model(FRAME, &layout::frame(), white, height, center),
    ));
    for &c in font::CHARSET {
        for ink in Ink::LETTERS {
            let key = letter_key(c, ink);
            let art = layout::letter(c, ink);
            models.push((key.clone(), model(&key, &art, white, 1.0, [0.0; 3])));
        }
    }
    for (i, (text, ink)) in layout::WORDS.into_iter().enumerate() {
        let key = word_key(i);
        let art = layout::word(text, ink);
        models.push((key.clone(), model(&key, &art, white, 1.0, [0.0; 3])));
    }
    models.push((
        SCREEN.into(),
        model(SCREEN, &deploy_board::screen::art(), white, 1.0, [0.0; 3]),
    ));
    for cell in Cell::ALL {
        let key = cell_key(cell);
        let art = layout::cell(cell);
        models.push((key.clone(), model(&key, &art, white, 0.6, [0.0; 3])));
    }
    for flash in [Flash::Live, Flash::Failed] {
        let key = flash_key(flash);
        models.push((
            key.into(),
            model(key, &layout::flash(flash), white, 10.0, [0.0; 3]),
        ));
    }
    let inventory = pack
        .inventory
        .as_mut()
        .ok_or("The Grid's inventory is missing")?;
    let texture = inventory
        .assets
        .iter()
        .find(|a| a.id.as_str() == texture)
        .ok_or("The Grid's white texture is not listed")?
        .id
        .clone();
    for (key, model) in models {
        let (sha256, bytes) = fingerprint(&model)?;
        inventory.assets.push(Asset {
            id: verse::imported::inventory::id(&format!("verse:model:{key}"))?,
            binding: Binding::Model { key: key.clone() },
            sha256,
            bytes,
            dependencies: vec![texture.clone()],
        });
        pack.models.insert(key, model);
    }
    let screen = screen_transform();
    let (_, screen_turn, screen_at) = screen.to_scale_rotation_translation();
    pack.placements.push(Placement {
        model: SCREEN.into(),
        position: screen_at.to_array(),
        rotation: screen_turn.to_array(),
        scale: 1.,
    });
    let rotation = Quat::from_rotation_y(layout::yaw() as f32);
    pack.placements.push(Placement {
        model: FRAME.into(),
        position: [layout::SITE[0] as f32, 0.0, layout::SITE[1] as f32],
        rotation: rotation.to_array(),
        scale: 1.,
    });
    Ok(())
}

/// The shared computer's screen: its display's lower left corner as a
/// reader at the spawn sees it, facing the spawn (world -z), so screen x
/// runs along world -x.
fn screen_transform() -> Mat4 {
    let at = pylons_view::terminal::POSITION;
    Mat4::from_translation(Vec3::new(
        at[0] as f32 + deploy_board::screen::WIDTH as f32 / 2.0,
        1.37,
        at[2] as f32 - 0.12,
    )) * Mat4::from_rotation_y(std::f32::consts::PI)
}

/// The board, fed from the page's Grid session.
pub struct BuildBoard {
    ingest: Ingest,
    subscribed: bool,
    /// The board's own transform: its site, turned to face the spawn, its
    /// face's left edge at the origin.
    transform: Mat4,
}

impl BuildBoard {
    /// An empty board; `test` shows only test events.
    pub fn new(test: bool) -> Self {
        Self {
            ingest: Ingest::new(nostr::deploy::ANNOUNCER, test),
            subscribed: false,
            transform: Mat4::from_rotation_translation(
                Quat::from_rotation_y(layout::yaw() as f32),
                Vec3::new(layout::SITE[0] as f32, 0.0, layout::SITE[1] as f32),
            ) * Mat4::from_translation(Vec3::new(-layout::WIDTH as f32 / 2.0, 0.0, 0.0)),
        }
    }

    /// Whether the page asked for test events (`?deploys=test`).
    pub fn wants_test(window: &web_sys::Window) -> bool {
        window.location().search().is_ok_and(|query| {
            query
                .trim_start_matches('?')
                .split('&')
                .any(|part| part == "deploys=test")
        })
    }

    /// Subscribes once on `presence`'s connection and folds in what it
    /// received since the last frame.
    pub fn tick(&mut self, presence: Option<&mut crate::grid::Presence>) {
        let Some(presence) = presence else {
            return;
        };
        if !self.subscribed {
            presence.watch(SUBSCRIPTION, deploy_board::filters());
            self.subscribed = true;
        }
        let wall = wall();
        for event in presence.take_watched() {
            self.ingest.event(&event, wall);
        }
    }

    /// This frame's board instances, given the `used` instances the frame
    /// already holds.
    pub fn instances(&self, used: usize, out: &mut Vec<Instance>) {
        let room = FRAME_INSTANCES.saturating_sub(used);
        for placed in layout::pieces(&self.ingest.board, wall(), room) {
            let at = Vec3::new(
                placed.at[0] as f32,
                placed.at[1] as f32,
                placed.at[2] as f32,
            );
            let place = self.transform * Mat4::from_translation(at);
            let (model, transform) = match placed.piece {
                Piece::Letter(c, ink) => (
                    letter_key(c, ink),
                    place * Mat4::from_scale(Vec3::splat(placed.size as f32)),
                ),
                Piece::Word(i) => (
                    word_key(i),
                    place * Mat4::from_scale(Vec3::splat(placed.size as f32)),
                ),
                Piece::Cell(cell) => (cell_key(cell), place),
                Piece::Flash(flash) => (flash_key(flash).to_owned(), place),
            };
            out.push(verse::grid_frame::fixed(&model, transform));
        }
    }

    /// This frame's screen instances on the shared computer, with the
    /// Pylons the page shows (`pylons`, when it knows them).
    pub fn screen_instances(&self, pylons: Option<usize>, out: &mut Vec<Instance>) {
        let screen = screen_transform();
        for placed in deploy_board::screen::pieces(pylons, Some(&self.ingest.board), wall()) {
            let Piece::Letter(c, ink) = placed.piece else {
                continue;
            };
            let at = Vec3::new(
                placed.at[0] as f32,
                placed.at[1] as f32,
                placed.at[2] as f32,
            );
            out.push(verse::grid_frame::fixed(
                &letter_key(c, ink),
                screen
                    * Mat4::from_translation(at)
                    * Mat4::from_scale(Vec3::splat(placed.size as f32)),
            ));
        }
    }

    /// What the board shows, for checks (`verse_build_board()`).
    pub fn report(&self) -> serde_json::Value {
        let wall = wall();
        let lane = |env| {
            let lane = self.ingest.board.lane(env, wall);
            serde_json::json!({
                "word": lane.current.as_ref().map(|c| c.word),
                "commit": lane.current.as_ref().and_then(|c| c.commit.clone()),
                "steps": lane.current.as_ref().map(|c| format!("{:?}", c.steps)),
                "last": lane.last.map(|(o, _)| o.word()),
                "history": lane.history.len(),
                "flash": lane.flash.map(|(f, _)| format!("{f:?}")),
            })
        };
        serde_json::json!({
            "staging": lane(nostr::deploy::Env::Staging),
            "production": lane(nostr::deploy::Env::Production),
            "refused": self.ingest.refused,
            "subscribed": self.subscribed,
        })
    }
}

/// The wall clock, Unix seconds with a fraction.
fn wall() -> f64 {
    js_sys::Date::now() / 1000.0
}
