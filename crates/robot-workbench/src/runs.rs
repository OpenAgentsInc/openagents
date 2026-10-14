//! Explicit experiment execution. Models stay on their owning worker thread;
//! viewing a saved document never calls this adapter.
use crate::{
    experiments::{Config, Session},
    host::Host,
};
use robot_lab::Reference;
use robot_sim::{jobs::Book, runner::Mode};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Pause,
    Resume,
    Step,
    Stop,
    Reset { next_job: String, seed: u64 },
}
struct Command {
    action: Action,
    reply: mpsc::Sender<Result<serde_json::Value, String>>,
}
pub struct Runs {
    pub config: Config,
    active: Arc<Mutex<BTreeMap<String, mpsc::SyncSender<Command>>>>,
    handles: Arc<Mutex<BTreeMap<String, String>>>,
}
impl Runs {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            active: Arc::new(Mutex::new(BTreeMap::new())),
            handles: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
    pub fn start(
        &self,
        mut host: Host,
        experiment: Reference,
        seed: u64,
        job: String,
        batch: bool,
    ) -> Result<serde_json::Value, String> {
        let (sender, receiver) = mpsc::sync_channel::<Command>(8);
        {
            let mut active = self
                .active
                .lock()
                .map_err(|_| "The run controls couldn't be opened")?;
            if active.len() >= 2 || active.contains_key(&job) {
                return Err("Stop another run or choose a new run name".into());
            }
            active.insert(job.clone(), sender.clone());
        }
        // The worker must not retain its own command registry through Host.
        host.runs = None;
        let config = self.config.clone();
        let active = self.active.clone();
        let handles = self.handles.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let key = job.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let mut session = Session::start(
                    host,
                    experiment,
                    seed,
                    &job,
                    if batch {
                        Mode::Batch
                    } else {
                        Mode::Interactive
                    },
                    &config,
                )?;
                ready_tx
                    .send(Ok(status(&session)))
                    .map_err(|_| "The run request ended")?;
                if batch {
                    session.runner.resume();
                }
                while !session.runner.finished() {
                    let command = if session.runner.paused() {
                        receiver.recv_timeout(Duration::from_secs(60)).ok()
                    } else {
                        receiver.try_recv().ok()
                    };
                    if let Some(command) = command {
                        let result = (|| {
                            match command.action {
                                Action::Pause => session.runner.pause(),
                                Action::Resume => session.runner.resume(),
                                Action::Step => {
                                    session.runner.pause();
                                    session.tick(true)?;
                                }
                                Action::Stop => {
                                    session.stop()?;
                                }
                                Action::Reset { next_job, seed } => {
                                    // Keep routing under the original control handle;
                                    // the new durable attempt has its own name.
                                    session.reset(&next_job, seed)?;
                                    if let Ok(mut handles) = handles.lock() {
                                        handles.retain(|_, original| original != &key);
                                        handles.insert(next_job, key.clone());
                                    }
                                }
                            }
                            if !session.runner.finished() {
                                session.tick(false)?;
                            }
                            Ok(status(&session))
                        })();
                        let _ = command.reply.send(result);
                    } else if session.runner.paused() {
                        // An abandoned interactive run saves its partial work.
                        session.stop()?;
                    }
                    if !session.runner.finished() && !session.runner.paused() {
                        session.tick(false)?;
                    }
                }
                Ok::<(), String>(())
            })();
            if let Err(error) = result {
                let _ = ready_tx.send(Err(error));
            }
            if let Ok(mut handles) = handles.lock() {
                handles.retain(|_, original| original != &key);
            }
            if let Ok(mut map) = active.lock() {
                map.remove(&key);
            }
        });
        match ready_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(result) => result,
            Err(_) => {
                let (reply, _) = mpsc::channel();
                let _ = sender.try_send(Command {
                    action: Action::Stop,
                    reply,
                });
                Err("The run didn't start in time".into())
            }
        }
    }
    pub fn control(&self, job: &str, action: Action) -> Result<serde_json::Value, String> {
        let handle = self
            .handles
            .lock()
            .map_err(|_| "The run controls couldn't be opened")?
            .get(job)
            .cloned()
            .unwrap_or_else(|| job.to_owned());
        let sender = self
            .active
            .lock()
            .map_err(|_| "The run controls couldn't be opened")?
            .get(&handle)
            .cloned()
            .ok_or("This run has finished or isn't running here")?;
        let (reply, receive) = mpsc::channel();
        sender
            .try_send(Command { action, reply })
            .map_err(|_| "Wait for the current run command to finish")?;
        receive
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| "The run command didn't finish in time")?
    }
    pub fn inspect(host: &Host, job: &str) -> Result<serde_json::Value, String> {
        let saved = Book::open(host.store.clone())
            .and_then(|b| b.get(job))
            .map_err(|e| e.to_string())?;
        Ok(
            serde_json::json!({"job":saved.id,"experiment":saved.experiment,"seed":saved.seed,"mode":saved.mode,"status":saved.status,"step":saved.recording.as_ref().and_then(|r|r.episode.steps.last()).map(|s|s.index),"episode":saved.episode}),
        )
    }
}
impl Drop for Runs {
    fn drop(&mut self) {
        if let Ok(active) = self.active.lock() {
            for sender in active.values() {
                let (reply, _) = mpsc::channel();
                let _ = sender.try_send(Command {
                    action: Action::Stop,
                    reply,
                });
            }
        }
    }
}
fn status(session: &Session) -> serde_json::Value {
    let job = session.worker.job();
    serde_json::json!({"job":job.id,"status":job.status,"step":session.runner.recording().episode.steps.last().map(|s|s.index),"episode":job.episode})
}
