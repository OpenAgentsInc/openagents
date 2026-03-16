use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::ArcClientError;
use crate::local::LocalArcEnvironment;
use crate::models::{
    ArcCloseScorecardRequest, ArcCompatibilityFrameResponse, ArcComplexActionCommand,
    ArcEnvironmentInfo, ArcOpenScorecardRequest, ArcOpenScorecardResponse, ArcResetCommand,
    ArcScorecardEnvironment, ArcScorecardRunSummary, ArcScorecardSummary, ArcSessionFrame,
};

const MAX_OPAQUE_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcRegisteredEnvironment {
    pub info: ArcEnvironmentInfo,
    pub package_path: PathBuf,
}

impl ArcRegisteredEnvironment {
    #[must_use]
    pub fn new(info: ArcEnvironmentInfo, package_path: impl Into<PathBuf>) -> Self {
        let package_path = package_path.into();
        Self {
            info: info.with_local_package_path(package_path.clone()),
            package_path,
        }
    }

    fn public_info(&self) -> ArcEnvironmentInfo {
        let mut info = self.info.clone();
        info.local_package_path = None;
        info
    }
}

#[derive(Clone)]
pub struct ArcCompatibilityServer {
    state: SharedState,
}

impl ArcCompatibilityServer {
    #[must_use]
    pub fn new(environments: Vec<ArcRegisteredEnvironment>) -> Self {
        Self::new_with_config(environments, ArcCompatibilityServerConfig::default())
    }

    #[must_use]
    pub fn new_with_config(
        environments: Vec<ArcRegisteredEnvironment>,
        config: ArcCompatibilityServerConfig,
    ) -> Self {
        let environments = environments
            .into_iter()
            .map(|environment| (environment.info.game_id.to_string(), environment))
            .collect::<BTreeMap<_, _>>();
        Self {
            state: Arc::new(Mutex::new(ArcCompatibilityServerState {
                environments,
                scorecards: BTreeMap::new(),
                sessions: BTreeMap::new(),
                next_scorecard_id: 1,
                config,
            })),
        }
    }

    #[must_use]
    pub fn router(&self) -> Router {
        Router::new()
            .route("/api/games", get(list_games))
            .route("/api/games/{game_id}", get(get_game))
            .route("/api/scorecard/open", post(open_scorecard))
            .route("/api/scorecard/close", post(close_scorecard))
            .route("/api/scorecard/{card_id}", get(get_scorecard))
            .route(
                "/api/scorecard/{card_id}/{game_id}",
                get(get_scorecard_for_game),
            )
            .route("/api/cmd/RESET", post(reset_command))
            .route("/api/cmd/ACTION1", post(action1_command))
            .route("/api/cmd/ACTION2", post(action2_command))
            .route("/api/cmd/ACTION3", post(action3_command))
            .route("/api/cmd/ACTION4", post(action4_command))
            .route("/api/cmd/ACTION5", post(action5_command))
            .route("/api/cmd/ACTION6", post(action6_command))
            .route("/api/cmd/ACTION7", post(action7_command))
            .route("/api/healthcheck", get(healthcheck))
            .with_state(self.state.clone())
    }

    pub async fn serve(self, listener: tokio::net::TcpListener) -> std::io::Result<()> {
        axum::serve(listener, self.router()).await
    }
}

type SharedState = Arc<Mutex<ArcCompatibilityServerState>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcCompatibilityServerConfig {
    pub stale_after: Option<Duration>,
}

impl Default for ArcCompatibilityServerConfig {
    fn default() -> Self {
        Self {
            stale_after: Some(Duration::from_secs(15 * 60)),
        }
    }
}

struct ArcCompatibilityServerState {
    environments: BTreeMap<String, ArcRegisteredEnvironment>,
    scorecards: BTreeMap<String, CompatibilityScorecardRecord>,
    sessions: BTreeMap<String, CompatibilitySessionRecord>,
    next_scorecard_id: u64,
    config: ArcCompatibilityServerConfig,
}

struct CompatibilitySessionRecord {
    card_id: String,
    game_id: String,
    run_index: usize,
    environment: LocalArcEnvironment,
}

