//! Gym boards load only while a surface is active inside the Gym building.
//!
//! The worker owns network I/O. A frame polls bounded messages; it never awaits
//! a relay, opens a run file, or launches a process. Leaving discards the worker,
//! but cannot cancel work the remote host may already have admitted.
use gym_bridge::{Client, Connection, Error, ErrorCode, LaunchReceipt, Recipe, Run, Snapshot};
use secp256k1::SecretKey;
use serde::Serialize;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc as async_mpsc, watch};

const REFRESH: Duration = Duration::from_secs(5);
const DEADLINE: Duration = Duration::from_secs(12);

/// A retained logical launch. An uncertain response must reuse this request ID.
#[derive(Clone, Debug, Serialize)]
pub struct LaunchView {
    pub request_id: String,
    pub phase: String,
    pub error: Option<String>,
    pub receipt: Option<LaunchReceipt>,
}

/// A bounded presentation snapshot shared by native and desktop views.
#[derive(Clone, Debug, Serialize)]
pub struct BoardView {
    pub revision: u64,
    pub active: bool,
    pub configured: bool,
    pub public_key: String,
    pub status: String,
    pub error: Option<String>,
    pub stale: bool,
    pub observed_at: Option<u64>,
    pub runs: Vec<Run>,
    pub recipes: Vec<Recipe>,
    pub selected_run: Option<Run>,
    pub selected_recipe: Option<Recipe>,
    pub launch: Option<LaunchView>,
    pub notices: Vec<String>,
}

#[derive(Clone)]
struct Launch {
    request_id: String,
    recipe_id: String,
    revision: String,
}

enum Update {
    Snapshot(Result<Snapshot, Error>),
    Launch(String, Result<LaunchReceipt, Error>),
}

struct Worker {
    launch: async_mpsc::Sender<Launch>,
    updates: Receiver<(u64, Update)>,
    cancel: watch::Sender<bool>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Cancellation drops the in-flight socket future. No join on the UI
        // thread, and no command or subscription survives into another visit.
        let _ = self.cancel.send(true);
    }
}

impl Worker {
    fn start(connection: Connection, secret: SecretKey, epoch: u64) -> Result<Self, String> {
        let (launch, mut commands) = async_mpsc::channel::<Launch>(2);
        let (output, updates) = mpsc::sync_channel(8);
        let (cancel, mut cancellation) = watch::channel(false);
        std::thread::Builder::new()
            .name("verse-gym".into())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    send_update(&output, epoch, Update::Snapshot(Err(unavailable())));
                    return;
                };
                runtime.block_on(async move {
                    let client = match Client::new(connection, secret) {
                        Ok(client) => client,
                        Err(error) => { send_update(&output, epoch, Update::Snapshot(Err(error))); return; }
                    };
                    let mut interval = tokio::time::interval(REFRESH);
                    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    loop {
                        tokio::select! {
                            biased;
                            _ = cancellation.changed() => break,
                            command = commands.recv() => {
                                let Some(command) = command else { break };
                                let result = tokio::select! {
                                    biased;
                                    _ = cancellation.changed() => break,
                                    result = tokio::time::timeout(DEADLINE, client.launch(
                                        &command.request_id, &command.recipe_id, &command.revision
                                    )) => result.unwrap_or_else(|_| Err(unavailable())),
                                };
                                send_update(&output, epoch, Update::Launch(command.request_id, result));
                            }
                            _ = interval.tick() => {
                                let result = tokio::select! {
                                    biased;
                                    _ = cancellation.changed() => break,
                                    result = tokio::time::timeout(DEADLINE, client.snapshot()) =>
                                        result.unwrap_or_else(|_| Err(unavailable())),
                                };
                                send_update(&output, epoch, Update::Snapshot(result));
                            }
                        }
                    }
                });
            })
            .map_err(|_| "The Gym connection worker could not start.".to_owned())?;
        Ok(Self {
            launch,
            updates,
            cancel,
        })
    }
}

fn unavailable() -> Error {
    Error::new(ErrorCode::Unavailable, "Gym response unavailable")
}

fn send_update(output: &SyncSender<(u64, Update)>, epoch: u64, update: Update) {
    // A stopped or slow view must not block cancellation or the runtime.
    let _ = output.try_send((epoch, update));
}

/// Explicitly scoped presentation and transport owner. Construction is inert.
pub struct Board {
    secret: SecretKey,
    synthetic: bool,
    connection: Option<Connection>,
    worker: Option<Worker>,
    epoch: u64,
    state: BoardView,
    pending: Option<Launch>,
    sent_at: Option<Instant>,
}

