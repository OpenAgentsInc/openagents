use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CadError, CadResult};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextProfile {
    pub points: Vec<[f64; 2]>,
    pub is_hole: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlyphDefinition {
    pub advance_em: f64,
    pub contours: Vec<Vec<[f64; 2]>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Font {
    pub name: String,
    pub units_per_em: f64,
    glyphs: BTreeMap<char, GlyphDefinition>,
    fallback_advance_em: f64,
}

impl Font {
    pub fn glyph(&self, c: char) -> Option<&GlyphDefinition> {
        self.glyphs
            .get(&c)
            .or_else(|| self.glyphs.get(&c.to_ascii_uppercase()))
    }

    pub fn advance_em(&self, c: char) -> f64 {
        self.glyph(c)
            .map(|glyph| glyph.advance_em)
            .unwrap_or(self.fallback_advance_em)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FontRegistry {
    fonts: BTreeMap<String, Font>,
}

impl FontRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: &str, font: Font) {
        self.fonts.insert(name.to_string(), font);
    }

    pub fn get(&self, name: &str) -> Option<&Font> {
        self.fonts.get(name)
    }

    pub fn builtin_sans() -> Font {
        let mut glyphs = BTreeMap::new();
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars() {
            glyphs.insert(c, synth_glyph(c));
        }
        Font {
            name: "sans-serif".to_string(),
            units_per_em: 1000.0,
            glyphs,
            fallback_advance_em: 0.6,
        }
    }

    pub fn get_or_builtin(&self, name: &str) -> Font {
        self.get(name).cloned().unwrap_or_else(Self::builtin_sans)
    }
}

pub fn text_to_profiles(
    text: &str,
    font: &Font,
    height_mm: f64,
    letter_spacing: f64,
    line_spacing: f64,
    alignment: TextAlignment,
) -> CadResult<Vec<TextProfile>> {
    validate_text_layout(height_mm, letter_spacing, line_spacing)?;
    if text.is_empty() {
        return Ok(Vec::new());
    }

    let line_height = height_mm * line_spacing;
    let mut profiles = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        let line_width = calculate_line_width(line, font, height_mm, letter_spacing);
        let x_offset = match alignment {
            TextAlignment::Left => 0.0,
            TextAlignment::Center => -line_width / 2.0,
            TextAlignment::Right => -line_width,
        };
        let y_offset = -(line_idx as f64) * line_height;

        let mut cursor_x = x_offset;
        for c in line.chars() {
            if c.is_whitespace() {
                cursor_x += font.advance_em(c) * height_mm * letter_spacing;
                continue;
            }
            if let Some(glyph) = font.glyph(c) {
                let glyph_profiles = glyph_to_profiles(glyph, height_mm, cursor_x, y_offset);
                profiles.extend(glyph_profiles);
            }
            cursor_x += font.advance_em(c) * height_mm * letter_spacing;
        }
    }

    Ok(profiles)
}

pub fn text_bounds(
    text: &str,
    font: &Font,
    height_mm: f64,
    letter_spacing: f64,
    line_spacing: f64,
) -> CadResult<(f64, f64)> {
    validate_text_layout(height_mm, letter_spacing, line_spacing)?;
    if text.is_empty() {
        return Ok((0.0, 0.0));
    }
    let max_width = text
        .lines()
        .map(|line| calculate_line_width(line, font, height_mm, letter_spacing))
        .fold(0.0_f64, f64::max);
    let num_lines = text.lines().count().max(1);
    let total_height = num_lines as f64 * height_mm * line_spacing;
    Ok((max_width, total_height))
}

fn validate_text_layout(height_mm: f64, letter_spacing: f64, line_spacing: f64) -> CadResult<()> {
    if !height_mm.is_finite() || height_mm <= 0.0 {
        return Err(CadError::InvalidParameter {
            name: "height_mm".to_string(),
            reason: "text height must be finite and positive".to_string(),
        });
    }
    if !letter_spacing.is_finite() || letter_spacing <= 0.0 {
        return Err(CadError::InvalidParameter {
            name: "letter_spacing".to_string(),
            reason: "letter spacing must be finite and positive".to_string(),
        });
    }
    if !line_spacing.is_finite() || line_spacing <= 0.0 {
        return Err(CadError::InvalidParameter {
            name: "line_spacing".to_string(),
            reason: "line spacing must be finite and positive".to_string(),
        });
    }
    Ok(())
}

fn calculate_line_width(line: &str, font: &Font, height_mm: f64, letter_spacing: f64) -> f64 {
    line.chars()
        .map(|c| font.advance_em(c) * height_mm * letter_spacing)
        .sum()
}

fn glyph_to_profiles(
    glyph: &GlyphDefinition,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
) -> Vec<TextProfile> {
    glyph
        .contours
        .iter()
        .enumerate()
        .map(|(idx, contour)| TextProfile {
            points: contour
                .iter()
                .map(|point| [point[0] * scale + offset_x, point[1] * scale + offset_y])
                .collect(),
            is_hole: idx > 0,
        })
        .collect()
}

fn synth_glyph(c: char) -> GlyphDefinition {
    let index = (c as u32) % 7;
    let advance_em = 0.52 + index as f64 * 0.045;
    let outer = vec![[0.0, 0.0], [advance_em, 0.0], [advance_em, 1.0], [0.0, 1.0]];

    let has_hole = matches!(
        c,
        'A' | 'B' | 'D' | 'O' | 'P' | 'Q' | 'R' | '0' | '6' | '8' | '9'
    );
    if !has_hole {
        return GlyphDefinition {
            advance_em,
            contours: vec![outer],
        };
    }

    let inner_margin_x = advance_em * 0.22;
    let inner_margin_y = 0.2;
    let inner = vec![
        [inner_margin_x, inner_margin_y],
        [advance_em - inner_margin_x, inner_margin_y],
        [advance_em - inner_margin_x, 1.0 - inner_margin_y],
        [inner_margin_x, 1.0 - inner_margin_y],
    ];
    GlyphDefinition {
        advance_em,
        contours: vec![outer, inner],
    }
}

#[cfg(test)]
mod tests {
    use super::{FontRegistry, TextAlignment, text_bounds, text_to_profiles};
    use crate::CadError;

    #[test]
    fn text_profiles_empty_input_is_empty() {
        let font = FontRegistry::builtin_sans();
        let profiles =
            text_to_profiles("", &font, 10.0, 1.0, 1.2, TextAlignment::Left).expect("profiles");
        assert!(profiles.is_empty());
    }

    #[test]
    fn text_profiles_generate_outer_and_hole_contours() {
        let font = FontRegistry::builtin_sans();
        let profiles =
            text_to_profiles("A0", &font, 10.0, 1.0, 1.2, TextAlignment::Left).expect("profiles");
        assert!(!profiles.is_empty());
        assert!(profiles.iter().any(|profile| profile.is_hole));
    }

    #[test]
    fn text_bounds_multiline_scales_line_height() {
        let font = FontRegistry::builtin_sans();
        let (_, h1) = text_bounds("A", &font, 10.0, 1.0, 1.2).expect("bounds");
        let (_, h2) = text_bounds("A\nB", &font, 10.0, 1.0, 1.2).expect("bounds");
        assert!(h2 > h1 * 1.5);
    }

    #[test]
    fn text_alignment_shifts_profiles() {
        let font = FontRegistry::builtin_sans();
        let left =
            text_to_profiles("TEST", &font, 10.0, 1.0, 1.2, TextAlignment::Left).expect("left");
        let center =
            text_to_profiles("TEST", &font, 10.0, 1.0, 1.2, TextAlignment::Center).expect("center");
        let min_x_left = left
            .iter()
            .flat_map(|profile| profile.points.iter().map(|point| point[0]))
            .fold(f64::INFINITY, f64::min);
        let min_x_center = center
            .iter()
            .flat_map(|profile| profile.points.iter().map(|point| point[0]))
            .fold(f64::INFINITY, f64::min);
        assert!(min_x_center < min_x_left);
    }

    #[test]
    fn invalid_height_maps_to_error_model() {
        let font = FontRegistry::builtin_sans();
        let err = text_to_profiles("A", &font, 0.0, 1.0, 1.2, TextAlignment::Left)
            .expect_err("invalid height");
        assert!(matches!(err, CadError::InvalidParameter { .. }));
    }
}