#[derive(Clone)]
struct CompatibilityScorecardRecord {
    owner_api_key: String,
    source_url: Option<String>,
    tags: Vec<String>,
    opaque: Option<serde_json::Value>,
    open_at: OffsetDateTime,
    last_update: OffsetDateTime,
    published_at: Option<OffsetDateTime>,
    closed: bool,
    competition_mode: bool,
    environments: BTreeMap<String, CompatibilityEnvironmentRecord>,
}

#[derive(Clone)]
struct CompatibilityEnvironmentRecord {
    id: arc_core::ArcTaskId,
    level_count: u32,
    runs: Vec<CompatibilityRunRecord>,
}

#[derive(Clone)]
struct CompatibilityRunRecord {
    id: String,
    guid: String,
    score: u32,
    levels_completed: u16,
    actions: u32,
    resets: u32,
    state: arc_core::ArcGameState,
    completed: bool,
    level_count: u32,
    baseline_actions: Vec<u32>,
}

impl CompatibilityScorecardRecord {
    fn new(request: ArcOpenScorecardRequest, owner_api_key: String) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            owner_api_key,
            source_url: request.source_url,
            tags: request.tags,
            opaque: request.opaque,
            open_at: now,
            last_update: now,
            published_at: None,
            closed: false,
            competition_mode: request.competition_mode.unwrap_or(false),
            environments: BTreeMap::new(),
        }
    }

    fn touch(&mut self) {
        self.last_update = OffsetDateTime::now_utc();
    }

    fn close(&mut self, published_at: OffsetDateTime) {
        if !self.closed {
            self.closed = true;
            self.published_at = Some(published_at);
            self.last_update = published_at;
        }
    }

    fn ensure_environment(
        &mut self,
        game_id: &arc_core::ArcTaskId,
        _info: &ArcEnvironmentInfo,
        level_count: u32,
    ) -> &mut CompatibilityEnvironmentRecord {
        self.environments
            .entry(game_id.to_string())
            .or_insert_with(|| CompatibilityEnvironmentRecord {
                id: game_id.clone(),
                level_count,
                runs: Vec::new(),
            })
    }

    fn summary_for(&self, card_id: &str, game_id: Option<&str>) -> ArcScorecardSummary {
        let environments = self
            .environments
            .values()
            .filter(|environment| {
                game_id.is_none_or(|requested| {
                    environment.id.as_str() == requested
                        || environment
                            .id
                            .as_str()
                            .starts_with(&format!("{requested}-"))
                })
            })
            .map(CompatibilityEnvironmentRecord::to_summary)
            .collect::<Vec<_>>();

        let total_environments_completed = environments
            .iter()
            .filter(|environment| environment.completed)
            .count();
        let total_levels_completed = environments
            .iter()
            .map(|environment| u32::from(environment.levels_completed))
            .sum();
        let total_levels = environments
            .iter()
            .map(|environment| environment.level_count.unwrap_or_default())
            .sum();
        let total_actions = environments
            .iter()
            .map(|environment| environment.actions)
            .sum();

        ArcScorecardSummary {
            card_id: card_id.to_owned(),
            score: 0,
            source_url: self.source_url.clone(),
            tags: self.tags.clone(),
            opaque: self.opaque.clone(),
            user_name: None,
            user_id: None,
            published_at: self.published_at.map(format_timestamp),
            open_at: Some(format_timestamp(self.open_at)),
            last_update: Some(format_timestamp(self.last_update)),
            total_environments_completed: Some(
                u32::try_from(total_environments_completed).unwrap_or(u32::MAX),
            ),
            total_environments: Some(u32::try_from(environments.len()).unwrap_or(u32::MAX)),
            total_levels_completed: Some(total_levels_completed),
            total_levels: Some(total_levels),
            total_actions: Some(total_actions),
            environments,
            tags_scores: Vec::new(),
        }
    }
}

