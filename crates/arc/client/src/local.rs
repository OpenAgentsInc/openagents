use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_core::{ArcAction, ArcActionKind};
use arc_engine::{ArcEngine, ArcEngineStepOutcome, load_game_package};

use crate::ArcClientError;
use crate::models::{ArcEnvironmentInfo, ArcSessionFrame};

static LOCAL_GUID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct LocalArcEnvironment {
    info: ArcEnvironmentInfo,
    scorecard_id: String,
    guid: String,
    engine: ArcEngine,
    last_response: Option<ArcSessionFrame>,
}

impl LocalArcEnvironment {
    pub fn load_from_path(
        info: ArcEnvironmentInfo,
        package_path: impl AsRef<Path>,
        scorecard_id: impl Into<String>,
    ) -> Result<Self, ArcClientError> {
        let package_path = package_path.as_ref();
        let package = load_game_package(package_path)?;
        let info = info.with_local_package_path(package_path.to_path_buf());
        Self::from_package(info, package, scorecard_id)
    }

    pub fn from_package(
        info: ArcEnvironmentInfo,
        package: arc_engine::ArcGamePackage,
        scorecard_id: impl Into<String>,
    ) -> Result<Self, ArcClientError> {
        let guid = format!(
            "local-{}-{}",
            info.game_id,
            LOCAL_GUID_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        Ok(Self {
            info,
            scorecard_id: scorecard_id.into(),
            guid,
            engine: ArcEngine::from_package(package)?,
            last_response: None,
        })
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
    pub fn guid(&self) -> &str {
        &self.guid
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
        let outcome = self.engine.step(ArcAction::Reset)?;
        let response = self.outcome_to_frame(outcome);
        self.last_response = Some(response.clone());
        Ok(response)
    }

    pub fn step(&mut self, action: ArcAction) -> Result<ArcSessionFrame, ArcClientError> {
        let outcome = self.engine.step(action)?;
        let response = self.outcome_to_frame(outcome);
        self.last_response = Some(response.clone());
        Ok(response)
    }

    fn outcome_to_frame(&self, outcome: ArcEngineStepOutcome) -> ArcSessionFrame {
        ArcSessionFrame {
            game_id: self.info.game_id.clone(),
            guid: self.guid.clone(),
            frames: outcome.frames,
            game_state: outcome.observation.game_state,
            levels_completed: outcome.levels_completed,
            win_levels: outcome.win_levels,
            action: outcome.action,
            available_actions: outcome.observation.available_actions,
            full_reset: outcome.full_reset,
        }
    }
}
