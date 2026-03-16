use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use arc_core::{ArcAction, ArcActionKind, ArcOperationMode, ArcRecording};
use reqwest::blocking::Client;
use reqwest::header::HeaderMap as ReqwestHeaderMap;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::ArcClientError;
use crate::models::{
    ArcCloseScorecardRequest, ArcCompatibilityFrameResponse, ArcComplexActionCommand,
    ArcEnvironmentInfo, ArcOpenScorecardRequest, ArcOpenScorecardResponse, ArcRemoteSession,
    ArcResetCommand, ArcScorecardSummary, ArcSessionFrame, ArcSimpleActionCommand,
};
use crate::recording::session_frames_to_recording;

#[derive(Clone)]
pub struct ArcRemoteClient {
    base_url: String,
    api_key: String,
    http: Client,
    retry_policy: ArcRemoteRetryPolicy,
}

pub struct RemoteArcEnvironment {
    client: ArcRemoteClient,
    info: ArcEnvironmentInfo,
    scorecard_id: String,
    operation_mode: ArcOperationMode,
    session: Option<ArcRemoteSession>,
    last_response: Option<ArcSessionFrame>,
    history: Vec<ArcSessionFrame>,
}

#[derive(Clone)]
pub struct ArcRemoteArcade {
    client: ArcRemoteClient,
    operation_mode: ArcOperationMode,
    default_open_request: ArcOpenScorecardRequest,
    default_scorecard_id: Arc<Mutex<Option<String>>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcRemoteRetryPolicy {
    pub max_retries: u8,
    pub initial_delay: Duration,
    pub backoff_factor: f64,
    pub max_delay: Duration,
}

impl Default for ArcRemoteRetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            backoff_factor: 2.0,
            max_delay: Duration::from_secs(60),
        }
    }
}

impl ArcRemoteClient {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, ArcClientError> {
        let http = Client::builder()
            .cookie_store(true)
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
            http,
            retry_policy: ArcRemoteRetryPolicy::default(),
        })
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    #[must_use]
    pub fn retry_policy(&self) -> ArcRemoteRetryPolicy {
        self.retry_policy
    }

    #[must_use]
    pub fn with_retry_policy(mut self, retry_policy: ArcRemoteRetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    pub fn list_games(&self) -> Result<Vec<ArcEnvironmentInfo>, ArcClientError> {
        self.get_json("/api/games")
    }

    pub fn open_scorecard(
        &self,
        request: &ArcOpenScorecardRequest,
    ) -> Result<ArcOpenScorecardResponse, ArcClientError> {
        self.post_json("/api/scorecard/open", request)
    }

    pub fn close_scorecard(
        &self,
        request: &ArcCloseScorecardRequest,
    ) -> Result<ArcScorecardSummary, ArcClientError> {
        self.post_json("/api/scorecard/close", request)
    }

    pub fn get_scorecard(
        &self,
        card_id: &str,
        game_id: Option<&arc_core::ArcTaskId>,
    ) -> Result<ArcScorecardSummary, ArcClientError> {
        let path = match game_id {
            Some(game_id) => format!("/api/scorecard/{card_id}/{game_id}"),
            None => format!("/api/scorecard/{card_id}"),
        };
        self.get_json(&path)
    }

    pub fn reset_session(
        &self,
        game_id: arc_core::ArcTaskId,
        card_id: impl Into<String>,
        guid: Option<&str>,
    ) -> Result<(ArcRemoteSession, ArcSessionFrame), ArcClientError> {
        let card_id = card_id.into();
        let request = ArcResetCommand {
            game_id: game_id.clone(),
            card_id: card_id.clone(),
            guid: guid.map(ToOwned::to_owned),
        };
        let response: ArcCompatibilityFrameResponse = self.post_json("/api/cmd/RESET", &request)?;
        let session_guid = response.guid.clone();
        let frame = response.try_into_session_frame()?;
        Ok((
            ArcRemoteSession {
                game_id,
                scorecard_id: card_id,
                guid: session_guid,
            },
            frame,
        ))
    }

    pub fn execute_action(
        &self,
        session: &ArcRemoteSession,
        action: &ArcAction,
        reasoning: Option<serde_json::Value>,
    ) -> Result<ArcSessionFrame, ArcClientError> {
        let path = format!("/api/cmd/{}", action_path_segment(action));
        let response = match action {
            ArcAction::Action6 { x, y } => {
                let request = ArcComplexActionCommand {
                    game_id: session.game_id.clone(),
                    guid: session.guid.clone(),
                    x: *x,
                    y: *y,
                    reasoning,
                };
                self.post_json::<_, ArcCompatibilityFrameResponse>(&path, &request)?
            }
            ArcAction::Reset => {
                let request = ArcResetCommand {
                    game_id: session.game_id.clone(),
                    card_id: session.scorecard_id.clone(),
                    guid: Some(session.guid.clone()),
                };
                self.post_json::<_, ArcCompatibilityFrameResponse>(&path, &request)?
            }
            _ => {
                let request = ArcSimpleActionCommand {
                    game_id: session.game_id.clone(),
                    guid: session.guid.clone(),
                    reasoning,
                };
                self.post_json::<_, ArcCompatibilityFrameResponse>(&path, &request)?
            }
        };
        response.try_into_session_frame()
    }

    fn get_json<T>(&self, path: &str) -> Result<T, ArcClientError>
    where
        T: DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url, path);
        let response = self.send_with_retry(path, || {
            self.http
                .get(&url)
                .header("X-Api-Key", &self.api_key)
                .send()
        })?;
        Self::decode_response(path, response)
    }

    fn post_json<Body, Response>(&self, path: &str, body: &Body) -> Result<Response, ArcClientError>
    where
        Body: Serialize,
        Response: DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url, path);
        let response = self.send_with_retry(path, || {
            self.http
                .post(&url)
                .header("X-Api-Key", &self.api_key)
                .json(body)
                .send()
        })?;
        Self::decode_response(path, response)
    }

    fn send_with_retry<F>(
        &self,
        path: &str,
        mut send: F,
    ) -> Result<reqwest::blocking::Response, ArcClientError>
    where
        F: FnMut() -> Result<reqwest::blocking::Response, reqwest::Error>,
    {
        let mut delay = self.retry_policy.initial_delay;
        let max_retries = usize::from(self.retry_policy.max_retries);

        for attempt in 0..=max_retries {
            match send() {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() || !is_retryable_status(status) {
                        return Ok(response);
                    }

                    let retry_after = retry_after_delay(response.headers());
                    let body = response.text()?;
                    if attempt == max_retries {
                        return Err(ArcClientError::RetryBudgetExhausted {
                            path: path.to_owned(),
                            attempts: u32::try_from(attempt + 1).unwrap_or(u32::MAX),
                            status,
                            body,
                        });
                    }

                    std::thread::sleep(retry_after.unwrap_or(delay));
                    delay = next_delay(delay, self.retry_policy);
                }
                Err(error) => {
                    if attempt == max_retries || !(error.is_connect() || error.is_timeout()) {
                        return Err(error.into());
                    }
                    std::thread::sleep(delay);
                    delay = next_delay(delay, self.retry_policy);
                }
            }
        }

        unreachable!("retry loop should return or error before exhaustion")
    }

    fn decode_response<T>(
        path: &str,
        response: reqwest::blocking::Response,
    ) -> Result<T, ArcClientError>
    where
        T: DeserializeOwned,
    {
        let status = response.status();
        if !status.is_success() {
            let body = response.text()?;
            return Err(ArcClientError::UnexpectedStatus {
                status,
                path: path.to_owned(),
                body,
            });
        }
        response.json().map_err(Into::into)
    }
}