impl CompatibilityEnvironmentRecord {
    fn to_summary(&self) -> ArcScorecardEnvironment {
        let score = self
            .runs
            .iter()
            .map(|run| run.score)
            .max()
            .unwrap_or_default();
        let levels_completed = self
            .runs
            .iter()
            .map(|run| run.levels_completed)
            .max()
            .unwrap_or_default();
        let actions = self.runs.iter().map(|run| run.actions).sum();
        let completed = self.runs.iter().any(|run| run.completed);
        let resets = self.runs.iter().map(|run| run.resets).sum();
        ArcScorecardEnvironment {
            id: self.id.clone(),
            runs: self
                .runs
                .iter()
                .map(CompatibilityRunRecord::to_summary)
                .collect(),
            score,
            actions,
            levels_completed,
            completed,
            level_count: Some(self.level_count),
            resets,
        }
    }
}

impl CompatibilityRunRecord {
    fn new(
        info: &ArcEnvironmentInfo,
        guid: String,
        level_count: u32,
        frame: &ArcSessionFrame,
    ) -> Self {
        Self {
            id: info.game_id.to_string(),
            guid,
            score: 0,
            levels_completed: frame.levels_completed,
            actions: 0,
            resets: 0,
            state: frame.game_state,
            completed: is_terminal(frame.game_state),
            level_count,
            baseline_actions: info
                .baseline_actions
                .iter()
                .copied()
                .map(u32::from)
                .collect(),
        }
    }

    fn apply_frame(&mut self, frame: &ArcSessionFrame) {
        self.levels_completed = frame.levels_completed;
        self.state = frame.game_state;
        self.completed = is_terminal(frame.game_state);
    }

    fn to_summary(&self) -> ArcScorecardRunSummary {
        ArcScorecardRunSummary {
            id: self.id.clone(),
            guid: self.guid.clone(),
            score: self.score,
            levels_completed: self.levels_completed,
            actions: self.actions,
            resets: self.resets,
            state: self.state,
            completed: self.completed,
            level_scores: Vec::new(),
            level_actions: Vec::new(),
            level_baseline_actions: self.baseline_actions.clone(),
            number_of_levels: Some(self.level_count),
            number_of_environments: Some(1),
        }
    }
}

#[derive(Debug)]
struct CompatibilityApiError {
    status: StatusCode,
    error: &'static str,
    message: String,
}

impl CompatibilityApiError {
    fn validation(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            error: "VALIDATION_ERROR",
            message: message.into(),
        }
    }

    fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            error: "SERVER_ERROR",
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            error: "SERVER_ERROR",
            message: message.into(),
        }
    }

    fn server(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "SERVER_ERROR",
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            error: "SERVER_ERROR",
            message: message.into(),
        }
    }

    fn from_client_error(error: ArcClientError) -> Self {
        Self::server(error.to_string())
    }
}

impl IntoResponse for CompatibilityApiError {
    fn into_response(self) -> Response {
        let body = Json(CompatibilityErrorBody {
            error: self.error,
            message: self.message,
        });
        (self.status, body).into_response()
    }
}

#[derive(Serialize)]
struct CompatibilityErrorBody {
    error: &'static str,
    message: String,
}

async fn list_games(
    State(state): State<SharedState>,
) -> Result<Json<Vec<ArcEnvironmentInfo>>, CompatibilityApiError> {
    let state = lock_state(&state)?;
    let mut environments = state
        .environments
        .values()
        .map(ArcRegisteredEnvironment::public_info)
        .collect::<Vec<_>>();
    environments.sort_by(|left, right| {
        let left_key = left.title.as_deref().unwrap_or(left.game_id.as_str());
        let right_key = right.title.as_deref().unwrap_or(right.game_id.as_str());
        left_key.cmp(right_key)
    });
    Ok(Json(environments))
}

async fn get_game(
    State(state): State<SharedState>,
    Path(game_id): Path<String>,
) -> Result<Json<ArcEnvironmentInfo>, CompatibilityApiError> {
    let state = lock_state(&state)?;
    let environment = state
        .environments
        .values()
        .find(|environment| {
            let id = environment.info.game_id.as_str();
            id == game_id || id.starts_with(&format!("{game_id}-"))
        })
        .ok_or_else(|| CompatibilityApiError::not_found(format!("game {game_id} not found")))?;
    Ok(Json(environment.public_info()))
}

