//! Group mode (#12402): a pylon that heads a model group instead of serving
//! a model itself.
//!
//! The head supervises the group's stage workers through
//! [`crate::stage_group::StageGroup`] (W10, #12397), publishes the group's
//! one routable head beacon and a beacon for each stage (W13, #12400), and
//! answers NIP-CJ jobs by driving the group: the conversation goes to the
//! first stage, the last stage samples with the head's settings, and the
//! tokens come back through the head. When the group stops for good, the
//! head publishes its beacons offline and stops publishing, so no reader
//! keeps a group that reports `Recovering` forever.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nostr::pylon::Layers;
use serde_json::json;
use tokio::sync::oneshot;

use crate::engine::{Engine, Finish, Generation, Pending, Sampling, Turn};
use crate::identity::{Identity, load_owner, npub};
use crate::lease::{Dedicated, Machine};
use crate::now;
use crate::online::{Log, Phase, Shared, add, set};
use crate::provider::{Counters, HeadGroup, Provider};
use crate::share::Settings;
use crate::stage_group::StageGroup;

/// The engine a group head runs jobs on: one request at a time through
/// the stage group.
pub struct GroupEngine {
    group: Arc<StageGroup>,
    /// `<name>@<artifact>`.
    model: String,
    sampling: Sampling,
    max_output: u32,
    /// Where each answer's timing goes: first-token time and decode rate.
    log: Option<Log>,
}

impl GroupEngine {
    #[must_use]
    pub fn new(group: Arc<StageGroup>, model: String, sampling: Sampling, max_output: u32) -> Self {
        Self {
            group,
            model,
            sampling,
            max_output,
            log: None,
        }
    }

    /// Log each answer's first-token time and decode rate to `log`.
    #[must_use]
    pub fn with_log(mut self, log: Log) -> Self {
        self.log = Some(log);
        self
    }
}

impl Engine for GroupEngine {
    fn model(&self) -> &str {
        &self.model
    }

    fn healthy(&self) -> Pending<'_, bool> {
        let status = self.group.status();
        Box::pin(async move { status.ready && status.failed.is_none() })
    }

    fn generate<'a>(
        &'a self,
        turns: &'a [Turn],
        max_tokens: u32,
    ) -> Pending<'a, Result<Generation, String>> {
        Box::pin(async move {
            let steps = max_tokens.clamp(1, self.max_output);
            let prompt = json!(
                turns
                    .iter()
                    .map(|t| json!({"role": t.role, "content": t.content}))
                    .collect::<Vec<_>>()
            );
            let started = std::time::Instant::now();
            let mut first: Option<std::time::Instant> = None;
            let mut run = self
                .group
                .generate_prompt(prompt, u64::from(steps), self.sampling)
                .await?;
            let (mut ids, mut text) = (Vec::new(), String::new());
            let (mut reasoning, mut content, mut parsed) = (String::new(), String::new(), false);
            let (mut tokens_open, mut text_open, mut answer_open) = (true, true, true);
            while tokens_open || text_open || answer_open {
                tokio::select! {
                    token = run.tokens.recv(), if tokens_open => match token {
                        Some(token) => {
                            first.get_or_insert_with(std::time::Instant::now);
                            ids.push(token);
                        }
                        None => tokens_open = false,
                    },
                    piece = run.text.recv(), if text_open => match piece {
                        Some(piece) => text.push_str(&piece),
                        None => text_open = false,
                    },
                    piece = run.answer.recv(), if answer_open => match piece {
                        Some(piece) => {
                            parsed = true;
                            reasoning.push_str(&piece.reasoning);
                            content.push_str(&piece.content);
                        }
                        None => answer_open = false,
                    },
                }
            }
            let done = run
                .done
                .await
                .map_err(|_| "the stage group stopped".to_string())?
                .map_err(|failure| failure.to_string())?;
            if let (Some(log), Some(first)) = (&self.log, first) {
                let decode = first.elapsed().as_secs_f64();
                let rate = if ids.len() > 1 && decode > 0.0 {
                    (ids.len() - 1) as f64 / decode
                } else {
                    0.0
                };
                log(format!(
                    "model group answered {} tokens: first token after {} ms, then {rate:.2} tokens/s",
                    ids.len(),
                    (first - started).as_millis()
                ));
            }
            // A last stage that parses the answer (W11's V4.1 worker) splits
            // reasoning from the final answer at the model's own `</think>`
            // token and says how the answer ended.
            let reported = match done.finish.as_deref() {
                None => None,
                Some("stop") => Some(Finish::Stop),
                Some("length") => Some(Finish::Length),
                Some(other) => {
                    return Err(format!(
                        "the model ended its answer with {other}, not a final text answer"
                    ));
                }
            };
            if parsed || reported.is_some() {
                if reported == Some(Finish::Stop) && content.trim().is_empty() {
                    return Err(if reasoning.is_empty() {
                        "model returned an empty answer".into()
                    } else {
                        "model returned reasoning without a final answer".into()
                    });
                }
                text = content;
            } else if text.is_empty() {
                // Stages that decode nothing answer with the token ids.
                text = ids.iter().map(u32::to_string).collect::<Vec<_>>().join(" ");
            }
            Ok(Generation {
                text,
                input_tokens: None,
                output_tokens: Some(done.tokens),
                model: self.model.clone(),
                // Fewer tokens than asked for means the last stage sampled
                // the end of the answer.
                finish: if let Some(finish) = reported {
                    finish
                } else if done.tokens < u64::from(steps) {
                    Finish::Stop
                } else {
                    Finish::Length
                },
            })
        })
    }

    /// A dropped request cancels the group's run; the group holds admission
    /// until every stage has stopped (W10), and so does the head's slot.
    fn settled(&self) -> Pending<'_, ()> {
        let mut changes = self.group.watch();
        Box::pin(async move {
            loop {
                let status = changes.borrow_and_update().clone();
                if !status.busy || status.failed.is_some() {
                    return;
                }
                if changes.changed().await.is_err() {
                    return;
                }
            }
        })
    }
}

