//! The desktop window: winit events in, a frame out.

use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::Vec3;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::agent::Agent;
use crate::avatar::{self, Gait};
use crate::brain::{self, Brain};
use crate::camera::FollowCamera;
use crate::chat::{self, Channel};
use crate::controller::{InputState, PlayerController};
use crate::feed::Feed;
use crate::hud;
use crate::render::{self, Renderer, View};
use crate::replay::{self, Place, Replay};
use crate::session::{self, Session, Status};
use crate::ui::Atlas;
use crate::world::{self, World};
use crate::xp;

/// How the window joins the shared world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// Profile name; each profile is its own player key.
    pub profile: String,
    /// Relay URL, or `None` to play offline.
    pub relay: Option<String>,
    /// Relay the quest board and XP read, when it isn't `relay`.
    pub xp_relay: Option<String>,
    /// More public keys (npub or hex) whose XP counts as the player's.
    pub xp_keys: Vec<String>,
    /// More referees (npub or hex) to trust, beyond the trust file.
    pub xp_referees: Vec<String>,
    /// A Microcoder run to replay on launch: its directory or Gym run ID.
    pub replay: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            profile: "default".into(),
            relay: Some(session::DEFAULT_RELAY.into()),
            xp_relay: None,
            xp_keys: Vec::new(),
            xp_referees: Vec::new(),
            replay: None,
        }
    }
}

/// Opens the Verse window and runs until it closes.
///
/// # Errors
///
/// Returns a message when the identity, the event loop, or the renderer
/// cannot start.
pub fn run(options: &Options) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| format!("cannot start the event loop: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(options)?;
    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("the event loop failed: {e}"))?;
    app.error.map_or(Ok(()), Err)
}

/// What a capture shows of quests and XP.
#[derive(Clone, Debug, Default)]
pub struct CaptureXp {
    /// Relay to read quests and awards from; `None` shows the offline HUD.
    pub relay: Option<String>,
    /// Public keys whose XP the HUD shows as the player's.
    pub keys: Vec<String>,
    /// More referees to trust.
    pub referees: Vec<String>,
    /// Whether the quest board is open.
    pub board: bool,
    /// A Microcoder run to show replaying: its directory or Gym run ID.
    pub replay: Option<String>,
    /// Seconds into the replay to show; the middle when `None`.
    pub at: Option<f64>,
}

/// Renders the spawn view through `camera` to a PNG, without a window.
///
/// # Errors
///
/// Returns a message when no GPU is available or the file cannot be written.
pub fn capture(
    path: &std::path::Path,
    width: u32,
    height: u32,
    camera: FollowCamera,
    shot: &CaptureXp,
) -> Result<(), String> {
    let world = world::build();
    let player = PlayerController::new(world::SPAWN, 0.0);
    let view = view(&camera, &player, width as f32 / height as f32);
    let mut dynamic = avatar::mesh(&player, &Gait::default());
    let shown = match &shot.replay {
        Some(arg) => {
            let run = replay::find(arg)?;
            let mut r = Replay::load(&run, Place::Plaza.stand(false))?;
            if let Err(why) = &r.ghost {
                eprintln!("verse: no ghost: {why}");
            }
            let at = shot
                .at
                .map_or(r.clock.duration_ms as f64 / 2.0, |s| s * 1000.0);
            r.clock.seek(at);
            r.clock.playing = false;
            r.settle();
            let [mine, ghost] = r.carrots();
            let (agent, ghost) = (Agent::at(mine, 0.0), Agent::at(ghost, 0.0));
            dynamic.extend(&agent.mesh());
            if r.ghost.is_ok() {
                dynamic.extend(&ghost.mesh_at(coder_terminal::Intensity::ThreeQuarters));
            }
            Some((r, agent, ghost))
        }
        None => {
            dynamic.extend(&Agent::new(&player).mesh());
            None
        }
    };
    let atlas = Atlas::new(14.0);
    let board = shot.relay.as_ref().map(|relay| {
        let mut board = xp::Board::start(relay, &shot.referees, None);
        board.settle(Duration::from_millis(1500), Duration::from_secs(12));
        board
    });
    let mine = xp::my_keys(None, &shot.keys);
    let mut frame = sample_hud(&view, [width as f32, height as f32], &player);
    frame.xp = xp::strip(board.as_ref(), &mine);
    frame.board = shot
        .board
        .then(|| xp::board_lines(board.as_ref(), unix_now()));
    if let Some((r, agent, ghost)) = &shown {
        frame.replay = r.hud_lines();
        let mut overheads = landmark_overheads(player.pos);
        overheads.extend(replay_overheads(r, agent, ghost));
        overheads.push(frame.overheads[0].clone());
        frame.overheads = Box::leak(overheads.into_boxed_slice());
    }
    let (ui, _) = hud::build(&atlas, &frame);
    render::capture(
        path,
        width,
        height,
        &world.mesh,
        view,
        &dynamic,
        &ui,
        &atlas,
    )
}