async fn open_scorecard(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ArcOpenScorecardRequest>,
) -> Result<Json<ArcOpenScorecardResponse>, CompatibilityApiError> {
    if let Some(opaque) = &request.opaque {
        let bytes = serde_json::to_vec(opaque)
            .map_err(|error| CompatibilityApiError::validation(error.to_string()))?;
        if bytes.len() > MAX_OPAQUE_BYTES {
            return Err(CompatibilityApiError::validation(
                "opaque exceeds 16 KiB limit",
            ));
        }
    }

    let mut state = lock_state(&state)?;
    state.close_stale_scorecards();
    let api_key = request_api_key(&headers);
    if request.competition_mode.unwrap_or(false)
        && state.has_open_competition_scorecard_for(&api_key)
    {
        return Err(CompatibilityApiError::conflict(
            "cannot open multiple scorecards in competition mode",
        ));
    }
    let card_id = format!("local-card-{}", state.next_scorecard_id);
    state.next_scorecard_id += 1;
    state.scorecards.insert(
        card_id.clone(),
        CompatibilityScorecardRecord::new(request, api_key),
    );
    Ok(Json(ArcOpenScorecardResponse { card_id }))
}

async fn close_scorecard(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ArcCloseScorecardRequest>,
) -> Result<Json<ArcScorecardSummary>, CompatibilityApiError> {
    let mut state = lock_state(&state)?;
    state.close_stale_scorecards();
    let summary = state.close_scorecard_for(&request.card_id, &request_api_key(&headers))?;
    Ok(Json(summary))
}

async fn get_scorecard(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(card_id): Path<String>,
) -> Result<Json<ArcScorecardSummary>, CompatibilityApiError> {
    let mut state = lock_state(&state)?;
    state.close_stale_scorecards();
    let summary = state.scorecard_summary_for(&card_id, None, &request_api_key(&headers))?;
    Ok(Json(summary))
}

async fn get_scorecard_for_game(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((card_id, game_id)): Path<(String, String)>,
) -> Result<Json<ArcScorecardSummary>, CompatibilityApiError> {
    let mut state = lock_state(&state)?;
    state.close_stale_scorecards();
    let summary = state.scorecard_summary_for(
        &card_id,
        Some(game_id.as_str()),
        &request_api_key(&headers),
    )?;
    Ok(Json(summary))
}

async fn reset_command(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ArcResetCommand>,
) -> Result<Response, CompatibilityApiError> {
    let api_key = request_api_key(&headers);
    let frame = {
        let mut state = lock_state(&state)?;
        state.close_stale_scorecards();
        state.reset_session(request, &api_key)?
    };
    Ok(frame_response(headers, frame))
}

async fn action1_command(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<crate::models::ArcSimpleActionCommand>,
) -> Result<Response, CompatibilityApiError> {
    execute_simple_action(state, headers, request, arc_core::ArcAction::Action1)
}

async fn action2_command(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<crate::models::ArcSimpleActionCommand>,
) -> Result<Response, CompatibilityApiError> {
    execute_simple_action(state, headers, request, arc_core::ArcAction::Action2)
}

async fn action3_command(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<crate::models::ArcSimpleActionCommand>,
) -> Result<Response, CompatibilityApiError> {
    execute_simple_action(state, headers, request, arc_core::ArcAction::Action3)
}

async fn action4_command(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<crate::models::ArcSimpleActionCommand>,
) -> Result<Response, CompatibilityApiError> {
    execute_simple_action(state, headers, request, arc_core::ArcAction::Action4)
}

async fn action5_command(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<crate::models::ArcSimpleActionCommand>,
) -> Result<Response, CompatibilityApiError> {
    execute_simple_action(state, headers, request, arc_core::ArcAction::Action5)
}

async fn action6_command(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ArcComplexActionCommand>,
) -> Result<Response, CompatibilityApiError> {
    let api_key = request_api_key(&headers);
    let action = arc_core::ArcAction::action6(request.x, request.y)
        .map_err(|error| CompatibilityApiError::validation(error.to_string()))?;
    let frame = {
        let mut state = lock_state(&state)?;
        state.close_stale_scorecards();
        state.execute_action(request.game_id, request.guid, action, &api_key)?
    };
    Ok(frame_response(headers, frame))
}

async fn action7_command(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<crate::models::ArcSimpleActionCommand>,
) -> Result<Response, CompatibilityApiError> {
    execute_simple_action(state, headers, request, arc_core::ArcAction::Action7)
}

