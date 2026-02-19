use std::any::Any;
use std::collections::BTreeMap;
use std::sync::Arc;

use super::color::{ThemeColor, ThemeColorSettings, create_theme_color};
use super::dynamic::{
    ThemeBreakpointSetting, ThemeBreakpoints, ThemeMultiplier, ThemeMultiplierSettings, ThemeUnit,
    ThemeUnitSettings, create_theme_breakpoints, create_theme_multiplier, create_theme_unit,
};
use super::style::{ThemeStyle, ThemeStyleSettings, create_theme_style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeStructureKind {
    Multiplier,
    Unit,
    Color,
    Style,
    Breakpoints,
    Other,
}

#[derive(Clone, Debug)]
pub enum ThemeStructure {
    Group(BTreeMap<String, ThemeStructure>),
    Leaf(ThemeStructureKind),
}

pub type ThemeOtherValue = Arc<dyn Any + Send + Sync>;

#[derive(Clone)]
pub enum ThemeSettingsValue {
    Multiplier(ThemeMultiplierSettings),
    Unit(ThemeUnitSettings),
    Color(ThemeColorSettings),
    Style(ThemeStyleSettings),
    Breakpoints(Vec<ThemeBreakpointSetting>),
    Other(ThemeOtherValue),
    Group(BTreeMap<String, ThemeSettingsValue>),
}

#[derive(Clone)]
pub enum ThemeValue {
    Multiplier(ThemeMultiplier),
    Unit(ThemeUnit),
    Color(ThemeColor),
    Style(ThemeStyle),
    Breakpoints(ThemeBreakpoints),
    Other(ThemeOtherValue),
    Group(BTreeMap<String, ThemeValue>),
}

#[derive(Clone, Debug)]
pub enum ThemeBuilderError {
    MissingDefault(&'static str),
    InvalidStructure(&'static str),
}

#[derive(Clone)]
pub enum ThemeSettingsExtensions {
    Single(ThemeSettingsValue),
    Multiple(Vec<ThemeSettingsValue>),
}

pub struct ThemeCreator {
    structure: ThemeStructure,
    defaults: ThemeSettingsValue,
}

impl ThemeCreator {
    pub fn new(structure: ThemeStructure, defaults: ThemeSettingsValue) -> Self {
        Self {
            structure,
            defaults,
        }
    }

    pub fn create(
        &self,
        extensions: Option<ThemeSettingsExtensions>,
    ) -> Result<ThemeValue, ThemeBuilderError> {
        let mut settings = self.defaults.clone();

        if let Some(extensions) = extensions {
            match extensions {
                ThemeSettingsExtensions::Single(ext) => {
                    settings = merge_settings(&self.structure, &settings, Some(&ext))?;
                }
                ThemeSettingsExtensions::Multiple(list) => {
                    for ext in list {
                        settings = merge_settings(&self.structure, &settings, Some(&ext))?;
                    }
                }
            }
        }

        build_theme(&self.structure, &settings)
    }
}

pub fn create_create_theme(
    structure: ThemeStructure,
    defaults: ThemeSettingsValue,
) -> ThemeCreator {
    ThemeCreator::new(structure, defaults)
}

fn merge_settings(
    structure: &ThemeStructure,
    defaults: &ThemeSettingsValue,
    extension: Option<&ThemeSettingsValue>,
) -> Result<ThemeSettingsValue, ThemeBuilderError> {
    match structure {
        ThemeStructure::Leaf(_) => Ok(extension.cloned().unwrap_or_else(|| defaults.clone())),
        ThemeStructure::Group(children) => {
            let defaults_map = match defaults {
                ThemeSettingsValue::Group(map) => map,
                _ => return Err(ThemeBuilderError::MissingDefault("group")),
            };

            let extension_map = match extension {
                Some(ThemeSettingsValue::Group(map)) => Some(map),
                _ => None,
            };

            let mut merged = BTreeMap::new();
            for (key, child) in children {
                let default_child = defaults_map
                    .get(key)
                    .ok_or(ThemeBuilderError::MissingDefault("child"))?;
                let ext_child = extension_map.and_then(|map| map.get(key));
                let merged_child = merge_settings(child, default_child, ext_child)?;
                merged.insert(key.clone(), merged_child);
            }

            Ok(ThemeSettingsValue::Group(merged))
        }
    }
}

fn build_theme(
    structure: &ThemeStructure,
    settings: &ThemeSettingsValue,
) -> Result<ThemeValue, ThemeBuilderError> {
    match structure {
        ThemeStructure::Leaf(kind) => match (kind, settings) {
            (ThemeStructureKind::Multiplier, ThemeSettingsValue::Multiplier(value)) => Ok(
                ThemeValue::Multiplier(create_theme_multiplier(value.clone())),
            ),
            (ThemeStructureKind::Unit, ThemeSettingsValue::Unit(value)) => {
                Ok(ThemeValue::Unit(create_theme_unit(value.clone())))
            }
            (ThemeStructureKind::Color, ThemeSettingsValue::Color(value)) => {
                Ok(ThemeValue::Color(create_theme_color(value.clone())))
            }
            (ThemeStructureKind::Style, ThemeSettingsValue::Style(value)) => {
                Ok(ThemeValue::Style(create_theme_style(value.clone())))
            }
            (ThemeStructureKind::Breakpoints, ThemeSettingsValue::Breakpoints(value)) => Ok(
                ThemeValue::Breakpoints(create_theme_breakpoints(value.clone())),
            ),
            (ThemeStructureKind::Other, ThemeSettingsValue::Other(value)) => {
                Ok(ThemeValue::Other(value.clone()))
            }
            _ => Err(ThemeBuilderError::InvalidStructure("leaf mismatch")),
        },
        ThemeStructure::Group(children) => {
            let settings_map = match settings {
                ThemeSettingsValue::Group(map) => map,
                _ => return Err(ThemeBuilderError::InvalidStructure("group mismatch")),
            };

            let mut output = BTreeMap::new();
            for (key, child) in children {
                let settings_child = settings_map
                    .get(key)
                    .ok_or(ThemeBuilderError::MissingDefault("child"))?;
                let value = build_theme(child, settings_child)?;
                output.insert(key.clone(), value);
            }
            Ok(ThemeValue::Group(output))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::color::{ThemeColorInput, ThemeColorOptions};

    #[test]
    fn test_theme_creator_merges_settings() {
        let mut structure = BTreeMap::new();
        structure.insert(
            "scale".to_string(),
            ThemeStructure::Leaf(ThemeStructureKind::Multiplier),
        );
        structure.insert(
            "palette".to_string(),
            ThemeStructure::Leaf(ThemeStructureKind::Color),
        );
        let structure = ThemeStructure::Group(structure);

        let mut defaults = BTreeMap::new();
        defaults.insert(
            "scale".to_string(),
            ThemeSettingsValue::Multiplier(ThemeMultiplierSettings::Scalar(2.0)),
        );
        defaults.insert(
            "palette".to_string(),
            ThemeSettingsValue::Color(ThemeColorSettings::Series(vec![ThemeColorInput::hsl(
                0.0, 0.0, 50.0, 1.0,
            )])),
        );
        let defaults = ThemeSettingsValue::Group(defaults);

        let creator = create_create_theme(structure, defaults);

        let mut overrides = BTreeMap::new();
        overrides.insert(
            "scale".to_string(),
            ThemeSettingsValue::Multiplier(ThemeMultiplierSettings::Scalar(3.0)),
        );
        let theme = creator
            .create(Some(ThemeSettingsExtensions::Single(
                ThemeSettingsValue::Group(overrides),
            )))
            .expect("theme");

        if let ThemeValue::Group(values) = theme {
            if let ThemeValue::Multiplier(multiplier) = values.get("scale").unwrap() {
                assert!((multiplier.value(2.0) - 6.0).abs() < 0.001);
            } else {
                panic!("expected multiplier");
            }

            if let ThemeValue::Color(color) = values.get("palette").unwrap() {
                let color = color.value(0.0, ThemeColorOptions::default());
                assert!(color.l > 0.4 && color.l < 0.6);
            } else {
                panic!("expected color");
            }
        } else {
            panic!("expected theme group");
        }
    }
}
