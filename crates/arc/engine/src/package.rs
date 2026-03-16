use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use arc_core::{
    ARC_FRAME_MAX_EDGE, ARC_FRAME_PALETTE_SIZE, ARC_PALETTE_SIZE, ArcActionKind, ArcGrid, ArcTaskId,
};
use serde::{Deserialize, Serialize};

use crate::runtime::ArcEngineError;

/// First local JSON package schema for `arc-engine`.
pub const ARC_ENGINE_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcPoint {
    pub x: u8,
    pub y: u8,
}

impl ArcPoint {
    #[must_use]
    pub fn within(self, width: u8, height: u8) -> bool {
        self.x < width && self.y < height
    }

    pub fn checked_offset(self, dx: i16, dy: i16, width: u8, height: u8) -> Option<Self> {
        let next_x = i16::from(self.x) + dx;
        let next_y = i16::from(self.y) + dy;
        let Ok(next_x) = u8::try_from(next_x) else {
            return None;
        };
        let Ok(next_y) = u8::try_from(next_y) else {
            return None;
        };
        let point = Self {
            x: next_x,
            y: next_y,
        };
        point.within(width, height).then_some(point)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcBlockingMode {
    NotBlocked,
    BoundingBox,
    PixelPerfect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcInteractionMode {
    Tangible,
    Intangible,
    Invisible,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcSprite {
    pub id: String,
    pub width: u8,
    pub height: u8,
    pub pixels: Vec<Option<u8>>,
    #[serde(default = "default_blocking_mode")]
    pub blocking: ArcBlockingMode,
    #[serde(default = "default_interaction_mode")]
    pub interaction: ArcInteractionMode,
    #[serde(default)]
    pub z_index: i16,
}

impl ArcSprite {
    pub fn validate(&self) -> Result<(), ArcEngineError> {
        validate_identifier("sprite id", &self.id)?;
        if self.width == 0 || self.width > ARC_FRAME_MAX_EDGE {
            return Err(ArcEngineError::InvalidSpriteWidth {
                sprite_id: self.id.clone(),
                width: self.width,
            });
        }
        if self.height == 0 || self.height > ARC_FRAME_MAX_EDGE {
            return Err(ArcEngineError::InvalidSpriteHeight {
                sprite_id: self.id.clone(),
                height: self.height,
            });
        }
        let expected = usize::from(self.width) * usize::from(self.height);
        if self.pixels.len() != expected {
            return Err(ArcEngineError::SpritePixelCountMismatch {
                sprite_id: self.id.clone(),
                expected,
                actual: self.pixels.len(),
            });
        }
        if let Some(color) = self
            .pixels
            .iter()
            .flatten()
            .copied()
            .find(|color| *color >= ARC_FRAME_PALETTE_SIZE)
        {
            return Err(ArcEngineError::InvalidSpriteColor {
                sprite_id: self.id.clone(),
                color,
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn pixel_at(&self, x: u8, y: u8) -> Option<Option<u8>> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = usize::from(y) * usize::from(self.width) + usize::from(x);
        self.pixels.get(index).copied()
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        matches!(
            self.interaction,
            ArcInteractionMode::Tangible | ArcInteractionMode::Intangible
        )
    }

    #[must_use]
    pub fn is_collidable(&self) -> bool {
        matches!(
            self.interaction,
            ArcInteractionMode::Tangible | ArcInteractionMode::Invisible
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcCamera {
    #[serde(default)]
    pub x: u8,
    #[serde(default)]
    pub y: u8,
    pub width: u8,
    pub height: u8,
    #[serde(default)]
    pub background: u8,
    #[serde(default)]
    pub letter_box: u8,
}

impl ArcCamera {
    pub fn validate(
        &self,
        level_id: &str,
        world_width: u8,
        world_height: u8,
    ) -> Result<(), ArcEngineError> {
        if self.width == 0 || self.width > ARC_FRAME_MAX_EDGE {
            return Err(ArcEngineError::InvalidCameraViewportWidth {
                level_id: level_id.to_owned(),
                width: self.width,
            });
        }
        if self.height == 0 || self.height > ARC_FRAME_MAX_EDGE {
            return Err(ArcEngineError::InvalidCameraViewportHeight {
                level_id: level_id.to_owned(),
                height: self.height,
            });
        }
        if self.background >= ARC_FRAME_PALETTE_SIZE {
            return Err(ArcEngineError::InvalidCameraColor {
                level_id: level_id.to_owned(),
                field: "background",
                color: self.background,
            });
        }
        if self.letter_box >= ARC_FRAME_PALETTE_SIZE {
            return Err(ArcEngineError::InvalidCameraColor {
                level_id: level_id.to_owned(),
                field: "letter_box",
                color: self.letter_box,
            });
        }
        if self.x.saturating_add(self.width) > world_width
            || self.y.saturating_add(self.height) > world_height
        {
            return Err(ArcEngineError::CameraOutsideLevel {
                level_id: level_id.to_owned(),
                world_width,
                world_height,
                origin_x: self.x,
                origin_y: self.y,
                viewport_width: self.width,
                viewport_height: self.height,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcInteractionTrigger {
    OnEnter,
    Action5,
    Action6,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcLevelEffect {
    ToggleFlag { flag_id: String },
    CompleteLevel,
    TeleportPlayer { destination: ArcPoint },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcLevelTarget {
    pub id: String,
    pub trigger: ArcInteractionTrigger,
    pub position: ArcPoint,
    pub effect: ArcLevelEffect,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcStateGate {
    FlagSet { flag_id: String },
    FlagUnset { flag_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcSpriteInstance {
    pub sprite_id: String,
    pub position: ArcPoint,
    #[serde(default = "default_scale")]
    pub scale: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_gate: Option<ArcStateGate>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcLevelDefinition {
    pub id: String,
    pub background: ArcGrid,
    pub camera: ArcCamera,
    pub available_actions: Vec<ArcActionKind>,
    pub player_spawn: ArcPoint,
    pub player_sprite_id: String,
    #[serde(default)]
    pub static_sprites: Vec<ArcSpriteInstance>,
    #[serde(default)]
    pub targets: Vec<ArcLevelTarget>,
    #[serde(default)]
    pub solid_background_colors: Vec<u8>,
    pub max_actions: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcGamePackage {
    #[serde(default = "engine_schema_version")]
    pub schema_version: u16,
    pub name: String,
    pub task_id: ArcTaskId,
    pub version: String,
    pub sprites: Vec<ArcSprite>,
    pub levels: Vec<ArcLevelDefinition>,
}

pub fn load_game_package(path: impl AsRef<Path>) -> Result<ArcGamePackage, ArcEngineError> {
    let raw = fs::read_to_string(path.as_ref())?;
    let package: ArcGamePackage = serde_json::from_str(&raw)?;
    validate_package(&package)?;
    Ok(package)
}

pub(crate) fn validate_package(package: &ArcGamePackage) -> Result<(), ArcEngineError> {
    if package.schema_version != ARC_ENGINE_SCHEMA_VERSION {
        return Err(ArcEngineError::UnsupportedSchemaVersion {
            version: package.schema_version,
        });
    }
    validate_nonempty_label("package name", &package.name)?;
    validate_nonempty_label("package version", &package.version)?;
    if package.levels.is_empty() {
        return Err(ArcEngineError::NoLevels);
    }

    let mut sprite_index = BTreeMap::new();
    for sprite in &package.sprites {
        sprite.validate()?;
        if sprite_index.insert(sprite.id.clone(), sprite).is_some() {
            return Err(ArcEngineError::DuplicateSpriteId(sprite.id.clone()));
        }
    }

    let mut level_ids = BTreeSet::new();
    for level in &package.levels {
        validate_identifier("level id", &level.id)?;
        if !level_ids.insert(level.id.clone()) {
            return Err(ArcEngineError::DuplicateLevelId(level.id.clone()));
        }
        validate_level(level, &sprite_index)?;
    }

    Ok(())
}

fn validate_level(
    level: &ArcLevelDefinition,
    sprite_index: &BTreeMap<String, &ArcSprite>,
) -> Result<(), ArcEngineError> {
    if level.max_actions == 0 {
        return Err(ArcEngineError::InvalidMaxActions(level.id.clone()));
    }
    let world_width = level.background.width();
    let world_height = level.background.height();
    level
        .camera
        .validate(&level.id, world_width, world_height)?;

    if level.available_actions.is_empty() {
        return Err(ArcEngineError::MissingAvailableActions(level.id.clone()));
    }
    if !level.player_spawn.within(world_width, world_height) {
        return Err(point_outside_level(&level.id, level.player_spawn));
    }
    validate_identifier("player sprite id", &level.player_sprite_id)?;
    if !sprite_index.contains_key(&level.player_sprite_id) {
        return Err(ArcEngineError::UnknownSpriteId {
            level_id: level.id.clone(),
            sprite_id: level.player_sprite_id.clone(),
        });
    }

    if let Some(color) = level
        .solid_background_colors
        .iter()
        .copied()
        .find(|color| *color >= ARC_PALETTE_SIZE)
    {
        return Err(ArcEngineError::InvalidBackgroundColor {
            level_id: level.id.clone(),
            color,
        });
    }

    let mut target_ids = BTreeSet::new();
    for target in &level.targets {
        validate_identifier("level target id", &target.id)?;
        if !target_ids.insert(target.id.clone()) {
            return Err(ArcEngineError::DuplicateTargetId {
                level_id: level.id.clone(),
                target_id: target.id.clone(),
            });
        }
        if !target.position.within(world_width, world_height) {
            return Err(point_outside_level(&level.id, target.position));
        }
        match &target.effect {
            ArcLevelEffect::ToggleFlag { flag_id } => validate_identifier("flag id", flag_id)?,
            ArcLevelEffect::TeleportPlayer { destination } => {
                if !destination.within(world_width, world_height) {
                    return Err(point_outside_level(&level.id, *destination));
                }
            }
            ArcLevelEffect::CompleteLevel => {}
        }
    }

    for instance in &level.static_sprites {
        validate_identifier("sprite id", &instance.sprite_id)?;
        let Some(sprite) = sprite_index.get(&instance.sprite_id) else {
            return Err(ArcEngineError::UnknownSpriteId {
                level_id: level.id.clone(),
                sprite_id: instance.sprite_id.clone(),
            });
        };
        if instance.scale == 0 {
            return Err(ArcEngineError::InvalidSpriteScale {
                level_id: level.id.clone(),
                sprite_id: instance.sprite_id.clone(),
                scale: instance.scale,
            });
        }
        validate_sprite_footprint(
            level,
            instance.position,
            sprite.width,
            sprite.height,
            instance.scale,
        )?;
        if let Some(gate) = &instance.state_gate {
            match gate {
                ArcStateGate::FlagSet { flag_id } | ArcStateGate::FlagUnset { flag_id } => {
                    validate_identifier("flag id", flag_id)?
                }
            }
        }
    }

    Ok(())
}

fn validate_sprite_footprint(
    level: &ArcLevelDefinition,
    position: ArcPoint,
    sprite_width: u8,
    sprite_height: u8,
    scale: u8,
) -> Result<(), ArcEngineError> {
    if !position.within(level.background.width(), level.background.height()) {
        return Err(point_outside_level(&level.id, position));
    }
    let scaled_width = sprite_width.saturating_mul(scale);
    let scaled_height = sprite_height.saturating_mul(scale);
    if position.x.saturating_add(scaled_width) > level.background.width()
        || position.y.saturating_add(scaled_height) > level.background.height()
    {
        return Err(sprite_outside_level(
            &level.id,
            position,
            scaled_width,
            scaled_height,
        ));
    }
    Ok(())
}

pub(crate) fn validate_identifier(kind: &'static str, raw: &str) -> Result<(), ArcEngineError> {
    if raw.trim().is_empty() {
        return Err(ArcEngineError::EmptyIdentifier { kind });
    }
    if raw.chars().any(char::is_whitespace) {
        return Err(ArcEngineError::InvalidIdentifier {
            kind,
            value: raw.to_owned(),
        });
    }
    Ok(())
}

fn validate_nonempty_label(kind: &'static str, raw: &str) -> Result<(), ArcEngineError> {
    if raw.trim().is_empty() {
        return Err(ArcEngineError::EmptyLabel { kind });
    }
    Ok(())
}

pub(crate) fn point_outside_level(level_id: &str, point: ArcPoint) -> ArcEngineError {
    ArcEngineError::PointOutsideLevel {
        level_id: level_id.to_owned(),
        x: point.x,
        y: point.y,
    }
}

pub(crate) fn sprite_outside_level(
    level_id: &str,
    position: ArcPoint,
    sprite_width: u8,
    sprite_height: u8,
) -> ArcEngineError {
    ArcEngineError::SpriteOutsideLevel {
        level_id: level_id.to_owned(),
        x: position.x,
        y: position.y,
        sprite_width,
        sprite_height,
    }
}

const fn default_blocking_mode() -> ArcBlockingMode {
    ArcBlockingMode::PixelPerfect
}

const fn default_interaction_mode() -> ArcInteractionMode {
    ArcInteractionMode::Tangible
}

const fn default_scale() -> u8 {
    1
}

const fn engine_schema_version() -> u16 {
    ARC_ENGINE_SCHEMA_VERSION
}
