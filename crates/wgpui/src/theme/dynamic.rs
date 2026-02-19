use std::sync::Arc;

pub type ThemeMultiplierFn = Arc<dyn Fn(f32) -> f32 + Send + Sync>;
pub type ThemeUnitFn = Arc<dyn Fn(f32) -> String + Send + Sync>;

#[derive(Clone)]
pub enum ThemeMultiplierSettings {
    Scalar(f32),
    Series(Vec<f32>),
    Function(ThemeMultiplierFn),
}

#[derive(Clone)]
pub struct ThemeMultiplier {
    settings: ThemeMultiplierSettings,
}

impl ThemeMultiplier {
    pub fn new(settings: impl Into<ThemeMultiplierSettings>) -> Self {
        Self {
            settings: settings.into(),
        }
    }

    pub fn value(&self, index: f32) -> f32 {
        match &self.settings {
            ThemeMultiplierSettings::Scalar(base) => base * index,
            ThemeMultiplierSettings::Series(series) => {
                if series.is_empty() {
                    return 0.0;
                }
                let index = index.round().max(0.0) as usize;
                let clamped = index.min(series.len().saturating_sub(1));
                series[clamped]
            }
            ThemeMultiplierSettings::Function(f) => f(index),
        }
    }
}

impl From<f32> for ThemeMultiplierSettings {
    fn from(value: f32) -> Self {
        ThemeMultiplierSettings::Scalar(value)
    }
}

impl From<Vec<f32>> for ThemeMultiplierSettings {
    fn from(series: Vec<f32>) -> Self {
        ThemeMultiplierSettings::Series(series)
    }
}

impl From<&[f32]> for ThemeMultiplierSettings {
    fn from(series: &[f32]) -> Self {
        ThemeMultiplierSettings::Series(series.to_vec())
    }
}

impl<F> From<F> for ThemeMultiplierSettings
where
    F: Fn(f32) -> f32 + Send + Sync + 'static,
{
    fn from(func: F) -> Self {
        ThemeMultiplierSettings::Function(Arc::new(func))
    }
}

pub fn create_theme_multiplier(settings: impl Into<ThemeMultiplierSettings>) -> ThemeMultiplier {
    ThemeMultiplier::new(settings)
}

#[derive(Clone)]
pub enum ThemeUnitSettings {
    Series(Vec<String>),
    Function(ThemeUnitFn),
}

#[derive(Clone)]
pub struct ThemeUnit {
    settings: ThemeUnitSettings,
}

impl ThemeUnit {
    pub fn new(settings: impl Into<ThemeUnitSettings>) -> Self {
        Self {
            settings: settings.into(),
        }
    }

    pub fn value(&self, input: impl Into<UnitInput>) -> String {
        let input = input.into();
        let values = match input {
            UnitInput::Single(value) => vec![value],
            UnitInput::List(list) => list,
        };

        match &self.settings {
            ThemeUnitSettings::Series(series) => {
                if series.is_empty() {
                    return String::new();
                }
                values
                    .into_iter()
                    .map(|value| match value {
                        UnitValue::Literal(text) => text,
                        UnitValue::Number(index) => series_item(series, index),
                    })
                    .collect::<Vec<String>>()
                    .join(" ")
            }
            ThemeUnitSettings::Function(func) => values
                .into_iter()
                .map(|value| match value {
                    UnitValue::Literal(text) => text,
                    UnitValue::Number(index) => func(index),
                })
                .collect::<Vec<String>>()
                .join(" "),
        }
    }
}

impl From<Vec<String>> for ThemeUnitSettings {
    fn from(series: Vec<String>) -> Self {
        ThemeUnitSettings::Series(series)
    }
}

impl From<Vec<&str>> for ThemeUnitSettings {
    fn from(series: Vec<&str>) -> Self {
        ThemeUnitSettings::Series(series.into_iter().map(|s| s.to_string()).collect())
    }
}

impl From<&[&str]> for ThemeUnitSettings {
    fn from(series: &[&str]) -> Self {
        ThemeUnitSettings::Series(series.iter().map(|s| s.to_string()).collect())
    }
}

impl<F> From<F> for ThemeUnitSettings
where
    F: Fn(f32) -> String + Send + Sync + 'static,
{
    fn from(func: F) -> Self {
        ThemeUnitSettings::Function(Arc::new(func))
    }
}