impl ArcRemoteArcade {
    #[must_use]
    pub fn new(client: ArcRemoteClient, operation_mode: ArcOperationMode) -> Self {
        let mut default_open_request = ArcOpenScorecardRequest::default();
        if operation_mode == ArcOperationMode::Competition {
            default_open_request.competition_mode = Some(true);
        }
        Self {
            client,
            operation_mode,
            default_open_request,
            default_scorecard_id: Arc::new(Mutex::new(None)),
        }
    }

    #[must_use]
    pub fn with_default_open_request(mut self, request: ArcOpenScorecardRequest) -> Self {
        self.default_open_request = request;
        self
    }

    #[must_use]
    pub fn client(&self) -> &ArcRemoteClient {
        &self.client
    }

    #[must_use]
    pub fn default_open_request(&self) -> &ArcOpenScorecardRequest {
        &self.default_open_request
    }

    pub fn default_scorecard_id(&self) -> Result<Option<String>, ArcClientError> {
        Ok(self.default_scorecard_lock()?.clone())
    }

    pub fn open_scorecard(
        &self,
        request: &ArcOpenScorecardRequest,
    ) -> Result<ArcOpenScorecardResponse, ArcClientError> {
        self.client.open_scorecard(request)
    }

    pub fn ensure_default_scorecard(&self) -> Result<String, ArcClientError> {
        if let Some(card_id) = self.default_scorecard_id()? {
            return Ok(card_id);
        }

        let response = self.client.open_scorecard(&self.default_open_request)?;
        *self.default_scorecard_lock()? = Some(response.card_id.clone());
        Ok(response.card_id)
    }

    pub fn get_scorecard(
        &self,
        card_id: Option<&str>,
        game_id: Option<&arc_core::ArcTaskId>,
    ) -> Result<ArcScorecardSummary, ArcClientError> {
        let card_id = match card_id {
            Some(card_id) => card_id.to_owned(),
            None => self.ensure_default_scorecard()?,
        };
        self.client.get_scorecard(&card_id, game_id)
    }

    pub fn close_scorecard(
        &self,
        card_id: Option<&str>,
    ) -> Result<Option<ArcScorecardSummary>, ArcClientError> {
        let Some(card_id) = card_id
            .map(ToOwned::to_owned)
            .or(self.default_scorecard_id()?)
        else {
            return Ok(None);
        };

        let summary = self.client.close_scorecard(&ArcCloseScorecardRequest {
            card_id: card_id.clone(),
        })?;
        let mut default_scorecard_id = self.default_scorecard_lock()?;
        if default_scorecard_id.as_deref() == Some(card_id.as_str()) {
            *default_scorecard_id = None;
        }
        Ok(Some(summary))
    }

