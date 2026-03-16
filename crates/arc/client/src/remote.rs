use arc_core::{ArcAction, ArcActionKind, ArcFrameData, ArcGameState};
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::ArcClientError;
use crate::models::{
    ArcCloseScorecardRequest, ArcEnvironmentInfo, ArcOpenScorecardRequest,
    ArcOpenScorecardResponse, ArcRemoteSession, ArcScorecardSummary, ArcSessionFrame,
};

#[derive(Clone)]
pub struct ArcRemoteClient {
    base_url: String,
    api_key: String,
    http: Client,
}

pub struct RemoteArcEnvironment {
    client: ArcRemoteClient,
    info: ArcEnvironmentInfo,
    scorecard_id: String,
    session: Option<ArcRemoteSession>,
    last_response: Option<ArcSessionFrame>,
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
        })
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
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
        let request = ResetCommand {
            game_id: game_id.clone(),
            card_id: card_id.clone(),
            guid: guid.map(ToOwned::to_owned),
        };
        let response: ArcFrameResponseWire = self.post_json("/api/cmd/RESET", &request)?;
        let session_guid = response
            .guid
            .clone()
            .ok_or_else(|| ArcClientError::MissingGuid {
                game_id: game_id.clone(),
            })?;
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
                let request = ComplexActionCommand {
                    game_id: session.game_id.clone(),
                    guid: session.guid.clone(),
                    x: *x,
                    y: *y,
                    reasoning,
                };
                self.post_json::<_, ArcFrameResponseWire>(&path, &request)?
            }
            ArcAction::Reset => {
                let request = ResetCommand {
                    game_id: session.game_id.clone(),
                    card_id: session.scorecard_id.clone(),
                    guid: Some(session.guid.clone()),
                };
                self.post_json::<_, ArcFrameResponseWire>(&path, &request)?
            }
            _ => {
                let request = SimpleActionCommand {
                    game_id: session.game_id.clone(),
                    guid: session.guid.clone(),
                    reasoning,
                };
                self.post_json::<_, ArcFrameResponseWire>(&path, &request)?
            }
        };
        response.try_into_session_frame()
    }

    fn get_json<T>(&self, path: &str) -> Result<T, ArcClientError>
    where
        T: DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .get(url)
            .header("X-Api-Key", &self.api_key)
            .send()?;
        Self::decode_response(path, response)
    }

    fn post_json<Body, Response>(&self, path: &str, body: &Body) -> Result<Response, ArcClientError>
    where
        Body: Serialize,
        Response: DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .post(url)
            .header("X-Api-Key", &self.api_key)
            .json(body)
            .send()?;
        Self::decode_response(path, response)
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