/// A sample conversation, so a capture shows the chat design.
fn sample_hud<'a>(view: &View, size: [f32; 2], player: &PlayerController) -> hud::Frame<'a> {
    use crate::chat::{Line, Log};
    let mut log = Log::default();
    let line = |channel: Channel, from: &str, text: &str, note: Option<&str>| Line {
        channel: Some(channel),
        from: from.into(),
        to: None,
        text: text.into(),
        note: note.map(str::to_owned),
    };
    log.push(Line::system("Player north has logged in"));
    log.push(line(Channel::All, "north", "gm verse", None));
    log.push(line(
        Channel::Ads,
        "south",
        "*BUYING* a spare pylon, PM me",
        Some("[4 listening]"),
    ));
    log.push(line(
        Channel::Zone,
        "kiki",
        "anyone on the plaza want to race?",
        None,
    ));
    log.push(Line::system("Player south has logged in"));
    log.push(line(
        Channel::Near,
        "south",
        "the tower by the pylon is huge",
        None,
    ));
    log.push(line(Channel::Here, "north", "hi!", Some("(2 here)")));
    log.push(line(
        Channel::Room("lounge".into()),
        "kiki",
        "welcome in",
        None,
    ));
    log.push(Line {
        channel: Some(Channel::Pm("x".into())),
        from: "kiki".into(),
        to: Some("north".into()),
        text: "want to build together?".into(),
        note: None,
    });
    log.push(Line {
        channel: Some(Channel::Agent),
        from: "you".into(),
        to: None,
        text: "what's that tall thing?".into(),
        note: None,
    });
    log.push(Line {
        channel: Some(Channel::Agent),
        from: "agent".into(),
        to: None,
        text: "That's the pylon at the heart of the Plaza.".into(),
        note: None,
    });
    let agent = Agent::new(player);
    let overheads: &'a [hud::Overhead] = Box::leak(Box::new([
        hud::Overhead {
            feet: player.pos,
            lift: 2.2,
            name: Some("you".into()),
            name_step: coder_terminal::Intensity::ThreeQuarters,
            bubble: Some("gm verse".into()),
        },
        hud::Overhead {
            feet: agent.pos,
            lift: 0.5,
            name: None,
            name_step: coder_terminal::Intensity::Half,
            bubble: Some("That's the pylon at the heart of the Plaza.".into()),
        },
        hud::Overhead {
            feet: world::QUEST_BOARD,
            lift: 5.4,
            name: Some("QUEST BOARD".into()),
            name_step: coder_terminal::Intensity::Half,
            bubble: None,
        },
    ]));
    let pills = session::ROOMS.iter().fold(
        vec![
            ("ALL".to_owned(), true),
            ("ADS".to_owned(), false),
            ("ZONE".to_owned(), false),
            ("NEAR".to_owned(), false),
            ("HERE".to_owned(), false),
        ],
        |mut v, r| {
            v.push((format!("#{r}"), false));
            v
        },
    );
    let mut pills = pills;
    pills.push(("AGENT".into(), false));
    hud::Frame {
        size,
        scale: 1.0,
        view_proj: view.view_proj,
        log: Box::leak(Box::new(log)),
        nostr: Box::leak(Box::new(std::collections::VecDeque::new())),
        nostr_title: "live public notes · damus · primal".into(),
        left_tab: hud::LeftTab::World,
        input: Box::leak(Box::new(hud::Input::default())),
        pills,
        hint: format!("everyone in {}", session::WORLD),
        limit: Some(chat::MAX_BROADCAST),
        world_title: hud::world_title(session::WORLD, player.pos),
        overheads,
        time: 0.0,
        xp: Vec::new(),
        board: None,
        board_scroll: 0,
        replay: Vec::new(),
        picker: None,
        picker_scroll: 0,
    }
}

/// Name tags over the replay landmarks within 80 m of `from`.
fn landmark_overheads(from: Vec3) -> Vec<hud::Overhead> {
    use coder_terminal::Intensity;
    Place::LANDMARKS
        .iter()
        .filter(|p| p.position().distance(from) <= 80.0)
        .map(|p| hud::Overhead {
            feet: p.position(),
            // Above the spades' own tags when they visit.
            lift: match p {
                Place::Oracle => 7.4,
                Place::Library => 6.0,
                _ => 4.4,
            },
            name: Some(p.name().to_uppercase()),
            name_step: Intensity::Half,
            bubble: None,
        })
        .collect()
}