/// Head the group in `settings` until `halt` resolves, or until the group
/// stops for good: then publish its beacons offline, record why in
/// `live.json`, and wait for `halt`.
pub(crate) async fn run(
    home: PathBuf,
    settings: Settings,
    machine: Arc<dyn Machine>,
    live: Shared,
    log: Log,
    mut halt: oneshot::Receiver<()>,
) {
    let Some(lane) = settings.group.clone() else {
        return;
    };
    let pause = |why: String| {
        set(&live, |l| {
            l.state = Phase::Paused;
            l.why = Some(why);
        });
    };
    if let Err(why) = lane.validate() {
        log(format!("model group not started: {why}"));
        pause(why);
        let _ = halt.await;
        return;
    }
    set(&live, |l| {
        l.state = Phase::Starting;
        l.why = Some("starting the model group's stages".into());
        l.pylon = Some(settings.pylon.clone());
        l.model = Some(format!("{}@{}", lane.model, lane.artifact));
        l.lanes.clear();
        l.serving_since = None;
    });
    let group = match StageGroup::start(lane.stage_config(), Arc::clone(&log)).await {
        Ok(group) => Arc::new(group),
        Err(why) => {
            log(format!("model group not started: {why}"));
            pause(why);
            let _ = halt.await;
            return;
        }
    };
    let built = (|| {
        let mut config = settings.config(&home);
        config.owner = load_owner(&home)?;
        config.slots = 1;
        config.max_tokens = settings.max_tokens.clamp(1, lane.max_output_tokens);
        config.job_timeout = Duration::from_secs(lane.deadline_secs);
        config.group = Some(HeadGroup {
            spec: lane.spec(),
            readiness: Arc::clone(&group) as Arc<dyn crate::group::GroupReadiness>,
            stages: lane
                .stages
                .iter()
                .map(|s| {
                    (
                        s.name.clone(),
                        Layers {
                            from: s.from,
                            to: s.to,
                        },
                    )
                })
                .collect(),
        });
        let identity = Identity::load_or_create(&home.join("provider.key"))?;
        let engine = Arc::new(
            GroupEngine::new(
                Arc::clone(&group),
                format!("{}@{}", lane.model, lane.artifact),
                lane.sampling,
                lane.max_output_tokens,
            )
            .with_log(Arc::clone(&log)),
        );
        let machine: Arc<dyn Machine> = if settings.dedicated {
            Arc::new(Dedicated)
        } else {
            machine
        };
        Provider::on(config, identity, engine, machine)
    })();
    let provider = match built {
        Ok(provider) => provider,
        Err(why) => {
            log(format!("model group not served: {why}"));
            group.stop().await;
            pause(format!("could not start: {why}"));
            let _ = halt.await;
            return;
        }
    };
    set(&live, |l| {
        l.npub = Some(npub(provider.pubkey()));
        l.serving_since = Some(now());
    });
    let (stop_provider, stopped) = oneshot::channel::<()>();
    let serving = tokio::spawn(Arc::clone(&provider).run(async {
        let _ = stopped.await;
    }));
    let mut changes = group.watch();
    let mut seen = Counters::default();
    let ended = loop {
        let status = changes.borrow_and_update().clone();
        provider.refresh_models().await;
        let counters = provider.counters().await;
        set(&live, |l| {
            add(l, seen, counters);
            (l.state, l.why) = if let Some(failed) = &status.failed {
                (
                    Phase::Paused,
                    Some(format!("the model group stopped: {failed}")),
                )
            } else if status.ready {
                (Phase::Serving, None)
            } else {
                (Phase::Starting, Some(status.reason.clone()))
            };
            l.group = Some(status.clone());
        });
        seen = counters;
        if let Some(failed) = status.failed {
            break Some(failed);
        }
        tokio::select! {
            _ = &mut halt => break None,
            changed = changes.changed() => if changed.is_err() { break Some("the stage group ended".into()) },
            () = tokio::time::sleep(Duration::from_secs(1)) => {}
        }
    };
    if let Some(reason) = &ended {
        log(format!(
            "model group {} stopped for good: {reason}; its beacons go offline",
            lane.id
        ));
    }
    // Offline beacons for the head and every stage, then no more.
    let _ = stop_provider.send(());
    let _ = serving.await;
    group.stop().await;
    let counters = provider.counters().await;
    let status = group.status();
    set(&live, |l| {
        add(l, seen, counters);
        l.group = Some(status);
    });
    if ended.is_some() {
        let _ = halt.await;
    }
}
