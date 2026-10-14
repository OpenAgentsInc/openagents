//! Explicit LeRobot normalization and calibrated RGB resize. Input images
//! use bilinear interpolation with align_corners=false and no random crop.
use crate::{Result, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Normalize {
    Identity,
    MeanStd { mean: Vec<f32>, std: Vec<f32> },
    MinMax { min: Vec<f32>, max: Vec<f32> },
}
impl Normalize {
    pub fn validate(&self, width: usize) -> Result<()> {
        require(
            (1..=64).contains(&width),
            "Use a bounded policy feature vector.",
        )?;
        match self {
            Self::Identity => Ok(()),
            Self::MeanStd { mean, std } => require(
                mean.len() == width
                    && std.len() == width
                    && mean.iter().all(|v| v.is_finite())
                    && std.iter().all(|v| v.is_finite() && *v >= 0.),
                "Use finite means and nonnegative standard deviations for every feature.",
            ),
            Self::MinMax { min, max } => require(
                min.len() == width
                    && max.len() == width
                    && min
                        .iter()
                        .zip(max)
                        .all(|(a, b)| a.is_finite() && b.is_finite() && a <= b),
                "Use finite minimum and maximum values for every feature.",
            ),
        }
    }
    pub fn apply(&self, values: &[f32]) -> Result<Vec<f32>> {
        self.validate(values.len())?;
        require(
            values.iter().all(|v| v.is_finite()),
            "Use finite observed policy inputs.",
        )?;
        let output = match self {
            Self::Identity => values.to_vec(),
            Self::MeanStd { mean, std } => values
                .iter()
                .zip(mean)
                .zip(std)
                .map(|((x, m), s)| (x - m) / (s + 1e-8))
                .collect(),
            Self::MinMax { min, max } => values
                .iter()
                .zip(min)
                .zip(max)
                .map(|((x, a), b)| (x - a) / (b - a + 1e-8) * 2. - 1.)
                .collect(),
        };
        require(
            output.iter().all(|v: &f32| v.is_finite()),
            "The policy's normalized inputs exceed their numeric range.",
        )?;
        Ok(output)
    }
    pub fn undo(&self, values: &[f32]) -> Result<Vec<f32>> {
        self.validate(values.len())?;
        require(
            values.iter().all(|v| v.is_finite()),
            "The policy returned an invalid action.",
        )?;
        let output = match self {
            Self::Identity => values.to_vec(),
            Self::MeanStd { mean, std } => values
                .iter()
                .zip(mean)
                .zip(std)
                .map(|((x, m), s)| x * s + m)
                .collect(),
            Self::MinMax { min, max } => values
                .iter()
                .zip(min)
                .zip(max)
                .map(|((x, a), b)| (x + 1.) / 2. * (b - a) + a)
                .collect(),
        };
        require(
            output.iter().all(|v: &f32| v.is_finite()),
            "The policy's actions exceed their numeric range.",
        )?;
        Ok(output)
    }
}

/// Decode only the pinned camera's dimensions, remove capture alpha, resize
/// its RGB samples, and apply one declared normalization per channel.
pub fn image(
    png: &[u8],
    source: [u32; 2],
    target: [u32; 2],
    normalization: &Normalize,
) -> Result<Vec<f32>> {
    let [width, height] = source;
    let [out_width, out_height] = target;
    require(
        width > 0
            && height > 0
            && width <= 2048
            && height <= 2048
            && (16..=640).contains(&out_width)
            && (16..=640).contains(&out_height),
        "Use the policy's bounded camera dimensions.",
    )?;
    normalization.validate(3)?;
    let pixels =
        robot_data::export::rgb(png, source).map_err(|e| crate::Error::Invalid(e.to_string()))?;
    let plane = out_width as usize * out_height as usize;
    let mut output = vec![0.; plane * 3];
    for y in 0..out_height {
        let position = ((y as f32 + 0.5) * (height as f32 / out_height as f32) - 0.5).max(0.);
        let top = position.floor() as u32;
        let bottom = (top + 1).min(height - 1);
        let dy = position - top as f32;
        for x in 0..out_width {
            let position = ((x as f32 + 0.5) * (width as f32 / out_width as f32) - 0.5).max(0.);
            let left = position.floor() as u32;
            let right = (left + 1).min(width - 1);
            let dx = position - left as f32;
            let sample = |x: u32, y: u32, channel: usize| -> f32 {
                f32::from(pixels[(y as usize * width as usize + x as usize) * 3 + channel]) / 255.
            };
            for channel in 0..3 {
                let upper =
                    sample(left, top, channel) * (1. - dx) + sample(right, top, channel) * dx;
                let lower =
                    sample(left, bottom, channel) * (1. - dx) + sample(right, bottom, channel) * dx;
                let value = upper * (1. - dy) + lower * dy;
                output[channel * plane + y as usize * out_width as usize + x as usize] =
                    match normalization {
                        Normalize::Identity => value,
                        Normalize::MeanStd { mean, std } => {
                            (value - mean[channel]) / (std[channel] + 1e-8)
                        }
                        Normalize::MinMax { min, max } => {
                            (value - min[channel]) / (max[channel] - min[channel] + 1e-8) * 2. - 1.
                        }
                    };
            }
        }
    }
    require(
        output.iter().all(|value| value.is_finite()),
        "The policy's camera normalization exceeds its numeric range.",
    )?;
    Ok(output)
}
