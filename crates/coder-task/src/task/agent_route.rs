//! What the owner's request to a workshop agent asks for, as one typed
//! question set (`questions/agent-request.json`) that Jev answers.
//!
//! The workspace rule forbids routing a request by the words in it, so the
//! Merge station, a note to keep, a standing preference, and task mode are
//! all read from Jev's answers here, each above a provisional threshold.
//! Below a threshold, or when Jev isn't set up or doesn't answer, she
//! abstains: the request is ordinary work in terminal mode and nothing is
//! stored. A merge never happens on this answer alone; the owner still
//! confirms it at her lectern.

use std::collections::VecDeque;
use std::sync::{Arc, LazyLock, Mutex};

use serde_json::{Value, json};

use super::agent::Store;
use crate::questions::{Fill, Set};

const SET_JSON: &str = include_str!("../../../../questions/agent-request.json");

static SET: LazyLock<Set> = LazyLock::new(|| {
    let set: Set = serde_json::from_str(SET_JSON).expect("the agent-request set parses");
    set.validate()
        .expect("the agent-request set is one this host asks");
    set
});

/// The route question.
pub const ROUTE: &str = "route";
/// The standing-preference question.
pub const STATES_PREFERENCE: &str = "states_preference";
/// The task-mode question.
pub const CHANGES_FILES: &str = "changes_files";

/// The probability of a non-work route at or above which she takes it.
/// Provisional: a measurement document must calibrate it.
pub const ROUTE_AT: f64 = 0.7;
/// The probability of `merge_own` at or above which she asks the owner to
/// confirm a merge. Provisional, as [`ROUTE_AT`]; higher, since the merge
/// lands on the owner's branch.
pub const MERGE_AT: f64 = 0.85;
/// The probability of `states_preference` at or above which the request
/// becomes a candidate preference. Provisional, as [`ROUTE_AT`].
pub const PREFERENCE_AT: f64 = 0.8;
/// The probability of `changes_files` at or above which an unspecified
/// mode is task mode. Provisional, as [`ROUTE_AT`].
pub const TASK_AT: f64 = 0.6;
/// The most bytes of the request the state carries.
const REQUEST_MAX: usize = 6000;

/// The agent-request question set.
#[must_use]
pub fn request_set() -> &'static Set {
    &SET
}

/// What a request asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Route {
    /// Ordinary work, which is also what she does when she abstains.
    #[default]
    Work,
    /// Where the Merge station is.
    MergeStationWhere,
    /// Merge her own change, after the owner confirms.
    MergeOwn,
    /// Keep a note.
    Remember,
}

impl Route {
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "work" => Self::Work,
            "merge_station_where" => Self::MergeStationWhere,
            "merge_own" => Self::MergeOwn,
            "remember" => Self::Remember,
            _ => return None,
        })
    }
}

/// Jev's answers for one request.
#[derive(Clone, Debug, PartialEq)]
pub struct Answers {
    /// The route the Choice picked.
    pub route: Route,
    /// The probability of that route.
    pub route_p: f64,
    /// The probability the request states a standing preference.
    pub preference: f64,
    /// The probability doing the request changes files.
    pub changes_files: f64,
}

/// What she does with a request, after the thresholds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Routing {
    pub route: Route,
    /// Keep the request as a candidate preference.
    pub preference: bool,
    /// An unspecified mode is task mode.
    pub task: bool,
}

impl Routing {
    /// What she does when nothing answered or nothing cleared a threshold:
    /// ordinary work, in terminal mode, storing nothing.
    #[must_use]
    pub fn abstain() -> Self {
        Self::default()
    }
}

/// The routing `answers` make: each route, preference, and task mode only
/// at or above its threshold, and otherwise the abstention.
#[must_use]
pub fn decide(answers: &Answers) -> Routing {
    let route = match answers.route {
        Route::MergeOwn if answers.route_p >= MERGE_AT => Route::MergeOwn,
        Route::MergeOwn => Route::Work,
        route if answers.route_p >= ROUTE_AT => route,
        _ => Route::Work,
    };
    Routing {
        route,
        preference: route == Route::Work && answers.preference >= PREFERENCE_AT,
        task: route == Route::Work && answers.changes_files >= TASK_AT,
    }
}