async fn healthcheck() -> impl IntoResponse {
    (StatusCode::OK, "okay")
}

fn execute_simple_action(
    state: SharedState,
    headers: HeaderMap,
    request: crate::models::ArcSimpleActionCommand,
    action: arc_core::ArcAction,
) -> Result<Response, CompatibilityApiError> {
    let api_key = request_api_key(&headers);
    let frame = {
        let mut state = lock_state(&state)?;
        state.close_stale_scorecards();
        state.execute_action(request.game_id, request.guid, action, &api_key)?
    };
    Ok(frame_response(headers, frame))
}

fn frame_response(headers: HeaderMap, frame: ArcSessionFrame) -> Response {
    let response = Json(ArcCompatibilityFrameResponse::from_session_frame(frame));
    if let Some(api_key) = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
    {
        let mut response = response.into_response();
        if let Ok(cookie_value) =
            header::HeaderValue::from_str(&format!("ARCLOCAL={api_key}; Path=/; HttpOnly"))
        {
            response
                .headers_mut()
                .append(header::SET_COOKIE, cookie_value);
        }
        response
    } else {
        response.into_response()
    }
}

fn request_api_key(headers: &HeaderMap) -> String {
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            headers
                .get(header::COOKIE)
                .and_then(|value| value.to_str().ok())
                .and_then(extract_arc_local_cookie)
        })
        .unwrap_or_else(|| "local-compatibility-anon".to_owned())
}

fn extract_arc_local_cookie(raw_cookie: &str) -> Option<String> {
    raw_cookie
        .split(';')
        .map(str::trim)
        .find_map(|cookie| cookie.strip_prefix("ARCLOCAL=").map(ToOwned::to_owned))
}

impl ArcCompatibilityServerState {
    fn close_stale_scorecards(&mut self) {
        let Some(stale_after) = self.config.stale_after else {
            return;
        };
        let Ok(stale_after) = time::Duration::try_from(stale_after) else {
            return;
        };
        let now = OffsetDateTime::now_utc();
        let stale_card_ids = self
            .scorecards
            .iter()
            .filter(|(_, scorecard)| {
                !scorecard.closed && now - scorecard.last_update >= stale_after
            })
            .map(|(card_id, _)| card_id.clone())
            .collect::<Vec<_>>();

        for card_id in stale_card_ids {
            self.close_scorecard_internal(&card_id);
        }
    }

    fn has_open_competition_scorecard_for(&self, api_key: &str) -> bool {
        self.scorecards.values().any(|scorecard| {
            !scorecard.closed && scorecard.competition_mode && scorecard.owner_api_key == api_key
        })
    }

    fn close_scorecard_for(
        &mut self,
        card_id: &str,
        api_key: &str,
    ) -> Result<ArcScorecardSummary, CompatibilityApiError> {
        self.scorecard_for_owner(card_id, api_key)?;
        self.close_scorecard_internal(card_id);
        Ok(self
            .scorecards
            .get(card_id)
            .ok_or_else(|| {
                CompatibilityApiError::not_found(format!("scorecard {card_id} not found"))
            })?
            .summary_for(card_id, None))
    }

    fn scorecard_summary_for(
        &self,
        card_id: &str,
        game_id: Option<&str>,
        api_key: &str,
    ) -> Result<ArcScorecardSummary, CompatibilityApiError> {
        let scorecard = self.scorecard_for_owner(card_id, api_key)?;
        if scorecard.competition_mode && !scorecard.closed {
            return Err(CompatibilityApiError::forbidden(
                "cannot get scorecard that is in competition mode",
            ));
        }
        Ok(scorecard.summary_for(card_id, game_id))
    }

