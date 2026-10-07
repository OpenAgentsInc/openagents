//! The host's half of a workshop agent's day plan (`agent_plan`): the
//! `plan` standing job makes the morning plan on a thread of its own, each
//! sweep moves the plan to the block under way and decomposes a block that
//! just started, and requests and job occurrences become the plan's
//! events. Everything is admitted the way a job's occurrence is: only
//! while the `plan` job is on, and against its budget and the capacity
//! book. With no plan for the day, nothing here changes how she works.

use world_tree::Known;

use super::super::agent_jobs::Trigger;
use super::super::agent_plan::{self, Event, Reacted, Reaction};
use super::*;

/// What one sweep's job occurrence was, for the plan.
pub(super) fn occurrence_event(jobs: &[agent_jobs::Job], job: &str, text: &str) -> Event {
    let scheduled = jobs
        .iter()
        .find(|j| j.job == job)
        .is_some_and(|j| matches!(j.trigger, Trigger::Schedule { .. }));
    if scheduled {
        return Event::Job {
            job: job.into(),
            text: text.into(),
        };
    }
    // A watched issue names itself; anything else is its job's.
    let source = text
        .split_once("Issue #")
        .and_then(|(_, rest)| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            (!digits.is_empty()).then(|| format!("issue:{digits}"))
        })
        .unwrap_or_else(|| format!("job:{job}"));
    let what = text.lines().filter(|l| !l.trim().is_empty()).last();
    Event::Observed {
        source,
        text: what.unwrap_or(text).to_string(),
    }
}

impl Agents {
    /// Plan with the services `planner` makes instead of her live model
    /// and Jev, as a test does.
    #[must_use]
    pub fn with_planner(mut self, planner: agent_plan::ServicesFactory) -> Self {
        self.planner = planner;
        self
    }

    fn plan_job(store: &Store) -> Option<agent_jobs::Job> {
        Jobs::new(store.clone())
            .load()
            .ok()?
            .into_iter()
            .find(|j| matches!(j.trigger, Trigger::Plan { .. }))
    }

    fn bound(job: Option<&agent_jobs::Job>) -> String {
        match job.map(|j| &j.trigger) {
            Some(Trigger::Plan {
                bound: Some(bound), ..
            }) => bound.clone(),
            _ => agent_plan::HOUSE.into(),
        }
    }

    fn known(store: &Store) -> Known {
        let tree = world_tree::everglade();
        super::super::agent_place::load_known(store.dir(), store.name(), tree)
            .unwrap_or_else(|_| Known::new(store.name(), tree))
    }

    /// Takes the planning slot for `name`, or says another plan call holds
    /// it.
    fn take_planning(&self, name: &str) -> bool {
        self.planning
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(name.to_string())
    }

    fn give_planning(planning: &Arc<Mutex<BTreeSet<String>>>, name: &str) {
        planning
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(name);
    }

    /// Runs a plan job's occurrence on a thread of its own: reads her real
    /// sources, drafts the day, stores and journals the plan, and meters
    /// the call to the job's budget. A failure is journaled and keeps the
    /// old plan.
    pub(super) fn plan_day(&self, store: &Store, job: &str, now: u64) {
        let name = store.name().to_string();
        if !self.take_planning(&name) {
            let _ = store.append(&Entry::new(
                now,
                Kind::Job,
                &format!(
                    "job {job} skipped: {} is planning already",
                    store.refer().they()
                ),
            ));
            return;
        }
        let agents = self.clone();
        let store = store.clone();
        let job = job.to_string();
        let queued: Vec<(String, String)> = self
            .lock()
            .live
            .get(&name)
            .map(|live| {
                live.queue
                    .iter()
                    .enumerate()
                    .map(|(i, q)| (format!("request:{}", i + 1), q.queued.text.clone()))
                    .collect()
            })
            .unwrap_or_default();
        std::thread::spawn(move || {
            let result = agents.draft_day(&store, &queued, now);
            match result {
                Ok(made) => {
                    let _ = agent_plan::replace(&store, &made.plan);
                    let _ = agent_plan::journal_made(&store, &made);
                    let _ = Jobs::new(store.clone()).meter(&job, made.usd);
                }
                Err(why) => {
                    let _ = store.append(&Entry::new(
                        now,
                        Kind::Job,
                        &format!("job {job} planned nothing: {why}"),
                    ));
                }
            }
            Self::give_planning(&agents.planning, store.name());
        });
    }

