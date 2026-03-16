use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use arc_core::{
    ARC_FRAME_MAX_EDGE, ArcAction, ArcActionKind, ArcBenchmark, ArcEpisodeStep, ArcFrameData,
    ArcFrameDataError, ArcObservation, ArcOperationMode, ArcRecording, ArcRecordingError,
    ArcTaskIdError,
};
use thiserror::Error;

use crate::package::{
    ArcBlockingMode, ArcGamePackage, ArcInteractionTrigger, ArcLevelDefinition, ArcLevelEffect,
    ArcPoint, ArcSprite, ArcSpriteInstance, ArcStateGate, load_game_package, point_outside_level,
    validate_package,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcEngineGameState {
    NotPlayed,
    NotFinished,
    Win,
    GameOver,
}

impl ArcEngineGameState {
    #[must_use]
    pub fn to_observation_state(self) -> arc_core::ArcGameState {
        match self {
            Self::NotPlayed | Self::NotFinished => arc_core::ArcGameState::NotFinished,
            Self::Win => arc_core::ArcGameState::Win,
            Self::GameOver => arc_core::ArcGameState::GameOver,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcEngineState {
    pub current_level_index: usize,
    pub player_position: ArcPoint,
    pub action_count: u32,
    pub total_step_count: u32,
    pub active_flags: BTreeSet<String>,
    pub game_state: ArcEngineGameState,
    pub levels_completed: u16,
    pub win_levels: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcEngineStepOutcome {
    pub step_index: u32,
    pub action: ArcAction,
    pub frames: Vec<ArcFrameData>,
    pub observation: ArcObservation,
    pub level_index: usize,
    pub level_completed: bool,
    pub advanced_level: bool,
    pub full_reset: bool,
    pub levels_completed: u16,
    pub win_levels: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ArcEngineSnapshot {
    current_level_index: usize,
    player_position: ArcPoint,
    action_count: u32,
    active_flags: BTreeSet<String>,
    game_state: ArcEngineGameState,
    levels_completed: u16,
}

pub struct ArcEngine {
    package: ArcGamePackage,
    sprite_index: BTreeMap<String, ArcSprite>,
    state: ArcEngineState,
    history: Vec<ArcEngineSnapshot>,
    recorded_steps: Vec<ArcEpisodeStep>,
}

impl ArcEngine {
    pub fn from_package(package: ArcGamePackage) -> Result<Self, ArcEngineError> {
        validate_package(&package)?;
        let sprite_index = package
            .sprites
            .iter()
            .cloned()
            .map(|sprite| (sprite.id.clone(), sprite))
            .collect::<BTreeMap<_, _>>();
        let first_player_spawn = package
            .levels
            .first()
            .ok_or(ArcEngineError::NoLevels)?
            .player_spawn;
        let win_levels = u16::try_from(package.levels.len()).unwrap_or(u16::MAX);

        Ok(Self {
            package,
            sprite_index,
            state: ArcEngineState {
                current_level_index: 0,
                player_position: first_player_spawn,
                action_count: 0,
                total_step_count: 0,
                active_flags: BTreeSet::new(),
                game_state: ArcEngineGameState::NotPlayed,
                levels_completed: 0,
                win_levels,
            },
            history: Vec::new(),
            recorded_steps: Vec::new(),
        })
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, ArcEngineError> {
        Self::from_package(load_game_package(path)?)
    }

    #[must_use]
    pub fn package(&self) -> &ArcGamePackage {
        &self.package
    }

    #[must_use]
    pub fn state(&self) -> &ArcEngineState {
        &self.state
    }

    pub fn observation(&self) -> Result<ArcObservation, ArcEngineError> {
        Ok(ArcObservation {
            frame: self.render_frame()?,
            available_actions: self.available_actions(),
            game_state: self.state.game_state.to_observation_state(),
        })
    }

    pub fn step(&mut self, action: ArcAction) -> Result<ArcEngineStepOutcome, ArcEngineError> {
        let step_index = self.state.total_step_count;
        let mut full_reset = false;
        let mut level_completed = false;
        let mut advanced_level = false;

        match action.clone() {
            ArcAction::Reset => {
                full_reset = self.handle_reset()?;
                self.state.total_step_count += 1;
            }
            ArcAction::Action7 => {
                if self.supports_action(ArcActionKind::Action7) {
                    if let Some(snapshot) = self.history.pop() {
                        self.restore_snapshot(snapshot);
                    }
                }
                self.state.total_step_count += 1;
            }
            _ if matches!(
                self.state.game_state,
                ArcEngineGameState::Win | ArcEngineGameState::GameOver
            ) =>
            {
                self.state.total_step_count += 1;
            }
            ArcAction::Action1 | ArcAction::Action2 | ArcAction::Action3 | ArcAction::Action4 => {
                self.history.push(self.snapshot());
                self.state.game_state = ArcEngineGameState::NotFinished;
                self.state.action_count += 1;
                self.state.total_step_count += 1;
                self.apply_movement(&action)?;
                let action_trigger = match action {
                    ArcAction::Action1 => ArcInteractionTrigger::Action1,
                    ArcAction::Action2 => ArcInteractionTrigger::Action2,
                    ArcAction::Action3 => ArcInteractionTrigger::Action3,
                    ArcAction::Action4 => ArcInteractionTrigger::Action4,
                    _ => unreachable!("action trigger mapping only applies to ACTION1-4"),
                };
                let action_transition =
                    self.apply_targets(action_trigger, self.state.player_position)?;
                let enter_transition =
                    self.apply_targets(ArcInteractionTrigger::OnEnter, self.state.player_position)?;
                level_completed =
                    action_transition.level_completed || enter_transition.level_completed;
                advanced_level =
                    action_transition.advanced_level || enter_transition.advanced_level;
            }
            ArcAction::Action5 => {
                self.history.push(self.snapshot());
                self.state.game_state = ArcEngineGameState::NotFinished;
                self.state.action_count += 1;
                self.state.total_step_count += 1;
                let transition =
                    self.apply_targets(ArcInteractionTrigger::Action5, self.state.player_position)?;
                level_completed = transition.level_completed;
                advanced_level = transition.advanced_level;
            }
            ArcAction::Action6 { x, y } => {
                self.history.push(self.snapshot());
                self.state.game_state = ArcEngineGameState::NotFinished;
                self.state.action_count += 1;
                self.state.total_step_count += 1;
                let target_point = self.display_to_grid(ArcPoint { x, y })?;
                let transition =
                    self.apply_targets(ArcInteractionTrigger::Action6, target_point)?;
                level_completed = transition.level_completed;
                advanced_level = transition.advanced_level;
            }
        }

        if self.state.game_state == ArcEngineGameState::NotFinished
            && self.state.action_count >= self.current_level().max_actions
        {
            self.state.game_state = ArcEngineGameState::GameOver;
        }

        let observation = self.observation()?;
        let frames = if level_completed {
            vec![
                success_flash_frame(&observation.frame)?,
                observation.frame.clone(),
            ]
        } else {
            vec![observation.frame.clone()]
        };
        self.record_step(step_index, action.clone(), &observation);

        Ok(ArcEngineStepOutcome {
            step_index,
            action,
            frames,
            observation,
            level_index: self.state.current_level_index,
            level_completed,
            advanced_level,
            full_reset,
            levels_completed: self.state.levels_completed,
            win_levels: self.state.win_levels,
        })
    }

    pub fn recording(&self) -> Result<Option<ArcRecording>, ArcEngineError> {
        if self.recorded_steps.is_empty() {
            return Ok(None);
        }

        let mut recording = ArcRecording::new(
            ArcBenchmark::ArcAgi3,
            self.package.task_id.clone(),
            self.recorded_steps.clone(),
        )?;
        recording.operation_mode = Some(ArcOperationMode::Offline);
        Ok(Some(recording))
    }

    pub fn replay(
        package: ArcGamePackage,
        actions: &[ArcAction],
    ) -> Result<ArcRecording, ArcEngineError> {
        let mut engine = Self::from_package(package)?;
        for action in actions {
            engine.step(action.clone())?;
        }
        engine.recording()?.ok_or(ArcEngineError::MissingRecording)
    }

    fn current_level(&self) -> &ArcLevelDefinition {
        &self.package.levels[self.state.current_level_index]
    }

    fn supports_action(&self, action: ArcActionKind) -> bool {
        self.current_level().available_actions.contains(&action)
            && (action != ArcActionKind::Action7 || !self.history.is_empty())
    }

    fn handle_reset(&mut self) -> Result<bool, ArcEngineError> {
        if self.state.action_count == 0 || self.state.game_state == ArcEngineGameState::Win {
            self.full_reset()?;
            Ok(true)
        } else {
            self.level_reset()?;
            Ok(false)
        }
    }

    fn full_reset(&mut self) -> Result<(), ArcEngineError> {
        self.state.levels_completed = 0;
        self.reset_level(0, true)?;
        self.state.game_state = ArcEngineGameState::NotFinished;
        Ok(())
    }

    fn level_reset(&mut self) -> Result<(), ArcEngineError> {
        self.reset_level(self.state.current_level_index, true)?;
        self.state.game_state = ArcEngineGameState::NotFinished;
        Ok(())
    }

    fn reset_level(
        &mut self,
        level_index: usize,
        clear_history: bool,
    ) -> Result<(), ArcEngineError> {
        let level = self
            .package
            .levels
            .get(level_index)
            .ok_or(ArcEngineError::UnknownLevelIndex(level_index))?;
        self.state.current_level_index = level_index;
        self.state.player_position = level.player_spawn;
        self.state.action_count = 0;
        self.state.active_flags.clear();
        if clear_history {
            self.history.clear();
        }
        Ok(())
    }

    fn restore_snapshot(&mut self, snapshot: ArcEngineSnapshot) {
        self.state.current_level_index = snapshot.current_level_index;
        self.state.player_position = snapshot.player_position;
        self.state.action_count = snapshot.action_count;
        self.state.active_flags = snapshot.active_flags;
        self.state.game_state = snapshot.game_state;
        self.state.levels_completed = snapshot.levels_completed;
    }

    fn snapshot(&self) -> ArcEngineSnapshot {
        ArcEngineSnapshot {
            current_level_index: self.state.current_level_index,
            player_position: self.state.player_position,
            action_count: self.state.action_count,
            active_flags: self.state.active_flags.clone(),
            game_state: self.state.game_state,
            levels_completed: self.state.levels_completed,
        }
    }

    fn apply_movement(&mut self, action: &ArcAction) -> Result<(), ArcEngineError> {
        let (dx, dy) = match action {
            ArcAction::Action1 => (0, -1),
            ArcAction::Action2 => (0, 1),
            ArcAction::Action3 => (-1, 0),
            ArcAction::Action4 => (1, 0),
            _ => return Ok(()),
        };

        let Some(next_position) = self.state.player_position.checked_offset(
            dx,
            dy,
            self.current_level().background.width(),
            self.current_level().background.height(),
        ) else {
            return Ok(());
        };
        if !self.is_blocked(next_position)? {
            self.state.player_position = next_position;
        }
        Ok(())
    }

    fn apply_targets(
        &mut self,
        trigger: ArcInteractionTrigger,
        point: ArcPoint,
    ) -> Result<ArcTransitionResult, ArcEngineError> {
        let targets = self
            .current_level()
            .targets
            .iter()
            .filter(|target| target.trigger == trigger && target.position == point)
            .cloned()
            .collect::<Vec<_>>();

        let mut result = ArcTransitionResult::default();
        for target in targets {
            match target.effect {
                ArcLevelEffect::ToggleFlag { flag_id } => {
                    if !self.state.active_flags.insert(flag_id.clone()) {
                        self.state.active_flags.remove(&flag_id);
                    }
                }
                ArcLevelEffect::CompleteLevel => {
                    self.complete_level(&mut result)?;
                }
                ArcLevelEffect::CompleteLevelIfActionCountAtLeast { threshold } => {
                    if self.state.action_count >= threshold {
                        self.complete_level(&mut result)?;
                    }
                }
                ArcLevelEffect::TeleportPlayer { destination } => {
                    if !self.is_blocked(destination)? {
                        self.state.player_position = destination;
                        let follow_up =
                            self.apply_targets(ArcInteractionTrigger::OnEnter, destination)?;
                        result.level_completed |= follow_up.level_completed;
                        result.advanced_level |= follow_up.advanced_level;
                    }
                }
            }
        }
        Ok(result)
    }

    fn complete_level(&mut self, result: &mut ArcTransitionResult) -> Result<(), ArcEngineError> {
        self.state.levels_completed = self.state.levels_completed.saturating_add(1);
        result.level_completed = true;
        let next_level_index = self.state.current_level_index + 1;
        if next_level_index < self.package.levels.len() {
            self.reset_level(next_level_index, false)?;
            self.state.game_state = ArcEngineGameState::NotFinished;
            result.advanced_level = true;
        } else {
            self.state.game_state = ArcEngineGameState::Win;
        }
        Ok(())
    }

    fn display_to_grid(&self, display_point: ArcPoint) -> Result<ArcPoint, ArcEngineError> {
        let camera = &self.current_level().camera;
        let scale = usize::from(
            (ARC_FRAME_MAX_EDGE / camera.width).min(ARC_FRAME_MAX_EDGE / camera.height),
        );
        let scale = scale.max(1);
        let scaled_width = usize::from(camera.width) * scale;
        let scaled_height = usize::from(camera.height) * scale;
        let x_padding = (usize::from(ARC_FRAME_MAX_EDGE) - scaled_width) / 2;
        let y_padding = (usize::from(ARC_FRAME_MAX_EDGE) - scaled_height) / 2;
        let display_x = usize::from(display_point.x);
        let display_y = usize::from(display_point.y);

        if display_x < x_padding || display_y < y_padding {
            return Err(ArcEngineError::Action6OutsideViewport {
                level_id: self.current_level().id.clone(),
                x: display_point.x,
                y: display_point.y,
                viewport_width: camera.width,
                viewport_height: camera.height,
            });
        }

        let grid_x = (display_x - x_padding) / scale;
        let grid_y = (display_y - y_padding) / scale;
        if grid_x >= usize::from(camera.width) || grid_y >= usize::from(camera.height) {
            return Err(ArcEngineError::Action6OutsideViewport {
                level_id: self.current_level().id.clone(),
                x: display_point.x,
                y: display_point.y,
                viewport_width: camera.width,
                viewport_height: camera.height,
            });
        }

        Ok(ArcPoint {
            x: camera.x + u8::try_from(grid_x).unwrap_or(0),
            y: camera.y + u8::try_from(grid_y).unwrap_or(0),
        })
    }

    fn is_blocked(&self, point: ArcPoint) -> Result<bool, ArcEngineError> {
        let level = self.current_level();
        let background_color = level
            .background
            .pixels()
            .get(background_index(&level.background, point))
            .copied()
            .ok_or_else(|| point_outside_level(&level.id, point))?;
        if level.solid_background_colors.contains(&background_color) {
            return Ok(true);
        }

        let player_sprite = self.lookup_sprite(&level.player_sprite_id)?;
        let player_instance = ArcSpriteInstance {
            sprite_id: level.player_sprite_id.clone(),
            position: point,
            scale: 1,
            state_gate: None,
        };

        for instance in &level.static_sprites {
            if !self.instance_active(instance) {
                continue;
            }
            let sprite = self.lookup_sprite(&instance.sprite_id)?;
            if collides(player_sprite, &player_instance, sprite, instance) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn render_frame(&self) -> Result<ArcFrameData, ArcEngineError> {
        let level = self.current_level();
        let camera = &level.camera;
        let mut raw =
            vec![camera.background; usize::from(camera.width) * usize::from(camera.height)];

        for view_y in 0..camera.height {
            for view_x in 0..camera.width {
                let world_x = camera.x + view_x;
                let world_y = camera.y + view_y;
                let index = usize::from(view_y) * usize::from(camera.width) + usize::from(view_x);
                raw[index] = level
                    .background
                    .pixels()
                    .get(background_index(
                        &level.background,
                        ArcPoint {
                            x: world_x,
                            y: world_y,
                        },
                    ))
                    .copied()
                    .ok_or_else(|| {
                        point_outside_level(
                            &level.id,
                            ArcPoint {
                                x: world_x,
                                y: world_y,
                            },
                        )
                    })?;
            }
        }

        let mut instances = level
            .static_sprites
            .iter()
            .filter(|instance| self.instance_active(instance))
            .collect::<Vec<_>>();
        instances.sort_by_key(|instance| {
            self.lookup_sprite(&instance.sprite_id)
                .map_or(0, |sprite| sprite.z_index)
        });

        for instance in instances {
            let sprite = self.lookup_sprite(&instance.sprite_id)?;
            overlay_sprite(&mut raw, camera, sprite, instance)?;
        }

        let player_instance = ArcSpriteInstance {
            sprite_id: level.player_sprite_id.clone(),
            position: self.state.player_position,
            scale: 1,
            state_gate: None,
        };
        let player_sprite = self.lookup_sprite(&level.player_sprite_id)?;
        overlay_sprite(&mut raw, camera, player_sprite, &player_instance)?;

        let scale = (ARC_FRAME_MAX_EDGE / camera.width).min(ARC_FRAME_MAX_EDGE / camera.height);
        let scaled = scale_up_frame(&raw, camera.width, camera.height, scale.max(1));
        let mut pixels = vec![
            camera.letter_box;
            usize::from(ARC_FRAME_MAX_EDGE) * usize::from(ARC_FRAME_MAX_EDGE)
        ];
        let x_padding = usize::from(ARC_FRAME_MAX_EDGE - camera.width * scale.max(1)) / 2;
        let y_padding = usize::from(ARC_FRAME_MAX_EDGE - camera.height * scale.max(1)) / 2;
        for y in 0..usize::from(camera.height * scale.max(1)) {
            for x in 0..usize::from(camera.width * scale.max(1)) {
                let dest_index =
                    (y_padding + y) * usize::from(ARC_FRAME_MAX_EDGE) + (x_padding + x);
                let source_index = y * usize::from(camera.width * scale.max(1)) + x;
                if let Some(pixel) = scaled.get(source_index) {
                    pixels[dest_index] = *pixel;
                }
            }
        }

        ArcFrameData::new(ARC_FRAME_MAX_EDGE, ARC_FRAME_MAX_EDGE, pixels).map_err(Into::into)
    }

    fn instance_active(&self, instance: &ArcSpriteInstance) -> bool {
        match &instance.state_gate {
            None => true,
            Some(ArcStateGate::FlagSet { flag_id }) => self.state.active_flags.contains(flag_id),
            Some(ArcStateGate::FlagUnset { flag_id }) => !self.state.active_flags.contains(flag_id),
        }
    }

    fn lookup_sprite(&self, sprite_id: &str) -> Result<&ArcSprite, ArcEngineError> {
        self.sprite_index
            .get(sprite_id)
            .ok_or_else(|| ArcEngineError::UnindexedSpriteId(sprite_id.to_owned()))
    }

    fn available_actions(&self) -> Vec<ArcActionKind> {
        self.current_level()
            .available_actions
            .iter()
            .copied()
            .filter(|action| *action != ArcActionKind::Action7 || !self.history.is_empty())
            .collect()
    }

    fn record_step(&mut self, step_index: u32, action: ArcAction, observation: &ArcObservation) {
        self.recorded_steps.push(ArcEpisodeStep {
            step_index,
            action,
            observation: observation.clone(),
            terminal: matches!(
                self.state.game_state,
                ArcEngineGameState::Win | ArcEngineGameState::GameOver
            ),
        });
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ArcTransitionResult {
    level_completed: bool,
    advanced_level: bool,
}

fn collides(
    player_sprite: &ArcSprite,
    player_instance: &ArcSpriteInstance,
    other_sprite: &ArcSprite,
    other_instance: &ArcSpriteInstance,
) -> bool {
    if !player_sprite.is_collidable() || !other_sprite.is_collidable() {
        return false;
    }
    if matches!(player_sprite.blocking, ArcBlockingMode::NotBlocked)
        || matches!(other_sprite.blocking, ArcBlockingMode::NotBlocked)
    {
        return false;
    }

    let player = rendered_sprite(player_sprite, player_instance.scale);
    let other = rendered_sprite(other_sprite, other_instance.scale);
    if !bounding_boxes_overlap(
        player_instance.position,
        &player,
        other_instance.position,
        &other,
    ) {
        return false;
    }

    if !matches!(player_sprite.blocking, ArcBlockingMode::PixelPerfect)
        && !matches!(other_sprite.blocking, ArcBlockingMode::PixelPerfect)
    {
        return true;
    }

    let x_min = player_instance.position.x.max(other_instance.position.x);
    let y_min = player_instance.position.y.max(other_instance.position.y);
    let x_max =
        (player_instance.position.x + player.width).min(other_instance.position.x + other.width);
    let y_max =
        (player_instance.position.y + player.height).min(other_instance.position.y + other.height);

    for world_y in y_min..y_max {
        for world_x in x_min..x_max {
            let player_x = world_x - player_instance.position.x;
            let player_y = world_y - player_instance.position.y;
            let other_x = world_x - other_instance.position.x;
            let other_y = world_y - other_instance.position.y;
            if player.pixel_at(player_x, player_y).flatten().is_some()
                && other.pixel_at(other_x, other_y).flatten().is_some()
            {
                return true;
            }
        }
    }

    false
}

fn bounding_boxes_overlap(
    left_position: ArcPoint,
    left: &RenderedSprite,
    right_position: ArcPoint,
    right: &RenderedSprite,
) -> bool {
    !(left_position.x >= right_position.x + right.width
        || left_position.x + left.width <= right_position.x
        || left_position.y >= right_position.y + right.height
        || left_position.y + left.height <= right_position.y)
}

fn overlay_sprite(
    raw: &mut [u8],
    camera: &crate::package::ArcCamera,
    sprite: &ArcSprite,
    instance: &ArcSpriteInstance,
) -> Result<(), ArcEngineError> {
    if !sprite.is_visible() {
        return Ok(());
    }

    let rendered = rendered_sprite(sprite, instance.scale);
    for sprite_y in 0..rendered.height {
        for sprite_x in 0..rendered.width {
            let world_x = instance.position.x + sprite_x;
            let world_y = instance.position.y + sprite_y;
            if world_x < camera.x
                || world_y < camera.y
                || world_x >= camera.x + camera.width
                || world_y >= camera.y + camera.height
            {
                continue;
            }
            let Some(color) = rendered
                .pixel_at(sprite_x, sprite_y)
                .and_then(|pixel| pixel)
            else {
                continue;
            };
            let view_x = world_x - camera.x;
            let view_y = world_y - camera.y;
            let index = usize::from(view_y) * usize::from(camera.width) + usize::from(view_x);
            let Some(cell) = raw.get_mut(index) else {
                return Err(ArcEngineError::RenderIndexOutOfRange {
                    x: view_x,
                    y: view_y,
                });
            };
            *cell = color;
        }
    }
    Ok(())
}

fn scale_up_frame(raw: &[u8], width: u8, height: u8, scale: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(
        usize::from(width) * usize::from(height) * usize::from(scale) * usize::from(scale),
    );
    for y in 0..height {
        let row =
            &raw[usize::from(y) * usize::from(width)..usize::from(y + 1) * usize::from(width)];
        let mut scaled_row = Vec::with_capacity(usize::from(width) * usize::from(scale));
        for pixel in row {
            for _ in 0..scale {
                scaled_row.push(*pixel);
            }
        }
        for _ in 0..scale {
            out.extend_from_slice(&scaled_row);
        }
    }
    out
}

fn success_flash_frame(frame: &ArcFrameData) -> Result<ArcFrameData, ArcEngineError> {
    let pixels = frame
        .pixels()
        .iter()
        .map(|pixel| if *pixel == 0 { 0 } else { 15 })
        .collect::<Vec<_>>();
    ArcFrameData::new(frame.width(), frame.height(), pixels).map_err(Into::into)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderedSprite {
    width: u8,
    height: u8,
    pixels: Vec<Option<u8>>,
}

impl RenderedSprite {
    fn pixel_at(&self, x: u8, y: u8) -> Option<Option<u8>> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = usize::from(y) * usize::from(self.width) + usize::from(x);
        self.pixels.get(index).copied()
    }
}

fn rendered_sprite(sprite: &ArcSprite, scale: u8) -> RenderedSprite {
    if scale <= 1 {
        return RenderedSprite {
            width: sprite.width,
            height: sprite.height,
            pixels: sprite.pixels.clone(),
        };
    }

    let width = sprite.width.saturating_mul(scale);
    let height = sprite.height.saturating_mul(scale);
    let mut pixels = Vec::with_capacity(usize::from(width) * usize::from(height));
    for y in 0..sprite.height {
        let mut scaled_row = Vec::with_capacity(usize::from(width));
        for x in 0..sprite.width {
            let pixel = sprite.pixel_at(x, y).unwrap_or(None);
            for _ in 0..scale {
                scaled_row.push(pixel);
            }
        }
        for _ in 0..scale {
            pixels.extend_from_slice(&scaled_row);
        }
    }
    RenderedSprite {
        width,
        height,
        pixels,
    }
}

#[derive(Debug, Error)]
pub enum ArcEngineError {
    #[error("failed to read ARC engine package: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse ARC engine package: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    TaskId(#[from] ArcTaskIdError),
    #[error(transparent)]
    Frame(#[from] ArcFrameDataError),
    #[error(transparent)]
    Recording(#[from] ArcRecordingError),
    #[error("ARC engine package only supports schema version 1, got {version}")]
    UnsupportedSchemaVersion { version: u16 },
    #[error("ARC engine package must include at least one level")]
    NoLevels,
    #[error("{kind} must not be empty")]
    EmptyIdentifier { kind: &'static str },
    #[error("{kind} must not contain whitespace: {value}")]
    InvalidIdentifier { kind: &'static str, value: String },
    #[error("{kind} must not be empty")]
    EmptyLabel { kind: &'static str },
    #[error("ARC engine package referenced duplicate sprite id `{0}`")]
    DuplicateSpriteId(String),
    #[error("ARC engine package referenced duplicate level id `{0}`")]
    DuplicateLevelId(String),
    #[error("ARC level `{level_id}` referenced duplicate target id `{target_id}`")]
    DuplicateTargetId { level_id: String, target_id: String },
    #[error(
        "ARC level `{level_id}` target `{target_id}` must use an action threshold of at least 1, got {threshold}"
    )]
    InvalidActionThreshold {
        level_id: String,
        target_id: String,
        threshold: u32,
    },
    #[error("ARC level `{0}` must allow at least one action")]
    InvalidMaxActions(String),
    #[error("ARC level `{0}` must declare at least one available action")]
    MissingAvailableActions(String),
    #[error("ARC level `{level_id}` referenced unknown sprite id `{sprite_id}`")]
    UnknownSpriteId { level_id: String, sprite_id: String },
    #[error("ARC engine runtime referenced unindexed sprite id `{0}`")]
    UnindexedSpriteId(String),
    #[error("ARC sprite `{sprite_id}` width must be in 1..=64, got {width}")]
    InvalidSpriteWidth { sprite_id: String, width: u8 },
    #[error("ARC sprite `{sprite_id}` height must be in 1..=64, got {height}")]
    InvalidSpriteHeight { sprite_id: String, height: u8 },
    #[error("ARC sprite `{sprite_id}` pixel count mismatch: expected {expected}, got {actual}")]
    SpritePixelCountMismatch {
        sprite_id: String,
        expected: usize,
        actual: usize,
    },
    #[error("ARC sprite `{sprite_id}` used invalid color {color}")]
    InvalidSpriteColor { sprite_id: String, color: u8 },
    #[error("ARC level `{level_id}` used invalid sprite scale {scale} for `{sprite_id}`")]
    InvalidSpriteScale {
        level_id: String,
        sprite_id: String,
        scale: u8,
    },
    #[error("ARC level `{level_id}` camera width must be in 1..=64, got {width}")]
    InvalidCameraViewportWidth { level_id: String, width: u8 },
    #[error("ARC level `{level_id}` camera height must be in 1..=64, got {height}")]
    InvalidCameraViewportHeight { level_id: String, height: u8 },
    #[error("ARC level `{level_id}` used invalid {field} color {color}")]
    InvalidCameraColor {
        level_id: String,
        field: &'static str,
        color: u8,
    },
    #[error(
        "ARC level `{level_id}` camera origin ({origin_x}, {origin_y}) with viewport {viewport_width}x{viewport_height} exceeds world {world_width}x{world_height}"
    )]
    CameraOutsideLevel {
        level_id: String,
        world_width: u8,
        world_height: u8,
        origin_x: u8,
        origin_y: u8,
        viewport_width: u8,
        viewport_height: u8,
    },
    #[error("ARC level `{level_id}` used invalid solid background color {color}")]
    InvalidBackgroundColor { level_id: String, color: u8 },
    #[error("ARC level `{level_id}` referenced point ({x}, {y}) outside the level")]
    PointOutsideLevel { level_id: String, x: u8, y: u8 },
    #[error(
        "ARC level `{level_id}` placed a sprite footprint {sprite_width}x{sprite_height} outside the level at ({x}, {y})"
    )]
    SpriteOutsideLevel {
        level_id: String,
        x: u8,
        y: u8,
        sprite_width: u8,
        sprite_height: u8,
    },
    #[error("ARC runtime attempted to render outside the view at ({x}, {y})")]
    RenderIndexOutOfRange { x: u8, y: u8 },
    #[error(
        "ARC ACTION6 ({x}, {y}) falls outside level `{level_id}` viewport {viewport_width}x{viewport_height}"
    )]
    Action6OutsideViewport {
        level_id: String,
        x: u8,
        y: u8,
        viewport_width: u8,
        viewport_height: u8,
    },
    #[error("ARC engine referenced unknown level index {0}")]
    UnknownLevelIndex(usize),
    #[error("ARC engine replay produced no recording")]
    MissingRecording,
}

fn background_index(background: &ArcFrameData, point: ArcPoint) -> usize {
    usize::from(point.y) * usize::from(background.width()) + usize::from(point.x)
}
