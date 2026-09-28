//! A deterministic measurer for tests, benchmarks, and a fallback when the
//! platform refuses a paragraph. It is not a text engine: every UTF-16 unit
//! advances by a fixed fraction of its font size.

use super::measure::{Line, MeasureRun, Measured, Measurer, inner_boundaries};

/// Every UTF-16 unit advances by `advance * size`; lines break after spaces
/// and at `\n`. Ascent and descent are 0.8 and 0.2 of the largest font on a
/// line. `calls` counts measurements, so tests can check caching.
#[derive(Clone, Debug)]
pub struct FixedMeasurer {
    pub advance: f32,
    pub calls: usize,
}

impl Default for FixedMeasurer {
    fn default() -> Self {
        Self {
            advance: 0.55,
            calls: 0,
        }
    }
}

impl Measurer for FixedMeasurer {
    fn measure(&mut self, text: &str, runs: &[MeasureRun], width: Option<f32>) -> Option<Measured> {
        self.calls += 1;
        let units: Vec<u16> = text.encode_utf16().collect();
        let size_at = |index: u32| {
            runs.iter()
                .find(|r| index >= r.start16 && index < r.end16)
                .or(runs.last())
                .map_or(16.0, |r| r.font.size)
        };
        let advances: Vec<f32> = (0..units.len() as u32)
            .map(|i| match units[i as usize] {
                0x0A => 0.0,
                // A low surrogate belongs to the unit before it.
                0xDC00..=0xDFFF => 0.0,
                _ => self.advance * size_at(i),
            })
            .collect();
        let limit = width.unwrap_or(f32::INFINITY);
        let mut measured = Measured::default();
        let mut start = 0usize;
        while start < units.len() {
            let mut end = start;
            let mut used = 0.0;
            let mut last_space = None;
            while end < units.len() {
                if units[end] == 0x0A {
                    end += 1;
                    break;
                }
                let next = used + advances[end];
                if next > limit + 0.001 && end > start {
                    // Keep a trailing space on the line it follows.
                    if units[end] == 0x20 {
                        while end < units.len() && units[end] == 0x20 {
                            end += 1;
                        }
                    } else if let Some(space) = last_space {
                        end = space;
                    } else if (0xDC00..=0xDFFF).contains(&units[end]) {
                        end += 1;
                    }
                    break;
                }
                used = next;
                end += 1;
                if units[end - 1] == 0x20 {
                    last_space = Some(end);
                }
            }
            let mut visible = end;
            while visible > start && matches!(units[visible - 1], 0x20 | 0x0A) {
                visible -= 1;
            }
            let width: f32 = advances[start..visible].iter().sum();
            let size = (start as u32..end as u32).map(size_at).fold(0.0, f32::max);
            let line = Line {
                start16: start as u32,
                end16: end as u32,
                width,
                ascent: 0.8 * size,
                descent: 0.2 * size,
                leading: 0.0,
            };
            for boundary in inner_boundaries(runs, line.start16, line.end16) {
                measured
                    .offsets
                    .push(advances[start..boundary as usize].iter().sum());
            }
            measured.lines.push(line);
            start = end;
        }
        Some(measured)
    }
}