    fn draft_day(
        &self,
        store: &Store,
        queued: &[(String, String)],
        now: u64,
    ) -> Result<agent_plan::Made, String> {
        let jobs = Jobs::new(store.clone()).load()?;
        let plan_job = jobs
            .iter()
            .find(|j| matches!(j.trigger, Trigger::Plan { .. }));
        let utc_offset = match plan_job.map(|j| &j.trigger) {
            Some(Trigger::Plan { utc_offset, .. }) => *utc_offset,
            _ => 0,
        };
        let bound = Self::bound(plan_job);
        let mut issues: Vec<(u64, String)> = Vec::new();
        for job in jobs.iter().filter(|j| j.enabled) {
            let Trigger::Issues { repository, label } = &job.trigger else {
                continue;
            };
            let Ok((open, pulls)) = self.facts.issues(repository, label) else {
                continue;
            };
            if let Ok(picked) = super::super::issue_pick::choose(&open, &pulls, &[], now, 24) {
                issues.extend(picked.into_iter().map(|p| (p.number, p.title)));
            }
        }
        let memory = Memory::new(store.clone(), self.screen.clone()).entries()?;
        let tree = world_tree::everglade();
        let known = Self::known(store);
        let inputs = agent_plan::Inputs {
            agent: store.name(),
            now,
            utc_offset,
            jobs: &jobs,
            issues: &issues,
            queued,
            memory: &memory,
            tree,
            known: &known,
            bound: &bound,
        };
        // An idle day makes no call, so it needs no model.
        let sources = agent_plan::sources(&inputs);
        if sources.iter().all(|s| s.slot.is_some()) {
            let mut none = agent_plan::Scripted::default();
            return agent_plan::draft(&inputs, &mut none, &self.screen);
        }
        let mut services = (self.planner)(store)?;
        agent_plan::draft(&inputs, services.writer.as_mut(), &self.screen)
    }

    /// Once a sweep: moves today's plan to the block under way, journals a
    /// block that started, and decomposes it on a thread of its own when
    /// the plan job admits the call.
    pub(super) fn follow_plan(&self, store: &Store, record: &Record, now: u64) {
        let started = agent_plan::update(store, now, |plan| {
            let (_, minute) = agent_plan::local(now, plan.utc_offset);
            agent_plan::begin(plan, minute).map(|index| (index, minute, plan.clone()))
        });
        let Ok(Some(Some((index, minute, plan)))) = started else {
            return;
        };
        let Some(block) = plan.blocks.get(index).cloned() else {
            return;
        };
        let _ = store.append(&Entry::new(
            now,
            Kind::Plan,
            &format!(
                "day plan block {}-{} started: {} [{}] at {}",
                coder_host::access::day_plan::clock(block.start),
                coder_host::access::day_plan::clock(block.end),
                block.title,
                block.source,
                block.node
            ),
        ));
        let Some(job) = Self::plan_job(store) else {
            return;
        };
        if !job.enabled || agent_jobs::admit(&job, record, self.facts.as_ref(), now).is_err() {
            return;
        }
        if !self.take_planning(store.name()) {
            return;
        }
        let agents = self.clone();
        let store = store.clone();
        std::thread::spawn(move || {
            let decomposed = (agents.planner)(&store).and_then(|mut services| {
                agent_plan::decompose(
                    &plan,
                    &block.title,
                    minute,
                    services.writer.as_mut(),
                    &agents.screen,
                )
            });
            match decomposed {
                Ok((steps, usd)) => {
                    let count = steps.len();
                    let _ = agent_plan::update(&store, now, |plan| {
                        if plan.current.and_then(|i| usize::try_from(i).ok()) == Some(index) {
                            plan.steps = steps;
                        }
                    });
                    let _ = store.append(&Entry::new(
                        now,
                        Kind::Plan,
                        &format!("day plan block {} has {count} steps", block.source),
                    ));
                    let _ = Jobs::new(store.clone()).meter(&job.job, usd);
                }
                Err(why) => {
                    let _ = store.append(&Entry::new(
                        now,
                        Kind::Plan,
                        &format!("day plan block {} has no steps: {why}", block.source),
                    ));
                }
            }
            Self::give_planning(&agents.planning, store.name());
        });
    }

    /// Applies `event` to today's plan, when there is one: code decides
    /// the owner's request and a scheduled job, and Jev the rest. Journals
    /// what it decided. `None` when there is no plan for today.
    pub(super) fn plan_event(&self, store: &Store, event: &Event, now: u64) -> Option<Reacted> {
        agent_plan::today(store, now)?;
        let job = Self::plan_job(store);
        if !job.as_ref().is_some_and(|j| j.enabled) {
            return None;
        }
        let tree = world_tree::everglade();
        let known = Self::known(store);
        let bound = Self::bound(job.as_ref());
        let places = agent_plan::places(tree, &known, &bound);
        let mut judge: Box<dyn agent_plan::Judge> = match event {
            Event::Observed { .. } => match (self.planner)(store) {
                Ok(services) => services.judge,
                Err(_) => Box::new(agent_plan::Answers::default()),
            },
            _ => Box::new(agent_plan::Answers::default()),
        };
        let reacted = agent_plan::update(store, now, |plan| {
            agent_plan::react(plan, event, now, &places, judge.as_mut())
        })
        .ok()
        .flatten()?;
        let what = match event {
            Event::Owner { source, .. } | Event::Observed { source, .. } => source.clone(),
            Event::Job { job, .. } => format!("job:{job}"),
        };
        let _ = store.append(&Entry::new(
            now,
            Kind::Plan,
            &format!(
                "day plan event {what}: {} by {}{}",
                reacted.reaction.as_str(),
                reacted.by,
                if reacted.replanned {
                    ", re-planned from the current block on"
                } else {
                    ""
                }
            ),
        ));
        Some(reacted)
    }

    /// Moves the request `ask` just queued for `name` to the front of her
    /// queue, as reacting now does.
    pub(super) fn hurry(&self, name: &str, reacted: Option<&Reacted>) {
        if reacted.is_some_and(|r| r.reaction == Reaction::ReactNow) {
            self.with_live(name, |live| {
                if let Some(queued) = live.queue.pop_back() {
                    live.queue.push_front(queued);
                }
            });
        }
    }
}