impl Board {
    #[must_use]
    pub fn new(secret: SecretKey, synthetic: bool) -> Self {
        Self {
            secret,
            synthetic,
            connection: None,
            worker: None,
            epoch: 0,
            pending: None,
            sent_at: None,
            state: BoardView {
                revision: 1,
                active: false,
                configured: synthetic,
                public_key: gym_bridge::pubkey(&secret),
                status: "Gym data loads when you enter the building.".into(),
                error: None,
                stale: true,
                observed_at: None,
                runs: Vec::new(),
                recipes: Vec::new(),
                selected_run: None,
                selected_recipe: None,
                launch: None,
                notices: Vec::new(),
            },
        }
    }

    fn changed(&mut self) {
        self.state.revision = self.state.revision.saturating_add(1);
    }

    /// Validate a separate Gym grant without contacting any network.
    pub fn configure(&mut self, code: &str) -> Result<(), String> {
        if self.pending.is_some() {
            return Err("Resolve the pending Gym launch before replacing this connection.".into());
        }
        let connection = Connection::parse(code)
            .map_err(|_| "The Gym connection code is invalid.".to_owned())?;
        connection
            .verify(
                &self.secret,
                gym_bridge::unix_time().unwrap_or(0),
                gym_bridge::RelayPolicy::Production,
            )
            .map_err(|_| {
                "The Gym connection is expired or belongs to another device.".to_owned()
            })?;
        self.worker = None;
        self.epoch = self.epoch.wrapping_add(1);
        self.connection = Some(connection);
        self.state.configured = true;
        self.state.runs.clear();
        self.state.recipes.clear();
        self.state.observed_at = None;
        self.state.selected_run = None;
        self.state.selected_recipe = None;
        self.state.launch = None;
        self.state.error = None;
        self.state.stale = true;
        self.state.status = "Gym connected. Enter the building to load its boards.".into();
        if self.state.active {
            self.start();
        }
        self.changed();
        Ok(())
    }

    /// The caller supplies `surface_active && world.gym(aspect).inside`.
    /// Entering subscribes; exiting cancels observation but never a remote run.
    pub fn set_active(&mut self, active: bool) {
        if active == self.state.active {
            return;
        }
        self.state.active = active;
        self.epoch = self.epoch.wrapping_add(1);
        if active {
            if self.synthetic {
                self.apply_snapshot(fixture());
            } else {
                self.start();
            }
        } else {
            self.worker = None;
            self.state.stale = true;
            self.state.status = "Gym updates paused outside the building.".into();
            self.state.selected_run = None;
            self.state.selected_recipe = None;
            if let Some(launch) = self.state.launch.as_mut()
                && launch.phase == "sending"
            {
                launch.phase = "unknown".into();
                launch.error = Some("Observation stopped. The host may have admitted this request. Retry the same request after returning.".into());
            }
        }
        self.changed();
    }

    fn start(&mut self) {
        if !self.state.active || self.worker.is_some() {
            return;
        }
        let Some(connection) = self.connection.clone() else {
            self.state.status = "Connect a Gym host to see its runs and enabled recipes.".into();
            return;
        };
        match Worker::start(connection, self.secret, self.epoch) {
            Ok(worker) => {
                self.worker = Some(worker);
                self.state.status = "Loading Gym boards…".into();
            }
            Err(error) => self.state.error = Some(error),
        }
    }

    /// At most eight results per frame. Parsing, validation, and I/O stay off it.
    pub fn poll(&mut self) {
        if !self.state.active {
            return;
        }
        if self.state.observed_at.is_some_and(|at| {
            gym_bridge::unix_time()
                .unwrap_or(u64::MAX)
                .saturating_sub(at)
                > 30
        }) && !self.state.stale
        {
            self.state.stale = true;
            self.state.status = "Gym observations are stale.".into();
            self.changed();
        }
        if self
            .sent_at
            .is_some_and(|at| at.elapsed() > DEADLINE + REFRESH)
            && let Some(launch) = self.state.launch.as_mut()
            && launch.phase == "sending"
        {
            launch.phase = "unknown".into();
            launch.error = Some(
                "No confirmed response. Retry the same request to recover its disposition.".into(),
            );
            self.sent_at = None;
            self.changed();
        }
        let updates = self.worker.as_ref().map_or_else(Vec::new, |worker| {
            worker.updates.try_iter().take(8).collect::<Vec<_>>()
        });
        for (epoch, update) in updates {
            self.apply(epoch, update);
        }
    }