/// What Jev reads about one request.
#[must_use]
pub fn route_state(request: &str) -> Value {
    let mut end = request.len().min(REQUEST_MAX);
    while !request.is_char_boundary(end) {
        end -= 1;
    }
    json!({ "request": request[..end].trim() })
}

/// Reads what a request asks for.
pub trait Router {
    /// The answers over `state`, the shape [`route_state`] builds.
    ///
    /// # Errors
    /// When nothing answered.
    fn route(&mut self, state: &Value) -> Result<Answers, String>;
}

/// Jev answers `questions/agent-request.json`.
pub struct JevRouter {
    client: jev::Client,
    runtime: tokio::runtime::Runtime,
}

impl JevRouter {
    /// # Errors
    /// When the runtime doesn't start.
    pub fn new(client: jev::Client) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("cannot start a runtime: {e}"))?;
        Ok(Self { client, runtime })
    }
}

impl Router for JevRouter {
    fn route(&mut self, state: &Value) -> Result<Answers, String> {
        use jev::Answer;
        let size = serde_json::to_vec(state).map_or(usize::MAX, |b| b.len());
        if let Some(max) = SET.policy.state_max_bytes
            && size as u64 > max
        {
            return Err(format!("the state is {size} bytes, over the set's {max}"));
        }
        let request = jev::SystemOneRequest::new(state.clone(), SET.build(&Fill::None)?);
        let response = self
            .runtime
            .block_on(self.client.system_one(request))
            .map_err(|e| format!("Jev: {e}"))?;
        let noul = |id: &str| match response.answers.get(id) {
            Some(Answer::Noul(answer)) => Ok(answer.noul),
            _ => Err(format!("Jev didn't answer `{id}`")),
        };
        let (route, route_p) = match response.answers.get(ROUTE) {
            Some(Answer::Choice(choice)) => {
                let route = Route::parse(&choice.choice)
                    .ok_or_else(|| format!("Jev chose an unknown route: {}", choice.choice))?;
                let p = choice
                    .probabilities
                    .get(&choice.choice)
                    .copied()
                    .unwrap_or(choice.confidence);
                (route, p)
            }
            _ => return Err(format!("Jev didn't answer `{ROUTE}`")),
        };
        Ok(Answers {
            route,
            route_p,
            preference: noul(STATES_PREFERENCE)?,
            changes_files: noul(CHANGES_FILES)?,
        })
    }
}

/// Recorded answers, in order, for tests. When they run out it refuses,
/// and she abstains.
#[derive(Clone, Debug, Default)]
pub struct ScriptedRouter {
    pub answers: Arc<Mutex<VecDeque<Answers>>>,
    pub states: Arc<Mutex<Vec<Value>>>,
}

impl ScriptedRouter {
    #[must_use]
    pub fn new(answers: Vec<Answers>) -> Self {
        Self {
            answers: Arc::new(Mutex::new(answers.into())),
            states: Arc::default(),
        }
    }
}

impl Router for ScriptedRouter {
    fn route(&mut self, state: &Value) -> Result<Answers, String> {
        if let Ok(mut states) = self.states.lock() {
            states.push(state.clone());
        }
        self.answers
            .lock()
            .map_err(|_| "the recorded answers are poisoned".to_string())?
            .pop_front()
            .ok_or_else(|| "no more answers were recorded".to_string())
    }
}

/// Makes the [`Router`] for one agent's request, or `None` when nothing
/// can answer, so she abstains.
pub type RouterFactory = Arc<dyn Fn(&Store) -> Option<Box<dyn Router>> + Send + Sync>;

