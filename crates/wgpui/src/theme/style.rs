use std::collections::BTreeMap;

pub type ThemeStyleValue = BTreeMap<String, String>;
pub type ThemeStyleSettings = Vec<ThemeStyleValue>;

#[derive(Clone, Debug)]
pub struct ThemeStyle {
    series: ThemeStyleSettings,
}

impl ThemeStyle {
    pub fn new(series: ThemeStyleSettings) -> Self {
        Self { series }
    }

    pub fn value(&self, index: f32) -> ThemeStyleValue {
        if self.series.is_empty() {
            return ThemeStyleValue::new();
        }

        let index = index.round().max(0.0) as usize;
        let clamped = index.min(self.series.len().saturating_sub(1));
        self.series[clamped].clone()
    }
}

pub fn create_theme_style(series: ThemeStyleSettings) -> ThemeStyle {
    ThemeStyle::new(series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_style_returns_last() {
        let mut style_a = ThemeStyleValue::new();
        style_a.insert("color".to_string(), "red".to_string());

        let mut style_b = ThemeStyleValue::new();
        style_b.insert("color".to_string(), "blue".to_string());

        let theme = create_theme_style(vec![style_a.clone(), style_b.clone()]);
        assert_eq!(theme.value(0.0), style_a);
        assert_eq!(theme.value(1.0), style_b.clone());
        assert_eq!(theme.value(5.0), style_b);
    }
}