    fn apply(&mut self, epoch: u64, update: Update) {
        if epoch != self.epoch || !self.state.active {
            return;
        }
        match update {
            Update::Snapshot(Ok(snapshot)) => self.apply_snapshot(snapshot),
            Update::Snapshot(Err(error)) => {
                self.state.stale = true;
                self.state.status = "Gym unavailable; retained values may be stale.".into();
                self.state.error = Some(error_text(&error));
            }
            Update::Launch(id, result) => {
                if !self
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.request_id == id)
                {
                    return;
                }
                self.sent_at = None;
                match result {
                    Ok(receipt) => {
                        self.state.launch = Some(LaunchView {
                            request_id: id,
                            phase: "accepted".into(),
                            error: None,
                            receipt: Some(receipt),
                        });
                        self.pending = None;
                    }
                    Err(error) => {
                        let uncertain = !gym_bridge::confirmed_refusal(&error);
                        self.state.launch = Some(LaunchView {
                            request_id: id,
                            phase: if uncertain { "unknown" } else { "rejected" }.into(),
                            error: Some(error_text(&error)),
                            receipt: None,
                        });
                        if !uncertain {
                            self.pending = None;
                        }
                    }
                }
            }
        }
        self.changed();
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot) {
        self.state.observed_at = Some(snapshot.observed_at);
        self.state.stale = gym_bridge::unix_time()
            .unwrap_or(u64::MAX)
            .saturating_sub(snapshot.observed_at)
            > 30;
        self.state.status = if self.synthetic {
            "Synthetic Gym preview; no real run or network connection.".into()
        } else if self.state.stale {
            "The Gym host returned stale observations.".into()
        } else {
            "Gym host observations; refreshed while you remain inside.".into()
        };
        self.state.error = None;
        self.state.runs = snapshot.runs;
        self.state.recipes = snapshot.recipes;
        self.state.notices = snapshot.notices;
        if let Some(selected) = &self.state.selected_run {
            self.state.selected_run = self
                .state
                .runs
                .iter()
                .find(|run| run.id == selected.id)
                .cloned();
        }
        if let Some(selected) = &self.state.selected_recipe
            && !self
                .state
                .recipes
                .iter()
                .any(|recipe| recipe.id == selected.id && recipe.revision == selected.revision)
        {
            self.state.selected_recipe = None;
            self.state.error = Some(
                "The selected recipe changed. Review its current version before starting.".into(),
            );
        }
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.revision
    }

    /// Clone only when the presentation revision changes, never every frame.
    #[must_use]
    pub fn view(&self) -> BoardView {
        self.state.clone()
    }

    pub fn select_run(&mut self, id: &str) -> Result<(), String> {
        self.require_active()?;
        let run = self
            .state
            .runs
            .iter()
            .find(|run| run.id == id)
            .cloned()
            .ok_or("This run is no longer on the board.")?;
        self.state.selected_run = Some(run);
        self.state.selected_recipe = None;
        self.changed();
        Ok(())
    }

    pub fn select_recipe(&mut self, id: &str) -> Result<(), String> {
        self.require_active()?;
        if self.state.stale || self.pending.is_some() {
            return Err("Wait for fresh Gym data and resolve any pending launch first.".into());
        }
        let recipe = self
            .state
            .recipes
            .iter()
            .find(|recipe| recipe.id == id)
            .cloned()
            .ok_or("This recipe is not enabled for this device.")?;
        self.state.selected_recipe = Some(recipe);
        self.state.selected_run = None;
        self.changed();
        Ok(())
    }

    /// Send only the exact recipe reviewed by the user. Entry never calls this.
    pub fn confirm_launch(&mut self) -> Result<(), String> {
        self.require_active()?;
        if self.state.stale || self.pending.is_some() {
            return Err("Fresh data and a resolved prior request are required.".into());
        }
        let recipe = self
            .state
            .selected_recipe
            .clone()
            .ok_or("Select and review a recipe first.")?;
        if !self
            .state
            .recipes
            .iter()
            .any(|current| current.id == recipe.id && current.revision == recipe.revision)
        {
            return Err("The recipe changed. Review it again before starting.".into());
        }
        let launch = Launch {
            request_id: gym_bridge::random_id(),
            recipe_id: recipe.id,
            revision: recipe.revision,
        };
        self.queue(launch)?;
        self.state.selected_recipe = None;
        self.changed();
        Ok(())
    }

    /// Explicit reconciliation reuses the logical request; it is never automatic.
    pub fn retry_launch(&mut self) -> Result<(), String> {
        self.require_active()?;
        if self
            .state
            .launch
            .as_ref()
            .is_some_and(|launch| launch.phase == "sending")
        {
            return Err("The request is already awaiting a response.".into());
        }
        self.queue(
            self.pending
                .clone()
                .ok_or("There is no uncertain launch to reconcile.")?,
        )
    }

    fn queue(&mut self, launch: Launch) -> Result<(), String> {
        if self.synthetic {
            self.state.launch = Some(LaunchView {
                request_id: launch.request_id,
                phase: "rejected".into(),
                error: Some("Preview only. No training or evaluation was started.".into()),
                receipt: None,
            });
        } else {
            self.worker
                .as_ref()
                .ok_or("Gym is not connected.")?
                .launch
                .try_send(launch.clone())
                .map_err(|_| "Gym is busy. No new request was queued.".to_owned())?;
            self.state.launch = Some(LaunchView {
                request_id: launch.request_id.clone(),
                phase: "sending".into(),
                error: None,
                receipt: None,
            });
            self.pending = Some(launch);
            self.sent_at = Some(Instant::now());
        }
        self.changed();
        Ok(())
    }

    pub fn close_detail(&mut self) {
        self.state.selected_run = None;
        self.state.selected_recipe = None;
        self.changed();
    }

    fn require_active(&self) -> Result<(), String> {
        if self.state.active {
            Ok(())
        } else {
            Err("Enter the Gym before using its boards.".into())
        }
    }
}