    fn reset_session(
        &mut self,
        request: ArcResetCommand,
        api_key: &str,
    ) -> Result<ArcSessionFrame, CompatibilityApiError> {
        if let Some(guid) = request.guid {
            let (card_id, game_id, run_index) = self.session_identity(&guid, &request.game_id)?;
            self.ensure_scorecard_open(&card_id, api_key)?;
            let frame = {
                let session = self.sessions.get_mut(&guid).ok_or_else(|| {
                    CompatibilityApiError::validation(format!("guid {guid} not found"))
                })?;
                session
                    .environment
                    .reset()
                    .map_err(CompatibilityApiError::from_client_error)?
            };
            let run = self.run_record_mut(&card_id, &game_id, run_index)?;
            run.resets += 1;
            run.apply_frame(&frame);
            let scorecard = self.scorecards.get_mut(&card_id).ok_or_else(|| {
                CompatibilityApiError::validation(format!("scorecard {card_id} not found"))
            })?;
            scorecard.touch();
            return Ok(frame);
        }

        let (scorecard_closed, competition_mode) = {
            let scorecard = self.scorecard_for_owner(&request.card_id, api_key)?;
            (scorecard.closed, scorecard.competition_mode)
        };
        if scorecard_closed {
            return Err(CompatibilityApiError::validation(format!(
                "scorecard {} is closed",
                request.card_id
            )));
        }

        let registered = self.registered_environment_for(&request.game_id)?;
        if competition_mode {
            let canonical_game_id = registered.info.game_id.as_str();
            let already_opened = self
                .scorecards
                .get(&request.card_id)
                .is_some_and(|scorecard| scorecard.environments.contains_key(canonical_game_id));
            if already_opened {
                return Err(CompatibilityApiError::conflict(format!(
                    "competition-mode scorecards may only open environment {canonical_game_id} once"
                )));
            }
        }

        let mut environment = LocalArcEnvironment::load_from_path(
            registered.info.clone(),
            &registered.package_path,
            request.card_id.clone(),
        )
        .map_err(CompatibilityApiError::from_client_error)?;
        let level_count = environment.level_count();
        let frame = environment
            .reset()
            .map_err(CompatibilityApiError::from_client_error)?;
        let guid = environment.guid().to_owned();
        let canonical_game_id = environment.info().game_id.clone();
        let scorecard = self.scorecards.get_mut(&request.card_id).ok_or_else(|| {
            CompatibilityApiError::validation(format!("scorecard {} not found", request.card_id))
        })?;
        let environment_record =
            scorecard.ensure_environment(&canonical_game_id, environment.info(), level_count);
        environment_record.runs.push(CompatibilityRunRecord::new(
            environment.info(),
            guid.clone(),
            level_count,
            &frame,
        ));
        let run_index = environment_record.runs.len() - 1;
        scorecard.touch();
        self.sessions.insert(
            guid.clone(),
            CompatibilitySessionRecord {
                card_id: request.card_id,
                game_id: canonical_game_id.to_string(),
                run_index,
                environment,
            },
        );
        Ok(frame)
    }

    fn execute_action(
        &mut self,
        game_id: arc_core::ArcTaskId,
        guid: String,
        action: arc_core::ArcAction,
        api_key: &str,
    ) -> Result<ArcSessionFrame, CompatibilityApiError> {
        let (card_id, stored_game_id, run_index) = self.session_identity(&guid, &game_id)?;
        self.ensure_scorecard_open(&card_id, api_key)?;
        let frame = {
            let session = self.sessions.get_mut(&guid).ok_or_else(|| {
                CompatibilityApiError::validation(format!("guid {guid} not found"))
            })?;
            session
                .environment
                .step(action)
                .map_err(CompatibilityApiError::from_client_error)?
        };
        let run = self.run_record_mut(&card_id, &stored_game_id, run_index)?;
        run.actions += 1;
        run.apply_frame(&frame);
        let scorecard = self.scorecards.get_mut(&card_id).ok_or_else(|| {
            CompatibilityApiError::validation(format!("scorecard {card_id} not found"))
        })?;
        scorecard.touch();
        Ok(frame)
    }

    fn session_identity(
        &self,
        guid: &str,
        requested_game_id: &arc_core::ArcTaskId,
    ) -> Result<(String, String, usize), CompatibilityApiError> {
        let session = self
            .sessions
            .get(guid)
            .ok_or_else(|| CompatibilityApiError::validation(format!("guid {guid} not found")))?;
        if session.game_id != requested_game_id.as_str() {
            return Err(CompatibilityApiError::validation(format!(
                "game {} with guid {guid} does not match requested game {}",
                session.game_id, requested_game_id
            )));
        }
        Ok((
            session.card_id.clone(),
            session.game_id.clone(),
            session.run_index,
        ))
    }