    pub fn remote_environment(
        &self,
        info: ArcEnvironmentInfo,
        scorecard_id: Option<&str>,
    ) -> Result<RemoteArcEnvironment, ArcClientError> {
        let scorecard_id = match scorecard_id {
            Some(scorecard_id) => scorecard_id.to_owned(),
            None => self.ensure_default_scorecard()?,
        };
        Ok(RemoteArcEnvironment::new_with_operation_mode(
            self.client.clone(),
            info,
            scorecard_id,
            self.operation_mode,
        ))
    }

    fn default_scorecard_lock(&self) -> Result<MutexGuard<'_, Option<String>>, ArcClientError> {
        self.default_scorecard_id
            .lock()
            .map_err(|_| ArcClientError::StatePoisoned {
                state: "arc_remote_arcade.default_scorecard_id",
            })
    }
}

impl RemoteArcEnvironment {
    #[must_use]
    pub fn new(
        client: ArcRemoteClient,
        info: ArcEnvironmentInfo,
        scorecard_id: impl Into<String>,
    ) -> Self {
        Self::new_with_operation_mode(client, info, scorecard_id, ArcOperationMode::Online)
    }

    #[must_use]
    pub fn new_with_operation_mode(
        client: ArcRemoteClient,
        info: ArcEnvironmentInfo,
        scorecard_id: impl Into<String>,
        operation_mode: ArcOperationMode,
    ) -> Self {
        Self {
            client,
            info,
            scorecard_id: scorecard_id.into(),
            operation_mode,
            session: None,
            last_response: None,
            history: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_operation_mode(mut self, operation_mode: ArcOperationMode) -> Self {
        self.operation_mode = operation_mode;
        self
    }

    #[must_use]
    pub fn client(&self) -> &ArcRemoteClient {
        &self.client
    }

    #[must_use]
    pub fn info(&self) -> &ArcEnvironmentInfo {
        &self.info
    }

    #[must_use]
    pub fn scorecard_id(&self) -> &str {
        &self.scorecard_id
    }

    #[must_use]
    pub fn operation_mode(&self) -> ArcOperationMode {
        self.operation_mode
    }

    #[must_use]
    pub fn session(&self) -> Option<&ArcRemoteSession> {
        self.session.as_ref()
    }

    #[must_use]
    pub fn observation(&self) -> Option<&ArcSessionFrame> {
        self.last_response.as_ref()
    }

    #[must_use]
    pub fn action_space(&self) -> Option<&[ArcActionKind]> {
        self.last_response
            .as_ref()
            .map(|response| response.available_actions.as_slice())
    }

    pub fn recording(&self) -> Result<Option<ArcRecording>, ArcClientError> {
        if self.history.is_empty() {
            return Ok(None);
        }
        session_frames_to_recording(
            self.info.game_id.clone(),
            self.operation_mode,
            &self.history,
        )
        .map(Some)
    }

    pub fn reset(&mut self) -> Result<ArcSessionFrame, ArcClientError> {
        let guid = self.session.as_ref().map(|session| session.guid.as_str());
        let (session, response) = self.client.reset_session(
            self.info.game_id.clone(),
            self.scorecard_id.clone(),
            guid,
        )?;
        self.session = Some(session);
        self.last_response = Some(response.clone());
        self.history.push(response.clone());
        Ok(response)
    }

    pub fn step(&mut self, action: ArcAction) -> Result<ArcSessionFrame, ArcClientError> {
        let Some(session) = self.session.as_ref() else {
            return Err(ArcClientError::MissingSessionGuid {
                game_id: self.info.game_id.clone(),
            });
        };
        let response = self.client.execute_action(session, &action, None)?;
        self.last_response = Some(response.clone());
        self.history.push(response.clone());
        Ok(response)
    }
}

fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 425 | 429 | 500 | 502 | 503 | 504)
}

fn retry_after_delay(headers: &ReqwestHeaderMap) -> Option<Duration> {
    headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<f64>().ok())
        .map(Duration::from_secs_f64)
}

fn next_delay(current: Duration, policy: ArcRemoteRetryPolicy) -> Duration {
    let scaled = current.mul_f64(policy.backoff_factor);
    if scaled > policy.max_delay {
        policy.max_delay
    } else {
        scaled
    }
}

fn action_path_segment(action: &ArcAction) -> &'static str {
    match action {
        ArcAction::Reset => "RESET",
        ArcAction::Action1 => "ACTION1",
        ArcAction::Action2 => "ACTION2",
        ArcAction::Action3 => "ACTION3",
        ArcAction::Action4 => "ACTION4",
        ArcAction::Action5 => "ACTION5",
        ArcAction::Action6 { .. } => "ACTION6",
        ArcAction::Action7 => "ACTION7",
    }
}