fn error_text(error: &Error) -> String {
    match error.code {
        ErrorCode::Expired => "The Gym grant expired. Ask the computer for a new connection.",
        ErrorCode::Revoked | ErrorCode::Forbidden => {
            "The Gym host refused this device or operation."
        }
        ErrorCode::Conflict => {
            "The request or recipe no longer matches the host's retained record."
        }
        ErrorCode::Unavailable | ErrorCode::Transport | ErrorCode::RateLimited => {
            "No confirmed host response. A launch may already exist; retry only the same request."
        }
        _ => "The Gym response could not be verified.",
    }
    .into()
}

fn fixture() -> Snapshot {
    // Clearly labeled synthetic UI evidence, constructed only after entry.
    serde_json::from_value(serde_json::json!({
        "observed_at": gym_bridge::unix_time().unwrap_or(0),
        "runs": [
            {"id":"1".repeat(64),"title":"Synthetic Microcoder experiment","category":"agent","status":"running",
             "completed":24,"total":40,"cost_usd":null,"elapsed_ms":125000,
             "metrics":[{"name":"Recorded checks","unit":"checks","points":[{"step":0,"value":1.0},{"step":8,"value":2.0},{"step":16,"value":5.0},{"step":24,"value":8.0}]}],
             "source":"synthetic","provenance":"Generated UI fixture; not a Microcoder measurement."},
            {"id":"2".repeat(64),"title":"Synthetic Terminal-Bench evaluation","category":"evaluation","status":"completed",
             "completed":8,"total":8,"cost_usd":0.12,"elapsed_ms":95000,
             "metrics":[{"name":"Passed","unit":"cases","points":[{"step":1,"value":1.0},{"step":4,"value":3.0},{"step":8,"value":6.0}]}],
             "source":"synthetic","provenance":"Generated UI fixture; not benchmark evidence."}
        ],
        "recipes":[{"id":"preview-recipe","title":"Preview evaluation recipe","revision":format!("sha256:{}", "a".repeat(64)),
            "budget":{"wall_ms":60000,"max_starts":1,"spend_limit_usd":null,"spend_enforced":false},
            "detail":"Synthetic preview only. The confirmation cannot execute a process or call a model."}],
        "notices":["Synthetic examples. Real boards require a separately authorized Gym connection."]
    })).expect("the fixed Gym fixture matches the portable DTOs")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn board() -> Board {
        Board::new(SecretKey::new(&mut secp256k1::rand::rng()), true)
    }

    #[test]
    fn entry_loads_but_exit_and_background_stop_observation() {
        let mut board = board();
        assert!(board.view().runs.is_empty());
        assert!(board.worker.is_none());
        assert!(board.select_recipe("preview-recipe").is_err());
        board.set_active(true);
        assert_eq!(board.view().runs.len(), 2);
        assert!(board.view().launch.is_none());
        board.set_active(false);
        assert!(board.view().stale);
        assert!(board.confirm_launch().is_err());
        assert!(board.worker.is_none());
    }

    #[test]
    fn a_previous_visit_cannot_update_the_new_board() {
        let mut board = board();
        board.set_active(true);
        let old = board.epoch;
        board.set_active(false);
        board.set_active(true);
        let mut poisoned = fixture();
        poisoned.runs.clear();
        board.apply(old, Update::Snapshot(Ok(poisoned)));
        assert_eq!(board.view().runs.len(), 2);
    }

    #[test]
    fn unknown_cost_and_metric_denominators_survive_projection() {
        let mut board = board();
        board.set_active(true);
        fixture()
            .validate(gym_bridge::unix_time().unwrap())
            .unwrap();
        board.select_run(&"1".repeat(64)).unwrap();
        let run = board.view().selected_run.unwrap();
        assert_eq!(run.cost_usd, None);
        assert_eq!(run.completed, Some(24));
        assert_eq!(run.total, Some(40));
        assert_eq!(run.metrics[0].points.len(), 4);
    }

    #[test]
    fn starting_requires_a_current_reviewed_recipe_and_explicit_confirmation() {
        let mut board = board();
        board.set_active(true);
        assert!(board.confirm_launch().is_err());
        board.select_recipe("preview-recipe").unwrap();
        assert!(board.view().launch.is_none());
        board.confirm_launch().unwrap();
        assert_eq!(board.view().launch.unwrap().phase, "rejected");
        assert!(board.pending.is_none());
        assert!(board.worker.is_none());
        assert!(board.view().selected_recipe.is_none());
        assert!(board.confirm_launch().is_err());
    }

    #[test]
    fn stale_and_changed_recipes_cannot_be_confirmed() {
        let mut board = board();
        board.set_active(true);
        board.select_recipe("preview-recipe").unwrap();
        let mut changed = fixture();
        changed.recipes[0].revision = "b".repeat(64);
        board.apply_snapshot(changed);
        assert!(board.confirm_launch().is_err());
        board.state.stale = true;
        assert!(board.select_recipe("preview-recipe").is_err());
    }

    #[test]
    fn leaving_during_launch_keeps_identity_and_marks_the_outcome_unknown() {
        let mut board = board();
        board.set_active(true);
        board.pending = Some(Launch {
            request_id: "retained".into(),
            recipe_id: "recipe".into(),
            revision: "a".repeat(64),
        });
        board.state.launch = Some(LaunchView {
            request_id: "retained".into(),
            phase: "sending".into(),
            error: None,
            receipt: None,
        });
        board.set_active(false);
        assert_eq!(board.view().launch.unwrap().phase, "unknown");
        assert_eq!(board.pending.as_ref().unwrap().request_id, "retained");
        assert!(board.configure("another host").is_err());
    }

    #[test]
    fn confirmation_and_reconciliation_queue_one_identical_logical_request() {
        let mut board = board();
        board.set_active(true);
        board.synthetic = false;
        let (launch, mut requests) = async_mpsc::channel(2);
        let (_, updates) = mpsc::sync_channel(8);
        let (cancel, _) = watch::channel(false);
        board.worker = Some(Worker {
            launch,
            updates,
            cancel,
        });
        board.select_recipe("preview-recipe").unwrap();
        board.confirm_launch().unwrap();
        let first = requests.try_recv().unwrap();
        assert!(board.confirm_launch().is_err());
        assert!(board.retry_launch().is_err());
        assert!(requests.try_recv().is_err());
        // An unverified reply after dispatch cannot prove refusal. Keep its
        // exact logical request even if the attacker used a forbidden shape.
        board.apply(
            board.epoch,
            Update::Launch(
                first.request_id.clone(),
                Err(Error::new(ErrorCode::Forbidden, "wrong reply signer")),
            ),
        );
        assert_eq!(board.view().launch.unwrap().phase, "unknown");
        board.retry_launch().unwrap();
        let retried = requests.try_recv().unwrap();
        assert_eq!(first.request_id, retried.request_id);
        assert_eq!(first.recipe_id, retried.recipe_id);
        assert_eq!(first.revision, retried.revision);
        assert!(requests.try_recv().is_err());
    }
}