impl RemoteArcEnvironment {
    #[must_use]
    pub fn new(
        client: ArcRemoteClient,
        info: ArcEnvironmentInfo,
        scorecard_id: impl Into<String>,
    ) -> Self {
        Self {
            client,
            info,
            scorecard_id: scorecard_id.into(),
            session: None,
            last_response: None,
        }
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

    pub fn reset(&mut self) -> Result<ArcSessionFrame, ArcClientError> {
        let guid = self.session.as_ref().map(|session| session.guid.as_str());
        let (session, response) = self.client.reset_session(
            self.info.game_id.clone(),
            self.scorecard_id.clone(),
            guid,
        )?;
        self.session = Some(session);
        self.last_response = Some(response.clone());
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
        Ok(response)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ResetCommand {
    game_id: arc_core::ArcTaskId,
    card_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guid: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SimpleActionCommand {
    game_id: arc_core::ArcTaskId,
    guid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reasoning: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ComplexActionCommand {
    game_id: arc_core::ArcTaskId,
    guid: String,
    x: u8,
    y: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reasoning: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct ArcFrameResponseWire {
    game_id: arc_core::ArcTaskId,
    #[serde(default)]
    guid: Option<String>,
    frame: Vec<Vec<Vec<u8>>>,
    state: ArcGameStateWire,
    levels_completed: u16,
    win_levels: u16,
    action_input: ArcActionInputWire,
    #[serde(default)]
    available_actions: Vec<u8>,
    #[serde(default)]
    full_reset: bool,
}

impl ArcFrameResponseWire {
    fn try_into_session_frame(self) -> Result<ArcSessionFrame, ArcClientError> {
        let guid = self.guid.ok_or_else(|| ArcClientError::MissingGuid {
            game_id: self.game_id.clone(),
        })?;
        let frames = self
            .frame
            .into_iter()
            .enumerate()
            .map(|(frame_index, frame)| flatten_remote_frame(frame_index, frame))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ArcSessionFrame {
            game_id: self.game_id,
            guid,
            frames,
            game_state: self.state.into(),
            levels_completed: self.levels_completed,
            win_levels: self.win_levels,
            action: self.action_input.try_into_action()?,
            available_actions: self
                .available_actions
                .into_iter()
                .map(action_kind_from_wire)
                .collect::<Result<Vec<_>, _>>()?,
            full_reset: self.full_reset,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ArcGameStateWire {
    NotStarted,
    NotFinished,
    Win,
    GameOver,
}

impl From<ArcGameStateWire> for ArcGameState {
    fn from(value: ArcGameStateWire) -> Self {
        match value {
            ArcGameStateWire::NotStarted => Self::NotStarted,
            ArcGameStateWire::NotFinished => Self::NotFinished,
            ArcGameStateWire::Win => Self::Win,
            ArcGameStateWire::GameOver => Self::GameOver,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct ArcActionInputWire {
    id: u8,
    #[serde(default)]
    data: serde_json::Map<String, serde_json::Value>,
}

impl ArcActionInputWire {
    fn try_into_action(self) -> Result<ArcAction, ArcClientError> {
        match self.id {
            0 => Ok(ArcAction::Reset),
            1 => Ok(ArcAction::Action1),
            2 => Ok(ArcAction::Action2),
            3 => Ok(ArcAction::Action3),
            4 => Ok(ArcAction::Action4),
            5 => Ok(ArcAction::Action5),
            6 => {
                let x = self
                    .data
                    .get("x")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or(ArcClientError::MissingAction6Coordinate { axis: "x" })?;
                let y = self
                    .data
                    .get("y")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or(ArcClientError::MissingAction6Coordinate { axis: "y" })?;
                let x = u8::try_from(x)
                    .map_err(|_| ArcClientError::MissingAction6Coordinate { axis: "x" })?;
                let y = u8::try_from(y)
                    .map_err(|_| ArcClientError::MissingAction6Coordinate { axis: "y" })?;
                ArcAction::action6(x, y).map_err(Into::into)
            }
            7 => Ok(ArcAction::Action7),
            id => Err(ArcClientError::UnsupportedActionId { id }),
        }
    }
}

fn action_kind_from_wire(id: u8) -> Result<ArcActionKind, ArcClientError> {
    match id {
        1 => Ok(ArcActionKind::Action1),
        2 => Ok(ArcActionKind::Action2),
        3 => Ok(ArcActionKind::Action3),
        4 => Ok(ArcActionKind::Action4),
        5 => Ok(ArcActionKind::Action5),
        6 => Ok(ArcActionKind::Action6),
        7 => Ok(ArcActionKind::Action7),
        other => Err(ArcClientError::UnsupportedActionId { id: other }),
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

fn flatten_remote_frame(
    frame_index: usize,
    rows: Vec<Vec<u8>>,
) -> Result<ArcFrameData, ArcClientError> {
    let height =
        u8::try_from(rows.len()).map_err(|_| arc_core::ArcFrameDataError::InvalidHeight(255))?;
    let width = rows.first().map(std::vec::Vec::len).unwrap_or_default();
    let width = u8::try_from(width).map_err(|_| arc_core::ArcFrameDataError::InvalidWidth(255))?;
    let mut pixels = Vec::with_capacity(usize::from(width) * usize::from(height));

    for (row_index, row) in rows.into_iter().enumerate() {
        if row.len() != usize::from(width) {
            return Err(ArcClientError::RaggedRemoteFrame {
                frame_index,
                row_index,
                expected: usize::from(width),
                actual: row.len(),
            });
        }
        pixels.extend(row);
    }

    ArcFrameData::new(width, height, pixels).map_err(Into::into)
}