    fn ensure_scorecard_open(
        &self,
        card_id: &str,
        api_key: &str,
    ) -> Result<(), CompatibilityApiError> {
        let scorecard = self.scorecard_for_owner(card_id, api_key)?;
        if scorecard.closed {
            return Err(CompatibilityApiError::validation(format!(
                "scorecard {card_id} is closed"
            )));
        }
        Ok(())
    }

    fn scorecard_for_owner(
        &self,
        card_id: &str,
        api_key: &str,
    ) -> Result<&CompatibilityScorecardRecord, CompatibilityApiError> {
        let scorecard = self.scorecards.get(card_id).ok_or_else(|| {
            CompatibilityApiError::not_found(format!("scorecard {card_id} not found"))
        })?;
        if scorecard.owner_api_key != api_key {
            return Err(CompatibilityApiError::not_found(format!(
                "scorecard {card_id} not found"
            )));
        }
        Ok(scorecard)
    }

    fn registered_environment_for(
        &self,
        requested_game_id: &arc_core::ArcTaskId,
    ) -> Result<ArcRegisteredEnvironment, CompatibilityApiError> {
        self.environments
            .get(requested_game_id.as_str())
            .or_else(|| {
                self.environments.values().find(|environment| {
                    environment
                        .info
                        .game_id
                        .as_str()
                        .starts_with(&format!("{}-", requested_game_id))
                })
            })
            .cloned()
            .ok_or_else(|| {
                CompatibilityApiError::validation(format!("game {} not found", requested_game_id))
            })
    }

    fn close_scorecard_internal(&mut self, card_id: &str) {
        let should_materialize = self
            .scorecards
            .get(card_id)
            .is_some_and(|scorecard| scorecard.competition_mode);
        if should_materialize {
            self.materialize_competition_environments(card_id);
        }

        if let Some(scorecard) = self.scorecards.get_mut(card_id) {
            scorecard.close(OffsetDateTime::now_utc());
        }
    }

    fn materialize_competition_environments(&mut self, card_id: &str) {
        let missing = {
            let Some(scorecard) = self.scorecards.get(card_id) else {
                return;
            };
            self.environments
                .values()
                .filter(|environment| {
                    !scorecard
                        .environments
                        .contains_key(environment.info.game_id.as_str())
                })
                .map(|environment| {
                    (
                        environment.info.clone(),
                        u32::try_from(environment.info.baseline_actions.len()).unwrap_or(u32::MAX),
                    )
                })
                .collect::<Vec<_>>()
        };

        let Some(scorecard) = self.scorecards.get_mut(card_id) else {
            return;
        };
        for (info, level_count) in missing {
            scorecard.ensure_environment(&info.game_id, &info, level_count);
        }
    }

    fn run_record_mut(
        &mut self,
        card_id: &str,
        game_id: &str,
        run_index: usize,
    ) -> Result<&mut CompatibilityRunRecord, CompatibilityApiError> {
        let scorecard = self.scorecards.get_mut(card_id).ok_or_else(|| {
            CompatibilityApiError::validation(format!("scorecard {card_id} not found"))
        })?;
        let environment = scorecard.environments.get_mut(game_id).ok_or_else(|| {
            CompatibilityApiError::validation(format!(
                "game {game_id} not found in scorecard {card_id}"
            ))
        })?;
        environment.runs.get_mut(run_index).ok_or_else(|| {
            CompatibilityApiError::validation(format!(
                "run {run_index} not found for game {game_id}"
            ))
        })
    }
}

fn lock_state(
    state: &SharedState,
) -> Result<std::sync::MutexGuard<'_, ArcCompatibilityServerState>, CompatibilityApiError> {
    state
        .lock()
        .map_err(|_| CompatibilityApiError::server("compatibility server state lock poisoned"))
}

fn format_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp
        .format(&Rfc3339)
        .unwrap_or_else(|_| timestamp.unix_timestamp().to_string())
}

fn is_terminal(state: arc_core::ArcGameState) -> bool {
    matches!(
        state,
        arc_core::ArcGameState::Win | arc_core::ArcGameState::GameOver
    )
}