/// Jev from the decision profile, or nothing when it isn't set up or the
/// agent's model path is unavailable. A unit test has none.
#[must_use]
pub fn default_router() -> RouterFactory {
    Arc::new(|store: &Store| {
        if cfg!(test) || super::sales::privacy::model_available(store).is_err() {
            return None;
        }
        match crate::decision::from_env() {
            Ok(Some(client)) => JevRouter::new(client)
                .ok()
                .map(|router| Box::new(router) as Box<dyn Router>),
            _ => None,
        }
    })
}

/// A factory that hands out `router` for every request, as a test does.
#[must_use]
pub fn scripted(router: ScriptedRouter) -> RouterFactory {
    Arc::new(move |_store: &Store| Some(Box::new(router.clone()) as Box<dyn Router>))
}

/// The routing for `request`: `router`'s answers through [`decide`], or
/// the abstention when there is no router or it doesn't answer.
#[must_use]
pub fn route(router: Option<Box<dyn Router>>, request: &str) -> Routing {
    router
        .and_then(|mut router| router.route(&route_state(request)).ok())
        .map_or_else(Routing::abstain, |answers| decide(&answers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answers(route: Route, route_p: f64, preference: f64, changes_files: f64) -> Answers {
        Answers {
            route,
            route_p,
            preference,
            changes_files,
        }
    }

    #[test]
    fn the_set_parses_and_names_its_questions() {
        let set = request_set();
        assert_eq!(set.id, "openagents.agent-request.v1");
        for id in [ROUTE, STATES_PREFERENCE, CHANGES_FILES] {
            assert!(set.questions.contains_key(id), "{id}");
        }
        let criteria = &set.questions[ROUTE]["criteria"];
        for word in ["work", "merge_station_where", "merge_own", "remember"] {
            assert!(Route::parse(word).is_some() && criteria.get(word).is_some());
        }
    }

    #[test]
    fn without_a_router_every_request_is_plain_work() {
        // No words route anything: with nothing to answer, a merge, a
        // note, a preference, and task mode are all off.
        for text in [
            "please merge your change",
            "Could you explain this merge conflict for me?",
            "don't merge this",
            "Remember to fix the login bug",
            "remember that mobile is its own workspace",
            "I never got the email",
            "Never mind, show me the diff",
            "always run the tests first",
            "fix the typo in the README",
        ] {
            assert_eq!(route(None, text), Routing::abstain(), "{text}");
        }
    }

    #[test]
    fn each_answer_acts_only_above_its_threshold() {
        let merge = decide(&answers(Route::MergeOwn, 0.95, 0.0, 0.0));
        assert_eq!(merge.route, Route::MergeOwn);
        // A likely-but-unsure merge is work, not a merge.
        let unsure = decide(&answers(Route::MergeOwn, 0.8, 0.0, 0.0));
        assert_eq!(unsure, Routing::abstain());
        assert_eq!(
            decide(&answers(Route::Remember, 0.9, 0.0, 0.0)).route,
            Route::Remember
        );
        assert_eq!(
            decide(&answers(Route::Remember, 0.5, 0.0, 0.0)),
            Routing::abstain()
        );
        assert!(decide(&answers(Route::Work, 0.9, 0.9, 0.0)).preference);
        assert!(!decide(&answers(Route::Work, 0.9, 0.3, 0.0)).preference);
        assert!(decide(&answers(Route::Work, 0.9, 0.0, 0.7)).task);
        assert!(!decide(&answers(Route::Work, 0.9, 0.0, 0.4)).task);
        // A note or a merge stores no preference and picks no mode.
        let note = decide(&answers(Route::Remember, 0.9, 0.9, 0.9));
        assert!(!note.preference && !note.task);
    }

    #[test]
    fn a_router_that_fails_abstains_and_reads_only_the_request() {
        let router = ScriptedRouter::new(vec![]);
        let states = router.states.clone();
        assert_eq!(
            route(Some(Box::new(router)), "please merge it"),
            Routing::abstain()
        );
        assert_eq!(
            states.lock().unwrap()[0],
            json!({"request": "please merge it"})
        );
        let long = "é".repeat(REQUEST_MAX);
        let state = route_state(&long);
        assert!(state["request"].as_str().unwrap().len() <= REQUEST_MAX);
    }
}