pub fn create_theme_unit(settings: impl Into<ThemeUnitSettings>) -> ThemeUnit {
    ThemeUnit::new(settings)
}

#[derive(Clone, Debug, PartialEq)]
pub enum UnitValue {
    Number(f32),
    Literal(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum UnitInput {
    Single(UnitValue),
    List(Vec<UnitValue>),
}

impl From<UnitValue> for UnitInput {
    fn from(value: UnitValue) -> Self {
        UnitInput::Single(value)
    }
}

impl From<Vec<UnitValue>> for UnitInput {
    fn from(values: Vec<UnitValue>) -> Self {
        UnitInput::List(values)
    }
}

impl From<f32> for UnitInput {
    fn from(value: f32) -> Self {
        UnitInput::Single(UnitValue::Number(value))
    }
}

impl From<i32> for UnitInput {
    fn from(value: i32) -> Self {
        UnitInput::Single(UnitValue::Number(value as f32))
    }
}

impl From<&str> for UnitInput {
    fn from(value: &str) -> Self {
        UnitInput::Single(UnitValue::Literal(value.to_string()))
    }
}

impl From<String> for UnitInput {
    fn from(value: String) -> Self {
        UnitInput::Single(UnitValue::Literal(value))
    }
}

impl From<Vec<f32>> for UnitInput {
    fn from(values: Vec<f32>) -> Self {
        UnitInput::List(values.into_iter().map(UnitValue::Number).collect())
    }
}

impl From<Vec<i32>> for UnitInput {
    fn from(values: Vec<i32>) -> Self {
        UnitInput::List(
            values
                .into_iter()
                .map(|v| UnitValue::Number(v as f32))
                .collect(),
        )
    }
}

impl From<Vec<&str>> for UnitInput {
    fn from(values: Vec<&str>) -> Self {
        UnitInput::List(
            values
                .into_iter()
                .map(|v| UnitValue::Literal(v.to_string()))
                .collect(),
        )
    }
}

impl From<Vec<String>> for UnitInput {
    fn from(values: Vec<String>) -> Self {
        UnitInput::List(values.into_iter().map(UnitValue::Literal).collect())
    }
}

pub fn number(value: f32) -> UnitValue {
    UnitValue::Number(value)
}

pub fn literal(value: impl Into<String>) -> UnitValue {
    UnitValue::Literal(value.into())
}

#[derive(Clone, Debug, PartialEq)]
pub enum ThemeBreakpointSetting {
    Value(String),
    KeyValue { key: String, value: String },
}

impl ThemeBreakpointSetting {
    pub fn value(value: impl Into<String>) -> Self {
        ThemeBreakpointSetting::Value(value.into())
    }

    pub fn named(key: impl Into<String>, value: impl Into<String>) -> Self {
        ThemeBreakpointSetting::KeyValue {
            key: key.into(),
            value: value.into(),
        }
    }

    pub fn key(&self) -> Option<&str> {
        match self {
            ThemeBreakpointSetting::KeyValue { key, .. } => Some(key.as_str()),
            _ => None,
        }
    }

    pub fn value_str(&self) -> &str {
        match self {
            ThemeBreakpointSetting::Value(value) => value.as_str(),
            ThemeBreakpointSetting::KeyValue { value, .. } => value.as_str(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ThemeBreakpoints {
    breakpoints: Vec<String>,
    settings: Vec<ThemeBreakpointSetting>,
}

impl ThemeBreakpoints {
    pub fn new(settings: Vec<ThemeBreakpointSetting>) -> Self {
        let breakpoints = settings
            .iter()
            .filter_map(|item| item.key().map(|k| k.to_string()))
            .collect();
        Self {
            breakpoints,
            settings,
        }
    }

    pub fn breakpoints(&self) -> &[String] {
        &self.breakpoints
    }

    pub fn settings(&self) -> &[ThemeBreakpointSetting] {
        &self.settings
    }

    pub fn up(&self, key: impl Into<BreakpointKey>) -> String {
        self.up_with(key, MediaQueryOptions::default())
    }

    pub fn up_with(&self, key: impl Into<BreakpointKey>, opts: MediaQueryOptions) -> String {
        let value = self.breakpoint_value(key.into());
        format!("{}(min-width: {})", media_prefix(opts), value)
    }

    pub fn down(&self, key: impl Into<BreakpointKey>) -> String {
        self.down_with(key, MediaQueryOptions::default())
    }

    pub fn down_with(&self, key: impl Into<BreakpointKey>, opts: MediaQueryOptions) -> String {
        let value = self.breakpoint_value(key.into());
        format!("{}(max-width: calc({} - 1px))", media_prefix(opts), value)
    }

    pub fn between(
        &self,
        start_key: impl Into<BreakpointKey>,
        end_key: impl Into<BreakpointKey>,
    ) -> String {
        self.between_with(start_key, end_key, MediaQueryOptions::default())
    }

    pub fn between_with(
        &self,
        start_key: impl Into<BreakpointKey>,
        end_key: impl Into<BreakpointKey>,
        opts: MediaQueryOptions,
    ) -> String {
        let min = self.breakpoint_value(start_key.into());
        let max = self.breakpoint_value(end_key.into());
        format!(
            "{}(min-width: {}) and (max-width: calc({} - 1px))",
            media_prefix(opts),
            min,
            max
        )
    }

    fn breakpoint_value(&self, key: BreakpointKey) -> String {
        match key {
            BreakpointKey::Name(name) => {
                for item in &self.settings {
                    if let ThemeBreakpointSetting::KeyValue { key, value } = item
                        && key == &name
                    {
                        return value.clone();
                    }
                }
                name
            }
            BreakpointKey::Index(index) => {
                if self.settings.is_empty() {
                    return String::new();
                }
                let idx = index.min(self.settings.len().saturating_sub(1));
                self.settings[idx].value_str().to_string()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MediaQueryOptions {
    pub strip: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BreakpointKey {
    Name(String),
    Index(usize),
}

impl From<&str> for BreakpointKey {
    fn from(value: &str) -> Self {
        BreakpointKey::Name(value.to_string())
    }
}

impl From<String> for BreakpointKey {
    fn from(value: String) -> Self {
        BreakpointKey::Name(value)
    }
}

impl From<usize> for BreakpointKey {
    fn from(value: usize) -> Self {
        BreakpointKey::Index(value)
    }
}

pub fn create_theme_breakpoints(settings: Vec<ThemeBreakpointSetting>) -> ThemeBreakpoints {
    ThemeBreakpoints::new(settings)
}

fn series_item(series: &[String], index: f32) -> String {
    let index = index.round().max(0.0) as usize;
    let clamped = index.min(series.len().saturating_sub(1));
    series[clamped].clone()
}

fn media_prefix(options: MediaQueryOptions) -> &'static str {
    if options.strip { "" } else { "@media " }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiplier_scalar() {
        let multiplier = ThemeMultiplier::new(2.0);
        assert_eq!(multiplier.value(0.0), 0.0);
        assert_eq!(multiplier.value(1.0), 2.0);
        assert_eq!(multiplier.value(2.0), 4.0);
        assert_eq!(multiplier.value(3.0), 6.0);
    }

    #[test]
    fn test_multiplier_function() {
        let multiplier = ThemeMultiplier::new(|i: f32| i * 3.0);
        assert_eq!(multiplier.value(0.0), 0.0);
        assert_eq!(multiplier.value(1.0), 3.0);
        assert_eq!(multiplier.value(2.0), 6.0);
        assert_eq!(multiplier.value(3.0), 9.0);
    }

    #[test]
    fn test_multiplier_series() {
        let multiplier = ThemeMultiplier::new(vec![0.0, 1.0, 2.0, 4.0, 8.0]);
        assert_eq!(multiplier.value(0.0), 0.0);
        assert_eq!(multiplier.value(1.0), 1.0);
        assert_eq!(multiplier.value(2.0), 2.0);
        assert_eq!(multiplier.value(3.0), 4.0);
        assert_eq!(multiplier.value(4.0), 8.0);
    }

    #[test]
    fn test_multiplier_series_clamp() {
        let multiplier = ThemeMultiplier::new(vec![0.0, 1.0, 2.0]);
        assert_eq!(multiplier.value(3.0), 2.0);
        assert_eq!(multiplier.value(4.0), 2.0);
    }

    #[test]
    fn test_multiplier_empty_series() {
        let multiplier = ThemeMultiplier::new(Vec::<f32>::new());
        assert_eq!(multiplier.value(0.0), 0.0);
        assert_eq!(multiplier.value(1.0), 0.0);
    }

    #[test]
    fn test_unit_function_values() {
        let unit = ThemeUnit::new(|i: f32| format!("{}px", i * 3.0));
        assert_eq!(unit.value(0.0), "0px");
        assert_eq!(unit.value(1.0), "3px");
        assert_eq!(unit.value(2.0), "6px");
        assert_eq!(unit.value(3.0), "9px");
    }

    #[test]
    fn test_unit_function_list() {
        let unit = ThemeUnit::new(|i: f32| format!("{}rem", i * 3.0));
        assert_eq!(unit.value(Vec::<f32>::new()), "");
        assert_eq!(unit.value(vec![2.0]), "6rem");
        assert_eq!(unit.value(vec![0.0, 2.0]), "0rem 6rem");
        assert_eq!(unit.value(vec![1.0, 1.0]), "3rem 3rem");
        assert_eq!(unit.value(vec![3.0, 0.0, 2.0]), "9rem 0rem 6rem");
    }

    #[test]
    fn test_unit_series_values() {
        let unit = ThemeUnit::new(vec!["0px", "1px", "2px", "4px", "8px"]);
        assert_eq!(unit.value(0.0), "0px");
        assert_eq!(unit.value(1.0), "1px");
        assert_eq!(unit.value(2.0), "2px");
        assert_eq!(unit.value(3.0), "4px");
        assert_eq!(unit.value(4.0), "8px");
    }

    #[test]
    fn test_unit_series_list() {
        let unit = ThemeUnit::new(vec!["0px", "1px", "2px", "4px", "8px"]);
        assert_eq!(unit.value(Vec::<f32>::new()), "");
        assert_eq!(unit.value(vec![2.0]), "2px");
        assert_eq!(unit.value(vec![0.0, 2.0]), "0px 2px");
        assert_eq!(unit.value(vec![1.0, 1.0]), "1px 1px");
        assert_eq!(unit.value(vec![3.0, 0.0, 2.0]), "4px 0px 2px");
    }

    #[test]
    fn test_unit_series_clamp() {
        let unit = ThemeUnit::new(vec!["0px", "1px", "2px"]);
        assert_eq!(unit.value(3.0), "2px");
        assert_eq!(unit.value(4.0), "2px");
    }

    #[test]
    fn test_unit_literal_passthrough() {
        let unit = ThemeUnit::new(|i: f32| format!("{}rem", i * 2.0));
        assert_eq!(unit.value("auto"), "auto");
        assert_eq!(
            unit.value(vec![number(2.0), literal("auto"), number(1.0)]),
            "4rem auto 2rem"
        );
    }

    #[test]
    fn test_unit_empty_series() {
        let unit = ThemeUnit::new(Vec::<String>::new());
        assert_eq!(unit.value(0.0), "");
        assert_eq!(unit.value(1.0), "");
    }

    #[test]
    fn test_breakpoints_keys() {
        let breakpoints = ThemeBreakpoints::new(vec![
            ThemeBreakpointSetting::named("a", "x"),
            ThemeBreakpointSetting::named("b", "y"),
            ThemeBreakpointSetting::named("c", "z"),
        ]);
        assert_eq!(breakpoints.breakpoints(), &["a", "b", "c"]);
        assert_eq!(breakpoints.settings().len(), 3);
    }

    #[test]
    fn test_breakpoints_up_down_between() {
        let breakpoints = ThemeBreakpoints::default();
        assert_eq!(breakpoints.up("100px"), "@media (min-width: 100px)");
        assert_eq!(
            breakpoints.up_with("100px", MediaQueryOptions { strip: true }),
            "(min-width: 100px)"
        );
        assert_eq!(
            breakpoints.down("20rem"),
            "@media (max-width: calc(20rem - 1px))"
        );
        assert_eq!(
            breakpoints.down_with("20rem", MediaQueryOptions { strip: true }),
            "(max-width: calc(20rem - 1px))"
        );
        assert_eq!(
            breakpoints.between("100px", "200px"),
            "@media (min-width: 100px) and (max-width: calc(200px - 1px))"
        );
        assert_eq!(
            breakpoints.between_with("100px", "200px", MediaQueryOptions { strip: true }),
            "(min-width: 100px) and (max-width: calc(200px - 1px))"
        );
    }
}