/// Who each replayed spade is and where it is: over the player's agent
/// and over the ghost.
fn replay_overheads(r: &Replay, agent: &Agent, ghost: &Agent) -> Vec<hud::Overhead> {
    use coder_terminal::Intensity;
    let (mine, theirs) = r.places();
    let mut out = vec![hud::Overhead {
        feet: agent.pos,
        lift: 0.6,
        name: Some(format!("Microcoder · {}", mine.name())),
        name_step: Intensity::Full,
        bubble: None,
    }];
    if let Some(theirs) = theirs {
        out.push(hud::Overhead {
            feet: ghost.pos,
            lift: 0.6,
            name: Some(format!(
                "{} · {}",
                gym::runs_beats_winner::REFERENCE,
                theirs.name()
            )),
            name_step: Intensity::ThreeQuarters,
            bubble: None,
        });
    }
    out
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn view(camera: &FollowCamera, player: &PlayerController, aspect: f32) -> View {
    View {
        view_proj: camera.view_proj(player.pos, player.yaw, aspect),
        eye: camera.eye(player.pos, player.yaw),
    }
}

/// Resolves a pubkey to a display name.
type NameOf<'a> = Box<dyn Fn(&str) -> String + 'a>;

/// Raw key and button state, resolved into [`InputState`] once per frame.
#[derive(Default)]
struct Keys {
    w: bool,
    s: bool,
    a: bool,
    d: bool,
    q: bool,
    e: bool,
    shift: bool,
    jump: bool,
    left_button: bool,
    right_button: bool,
}

impl Keys {
    fn input(&self) -> InputState {
        let both = self.left_button && self.right_button;
        InputState {
            forward: self.w || both,
            backward: self.s,
            left: self.a,
            right: self.d,
            strafe_left: self.q,
            strafe_right: self.e,
            mouse_look: self.right_button,
            sprint: self.shift,
            jump: self.jump,
        }
    }
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    world: World,
    player: PlayerController,
    camera: FollowCamera,
    gait: Gait,
    agent: Agent,
    keys: Keys,
    last: Instant,
    error: Option<String>,
    session: Option<Session>,
    title: String,
    frames: u64,
    atlas: Option<Atlas>,
    scale: f32,
    chat: hud::Input,
    method: Channel,
    brain: Brain,
    agent_says: Option<(String, Option<Instant>)>,
    feed: Option<Feed>,
    left_tab: hud::LeftTab,
    cursor: [f32; 2],
    layout: hud::Layout,
    pm_target: Option<String>,
    offline_log: chat::Log,
    started: Instant,
    xp: Option<xp::Board>,
    board_open: bool,
    board_scroll: usize,
    my_keys: Vec<String>,
    /// The replay playing, if any.
    replay: Option<Replay>,
    /// The replay's ghost: Fable's cheapest winning run on the task.
    ghost: Agent,
    /// Whether each side's finish has been marked.
    finished: [bool; 2],
    /// The replay list, while it is open.
    picker: Option<Picker>,
    /// The retained runs the list offers, read when it first opens.
    choices: Option<Vec<replay::Choice>>,
}

/// The replay list: the retained `beats-winner` runs and which is chosen.
struct Picker {
    choices: Vec<replay::Choice>,
    selected: usize,
    /// Why the last choice didn't start.
    notice: Option<String>,
}

impl Picker {
    fn lines(&self) -> Vec<(String, coder_terminal::Intensity)> {
        use coder_terminal::Intensity;
        let mut out = Vec::new();
        if self.choices.is_empty() {
            out.push((
                "No retained Microcoder pass beats Fable 5.1 low's cheapest or fastest winning run."
                    .to_owned(),
                Intensity::Half,
            ));
        }
        for (i, c) in self.choices.iter().enumerate() {
            let chosen = i == self.selected;
            out.push((
                format!("{} {}", if chosen { ">" } else { " " }, c.line),
                if chosen {
                    Intensity::Full
                } else {
                    Intensity::ThreeQuarters
                },
            ));
            out.push((
                format!("    [{}]", c.labels),
                if chosen {
                    Intensity::Half
                } else {
                    Intensity::Quarter
                },
            ));
        }
        out.push((
            "Each run: its cost and time, then Fable 5.1 low's cheapest winning run's. Up and Down choose; Enter plays.".to_owned(),
            Intensity::Quarter,
        ));
        if let Some(notice) = &self.notice {
            out.push((notice.clone(), Intensity::Full));
        }
        out
    }

    /// Rows to scroll so the chosen run stays in view: two rows a run.
    fn scroll(&self) -> usize {
        (self.selected * 2).saturating_sub(6)
    }
}

impl App {
    fn new(options: &Options) -> Result<Self, String> {
        let world = world::build();
        let mut player = PlayerController::new(world::SPAWN, 0.0);
        let mut session = match &options.relay {
            Some(relay) => Some(Session::start(&options.profile, relay)?),
            None => None,
        };
        let spawn = match &mut session {
            Some(s) => s.spawn(
                &world.blockers,
                world::HALF,
                std::time::Duration::from_millis(1500),
            ),
            None => session::Spawn {
                pos: session::random_spawn(&world.blockers),
                yaw: 0.0,
                resumed: false,
            },
        };
        player.pos = spawn.pos;
        player.yaw = spawn.yaw;
        if let Some(s) = &session {
            eprintln!(
                "verse: {} ({}…) on {}; {}",
                s.profile(),
                &s.pubkey()[..12],
                s.relay(),
                if spawn.resumed {
                    "resumed where you left"
                } else {
                    "new spawn on the plaza"
                }
            );
        }
        let xp_relay = options.xp_relay.clone().or_else(|| options.relay.clone());
        let board = xp_relay.as_deref().map(|relay| {
            xp::Board::start(
                relay,
                &options.xp_referees,
                session.as_ref().map(Session::signer),
            )
        });
        let my_keys = xp::my_keys(session.as_ref().map(Session::pubkey), &options.xp_keys);
        let agent = Agent::new(&player);
        let replay = match &options.replay {
            Some(arg) => {
                let run = replay::find(arg)?;
                let r = Replay::load(&run, agent.pos)?;
                if let Err(why) = &r.ghost {
                    eprintln!("verse: no ghost: {why}");
                }
                Some(r)
            }
            None => None,
        };
        Ok(Self {
            window: None,
            renderer: None,
            agent,
            world,
            player,
            camera: FollowCamera::default(),
            gait: Gait::default(),
            keys: Keys::default(),
            last: Instant::now(),
            error: None,
            session,
            title: String::new(),
            frames: 0,
            atlas: None,
            scale: 1.0,
            chat: hud::Input::default(),
            method: Channel::All,
            brain: Brain::start(&options.profile),
            agent_says: None,
            feed: options.relay.as_ref().map(|_| Feed::start()),
            left_tab: hud::LeftTab::World,
            cursor: [0.0, 0.0],
            layout: hud::Layout::default(),
            pm_target: None,
            offline_log: {
                let mut log = chat::Log::default();
                log.push(chat::Line::system(
                    "Welcome to Verse. Press Enter to chat, Tab to change channel.",
                ));
                log
            },
            started: Instant::now(),
            xp: board,
            board_open: false,
            board_scroll: 0,
            my_keys,
            replay,
            ghost: Agent::at(Place::Plaza.stand(true), 0.0),
            finished: [false; 2],
            picker: None,
            choices: None,
        })
    }

    /// Opens the replay list, reading the retained runs the first time.
    fn open_picker(&mut self) {
        self.board_open = false;
        let choices = self
            .choices
            .get_or_insert_with(replay::beats_winner_runs)
            .clone();
        self.picker = Some(Picker {
            choices,
            selected: 0,
            notice: None,
        });
    }

    /// Handles a key press while the replay list is open. Returns true
    /// when the list took it.
    fn picker_key(&mut self, code: KeyCode) -> bool {
        let Some(picker) = &mut self.picker else {
            return false;
        };
        let last = picker.choices.len().saturating_sub(1);
        match code {
            KeyCode::ArrowUp | KeyCode::KeyW => picker.selected = picker.selected.saturating_sub(1),
            KeyCode::ArrowDown | KeyCode::KeyS => picker.selected = (picker.selected + 1).min(last),
            KeyCode::PageUp => picker.selected = picker.selected.saturating_sub(5),
            KeyCode::PageDown => picker.selected = (picker.selected + 5).min(last),
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let Some(choice) = picker.choices.get(picker.selected) else {
                    return true;
                };
                match Replay::load(&choice.run, self.agent.pos) {
                    Ok(r) => self.start_replay(r),
                    Err(e) => {
                        if let Some(p) = &mut self.picker {
                            p.notice = Some(format!("Can't replay it: {e}"));
                        }
                    }
                }
            }
            KeyCode::Escape | KeyCode::KeyR => self.picker = None,
            _ => return false,
        }
        true
    }

    fn start_replay(&mut self, r: Replay) {
        self.ghost = Agent::at(Place::Plaza.stand(true), 0.0);
        self.finished = [false; 2];
        self.replay = Some(r);
        self.picker = None;
    }

    /// Advances the replay and flies both spades toward their places, or
    /// lets the agent follow the player when nothing is replaying.
    fn step_agents(&mut self, dt: f32) {
        let Some(r) = &mut self.replay else {
            self.agent.update(&self.player, dt);
            return;
        };
        r.tick(dt);
        let [mine, ghost] = r.carrots();
        self.agent.visit(mine, dt);
        self.ghost.visit(ghost, dt);
        let t = r.clock.elapsed_ms;
        let passed = |track: &replay::Track| track.result.starts_with("passed");
        if !self.finished[0] && r.mine.done(t) {
            self.finished[0] = true;
            if passed(&r.mine) {
                self.agent.celebrate();
            }
        }
        if let Ok(g) = &r.ghost
            && !self.finished[1]
            && g.done(t)
        {
            self.finished[1] = true;
            if passed(g) {
                self.ghost.celebrate();
            }
        }
        if t < 1.0 {
            self.finished = [false; 2];
        }
    }

    /// Records the player as offline on the relay before quitting.
    fn quit(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(session) = &mut self.session {
            session.leave(&self.player, &self.agent);
        }
        self.session = None;
        event_loop.exit();
    }

    fn update_title(&mut self) {
        let title = match &self.session {
            None => "Verse — offline".to_owned(),
            Some(s) => {
                let status = match s.status {
                    Status::Connecting => "connecting",
                    Status::Online => "online",
                    Status::Offline => "relay unreachable, retrying",
                };
                let shown = s.crowd.shown(Instant::now());
                let avatars = shown.iter().filter(|e| e.role == "avatar");
                let online = avatars.clone().filter(|e| e.online).count();
                let resting = avatars.count() - online;
                format!(
                    "Verse — {} — {} — {online} other players online, {resting} resting",
                    s.profile(),
                    status,
                )
            }
        };
        if title != self.title
            && let Some(window) = &self.window
        {
            window.set_title(&title);
            self.title = title;
        }
    }

    fn methods(&self) -> Vec<Channel> {
        let rooms: Vec<String> = session::ROOMS.iter().map(|r| (*r).to_owned()).collect();
        chat::methods(&rooms, self.pm_target.as_deref())
    }

    fn cycle_method(&mut self) {
        let methods = self.methods();
        let i = methods.iter().position(|m| *m == self.method).unwrap_or(0);
        self.method = methods[(i + 1) % methods.len()].clone();
    }

    fn open_chat(&mut self, seed: &str) {
        self.chat.open = true;
        self.chat.text = seed.to_owned();
        let (left, right) = (self.keys.left_button, self.keys.right_button);
        self.keys = Keys {
            left_button: left,
            right_button: right,
            ..Keys::default()
        };
    }

    /// Handles a key while the chat line is open.
    fn chat_key(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{Key, NamedKey};
        if !event.state.is_pressed() {
            return;
        }
        match &event.logical_key {
            Key::Named(NamedKey::Enter) => {
                let text = std::mem::take(&mut self.chat.text);
                self.chat.open = false;
                if !text.trim().is_empty() {
                    self.chat.last.clone_from(&text);
                    self.submit(&text);
                }
            }
            Key::Named(NamedKey::Escape) => {
                self.chat.open = false;
                self.chat.text.clear();
            }
            Key::Named(NamedKey::Backspace) => {
                self.chat.text.pop();
            }
            Key::Named(NamedKey::Tab) => self.cycle_method(),
            Key::Named(NamedKey::ArrowUp) => self.chat.text.clone_from(&self.chat.last),
            _ => {
                if let Some(text) = &event.text {
                    for c in text.chars().filter(|c| !c.is_control()) {
                        if self.chat.text.chars().count() < chat::MAX_LINE {
                            self.chat.text.push(c);
                        }
                    }
                }
            }
        }
    }

    /// Carries out one submitted chat line.
    fn submit(&mut self, text: &str) {
        let now = Instant::now();
        let command = chat::parse(text, &self.method);
        if let chat::Command::Send(Channel::Agent, text) = &command {
            self.ask_agent(text);
            return;
        }
        let Some(session) = &mut self.session else {
            self.offline_log
                .push(chat::Line::system("You are offline. Chat needs a relay."));
            return;
        };
        let notice = match command {
            chat::Command::Nothing => None,
            chat::Command::Send(channel, text) => session.say(&channel, &text, now).err(),
            chat::Command::Whisper(name, text) => match session.find_player(&name) {
                Some((pubkey, _)) => session.pm(&pubkey, &text).err(),
                None => Some("Could not find player to Private chat!".to_owned()),
            },
            chat::Command::Target(name) => match session.find_player(&name) {
                Some((pubkey, name)) => {
                    self.pm_target = Some(pubkey.clone());
                    self.method = Channel::Pm(pubkey);
                    Some(format!(
                        "Now chatting privately with {name}. Tab to switch back."
                    ))
                }
                None => Some("Could not find player to Private chat!".to_owned()),
            },
            chat::Command::Mute(word) => Some(session.set_mute(&word, true)),
            chat::Command::Unmute(word) => Some(session.set_mute(&word, false)),
        };
        if let Some(notice) = notice {
            session.log.push(chat::Line::system(notice));
        }
    }

    /// Sends a line to the player's own agent: logged privately, answered
    /// by the model, and spoken in a bubble over the spade.
    fn ask_agent(&mut self, text: &str) {
        let me = self
            .session
            .as_ref()
            .map_or_else(|| "you".to_owned(), |s| s.profile().to_owned());
        let line = chat::Line {
            channel: Some(Channel::Agent),
            from: me,
            to: None,
            text: text.to_owned(),
            note: None,
        };
        match &mut self.session {
            Some(s) => s.log.push(line),
            None => self.offline_log.push(line),
        }
        let surroundings = self.surroundings();
        self.brain.ask(brain::Ask {
            text: text.to_owned(),
            surroundings,
        });
        self.agent_says = Some(("…".to_owned(), None));
        let head = self.player.pos + Vec3::Y * 1.7;
        self.agent.greet(head);
    }

    /// What the agent can see, in plain sentences.
    fn surroundings(&self) -> String {
        let pos = self.player.pos;
        let mut out = vec![
            format!(
                "- You and your player are in the {} of world {}, at x {:.0}, z {:.0}.",
                chat::zone_name(chat::zone_of(pos)),
                session::WORLD,
                pos.x,
                pos.z
            ),
            format!(
                "- The tall amber pylon is {:.0} m away. Wireframe towers ring the plaza.",
                pos.distance(world::PYLON)
            ),
        ];
        if let Some(s) = &self.session {
            let now = Instant::now();
            let mut near: Vec<(f32, String)> = s
                .crowd
                .shown(now)
                .into_iter()
                .filter(|e| e.role == "avatar")
                .map(|e| {
                    let d = e.pos.distance(pos);
                    let state = if e.online {
                        "online"
                    } else {
                        "resting, offline"
                    };
                    (
                        d,
                        format!("{} ({state}, {d:.0} m away)", s.name_of(&e.pubkey)),
                    )
                })
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            if near.is_empty() {
                out.push("- No other players are around.".into());
            } else {
                let names: Vec<String> = near.into_iter().take(6).map(|(_, n)| n).collect();
                out.push(format!("- Players you can see: {}.", names.join("; ")));
            }
            let recent: Vec<String> = s
                .log
                .world
                .iter()
                .chain(&s.log.personal)
                .filter(|l| !matches!(l.channel, Some(Channel::Agent) | Some(Channel::Pm(_))))
                .rev()
                .take(5)
                .map(|l| {
                    if l.from.is_empty() {
                        l.text.clone()
                    } else {
                        format!("{}: {}", l.from, l.text)
                    }
                })
                .collect();
            if !recent.is_empty() {
                out.push(format!("- Recent public chat: {}", recent.join(" | ")));
            }
        }
        if let Some(feed) = &self.feed {
            let notes = feed.recent(3);
            if !notes.is_empty() {
                out.push(format!(
                    "- Stand-ins for people posting on Nostr gather around the pylon. Latest: {}",
                    notes.join(" | ")
                ));
            }
        }
        out.join("\n")
    }

    /// Streams the agent's reply into its bubble and the personal window.
    fn hear_agent(&mut self, now: Instant) {
        for reply in self.brain.drain() {
            let (text, done) = match reply {
                brain::Reply::Piece(piece) => {
                    let so_far = match &self.agent_says {
                        Some((t, None)) if t != "…" => format!("{t}{piece}"),
                        _ => piece,
                    };
                    (so_far, false)
                }
                brain::Reply::Done(text) | brain::Reply::Failed(text) => (text, true),
            };
            if done {
                let line = chat::Line {
                    channel: Some(Channel::Agent),
                    from: "agent".into(),
                    to: None,
                    text: text.clone(),
                    note: None,
                };
                match &mut self.session {
                    Some(s) => s.log.push(line),
                    None => self.offline_log.push(line),
                }
                let hold = Duration::from_secs(6) + Duration::from_millis(60 * text.len() as u64);
                self.agent_says = Some((text, Some(now + hold.min(Duration::from_secs(20)))));
            } else {
                self.agent_says = Some((text, None));
            }
        }
        if self
            .agent_says
            .as_ref()
            .is_some_and(|(_, until)| until.is_some_and(|u| u <= now))
        {
            self.agent_says = None;
        }
    }

    /// A left click at the cursor, if it lands on the HUD. Returns true
    /// when the HUD took it.
    fn click(&mut self) -> bool {
        let [x, y] = self.cursor;
        if !self.layout.owns(x, y) {
            return false;
        }
        if let Some(i) = self.layout.pills.iter().position(|r| r.contains(x, y)) {
            if let Some(method) = self.methods().get(i).cloned() {
                self.method = method;
                self.open_chat("");
            }
        } else if let Some(i) = self.layout.tabs.iter().position(|r| r.contains(x, y)) {
            self.left_tab = if i == 0 {
                hud::LeftTab::World
            } else {
                hud::LeftTab::Nostr
            };
        } else if self.layout.bar.contains(x, y) {
            self.open_chat("");
        }
        true
    }

    fn key(&mut self, code: KeyCode, pressed: bool, event_loop: &ActiveEventLoop) {
        if pressed && self.picker_key(code) {
            return;
        }
        let replaying = self.replay.is_some();
        match code {
            KeyCode::KeyR if pressed => self.open_picker(),
            KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3 if pressed && replaying => {
                let speed = match code {
                    KeyCode::Digit1 => replay::SPEEDS[0],
                    KeyCode::Digit2 => replay::SPEEDS[1],
                    _ => replay::SPEEDS[2],
                };
                if let Some(r) = &mut self.replay {
                    r.clock.set_speed(speed);
                }
            }
            KeyCode::KeyP if pressed && replaying => {
                if let Some(r) = &mut self.replay {
                    r.clock.toggle();
                }
            }
            KeyCode::Home if pressed && replaying => {
                if let Some(r) = &mut self.replay {
                    r.clock.seek(0.0);
                    r.clock.playing = true;
                }
            }
            KeyCode::KeyT if pressed => {
                self.method = Channel::Agent;
                self.open_chat("");
            }
            KeyCode::KeyN if pressed => {
                self.left_tab = match self.left_tab {
                    hud::LeftTab::World => hud::LeftTab::Nostr,
                    hud::LeftTab::Nostr => hud::LeftTab::World,
                };
            }
            KeyCode::Enter | KeyCode::NumpadEnter if pressed => self.open_chat(""),
            KeyCode::Slash if pressed => self.open_chat("/"),
            KeyCode::Tab if pressed => self.cycle_method(),
            KeyCode::KeyW | KeyCode::ArrowUp => self.keys.w = pressed,
            KeyCode::KeyS | KeyCode::ArrowDown => self.keys.s = pressed,
            KeyCode::KeyA | KeyCode::ArrowLeft => self.keys.a = pressed,
            KeyCode::KeyD | KeyCode::ArrowRight => self.keys.d = pressed,
            KeyCode::KeyQ => self.keys.q = pressed,
            KeyCode::KeyE => self.keys.e = pressed,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.keys.shift = pressed,
            KeyCode::Space if pressed => self.keys.jump = true,
            KeyCode::KeyB if pressed => self.board_open = !self.board_open,
            KeyCode::PageDown if pressed && self.board_open => self.scroll_board(8),
            KeyCode::PageUp if pressed && self.board_open => self.scroll_board(-8),
            KeyCode::Escape if pressed && self.board_open => self.board_open = false,
            KeyCode::Escape if pressed && replaying => self.replay = None,
            KeyCode::Escape if pressed => self.quit(event_loop),
            _ => {}
        }
    }

    /// Scrolls the open quest board by `rows`, within what it holds.
    fn scroll_board(&mut self, rows: i32) {
        let max = self.layout.board.map_or(0, |(_, max)| max);
        let next = self.board_scroll.min(max) as i64 + i64::from(rows);
        self.board_scroll = next.clamp(0, max as i64) as usize;
    }

    fn button(&mut self, button: MouseButton, pressed: bool) {
        match button {
            MouseButton::Left if pressed && !self.keys.left_button && self.click() => return,
            MouseButton::Left => self.keys.left_button = pressed,
            MouseButton::Right => {
                self.keys.right_button = pressed;
                if pressed {
                    self.player.yaw += self.camera.take_offset();
                }
            }
            _ => return,
        }
        self.capture(self.keys.left_button || self.keys.right_button);
    }

    /// Hides and locks the cursor while a mouse button drags the view.
    fn capture(&self, on: bool) {
        let Some(window) = &self.window else { return };
        if on {
            let _ = window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
        }
        window.set_cursor_visible(!on);
    }

    fn mouse(&mut self, dx: f32, dy: f32) {
        if self.keys.right_button {
            self.player.yaw =
                crate::controller::wrap(self.player.yaw + self.camera.mouselook(dx, dy));
        } else if self.keys.left_button {
            self.camera.orbit(dx, dy);
        }
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;

        let input = self.keys.input();
        self.keys.jump = false;
        self.player
            .update(&input, dt, &self.world.blockers, world::HALF);
        if self.player.speed > 0.1 && !self.keys.left_button {
            self.camera.settle(dt);
        }
        self.gait
            .advance(self.player.speed, self.player.airborne(), dt);
        self.step_agents(dt);

        // The look-around is the agent assessing what is near: it asks the
        // relay for entity states around it, then glances at what it found.
        if self.agent.take_scan() {
            match &mut self.session {
                Some(session) => session.request_scan(self.agent.pos),
                None => self.agent.look_around(&[]),
            }
        }
        let mut dynamic = avatar::mesh(&self.player, &self.gait);
        dynamic.extend(&self.agent.mesh());
        if self.replay.as_ref().is_some_and(|r| r.ghost.is_ok()) {
            dynamic.extend(&self.ghost.mesh_at(coder_terminal::Intensity::ThreeQuarters));
        }
        if let Some(session) = &mut self.session {
            session.tick(now, &self.player, &self.agent);
            if let Some(found) = session.scan_result(now, &self.agent) {
                self.agent.look_around(&found);
            }
            // Two agents that meet greet each other.
            if let Some((pubkey, at)) = session.greeting(now, &self.agent)
                && self.agent.greet(at)
            {
                session.greeted(&pubkey, at, &self.agent, now);
            }
            dynamic.extend(&session.crowd.mesh(now, dt));
        }
        if self.frames.is_multiple_of(30) {
            self.update_title();
        }
        self.frames = self.frames.wrapping_add(1);

        let Some(renderer) = &self.renderer else {
            return;
        };
        let view = view(&self.camera, &self.player, renderer.aspect());
        let size = renderer.size();
        if let Some(feed) = &mut self.feed {
            feed.tick(now);
            for v in &feed.visitors {
                let rot = glam::Quat::from_rotation_y(v.yaw);
                dynamic.extend(&avatar::figure(
                    v.pos,
                    rot,
                    &Gait::default(),
                    coder_terminal::Intensity::Half,
                ));
            }
        }
        self.hear_agent(now);
        if let Some(board) = &mut self.xp {
            board.tick();
        }
        let overheads = self.overheads(now);
        let ui = match &self.atlas {
            Some(atlas) => {
                let (log, name_of): (&chat::Log, NameOf<'_>) = match &self.session {
                    Some(s) => (&s.log, Box::new(|p: &str| s.name_of(p))),
                    None => (&self.offline_log, Box::new(str::to_owned)),
                };
                let pills = self
                    .methods()
                    .iter()
                    .map(|m| (hud::method_label(m, &name_of), *m == self.method))
                    .collect();
                let (near, here) = self.session.as_ref().map_or((0, 0), |s| {
                    let shown = s.crowd.shown(now);
                    let avatars = shown.iter().filter(|e| e.role == "avatar" && e.online);
                    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
                    let d = |e: &&crate::crowd::Shown| flat(e.pos).distance(flat(self.player.pos));
                    (
                        avatars
                            .clone()
                            .filter(|e| d(e) <= chat::NEAR_RADIUS)
                            .count(),
                        avatars.filter(|e| d(e) <= chat::HERE_RADIUS).count(),
                    )
                });
                let zone = chat::zone_of(self.player.pos);
                let hint = hud::audience(&self.method, session::WORLD, zone, near, here, &name_of);
                let limit = matches!(self.method, Channel::All | Channel::Ads)
                    .then_some(chat::MAX_BROADCAST);
                let empty = std::collections::VecDeque::new();
                let (nostr, nostr_title) = match &self.feed {
                    Some(feed) => (&feed.lines, feed.title()),
                    None => (&empty, "offline: start Verse with a relay".to_owned()),
                };
                let (ui, layout) = hud::build(
                    atlas,
                    &hud::Frame {
                        size,
                        scale: self.scale,
                        view_proj: view.view_proj,
                        log,
                        nostr,
                        nostr_title,
                        left_tab: self.left_tab,
                        input: &self.chat,
                        pills,
                        hint,
                        limit,
                        world_title: hud::world_title(session::WORLD, self.player.pos),
                        overheads: &overheads,
                        time: (now - self.started).as_secs_f32(),
                        xp: xp::strip(self.xp.as_ref(), &self.my_keys),
                        board: self
                            .board_open
                            .then(|| xp::board_lines(self.xp.as_ref(), unix_now())),
                        board_scroll: self.board_scroll,
                        replay: self
                            .replay
                            .as_ref()
                            .map(Replay::hud_lines)
                            .unwrap_or_default(),
                        picker: self.picker.as_ref().map(Picker::lines),
                        picker_scroll: self.picker.as_ref().map_or(0, Picker::scroll),
                    },
                );
                self.layout = layout;
                ui
            }
            None => crate::ui::UiBatch::default(),
        };
        if let Some(renderer) = &mut self.renderer {
            renderer.draw(view, &dynamic, &ui);
        }
    }

    /// Name tags and speech bubbles: over you, your agent, nearby players,
    /// and Nostr stand-ins.
    fn overheads(&self, now: Instant) -> Vec<hud::Overhead> {
        use coder_terminal::Intensity;
        let mut out = Vec::new();
        let snapshot = self.xp.as_ref().and_then(|b| b.snapshot.as_ref());
        let tagged = |name: String, keys: &[String]| match xp::level_tag(snapshot, keys) {
            Some(level) => format!("{name} · {level}"),
            None => name,
        };
        let (my_name, my_bubble) = match &self.session {
            Some(s) => (
                tagged(s.profile().to_owned(), &self.my_keys),
                s.bubbles
                    .iter()
                    .find(|b| b.pubkey == s.pubkey())
                    .map(|b| b.text.clone()),
            ),
            None => (tagged("you".to_owned(), &self.my_keys), None),
        };
        out.push(hud::Overhead {
            feet: self.player.pos,
            lift: 2.2,
            name: Some(my_name),
            name_step: Intensity::ThreeQuarters,
            bubble: my_bubble,
        });
        if let Some((text, _)) = &self.agent_says {
            out.push(hud::Overhead {
                feet: self.agent.pos,
                lift: 0.5,
                name: None,
                name_step: Intensity::Half,
                bubble: Some(text.clone()),
            });
        }
        if let Some(s) = &self.session {
            for e in s.crowd.shown(now) {
                if e.role != "avatar" || e.pos.distance(self.player.pos) > 60.0 {
                    continue;
                }
                out.push(hud::Overhead {
                    feet: e.pos,
                    lift: 2.2,
                    name: Some(tagged(
                        s.name_of(&e.pubkey),
                        std::slice::from_ref(&e.pubkey),
                    )),
                    name_step: if e.online {
                        Intensity::Half
                    } else {
                        Intensity::Quarter
                    },
                    bubble: s
                        .bubbles
                        .iter()
                        .find(|b| b.pubkey == e.pubkey)
                        .map(|b| b.text.clone()),
                });
            }
        }
        out.extend(landmark_overheads(self.player.pos));
        if let Some(r) = &self.replay {
            out.extend(replay_overheads(r, &self.agent, &self.ghost));
        }
        let board = world::QUEST_BOARD;
        if board.distance(self.player.pos) <= 80.0 {
            let near = board.distance(self.player.pos) <= xp::BOARD_REACH;
            out.push(hud::Overhead {
                feet: board,
                lift: 5.4,
                name: Some(if near && !self.board_open {
                    "QUEST BOARD · press B to read".to_owned()
                } else {
                    "QUEST BOARD".to_owned()
                }),
                name_step: if near {
                    Intensity::Full
                } else {
                    Intensity::Half
                },
                bubble: None,
            });
        }
        if let Some(feed) = &self.feed {
            for v in &feed.visitors {
                if v.pos.distance(self.player.pos) > 60.0 {
                    continue;
                }
                out.push(hud::Overhead {
                    feet: v.pos,
                    lift: 2.2,
                    name: Some(format!("{} · nostr", feed.name_of(&v.pubkey))),
                    name_step: Intensity::Quarter,
                    bubble: v.bubble.as_ref().map(|(t, _)| t.clone()),
                });
            }
        }
        out
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Verse")
            .with_inner_size(LogicalSize::new(1440.0, 900.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => {
                self.error = Some(format!("cannot open a window: {e}"));
                event_loop.exit();
                return;
            }
        };
        self.scale = window.scale_factor() as f32;
        let atlas = Atlas::new((14.0 * self.scale).round());
        match Renderer::new(window.clone(), &self.world.mesh, &atlas) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
                self.atlas = Some(atlas);
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
                return;
            }
        }
        window.focus_window();
        self.window = Some(window);
        self.last = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.quit(event_loop),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if self.chat.open {
                    self.chat_key(&event);
                } else if let PhysicalKey::Code(code) = event.physical_key
                    && !event.repeat
                {
                    self.key(code, event.state == ElementState::Pressed, event_loop);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = [position.x as f32, position.y as f32];
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.button(button, state == ElementState::Pressed);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                let [x, y] = self.cursor;
                if self.board_open && self.layout.board.is_some_and(|(r, _)| r.contains(x, y)) {
                    self.scroll_board((-lines * 3.0).round() as i32);
                } else {
                    self.camera.zoom(lines);
                }
            }
            WindowEvent::Focused(false) => {
                self.keys = Keys::default();
                self.capture(false);
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.mouse(dx as f32, dy as f32);
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_replay_list_marks_the_chosen_run_and_keeps_it_in_view() {
        let run = replay::find("microcoder/embedding-drift-monitor-1790393791").unwrap();
        let choice = |n: usize| replay::Choice {
            run: run.clone(),
            line: format!("run {n} · $0.02 vs $0.74 · 2m vs 2m 19s"),
            labels: "in-sample · knowledge-assisted · OpenRouter · billed cost".into(),
        };
        let mut picker = Picker {
            choices: (0..12).map(choice).collect(),
            selected: 5,
            notice: None,
        };
        let lines = picker.lines();
        let chosen: Vec<&str> = lines
            .iter()
            .map(|(t, _)| t.as_str())
            .filter(|t| t.starts_with('>'))
            .collect();
        assert_eq!(chosen, ["> run 5 · $0.02 vs $0.74 · 2m vs 2m 19s"]);
        assert!(
            lines.iter().any(|(t, _)| t.contains("[in-sample")),
            "labels print"
        );
        assert_eq!(picker.scroll(), 4);
        picker.selected = 0;
        assert_eq!(picker.scroll(), 0);
        picker.choices.clear();
        assert!(picker.lines()[0].0.starts_with("No retained"));
    }
}
